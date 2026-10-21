"""Contract controls for consumer dependency ownership and migration readiness."""
from pathlib import Path
import tempfile
import unittest

from verify_rendering_boundary import check_graph, check_sources

REVISION = "69d0f7e593b3190f94c484fbe0971de2ec3ca294"
SOURCE = f"git+https://github.com/acoliver/real-disunity.git?rev={REVISION}#{REVISION}"


def graph():
    names = ['chess3d', 'worldrunner', 'worldrunner-render-scene', 'worldrunner-render-shadow', 'wgpu']
    edges = [[1], [2], [3, 4], [4], []]
    return {'packages': [dict(id=name, name=name, source=('registry' if name == 'wgpu' else SOURCE if name.startswith('worldrunner') else None),
                              manifest_path=('/game/Cargo.toml' if i == 0 else f'/sdk/crates/{name}/Cargo.toml'))
                         for i, name in enumerate(names)],
            'resolve': {'nodes': [dict(id=name, dependencies=[names[j] for j in edges[i]])
                                  for i, name in enumerate(names)]}}


class RenderingBoundaryTests(unittest.TestCase):
    def check(self, metadata, **kwargs):
        return check_graph(metadata, Path('/game'), REVISION, viewer=False, **kwargs)

    def test_existing_published_backend_does_not_require_unpublished_packages(self):
        result = self.check(graph())
        self.assertFalse(result['canonical_ready'])
        self.assertEqual(result['facade'], 'worldrunner')

    def test_direct_gpu_or_backend_dependency_bypasses_facade(self):
        for name in ('wgpu', 'worldrunner-render-shadow'):
            metadata = graph()
            metadata['resolve']['nodes'][0]['dependencies'].append(name)
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'facade'):
                self.check(metadata)

    def test_unreachable_canonical_packages_do_not_count_as_ready(self):
        metadata = graph()
        for name in ('worldrunner-render-gpu', 'worldrunner-render-lighting', 'worldrunner-render-post'):
            metadata['packages'].append(dict(id=name, name=name, source=SOURCE,
                                              manifest_path=f'/sdk/crates/{name}/Cargo.toml'))
            metadata['resolve']['nodes'].append(dict(id=name, dependencies=[]))
        with self.assertRaisesRegex(ValueError, 'reachable'):
            self.check(metadata, require_canonical=True)
        metadata['resolve']['nodes'][2]['dependencies'] += [
            'worldrunner-render-gpu', 'worldrunner-render-lighting', 'worldrunner-render-post']
        self.assertTrue(self.check(metadata, require_canonical=True)['canonical_ready'])

    def test_private_workspace_dependency_and_duplicate_wgpu_fail(self):
        metadata = graph()
        metadata['packages'].append(dict(id='private', name='private', source=None,
                                          manifest_path='/private/Cargo.toml'))
        metadata['resolve']['nodes'].append(dict(id='private', dependencies=[]))
        metadata['resolve']['nodes'][0]['dependencies'].append('private')
        with self.assertRaisesRegex(ValueError, 'outside'):
            self.check(metadata)
        metadata = graph()
        metadata['packages'].append(dict(id='wgpu-second', name='wgpu', source='registry',
                                          manifest_path='/registry/wgpu/Cargo.toml'))
        metadata['resolve']['nodes'].append(dict(id='wgpu-second', dependencies=[]))
        metadata['resolve']['nodes'][3]['dependencies'].append('wgpu-second')
        with self.assertRaisesRegex(ValueError, 'multiple.*wgpu'):
            self.check(metadata)

    def test_every_sdk_package_requires_exact_git_origin_rev_and_checksum(self):
        bad_sources = [None, 'registry',
                       SOURCE.replace('acoliver/real-disunity', 'attacker/real-disunity'),
                       SOURCE.replace(REVISION + '#', 'a' * 40 + '#'),
                       SOURCE.rsplit('#', 1)[0] + '#' + 'b' * 40,
                       SOURCE.replace('?rev=' + REVISION, '?branch=main'),
                       SOURCE.replace('https://', 'file:///tmp/')]
        for index in (1, 2, 3):
            for source in bad_sources:
                metadata = graph()
                metadata['packages'][index]['source'] = source
                with self.subTest(package=index, source=source), self.assertRaisesRegex(ValueError, 'SDK source'):
                    self.check(metadata)

    def test_unreachable_sdk_copy_cannot_hide_wrong_origin_or_duplicate(self):
        for source in (SOURCE, SOURCE.replace('acoliver', 'attacker')):
            metadata = graph()
            metadata['packages'].append(dict(id='second-scene', name='worldrunner-render-scene',
                                              source=source, manifest_path='/other/Cargo.toml'))
            metadata['resolve']['nodes'].append(dict(id='second-scene', dependencies=[]))
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, 'SDK source|multiple'):
                self.check(metadata)

    def test_workspace_helpers_must_not_own_gpu_implementation(self):
        metadata = graph()
        metadata['packages'].append(dict(id='xtask', name='xtask', source=None,
                                          manifest_path='/game/xtask/Cargo.toml'))
        metadata['resolve']['nodes'].append(dict(id='xtask', dependencies=['wgpu']))
        metadata['workspace_members'] = ['chess3d', 'xtask']
        with self.assertRaisesRegex(ValueError, 'consumer-owned GPU'):
            self.check(metadata)

    def test_engine_revision_must_be_full_lowercase_checksum(self):
        for revision in ('main', REVISION[:7], REVISION.upper()):
            with self.subTest(revision=revision), self.assertRaisesRegex(ValueError, '40'):
                check_graph(graph(), Path('/game'), revision, viewer=False)

    def test_viewer_feature_must_actually_reach_viewer_package(self):
        with self.assertRaisesRegex(ValueError, 'viewer'):
            check_graph(graph(), Path('/game'), REVISION, viewer=True)

    def test_game_owned_shader_is_rejected_but_procedural_assets_are_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            (root / 'src/mesh.rs').write_text('pub fn procedural_mesh() {}')
            check_sources(root)
            (root / 'src/lighting.wgsl').write_text('@fragment fn main() {}')
            with self.assertRaisesRegex(ValueError, 'shader'):
                check_sources(root)


if __name__ == '__main__':
    unittest.main()
