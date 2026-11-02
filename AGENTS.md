# Working on rd-chess

rd-chess is hosted on the Hubnot hub on Andrew's Linux box, next to
real-disunity. GitHub `main` is a nightly copy published from the hub. Do not
open GitHub pull requests or push to GitHub for this repository.

## Identity and remotes

Your clone has a `hub` remote and your Hubnot identity, the same one your
session uses for real-disunity (`hn identity show`). Never read, print, copy
or attach anything under `.git/hn` or `.git/hn-ssh`; they hold private keys.
Run `git fetch hub` and `hn sync hub` before reading state.

The engine is pinned by `rev` in `Cargo.toml` and `engine-revision`. The gate
fetches it from the hub, so a revision that is on the hub but not yet on
GitHub builds there.

## Proposing and merging a change

1. Push your topic branch to the hub: `git push hub <branch>`.
2. Open a proposal: `hn proposal open --base hub/main --head <branch> "<title>"`,
   with a body that says what changed, why, and how you verified it.
3. Request the gate: `hn run request <proposal> gate`. The runner on the box
   runs `cargo xtask ci` under Xvfb with lavapipe and signs the result. Check it with
   `hn proposal status <proposal>`.
4. When the gate passes, merge your own proposal:
   `hn automerge <proposal> --rule ordinary --execute --remote hub`.
   No approval from another session is needed. Hubnot refuses the merge if
   the gate failed, a trusted reviewer requested changes, or Andrew rejected
   the proposal.
5. After a fix, `hn proposal revise` and request the gate again for the new
   head.

Run `hn automerge` in the checkout where `main` is checked out, clean and
fast-forwarded to `hub/main`, never in the topic worktree. If hub main moved
since you opened the proposal, merge it into your branch, push, revise and
gate the revision. If any `hn` command fails, or an automerge was cut off
before it printed `Recorded rule merge event`, activate the `hubnot` skill
(`~/.agents/skills/hubnot/SKILL.md`) before retrying: it has the exact
recovery steps and an error table.

Changes to protected paths (`.hn/`, `.github/`, `AGENTS.md`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`, `.cargo/`, `xtask/`) need Andrew's approval and
acceptance. Say so in the proposal and wait. Never push to the hub's `main`
and never merge another session's proposal.

## Issues

Issues live on the hub: `hn issue list`, `hn issue show`, `hn issue comment`.
Claim an issue with a comment before starting work on it.
