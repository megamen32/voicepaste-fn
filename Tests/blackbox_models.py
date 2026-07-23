#!/usr/bin/env python3
"""Black-box contract test for the Swift and Rust model-list clients.

The test starts a real local HTTP server, launches a production client probe
as a separate process, and checks the observable GET /models contract. It is
safe to run on Windows, macOS, and Ubuntu; the Swift probe is skipped off macOS.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUST_MANIFEST = ROOT / "VoicePasteTauri" / "src-tauri" / "Cargo.toml"
EXPECTED_MODELS = ["alpha", "whisper-1", "zeta"]
API_KEY = "blackbox-test-key"


class ModelsHandler(BaseHTTPRequestHandler):
    requests: list[tuple[str, str]] = []

    def do_GET(self) -> None:  # noqa: N802 (BaseHTTPRequestHandler API)
        authorization = self.headers.get("Authorization", "")
        self.requests.append((self.path, authorization))
        if self.path != "/v1/models" or authorization != f"Bearer {API_KEY}":
            self.send_response(401)
            self.end_headers()
            return

        body = json.dumps(
            {
                "data": [
                    {"id": "zeta"},
                    {"id": "whisper-1"},
                    {"id": "alpha"},
                ]
            }
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return


@contextmanager
def models_server() -> tuple[str, type[ModelsHandler]]:
    ModelsHandler.requests = []
    server = ThreadingHTTPServer(("127.0.0.1", 0), ModelsHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    host, port = server.server_address
    try:
        yield f"http://{host}:{port}/v1", ModelsHandler
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def run_probe(implementation: str, endpoint: str) -> dict[str, object]:
    if implementation == "rust":
        command = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(RUST_MANIFEST),
            "--bin",
            "voicepaste-model-probe",
            "--",
            "--endpoint",
            endpoint,
            "--api-key",
            API_KEY,
        ]
    else:
        command = [
            "swift",
            "run",
            "--package-path",
            str(ROOT),
            "voicepaste-model-probe",
            "--",
            "--endpoint",
            endpoint,
            "--api-key",
            API_KEY,
        ]

    environment = dict(os.environ)
    for variable in ("OPENAI_BASE_URL", "OPENAI_API_KEY", "TRANSCRIBE_MODEL"):
        environment.pop(variable, None)
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(
            f"{implementation} probe exited {result.returncode}:\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    output_lines = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    if not output_lines:
        raise AssertionError(f"{implementation} probe produced no JSON output")
    try:
        return json.loads(output_lines[-1])
    except json.JSONDecodeError as error:
        raise AssertionError(
            f"{implementation} probe did not produce JSON:\n{result.stdout}"
        ) from error


def run_case(implementation: str) -> None:
    with models_server() as (endpoint, handler):
        payload = run_probe(implementation, endpoint)
        requests = handler.requests

    assert payload == {"models": EXPECTED_MODELS}, (
        f"{implementation} returned {payload!r}; "
        f"expected sorted model IDs {EXPECTED_MODELS!r}"
    )
    assert requests == [("/v1/models", f"Bearer {API_KEY}")], (
        f"unexpected HTTP requests from {implementation}: {requests!r}"
    )
    print(f"PASS {implementation}: GET /v1/models -> {EXPECTED_MODELS}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--implementation",
        choices=("rust", "swift", "all"),
        default="all",
        help="probe to run; Swift is skipped off macOS",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="run all probes (alias for --implementation all)",
    )
    arguments = parser.parse_args()
    implementation = "all" if arguments.all else arguments.implementation

    implementations = ["rust"]
    if implementation == "swift":
        implementations = ["swift"]
    elif implementation == "all" and platform.system() == "Darwin":
        implementations.append("swift")

    failures = 0
    for implementation in implementations:
        if implementation == "swift" and platform.system() != "Darwin":
            print("SKIP swift: Swift package targets macOS")
            continue
        try:
            run_case(implementation)
        except (AssertionError, OSError) as error:
            failures += 1
            print(f"FAIL {implementation}: {error}", file=sys.stderr)

    if failures:
        print(f"{failures} black-box implementation(s) failed", file=sys.stderr)
        return 1
    print("Black-box model contract passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
