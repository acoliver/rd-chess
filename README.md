# rd-chess

An original procedural marble chess scene and position editor, built on the
**Real Disunity** engine. The root Rust package and executable are `chess3d`.
All meshes, marble textures, board geometry, camera presets and position documents
are game-owned. Rendering goes through the engine's `worldrunner` facade; no
engine implementation or third-party art is vendored here.

This is a scene/position vertical slice, **not a chess rules engine**. It supports
arbitrary validated board positions, captures and promotions in JSON, not turn
order, legal moves, AI or interactive piece editing. The optional window provides
view controls only.

## Independent checkout and build

Requirements: Git, Python 3.10+, rustup, and Rust 1.97.0 with Clippy and rustfmt.
The toolchain, edition, minimum supported version and lint thresholds match the
engine. Cargo compilation is bounded to two jobs and uses this repository's own
`target/`, never a source checkout's build directory.

From the rd-chess root:

```sh
# Cargo fetches the immutable engine Git revision automatically.
cargo xtask build
cargo run --locked -j2 -- --help
cargo run --locked -j2 -- --save /tmp/chess-position.json
cargo run --locked -j2 -- --load assets/positions/study.json
```

The `worldrunner` facade is a Git dependency pinned to the full commit
`69d0f7e593b3190f94c484fbe0971de2ec3ca294` from
`https://github.com/acoliver/real-disunity.git`. Cargo builds that revision,
not a sibling checkout. `engine-revision` records the same SHA, and the default
and viewer dependency graph gates verify every SDK package's exact Git origin,
requested revision and resolved checksum, with one rendering implementation.
When updating the engine, change the Cargo `rev`, `engine-revision` and lockfile
together.

For optional SDK inspection, `python3 tools/bootstrap_engine.py` fetches the pinned
revision into `../real-disunity`. It stages a clone and publishes it only after a
successful checkout and facade check. Existing sibling checkouts are read-only:
dirty, wrong-revision or non-engine directories fail without reset or checkout.
`--revision` or `ENGINE_REVISION` selects an inspection checkout only; it does
**not** override Cargo's immutable dependency. No sibling is required to build,
run or validate the game. `cargo xtask bootstrap` performs the same optional check.

HTTPS access to a private engine repository requires credentials. For the optional
bootstrap, `ENGINE_READ_TOKEN` supplies read-only authentication without persisting
it in Git config. Cargo uses its standard Git authentication for the pinned dependency. CI uses the `ENGINE_READ_TOKEN` repository secret when supplied,
otherwise its own workflow token. A workflow token does not grant access to a
separate private repository by itself.

`Cargo.lock` belongs to this game workspace and is checked in. Validation uses
`--locked`; after intentionally changing the engine or dependencies, regenerate
it explicitly with `cargo generate-lockfile` and review the change. No source
monorepo, copied history, generated evidence or unrelated game assets are needed.

## Rendering and position documents

```sh
# Select an installed Vulkan ICD explicitly (the filename varies by system).
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json
cargo run --locked -j2 -- --capture /tmp/chess-hero.png \
  --view hero --width 1400 --height 1050
cargo run --locked -j2 -- --load assets/positions/study.json \
  --save /tmp/study-roundtrip.json --capture /tmp/chess-study.png

# Windowing is opt-in both at build time and launch time.
cargo run --locked -j2 --features viewer -- --window --view hero
```

The default `scene` engine dependency excludes the window feature. The game's
`viewer` feature enables `worldrunner/scene-viewer`. Hero, overhead and wide
lineup views share the same prepared scene and directional-shadow settings for
offscreen and window rendering. Original curved sculpts and deterministic
seeded marble remain procedural source assets, not copied image textures.

- `--seed <u64>` controls marble veining, including the high seed bits.
- `--load` and `--save` read/write version-1 `chess3d-position` JSON. Seed/style,
  stable piece IDs, kinds, sides and current squares round-trip. `assets/positions/`
  contains the initial layout and a noninitial study. Edit the JSON to move,
  remove or promote pieces; at most 32 pieces, unique IDs/squares and board bounds
  are validated. Empty boards are accepted. No move legality is implied.
