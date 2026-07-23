#!/usr/bin/env python3
"""Run the portable VoicePaste test suite with one command.

The suite intentionally combines Rust unit/integration coverage, real HTTP
black-box probes, and an optional focused-field paste test. Swift tests run on
macOS and are skipped elsewhere by the runner.
"""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "VoicePasteTauri" / "src-tauri" / "Cargo.toml"


def run_case(label: str, command: list[str], environment: dict[str, str]) -> bool:
    print(f"\n=== {label} ===")
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        check=False,
    )
    if result.returncode == 0:
        print(f"PASS {label}")
        return True
    print(f"FAIL {label}: exit code {result.returncode}", file=sys.stderr)
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--skip-ui",
        action="store_true",
        help="skip the focused-field paste test",
    )
    arguments = parser.parse_args()

    environment = dict(os.environ)
    cases = [
        (
            "Rust cargo tests",
            ["cargo", "test", "--manifest-path", str(MANIFEST), "--lib"],
        ),
        (
            "Model-list black box",
            [sys.executable, str(ROOT / "Tests" / "blackbox_models.py"), "--all"],
        ),
    ]
    if platform.system() == "Darwin":
        cases.append(
            (
                "Swift tests",
                ["swift", "test", "--package-path", str(ROOT / "VoicePasteTauri" / "src-tauri")],
            )
        )
    if not arguments.skip_ui:
        cases.append(
            (
                "Focused-field paste black box",
                [sys.executable, str(ROOT / "Tests" / "blackbox_paste.py")],
            )
        )

    failures = [
        label
        for label, command in cases
        if not run_case(label, command, environment)
    ]
    if failures:
        print(f"\nFailed cases: {', '.join(failures)}", file=sys.stderr)
        return 1
    print("\nAll VoicePaste test cases passed or skipped cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
