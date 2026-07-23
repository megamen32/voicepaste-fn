#!/usr/bin/env python3
"""
Blackbox test: keypress → HTTP request to a test endpoint we listen on.

Architecture (true blackbox, no production binary modification):
  1. Start a tiny HTTP server on localhost:random_port. This is the
     "test endpoint" the user said the assistant should listen on.
  2. Launch the production `modifier_monitor` binary with the chosen hotkey.
  3. Wrap its stdout: every JSON line of type "pressed" is forwarded as
     an HTTP POST to our local endpoint. This simulates what the rest of
     the VoicePaste app would do (start recording, send to Whisper, etc.).
  4. In parallel, post a synthetic CGEvent for the hotkey into the system
     HID event tap. The real `modifier_monitor` (running in another
     process) picks it up via its event tap, prints "pressed", and our
     stdout shim turns that into an HTTP request.
  5. Assert the HTTP request arrived within the timeout.

Requirements:
  - macOS only.
  - Accessibility permission must be granted to the test runner (and to
    `modifier_monitor`). Otherwise the event tap will not receive posted
    events and the test will time out.

Usage:
  python3 blackbox_keypress_to_endpoint.py [hotkey]
  hotkey: one of fn, right_option, right_control, right_command, right_shift, caps_lock
          default: fn
"""

from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import threading
import time
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


HERE = Path(__file__).resolve().parent
SRC_TAURI = HERE.parent
MODIFIER_BIN = SRC_TAURI / "modifier_monitor-aarch64-apple-darwin"
KEY_INJECTOR = HERE / "key_injector.swift"

DEFAULT_TIMEOUT_S = 5.0
RECEIVED: list[dict] = []
RECEIVED_LOCK = threading.Lock()


class TestEndpointHandler(BaseHTTPRequestHandler):
    """Captures POSTs to /keypress and remembers the most recent one."""

    def do_POST(self) -> None:  # noqa: N802 (BaseHTTPRequestHandler API)
        length = int(self.headers.get("Content-Length", "0") or "0")
        body_raw = self.rfile.read(length) if length else b""
        try:
            body = json.loads(body_raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            body = {"_raw": body_raw.decode("utf-8", errors="replace")}

        record = {
            "path": self.path,
            "headers": dict(self.headers),
            "body": body,
            "received_at": time.time(),
        }
        with RECEIVED_LOCK:
            RECEIVED.append(record)
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"ok":true}')

    def log_message(self, format: str, *args) -> None:  # silence access log
        return


@contextmanager
def test_endpoint() -> tuple[str, ThreadingHTTPServer]:
    """Bind a localhost server on a free port and yield (base_url, server)."""
    # Bind to port 0 to let the OS pick a free one.
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), TestEndpointHandler)
    port = httpd.server_address[1]
    base_url = f"http://127.0.0.1:{port}"
    # Serve in a background thread so we can shut it down cleanly.
    server_thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    server_thread.start()
    try:
        yield base_url, httpd
    finally:
        httpd.shutdown()
        httpd.server_close()


def free_port_check(port: int) -> bool:
    """Make sure the picked port is actually free (paranoid)."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        return s.connect_ex(("127.0.0.1", port)) != 0


def post_pressed(base_url: str, key: str) -> None:
    """Forward a 'pressed' event from modifier_monitor stdout to the test endpoint."""
    payload = {"event": "hotkey-pressed", "key": key, "source": "modifier_monitor", "ts": time.time()}
    body = json.dumps(payload).encode("utf-8")
    # Use curl because it's already on macOS and avoids Python's urllib quirks.
    # -fsS = fail on HTTP error, silent except errors, show errors.
    subprocess.run(
        ["curl", "-fsS", "-X", "POST", "-H", "Content-Type: application/json",
         "-d", body, f"{base_url}/keypress"],
        check=True,
        stdout=subprocess.DEVNULL,
    )


def stdout_to_http(modifier: subprocess.Popen, base_url: str) -> threading.Thread:
    """Background thread: read modifier_monitor's stdout, POST on 'pressed'."""
    def loop() -> None:
        assert modifier.stdout is not None
        for line in modifier.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                evt = json.loads(line)
            except json.JSONDecodeError:
                continue
            if evt.get("type") == "pressed":
                try:
                    post_pressed(base_url, evt.get("key", "unknown"))
                except subprocess.CalledProcessError as e:
                    print(f"[shim] POST failed: {e}", file=sys.stderr)
    t = threading.Thread(target=loop, daemon=True)
    t.start()
    return t


