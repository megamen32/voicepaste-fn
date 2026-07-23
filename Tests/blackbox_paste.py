#!/usr/bin/env python3
"""Black-box test that pastes into the currently focused cross-platform field.

The test opens a tiny Tk entry, starts the production Rust paste probe, and
checks the value received by the entry. It skips cleanly on headless runners
or machines without the platform clipboard injection tools.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUST_MANIFEST = ROOT / "VoicePasteTauri" / "src-tauri" / "Cargo.toml"


TARGET_SCRIPT = textwrap.dedent(
    """
    import json
    import pathlib
    import sys
    import time
    import tkinter as tk

    ready_path = pathlib.Path(sys.argv[1])
    result_path = pathlib.Path(sys.argv[2])
    expected = sys.argv[3]
    root = tk.Tk()
    root.title("VoicePaste paste target")
    root.geometry("520x90")
    root.attributes("-topmost", True)
    entry = tk.Entry(root, width=70)
    entry.pack(padx=20, pady=25)
    deadline = time.monotonic() + 20

    def become_target():
        root.lift()
        root.focus_force()
        entry.focus_force()
        ready_path.write_text("ready", encoding="utf-8")

    def poll():
        value = entry.get()
        if value == expected:
            result_path.write_text(json.dumps({"ok": True, "value": value}), encoding="utf-8")
            root.destroy()
        elif time.monotonic() >= deadline:
            result_path.write_text(json.dumps({"ok": False, "value": value}), encoding="utf-8")
            root.destroy()
        else:
            root.after(50, poll)

    root.after(250, become_target)
    root.after(300, poll)
    root.mainloop()
    """
)


def missing_runtime_tools() -> list[str]:
    system = platform.system()
    if system == "Darwin":
        required = ["pbcopy"]
    elif system == "Windows":
        required = ["powershell"]
    else:
        required = ["xdotool"]
    missing = [tool for tool in required if shutil.which(tool) is None]
    if system not in ("Darwin", "Windows") and not (
        shutil.which("xclip") or shutil.which("xsel")
    ):
        missing.append("xclip or xsel")
    return missing


def run() -> int:
    if os.environ.get("VOICEPASTE_SKIP_UI_TEST") == "1":
        print("SKIP paste: VOICEPASTE_SKIP_UI_TEST=1")
        return 0

    if platform.system() == "Linux" and not (
        os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")
    ):
        print("SKIP paste: no graphical display detected")
        return 0

    missing = missing_runtime_tools()
    if missing:
        print(f"SKIP paste: missing runtime tool(s): {', '.join(missing)}")
        return 0

    try:
        import tkinter  # noqa: F401
    except ImportError:
        print("SKIP paste: Python tkinter is not installed")
        return 0

    expected = f"VoicePaste cursor test {time.time_ns()}"
    with tempfile.TemporaryDirectory(prefix="voicepaste-paste-") as temp_dir:
        temp_path = Path(temp_dir)
        ready_path = temp_path / "ready"
        result_path = temp_path / "result.json"
        target = subprocess.Popen(
            [sys.executable, "-c", TARGET_SCRIPT, str(ready_path), str(result_path), expected],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            deadline = time.monotonic() + 10
            while not ready_path.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            if not ready_path.exists():
                stderr = target.communicate(timeout=2)[1]
                raise AssertionError(f"paste target did not become ready: {stderr}")

            command = [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(RUST_MANIFEST),
                "--bin",
                "paste_probe",
                "--",
                "--text",
                expected,
            ]
            environment = dict(os.environ)
            if platform.system() == "Darwin":
                helper = Path("/Applications/VoicePaste.app/Contents/MacOS/modifier_monitor")
                if helper.exists():
                    pid_result = subprocess.run(
                        [
                            "osascript",
                            "-e",
                            'tell application "System Events" to unix id of first process whose frontmost is true',
                        ],
                        capture_output=True,
                        text=True,
                        check=False,
                    )
                    target_pid = pid_result.stdout.strip()
                    if pid_result.returncode == 0 and target_pid.isdigit():
                        environment["VOICEPASTE_MODIFIER_MONITOR"] = str(helper)
                        command.extend(["--target-pid", target_pid])
            probe = subprocess.run(
                command,
                cwd=ROOT,
                env=environment,
                capture_output=True,
                text=True,
                timeout=90,
                check=False,
            )
            if probe.returncode != 0:
                raise AssertionError(
                    f"paste probe exited {probe.returncode}:\n"
                    f"stdout:\n{probe.stdout}\n"
                    f"stderr:\n{probe.stderr}"
                )

            target.wait(timeout=25)
            if not result_path.exists():
                raise AssertionError("paste target produced no result")
            result = json.loads(result_path.read_text(encoding="utf-8"))
            if result != {"ok": True, "value": expected}:
                raise AssertionError(f"active cursor received {result!r}")
        finally:
            if target.poll() is None:
                target.terminate()
                target.wait(timeout=5)

    print(f"PASS paste: active cursor received {expected!r}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(run())
    except (AssertionError, OSError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"FAIL paste: {error}", file=sys.stderr)
        raise SystemExit(1)
