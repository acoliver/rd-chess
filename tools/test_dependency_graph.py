"""Verify the consumer reaches the published SDK backend only through its facade."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DependencyGraphTests(unittest.TestCase):
    def test_default_and_viewer_reach_shared_shadows_without_private_workspace(self):
        for features in ([], ["--features", "viewer"]):
            with self.subTest(features=features):
                result = subprocess.run(
                    ["cargo", "+1.97.0", "metadata", "--locked", "--format-version", "1", *features],
                    cwd=ROOT, capture_output=True, text=True, timeout=180, check=True,
                )
                metadata = json.loads(result.stdout)
                packages = {package["id"]: package for package in metadata["packages"]}
                nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
                game = next(package for package in packages.values() if package["name"] == "chess3d")
                direct = [packages[dependency] for dependency in nodes[game["id"]]["dependencies"]]
                self.assertEqual(
                    [package["name"] for package in direct if package["name"].startswith("worldrunner")],
                    ["worldrunner"],
                )
                reachable = set()
                pending = [game["id"]]
                while pending:
                    package_id = pending.pop()
                    if package_id not in reachable:
                        reachable.add(package_id)
                        pending.extend(nodes[package_id]["dependencies"])
                names = {packages[package_id]["name"] for package_id in reachable}
                self.assertIn("worldrunner-render-shadow", names)
                self.assertEqual("worldrunner-scene-viewer" in names, bool(features))
                sdk = (ROOT.parent / "real-disunity").resolve()
                for package_id in reachable:
                    package = packages[package_id]
                    if package["source"] is None:
                        manifest = Path(package["manifest_path"]).resolve()
                        self.assertTrue(
                            manifest.is_relative_to(ROOT) or manifest.is_relative_to(sdk),
                            f"dependency outside the consumer and SDK: {manifest}",
                        )
                scene = next(packages[package_id] for package_id in reachable
                             if packages[package_id]["name"] == "worldrunner-render-scene")
                self.assertIn(
                    "worldrunner-render-shadow",
                    {packages[dependency]["name"] for dependency in nodes[scene["id"]]["dependencies"]},
                )


if __name__ == "__main__":
    unittest.main()
