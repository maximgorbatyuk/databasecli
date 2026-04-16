#!/usr/bin/env python3
"""
databasecli release script.

Release flow:
  1. Determine version: use argument if given, otherwise read Cargo.toml
     and increment the patch number (e.g. 0.1.6 -> 0.1.7)
  2. Validate version format (X.Y.Z, all non-negative integers)
  3. Validate CHANGELOG.md has release notes for the new version and
     that it is the latest entry
  4. Check prerequisites: gh CLI installed and authenticated, just installed
  5. Switch to 'dev' branch, pull latest
  6. If uncommitted changes exist, commit them on dev with
     'Committing changes for version X.Y.Z'
  7. Run just verify (fmt, clippy, test, build)
  8. Update version in workspace Cargo.toml
  9. Run cargo check to verify Cargo.lock updates
  10. Commit version bump, push dev
  11. Switch to 'main' branch, pull latest
  12. Merge dev into main, push main
  13. Create and push tag vX.Y.Z
  14. Switch back to 'dev' branch, pull latest
  15. Print success summary

Usage:
  ./scripts/release.py 0.2.0    # explicit version
  ./scripts/release.py           # auto-increment patch from Cargo.toml
"""

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = REPO_ROOT / "Cargo.toml"
CHANGELOG = REPO_ROOT / "CHANGELOG.md"


