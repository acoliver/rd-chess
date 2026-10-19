"""Small strict decoder for the game's noninterlaced RGB/RGBA8 PNG captures.

Only stdlib; intentionally rejects other PNG modes rather than guessing pixels.
The acceptance hashes cover decoded RGBA, never compressed PNG bytes.
"""
import hashlib
import math
import struct
import zlib
from collections import Counter


MAX_PNG_BYTES = 80 * 1024 * 1024


def decode_png(path):
    if not path.is_file():
        raise ValueError(f"PNG input must be a regular file: {path}")
    with path.open("rb") as stream:
        if stream.seek(0, 2) > MAX_PNG_BYTES:
            raise ValueError("PNG file budget exceeded (80 MiB)")
        stream.seek(0)
        data = stream.read(MAX_PNG_BYTES + 1)
    if len(data) > MAX_PNG_BYTES:
        raise ValueError("PNG file budget exceeded (80 MiB)")
    if data[:8] != bytes.fromhex("89504e470d0a1a0a"):
        raise ValueError(f"not a PNG: {path}")
    offset, header, ended = 8, None, False
    seen_idat = idat_ended = False
    compressed = bytearray()
    while offset < len(data):
        if offset + 12 > len(data):
            raise ValueError("truncated PNG chunk")
        size = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4:offset + 8]
        end = offset + 8 + size
        payload = data[offset + 8:end]
        if end + 4 > len(data) or zlib.crc32(kind + payload) != struct.unpack_from(">I", data, end)[0]:
            raise ValueError("truncated PNG or CRC mismatch")
        if not all(65 <= c <= 90 or 97 <= c <= 122 for c in kind) or not 65 <= kind[2] <= 90:
            raise ValueError(f"invalid PNG chunk name/reserved bit: {kind!r}")
        if header is None and kind != b"IHDR":
            raise ValueError("PNG header must be first")
        if seen_idat and kind != b"IDAT":
            idat_ended = True
        if kind == b"IHDR":
            if header is not None or offset != 8 or size != 13:
                raise ValueError("invalid PNG header")
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            if idat_ended:
                raise ValueError("PNG IDAT chunks must be consecutive")
            seen_idat = True
            compressed.extend(payload)
        elif kind == b"IEND":
            if not seen_idat:
                raise ValueError("PNG requires IDAT before IEND")
            ended = True
            if size or end + 4 != len(data):
                raise ValueError("invalid PNG end")
            break
        elif kind in (b"tRNS", b"acTL", b"fcTL", b"fdAT") or not kind[0] & 32:
            raise ValueError(f"unsupported PNG chunk: {kind!r}")
        offset = end + 4
    if header is None or not ended:
        raise ValueError("incomplete PNG")
    width, height, depth, color, compression, filtering, interlace = header
    if not (0 < width <= 4096 and 0 < height <= 4096):
        raise ValueError("PNG dimensions outside acceptance bounds")
    if depth != 8 or color not in (2, 6) or (compression, filtering, interlace) != (0, 0, 0):
        raise ValueError("requires noninterlaced RGB/RGBA8 PNG")
    channels = 4 if color == 6 else 3
    stride = width * channels
    expected = height * (stride + 1)
    decoder = zlib.decompressobj()
    try:
        raw = decoder.decompress(compressed, expected + 1)
    except zlib.error as error:
        raise ValueError("invalid PNG zlib stream") from error
    if len(raw) != expected or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
        raise ValueError("invalid PNG decompressed size/stream")
    previous = bytearray(stride)
    pixels = bytearray()
    for y in range(height):
        start = y * (stride + 1)
        method = raw[start]
        row = bytearray(raw[start + 1:start + stride + 1])
        if method > 4:
            raise ValueError("invalid PNG filter")
        for x in range(stride):
            left = row[x - channels] if x >= channels else 0
            above = previous[x]
            upper_left = previous[x - channels] if x >= channels else 0
            if method == 1:
                predictor = left
            elif method == 2:
                predictor = above
            elif method == 3:
                predictor = (left + above) // 2
            elif method == 4:
                p = left + above - upper_left
                distances = (abs(p - left), abs(p - above), abs(p - upper_left))
                predictor = (left, above, upper_left)[distances.index(min(distances))]
            else:
                predictor = 0
            row[x] = (row[x] + predictor) & 255
        if channels == 4:
            pixels.extend(row)
        else:
            for x in range(0, stride, 3):
                pixels.extend(row[x:x + 3])
                pixels.append(255)
        previous = row
    return width, height, bytes(pixels)


def image_stats(image):
    width, height, pixels = image
    count = width * height
    histogram = Counter()
    luma_sum = luma_square = 0.0
    opaque = dark = light = colored = clipped = 0
    minima, maxima, sums = [255] * 3, [0] * 3, [0] * 3
    for i in range(0, len(pixels), 4):
        rgb = pixels[i:i + 3]
        luma = sum(c * weight for c, weight in zip(rgb, (0.2126, 0.7152, 0.0722)))
        luma_sum += luma
        luma_square += luma * luma
        opaque += pixels[i + 3] == 255
        dark += luma < 80
        light += luma > 150
        colored += max(rgb) - min(rgb) >= 12
        clipped += all(c >= 250 for c in rgb)
        histogram[tuple(c // 16 for c in rgb)] += 1
        for channel, c in enumerate(rgb):
            minima[channel] = min(minima[channel], c)
            maxima[channel] = max(maxima[channel], c)
            sums[channel] += c
    mean = luma_sum / count
    return {"width": width, "height": height, "rgba_sha256": hashlib.sha256(pixels).hexdigest(),
            "opaque_fraction": opaque / count, "dark_fraction": dark / count,
            "light_fraction": light / count, "colored_fraction": colored / count,
            "white_clipped_fraction": clipped / count,
            "luma_mean": mean, "luma_stddev": math.sqrt(max(0, luma_square / count - mean * mean)),
            "rgb_min": minima, "rgb_max": maxima, "rgb_mean": [v / count for v in sums],
            "quantized_color_bins": len(histogram), "largest_color_bin_fraction": max(histogram.values()) / count}


def validate_image(stats, dimensions):
    if (stats["width"], stats["height"]) != dimensions:
        raise ValueError(f"incorrect image dimensions: {stats}")
    # Broad corruption/composition gates, not a substitute for sculpture review.
    gates = {"opaque": stats["opaque_fraction"] == 1.0,
             "contrast": stats["luma_stddev"] >= 12,
             "palette": stats["quantized_color_bins"] >= 24,
             "dark coverage": stats["dark_fraction"] >= 0.005,
             "light coverage": stats["light_fraction"] >= 0.005,
             "color coverage": stats["colored_fraction"] >= 0.005,
             "not uniform": stats["largest_color_bin_fraction"] < 0.98}
    failed = [name for name, passed in gates.items() if not passed]
    if failed:
        raise ValueError(f"image failed {', '.join(failed)}: {stats}")
    return gates


def compare(a, b):
    if a[:2] != b[:2]:
        raise ValueError("comparison dimensions differ")
    changed = sum(a[2][i:i + 4] != b[2][i:i + 4] for i in range(0, len(a[2]), 4))
    return {"changed_pixels": changed, "changed_fraction": changed / (a[0] * a[1]),
            "mean_absolute_rgb_error": sum(abs(x - y) for i, (x, y) in enumerate(zip(a[2], b[2]))
                                           if i % 4 != 3) / (a[0] * a[1] * 3)}
