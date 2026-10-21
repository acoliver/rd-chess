"""Verify both real consumer graphs reach only the published facade/backend."""
import unittest

from verify_rendering_boundary import verify


class DependencyGraphTests(unittest.TestCase):
    def test_default_and_viewer_reach_shared_shadows_without_private_workspace(self):
        report = verify(require_canonical=True)
        self.assertEqual([graph['viewer'] for graph in report['graphs']], [False, True])
        for graph in report['graphs']:
            self.assertIn('worldrunner-render-shadow', graph['reachable_packages'])
            self.assertTrue(graph['canonical_ready'])
            self.assertTrue(graph['sdk_source'].startswith('git+https://github.com/acoliver/real-disunity.git?rev='))


if __name__ == '__main__':
    unittest.main()
