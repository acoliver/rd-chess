#!/usr/bin/env python3
"""Fetch real-disunity beside rd-chess without replacing an existing checkout.

Use --revision <full commit SHA> for immutable reproduction. The default comes
from engine-revision; ENGINE_REVISION overrides it. No source monorepo is used.
"""
from __future__ import annotations

import argparse
import base64
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/acoliver/real-disunity.git"


def validate_revision(value):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*", value) or ".." in value:
        raise ValueError("revision must be an explicit commit, tag or ref, not a Git option or expression")
    return value


def git(path, *args):
    environment = dict(os.environ, GIT_TERMINAL_PROMPT="0")
    token = environment.pop("ENGINE_READ_TOKEN", "")
    if token:
        authorization = base64.b64encode(f"x-access-token:{token}".encode()).decode()
        index = int(environment.get("GIT_CONFIG_COUNT", "0"))
        environment.update({"GIT_CONFIG_COUNT": str(index + 1),
                            f"GIT_CONFIG_KEY_{index}": "http.https://github.com/.extraheader",
                            f"GIT_CONFIG_VALUE_{index}": f"AUTHORIZATION: basic {authorization}"})
    result = subprocess.run(["git", "-C", str(path), *args], text=True, capture_output=True,
                            timeout=180, check=False, env=environment)
    if result.returncode:
        raise ValueError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def check_facade(path):
    if not (path / "crates/worldrunner/Cargo.toml").is_file():
        raise ValueError("engine revision does not contain crates/worldrunner/Cargo.toml")


def bootstrap(destination, revision, *, repository=REPOSITORY):
    revision = validate_revision(revision)
    destination = Path(destination).absolute()
    if destination.exists():
        if not (destination / ".git").exists():
            raise ValueError(f"refusing to replace existing non-checkout: {destination}")
        if git(destination, "status", "--porcelain"):
            raise ValueError(f"existing engine has uncommitted changes; left untouched: {destination}")
        # Existing checkouts are read-only: never fetch, reset or switch branches.
        expected = git(destination, "rev-parse", "--verify", f"{revision}^{{commit}}")
        actual = git(destination, "rev-parse", "HEAD")
        if expected != actual:
            raise ValueError(f"existing engine HEAD {actual} differs from {revision}; use a fresh sibling checkout")
        check_facade(destination)
        return actual
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".rd-chess-engine-", dir=destination.parent) as temp:
        staging = Path(temp) / "checkout"
        staging.mkdir()
        git(staging, "init", "-q")
        git(staging, "remote", "add", "origin", repository)
        git(staging, "fetch", "--depth=1", "origin", revision)
        git(staging, "checkout", "--detach", "FETCH_HEAD")
        check_facade(staging)
        actual = git(staging, "rev-parse", "HEAD")
        staging.rename(destination)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default=os.environ.get("ENGINE_REVISION") or
                        (ROOT / "engine-revision").read_text().strip())
    args = parser.parse_args()
    try:
        revision = bootstrap(ROOT.parent / "real-disunity", args.revision)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Engine bootstrap FAILED: {error}", file=sys.stderr)
        return 1
    print(f"real-disunity revision: {revision}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