def run(cmd: list[str], *, cwd: Path = REPO_ROOT, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command, print it, and abort on failure."""
    print(f"\n  $ {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if result.stdout.strip():
        print(result.stdout.strip())
    if result.stderr.strip():
        print(result.stderr.strip())
    if check and result.returncode != 0:
        print(f"\nERROR: command failed with exit code {result.returncode}")
        sys.exit(1)
    return result


def validate_version(version: str) -> None:
    """Validate that version matches X.Y.Z where each part is a non-negative integer."""
    if not re.match(r"^\d+\.\d+\.\d+$", version):
        print(f"ERROR: invalid version '{version}'. Expected format: X.Y.Z (e.g. 0.2.0)")
        sys.exit(1)

    print(f"  Version: {version}")


def read_current_version() -> str:
    """Read the workspace version from the [workspace.package] section of Cargo.toml."""
    content = CARGO_TOML.read_text()
    # Match version inside [workspace.package] to avoid hitting dependency versions
    match = re.search(
        r'^\[workspace\.package\]\s*\n(?:.*\n)*?version\s*=\s*"([^"]+)"',
        content,
        re.MULTILINE,
    )
    if not match:
        print("ERROR: could not find version in [workspace.package] in Cargo.toml")
        sys.exit(1)
    return match.group(1)


def increment_patch(version: str) -> str:
    """Increment the patch component: 0.1.6 -> 0.1.7."""
    parts = version.split(".")
    parts[-1] = str(int(parts[-1]) + 1)
    return ".".join(parts)


def validate_changelog(version: str) -> None:
    """Check that CHANGELOG.md contains release notes for version and it is the latest entry."""
    if not CHANGELOG.exists():
        print(f"ERROR: CHANGELOG.md not found at {CHANGELOG}")
        sys.exit(1)

    content = CHANGELOG.read_text()

    # Find all version headings like ## [0.1.7]
    headings = re.findall(r"^## \[(\d+\.\d+\.\d+)\]", content, re.MULTILINE)

    if not headings:
        print("ERROR: no version entries found in CHANGELOG.md")
        sys.exit(1)

    if version not in headings:
        print(f"ERROR: CHANGELOG.md has no release notes for version {version}")
        print(f"  Found versions: {', '.join(headings)}")
        print(f"  Add a '## [{version}]' section before releasing.")
        sys.exit(1)

    if headings[0] != version:
        print(f"ERROR: version {version} is not the latest entry in CHANGELOG.md")
        print(f"  Latest entry is: {headings[0]}")
        print(f"  The new version must be the first ## [X.Y.Z] heading.")
        sys.exit(1)

    print(f"  CHANGELOG.md: release notes for {version} found (latest entry)")


def check_gh_cli() -> None:
    """Check that the GitHub CLI is installed and authenticated."""
    result = subprocess.run(["which", "gh"], capture_output=True, text=True)
    if result.returncode != 0:
        print("ERROR: 'gh' CLI is not installed.")
        print("  Install it: brew install gh")
        print("  Then authenticate: gh auth login")
        sys.exit(1)

    result = subprocess.run(["gh", "auth", "status"], capture_output=True, text=True)
    if result.returncode != 0:
        print("ERROR: 'gh' CLI is not authenticated.")
        print("  Run: gh auth login")
        sys.exit(1)

    print("  gh CLI: installed and authenticated")


def check_just() -> None:
    """Check that the just command runner is installed."""
    result = subprocess.run(["which", "just"], capture_output=True, text=True)
    if result.returncode != 0:
        print("ERROR: 'just' is not installed.")
        print("  Install it: brew install just")
        sys.exit(1)

    print("  just: installed")


def ensure_clean_worktree(version: str) -> None:
    """If there are uncommitted changes, commit them for the release."""
    result = run(["git", "status", "--porcelain"], check=False)
    if result.stdout.strip():
        print("  Uncommitted changes detected — committing for release")
        run(["git", "add", "-A"])
        run(["git", "commit", "-m", f"Committing changes for version {version}"])
        print(f"  Committed all changes for version {version}")
    else:
        print("  Working tree: clean")


def run_verify() -> None:
    """Run the full verification workflow: fmt, clippy, test, build."""
    print("\n--- Running just verify ---")
    run(["just", "verify"])


def switch_branch(branch: str) -> None:
    """Switch to a branch and pull latest from remote."""
    print(f"\n--- Switching to '{branch}' branch ---")
    run(["git", "checkout", branch])
    run(["git", "pull", "--ff-only", "origin", branch])


def update_version(version: str) -> None:
    """Update the workspace version in Cargo.toml."""
    print(f"\n--- Updating version to {version} ---")

    content = CARGO_TOML.read_text()
    updated = re.sub(
        r'^(version\s*=\s*)"[^"]+"',
        f'\\1"{version}"',
        content,
        count=1,
        flags=re.MULTILINE,
    )

    if updated == content:
        print("ERROR: failed to find version field in Cargo.toml")
        sys.exit(1)

    CARGO_TOML.write_text(updated)
    print(f"  Updated Cargo.toml: version = \"{version}\"")

    # Regenerate Cargo.lock with new version
    run(["cargo", "check", "--workspace"])


def commit_and_push_version(version: str) -> None:
    """Commit the version bump and push to dev."""
    print(f"\n--- Committing version bump ---")
    run(["git", "add", "Cargo.toml", "Cargo.lock"])
    run(["git", "commit", "-m", f"chore: bump version to {version}"])
    run(["git", "push", "origin", "dev"])


def merge_to_main() -> None:
    """Switch to main, merge dev, and push."""
    print("\n--- Merging dev into main ---")
    switch_branch("main")
    run(["git", "merge", "dev", "--no-edit"])
    run(["git", "push", "origin", "main"])


def create_and_push_tag(version: str) -> None:
    """Create a version tag and push it to trigger the release workflow."""
    tag = f"v{version}"
    print(f"\n--- Creating tag {tag} ---")
    run(["git", "tag", tag])
    run(["git", "push", "origin", tag])


def main() -> None:
    if len(sys.argv) > 2:
        print("Usage: ./scripts/release.py [version]")
        print("Example: ./scripts/release.py 0.2.0")
        print("         ./scripts/release.py          # auto-increment patch")
        sys.exit(1)

    print("=== databasecli Release ===\n")

    # Step 1: Determine version
    print("Step 1: Determine version")
    if len(sys.argv) == 2:
        version = sys.argv[1]
        print(f"  Version provided: {version}")
    else:
        current = read_current_version()
        version = increment_patch(current)
        print(f"  Current version: {current}")
        print(f"  Next version:    {version}")

    # Step 2: Validate version
    print("\nStep 2: Validate version")
    validate_version(version)

    # Step 3: Validate changelog
    print("\nStep 3: Validate changelog")
    validate_changelog(version)

    # Step 4: Check prerequisites
    print("\nStep 4: Check prerequisites")
    check_gh_cli()
    check_just()

    # Step 5: Switch to dev branch (before committing so changes land on dev)
    print("\nStep 5: Switch to dev branch")
    switch_branch("dev")

    # Step 6: Commit uncommitted changes if any
    print("\nStep 6: Check working tree")
    ensure_clean_worktree(version)

    # Step 7: Run verification
    print("\nStep 7: Run verification")
    run_verify()

    # Step 8: Update version
    print("\nStep 8: Update version")
    update_version(version)

    # Step 9: Commit and push dev
    print("\nStep 9: Commit and push version bump")
    commit_and_push_version(version)

    # Step 10: Merge dev into main
    print("\nStep 10: Merge dev into main")
    merge_to_main()

    # Step 11: Create and push tag
    print("\nStep 11: Create and push tag")
    create_and_push_tag(version)

    # Step 12: Switch back to dev and pull (includes merge commit from main)
    print("\nStep 12: Switch back to dev branch")
    switch_branch("dev")

    # Done
    tag = f"v{version}"
    print(f"\n=== Release {tag} complete ===")
    print(f"  Tag: {tag}")
    print(f"  Branch: dev (switched back)")
    print(f"  GitHub Actions will build release artifacts via cargo-dist.")
    print(f"  Monitor: gh run list --workflow release.yml")


if __name__ == "__main__":
    main()