- `--seed` with `--load` is rejected. Image dimensions are bounded to 64–4096 per
  axis. Captures cannot alias position inputs/outputs. Position and PNG writes
  use atomic replacement; if save and render are requested, save completes first
  and the two files are not a single transaction.
- Inspection/save/load need no GPU. Captures require a functioning graphics
  adapter; Mesa lavapipe is a supported software Vulkan device.
- Left-drag orbits, wheel zooms, R/Home resets the exact preset, Escape exits.
  Resize and unmap/remap affect presentation only; camera changes never rewrite
  the position. `--capture` and `--window` cannot be combined.

## Validation

```sh
cargo xtask python-tests
cargo xtask check                    # fmt + strict default/viewer Clippy
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json
cargo xtask test                     # all targets with viewer; GPU required
cargo xtask acceptance --output artifacts/offscreen-001
cargo xtask window --output artifacts/window-001
# Or run every gate, including both rendering harnesses:
cargo xtask ci --output artifacts/ci-001
```

Output directories must be new/empty; previous evidence is not overwritten.
Use `CARGO_TARGET_DIR` to relocate build artifacts; the offscreen/window harnesses
also accept `--target-dir`. Keep evidence and target directories separate.

The migrated stdlib-only offscreen harness drives the real binary, not a fake
renderer. It records engine/game checkout revisions and dirty state, toolchain,
Vulkan adapter, ICD/binary hashes, command logs and decoded image measurements.
It checks:

- required-device Rust acceptance and nonblank, opaque, correctly sized images;
- exact same-device decoded-pixel repeats for hero, overhead and lineup;
- exact initial/study/edited save/load round-trips and different edited images;
- high seed bits changing rendered texture, plus projected board tile parity;
- invalid position/dimensions and a deliberately absent adapter returning errors,
  preserving existing files and not creating failed captures;
- the required Rust GPU test itself **failing** with a missing ICD.

`CHESS3D_REQUIRE_GPU=1` is set by the harness/xtask/CI. The game GPU test is
unconditionally required even without that environment variable: missing adapters
are failures, never skips. Python decoder/process/timeout/bootstrap tests are
supporting controls, not substitutes for observed GPU acceptance. Pixel equality
is required only on the same device/driver; no cross-GPU golden-image claim is made.

The original `tests/window_smoke.py` is retained, with its study fixture naturally
resolved from this standalone root. `cargo xtask window` builds the viewer and
launches it using an exclusive Xvfb `:100` via `xvfb-run`; it never uses the desktop.
Requirements: Xvfb, xvfb-run, xauth, xdotool, libX11 and Pillow. Missing prerequisites
fail explicitly. The smoke compares real window pixels to fresh offscreen frames,
exercises orbit/zoom/reset/resize/unmap/remap, checks saved positions and clean
Escape/window-manager exits. Its original readiness checks are bounded; the
wrapper bounds the whole window run to five minutes and cleans up its process group.

The GitHub Actions workflow fetches the independent engine, provisions Mesa
lavapipe and window-test prerequisites on the runner, and invokes `cargo xtask ci`.
Both GPU and Xvfb acceptance are mandatory, with logs/PNGs uploaded even on failure.
Manual runs accept an explicit `engine_revision` for the optional inspection
checkout only; the compiled SDK remains the Cargo-pinned revision. Nothing in the local commands
installs system packages, changes remote settings, commits or pushes.

## Limits

The scene uses albedo marble, dielectric surfaces, metallic rails and a fixed
studio sun with a PCF directional shadow map. It is stylized procedural art,
not measured stone or a complete simulation. Unusual aspect ratios or extreme
orbit/zoom can crop the composition. Generated acceptance captures are local/CI
artifacts only and are not bundled as source assets or historical evidence.
