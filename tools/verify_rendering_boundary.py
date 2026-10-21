#!/usr/bin/env python3
"""Verify the consumer uses SDK-owned rendering through the worldrunner facade."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
import subprocess
import sys

from bootstrap_engine import REPOSITORY

ROOT = Path(__file__).resolve().parents[1]
CANONICAL = ('worldrunner-render-gpu', 'worldrunner-render-lighting', 'worldrunner-render-post')
GPU_PACKAGES = {'wgpu', 'wgpu-core', 'wgpu-hal', 'naga', 'ash', 'glow'}
SHADER_SUFFIXES = {'.wgsl', '.glsl', '.vert', '.frag', '.hlsl', '.spv'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_graph(metadata, root: Path, revision: str, *, viewer: bool, require_canonical=False):
    require(re.fullmatch(r'[0-9a-f]{40}', revision) is not None,
            'engine-revision must be a full 40-character lowercase Git checksum')
    expected_source = f'git+{REPOSITORY}?rev={revision}#{revision}'
    packages = {package['id']: package for package in metadata['packages']}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    games = [package for package in packages.values() if package['name'] == 'chess3d']
    require(len(games) == 1, 'expected one chess3d consumer')
    game = games[0]
    direct = [packages[package_id] for package_id in nodes[game['id']]['dependencies']]
    engine_direct = [package['name'] for package in direct if package['name'].startswith('worldrunner')]
    require(engine_direct == ['worldrunner'], 'consumer must depend on the worldrunner facade only')
    require(not any(package['name'] in GPU_PACKAGES for package in direct),
            'direct GPU dependency bypasses the rendering facade')
    reachable, pending = set(), [game['id']]
    while pending:
        package_id = pending.pop()
        if package_id not in reachable:
            reachable.add(package_id)
            pending.extend(nodes[package_id]['dependencies'])
    names = {packages[package_id]['name'] for package_id in reachable}
    require('worldrunner-render-shadow' in names, 'shared shadow backend is not reachable')
    scene_ids = [package_id for package_id in reachable
                 if packages[package_id]['name'] == 'worldrunner-render-scene']
    require(len(scene_ids) == 1, 'expected one reachable scene backend')
    scene_dependencies = {packages[dependency]['name'] for dependency in nodes[scene_ids[0]]['dependencies']}
    require('worldrunner-render-shadow' in scene_dependencies, 'scene must use the SDK shadow backend')
    require(('worldrunner-scene-viewer' in names) == viewer, 'viewer feature reachability mismatch')
    sdk_packages = [package for package in packages.values() if package['name'].startswith('worldrunner')]
    for package in sdk_packages:
        require(package['source'] == expected_source,
                f'wrong SDK source for {package["name"]}: {package["source"]!r}; expected {expected_source}')
        require(sum(other['name'] == package['name'] for other in sdk_packages) == 1,
                f'multiple {package["name"]} implementations/versions')
    for package in packages.values():
        if package['source'] is None:
            manifest = Path(package['manifest_path']).resolve()
            require(manifest.is_relative_to(root), f'dependency outside consumer: {manifest}')
            dependencies = {packages[dependency]['name'] for dependency in nodes[package['id']]['dependencies']}
            require(not dependencies.intersection(GPU_PACKAGES),
                    f'consumer-owned GPU implementation bypasses facade: {package["name"]}')
    for name in GPU_PACKAGES:
        require(sum(packages[package_id]['name'] == name for package_id in reachable) <= 1,
                f'multiple reachable {name} implementations/versions')
    canonical = {name: name in names for name in CANONICAL}
    if require_canonical:
        require(all(canonical.values()), f'canonical packages not reachable: {canonical}')
    facade = next(package for package in sdk_packages if package['name'] == 'worldrunner')
    return {'facade': 'worldrunner', 'sdk_source': expected_source, 'sdk_revision': revision,
            'sdk_manifest': facade['manifest_path'],
            'sdk_packages': sorted(package['name'] for package in sdk_packages), 'viewer': viewer, 'reachable_packages': sorted(names),
            'canonical_packages': canonical, 'canonical_ready': all(canonical.values())}


def check_sources(root: Path):
    shaders = [str(path.relative_to(root)) for directory in ('src', 'examples', 'assets')
               for path in (root / directory).rglob('*') if path.suffix.lower() in SHADER_SUFFIXES]
    require(not shaders, f'game-owned GPU shader implementation: {shaders}')


def verify(root=ROOT, *, require_canonical=False):
    root = root.resolve()
    check_sources(root)
    results = []
    for viewer in (False, True):
        command = ['cargo', '+1.97.0', 'metadata', '--locked', '--format-version', '1']
        if viewer:
            command += ['--features', 'viewer']
        result = subprocess.run(command, cwd=root, text=True, capture_output=True, check=True, timeout=180)
        results.append(check_graph(json.loads(result.stdout), root, (root / 'engine-revision').read_text().strip(),
                                   viewer=viewer, require_canonical=require_canonical))
    return {'schema': 'chess3d-rendering-boundary-v1', 'graphs': results,
            'canonical_required': require_canonical}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require-canonical', action='store_true',
                        help='postmigration gate: GPU, light and post must be reachable in both graphs')
    parser.add_argument('--output', type=Path, help='new JSON report, never overwrite old evidence')
    args = parser.parse_args()
    try:
        report = verify(require_canonical=args.require_canonical)
        if args.output:
            with args.output.open('x') as stream:
                json.dump(report, stream, indent=2)
                stream.write('\n')
        else:
            print(json.dumps(report, indent=2))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f'rendering boundary FAILED: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