def wait_for_event(timeout: float) -> dict | None:
    """Block until a request arrives on the test endpoint, or timeout."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        with RECEIVED_LOCK:
            if RECEIVED:
                return RECEIVED[-1]
        time.sleep(0.05)
    return None


def inject_keypress(hotkey: str) -> None:
    """Post a synthetic CGEvent for the given hotkey via the Swift helper."""
    if hotkey == "fn":
        # Use keyDown/keyUp for keycode 63 (legacy Fn). Globe (179) works the same way.
        for direction in ("down", "up"):
            subprocess.run(
                ["swift", str(KEY_INJECTOR), "63", direction],
                check=True,
            )
    elif hotkey == "caps_lock":
        for direction in ("down", "up"):
            subprocess.run(
                ["swift", str(KEY_INJECTOR), "57", direction],
                check=True,
            )
    elif hotkey in ("right_option", "right_control", "right_command", "right_shift"):
        # Use flagsChanged with the right keycode for the modifier.
        # CGEventFlags raw values: maskAlternate=0x80000, maskControl=0x40000,
        # maskCommand=0x100000, maskShift=0x20000.
        # Right-side keycodes: option=61, control=62, command=54, shift=60.
        table = {
            "right_option":  (61, 0x80000),
            "right_control": (62, 0x40000),
            "right_command": (54, 0x100000),
            "right_shift":   (60, 0x20000),
        }
        keycode, mask_down = table[hotkey]
        # Down: flag set; Up: flag cleared.
        subprocess.run(["swift", str(KEY_INJECTOR), "flags", str(keycode), str(mask_down)], check=True)
        subprocess.run(["swift", str(KEY_INJECTOR), "flags", str(keycode), "0"], check=True)
    else:
        raise SystemExit(f"unknown hotkey: {hotkey}")


def main() -> int:
    hotkey = sys.argv[1] if len(sys.argv) > 1 else "fn"
    if not MODIFIER_BIN.exists():
        print(f"FAIL: {MODIFIER_BIN} not found", file=sys.stderr)
        return 2

    with test_endpoint() as (base_url, _httpd):
        print(f"[blackbox] test endpoint: {base_url}/keypress")
        print(f"[blackbox] hotkey: {hotkey}")

        modifier = subprocess.Popen(
            [str(MODIFIER_BIN), hotkey],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,  # line-buffered
        )
        shim = stdout_to_http(modifier, base_url)

        # Give the event tap a moment to install.
        time.sleep(0.5)

        # Sanity check: modifier_monitor should be alive.
        if modifier.poll() is not None:
            err = modifier.stderr.read() if modifier.stderr else "(no stderr)"
            print(f"FAIL: modifier_monitor exited early: {err}", file=sys.stderr)
            return 1

        try:
            print(f"[blackbox] injecting {hotkey} keypress...")
            inject_keypress(hotkey)
        except subprocess.CalledProcessError as e:
            print(f"FAIL: key_injector failed: {e}", file=sys.stderr)
            modifier.terminate()
            return 1

        print(f"[blackbox] waiting up to {DEFAULT_TIMEOUT_S}s for HTTP request...")
        event = wait_for_event(DEFAULT_TIMEOUT_S)

        # Always clean up the modifier process.
        modifier.terminate()
        try:
            modifier.wait(timeout=2)
        except subprocess.TimeoutExpired:
            modifier.kill()

        if event is None:
            # Print stderr to help diagnose (accessibility permission is the usual culprit).
            err = modifier.stderr.read() if modifier.stderr else ""
            print(f"FAIL: no HTTP request received within {DEFAULT_TIMEOUT_S}s", file=sys.stderr)
            if err:
                print(f"[modifier stderr]\n{err}", file=sys.stderr)
            return 1

        body = event["body"]
        print(f"[blackbox] received: {body}")
        if body.get("event") != "hotkey-pressed" or body.get("key") != hotkey:
            print(f"FAIL: unexpected payload: {body}", file=sys.stderr)
            return 1

        print("PASS: keypress → HTTP request to test endpoint")
        return 0


if __name__ == "__main__":
    sys.exit(main())
