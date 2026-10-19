/// Seeded domain-warped calcite veins, cloudy mineral body and fine branching fissures.
pub fn marble_albedo(seed: u64, size: u32, base: [u8; 3]) -> Vec<u8> {
    let size = size.clamp(8, 512);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let u = x as f32 / size as f32;
            let v = y as f32 / size as f32;
            let cloud = noise(seed, u * 8., v * 6., 8);
            let warp = noise(seed ^ 0x3419, u * 4., v * 4., 4) * 0.65
                + noise(seed ^ 0x5317, u * 16., v * 12., 16) * 0.13;
            let field = u * 3. + v * 0.75 + warp;
            let ridge = (field * std::f32::consts::TAU).sin().abs();
            let main = (-ridge * 75.).exp();
            let halo = (-ridge * 11.).exp();
            let branch_field = field * 2. + noise(seed ^ 0x81af, u * 8., v * 15., 8) * 0.35;
            let branch = (-(branch_field * std::f32::consts::TAU).sin().abs() * 90.).exp()
                * (1. - ridge).powi(3);
            let grain = noise(seed ^ 0xa731, u * 128., v * 128., 128) - 0.5;
            let mineral = main * 0.75 + halo * 0.2 + branch * 0.3;
            for (channel, c) in base.into_iter().enumerate() {
                let tint = if base[0] > 128 {
                    -mineral * [62., 58., 49.][channel]
                } else {
                    mineral * [56., 54., 47.][channel]
                };
                out.push(
                    (f32::from(c) + tint + (cloud - 0.5) * 11. + grain * 2.).clamp(0., 255.) as u8,
                );
            }
            out.push(255);
        }
    }
    out
}
/// An 8x8 atlas of individually cut emerald and ivory marble squares.
pub fn board_albedo(seed: u64, size: u32) -> Vec<u8> {
    let size = size.clamp(256, 512).next_multiple_of(8);
    let cell = size / 8;
    let tiles: Vec<_> = (0..64u64)
        .map(|i| {
            let file = i % 8;
            let rank = i / 8;
            let base = if (file + rank).is_multiple_of(2) {
                [35, 78, 68]
            } else {
                [215, 207, 187]
            };
            marble_albedo(seed ^ i.wrapping_mul(0x9e37_79b9_7f4a_7c15), cell, base)
        })
        .collect();
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let tile = (y / cell * 8 + x / cell) as usize;
            let index = ((y % cell * cell + x % cell) * 4) as usize;
            pixels.extend_from_slice(&tiles[tile][index..index + 4]);
        }
    }
    pixels
}

fn noise(seed: u64, x: f32, y: f32, period: i32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let smooth = |t: f32| t * t * (3. - 2. * t);
    let tx = smooth(x - x.floor());
    let ty = smooth(y - y.floor());
    let value = |a: i32, b: i32| {
        let mut h = seed
            ^ (a.rem_euclid(period) as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        (h >> 32) as f32 / u32::MAX as f32
    };
    let a = value(ix, iy) * (1. - tx) + value(ix + 1, iy) * tx;
    let b = value(ix, iy + 1) * (1. - tx) + value(ix + 1, iy + 1) * tx;
    a * (1. - ty) + b * ty
}
