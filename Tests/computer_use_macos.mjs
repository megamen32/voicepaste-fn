/**
 * Real macOS Computer Use parity canary for Rust and Swift bundles.
 *
 * Run from the Codex node_repl, not from a normal test runner:
 *   var { run } = await import('./Tests/computer_use_macos.mjs');
 *   await run({ sky, implementation: 'rust', appPath: '/Applications/VoicePaste.app' });
 *   await run({ sky, implementation: 'swift', appPath: './build/VoicePasteFn.app' });
 *
 * The canary uses a loopback OpenAI-compatible transcription endpoint and a
 * deterministic WAV supplied through VOICEPASTE_TEST_AUDIO. The hotkey,
 * overlay, transcription orchestration, paste helper, and foreground target
 * remain real product paths. It writes a redacted receipt under
 * `.agents/evidence/computer-use/` and never changes TCC or persistent config.
 */

import http from "node:http";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function command(executable, args, options = {}) {
    return new Promise((resolve, reject) => {
        const child = spawn(executable, args, {
            stdio: ["ignore", "pipe", "pipe"],
            ...options,
        });
        let stdout = "";
        let stderr = "";
        child.stdout.on("data", (chunk) => { stdout += chunk; });
        child.stderr.on("data", (chunk) => { stderr += chunk; });
        child.once("error", reject);
        child.once("close", (code, signal) => resolve({ code, signal, stdout, stderr }));
    });
}

async function binaryIdentity(file) {
    const hash = await command("shasum", ["-a", "256", file]);
    const signing = await command("codesign", ["-dv", "--verbose=4", file]);
    const bundleRoot = file.endsWith(".app") ? file : path.dirname(path.dirname(file));
    const bundleID = await command(
        "/usr/libexec/PlistBuddy",
        ["-c", "Print :CFBundleIdentifier", path.join(bundleRoot, "Contents", "Info.plist")],
    );
    return {
        path: file,
        sha256: hash.stdout.trim().split(/\s+/)[0] || null,
        codesign: `${signing.stdout}${signing.stderr}`.trim().slice(0, 1200),
        bundle_id: bundleID.code === 0 ? bundleID.stdout.trim() : null,
    };
}

async function probeOverlay(owner) {
    const probe = await command("swift", [path.resolve("Tests/window_probe.swift"), owner]);
    if (probe.code !== 0) throw new Error(`overlay probe failed: ${probe.stderr}`);
    return JSON.parse(probe.stdout);
}

async function findTextElement(sky, app) {
    const state = await sky.get_app_state({ app, disableDiff: true });
    const match = state.text.match(/^\s*(\d+) .*?\(settable, string\)/m);
    if (!match) throw new Error(`editable text element not found in ${app} AX state`);
    return { state, elementIndex: Number(match[1]) };
}

function wavProfileFromMultipart(body) {
    const riff = body.indexOf("RIFF");
    if (riff < 0) return { bytes: 0, has_first: false, has_second: false };
    const data = body.indexOf("data", riff + 12);
    if (data < 0 || data + 8 > body.length) return { bytes: 0, has_first: false, has_second: false };
    const size = Math.min(body.readUInt32LE(data + 4), body.length - data - 8);
    const samples = body.subarray(data + 8, data + 8 + size);
    let hasFirst = false;
    let hasSecond = false;
    for (let offset = 0; offset + 1 < samples.length; offset += 2) {
        const amplitude = Math.abs(samples.readInt16LE(offset));
        if (amplitude >= 2_700 && amplitude <= 3_300) hasFirst = true;
        if (amplitude >= 5_700 && amplitude <= 6_300) hasSecond = true;
    }
    return { bytes: size, has_first: hasFirst, has_second: hasSecond };
}

async function startFixtureServer(expectedText, verifyRealtimePipeline = false) {
    const requests = [];
    const server = http.createServer((request, response) => {
        if (request.method !== "POST" || !request.url.endsWith("/audio/transcriptions")) {
            response.writeHead(404).end();
            return;
        }
        const chunks = [];
        request.on("data", (chunk) => chunks.push(chunk));
        request.on("end", () => {
            const profile = wavProfileFromMultipart(Buffer.concat(chunks));
            requests.push(profile);
            let text = expectedText;
            if (verifyRealtimePipeline) {
                if (profile.has_first && profile.has_second) text = expectedText;
                else if (profile.has_first) text = "preview-one";
                else if (profile.has_second) text = "preview-two";
                else text = "";
            }
            const payload = JSON.stringify({ text });
            response.writeHead(200, { "content-type": "application/json" });
            response.end(payload);
        });
    });
    await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });
    return { server, port: server.address().port, requests };
}

async function waitForRequestCount(requests, count, timeoutMs = 10_000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (requests.length >= count) return;
        await sleep(50);
    }
    throw new Error(`expected ${count} transcription requests, observed ${requests.length}`);
}

async function waitForClipboard(expected, timeoutMs = 10_000) {
    const deadline = Date.now() + timeoutMs;
    let last = "";
    while (Date.now() < deadline) {
        const clipboard = await command("pbpaste", []);
        last = clipboard.stdout;
        if (last.trim() === expected) return last;
        await sleep(50);
    }
    throw new Error(`clipboard never reached ${JSON.stringify(expected)}; last=${JSON.stringify(last)}`);
}

async function writeRealtimeFixtureWav(directory) {
    const sampleRate = 16_000;
    const segments = [
        [0, 200],
        [3_000, 500],
        [0, 700],
        [6_000, 500],
        [0, 700],
    ];
    const sampleCount = segments.reduce((sum, [, ms]) => sum + Math.round(sampleRate * ms / 1000), 0);
    const data = Buffer.alloc(sampleCount * 2);
    let offset = 0;
    for (const [sample, ms] of segments) {
        const count = Math.round(sampleRate * ms / 1000);
        for (let index = 0; index < count; index += 1) {
            data.writeInt16LE(sample, offset);
            offset += 2;
        }
    }
    const header = Buffer.alloc(44);
    header.write("RIFF", 0);
    header.writeUInt32LE(36 + data.length, 4);
    header.write("WAVE", 8);
    header.write("fmt ", 12);
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20);
    header.writeUInt16LE(1, 22);
    header.writeUInt32LE(sampleRate, 24);
    header.writeUInt32LE(sampleRate * 2, 28);
    header.writeUInt16LE(2, 32);
    header.writeUInt16LE(16, 34);
    header.write("data", 36);
    header.writeUInt32LE(data.length, 40);
    const fixture = path.join(directory, "realtime-fixture.wav");
    await fs.writeFile(fixture, Buffer.concat([header, data]));
    return fixture;
}

async function writeFixtureWav(directory) {
    const sampleRate = 16_000;
    const samples = 8_000;
    const data = Buffer.alloc(samples * 2);
    for (let index = 0; index < samples; index += 1) {
        const sample = Math.round(Math.sin((2 * Math.PI * 440 * index) / sampleRate) * 0.18 * 32_767);
        data.writeInt16LE(sample, index * 2);
    }
    const header = Buffer.alloc(44);
    header.write("RIFF", 0);
    header.writeUInt32LE(36 + data.length, 4);
    header.write("WAVE", 8);
    header.write("fmt ", 12);
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20);
    header.writeUInt16LE(1, 22);
    header.writeUInt32LE(sampleRate, 24);
    header.writeUInt32LE(sampleRate * 2, 28);
    header.writeUInt16LE(2, 32);
    header.writeUInt16LE(16, 34);
    header.write("data", 36);
    header.writeUInt32LE(data.length, 40);
    const fixture = path.join(directory, "fixture.wav");
    await fs.writeFile(fixture, Buffer.concat([header, data]));
    return fixture;
}

async function waitForText(sky, app, elementIndex, expected, timeoutMs = 20_000) {
    const deadline = Date.now() + timeoutMs;
    let lastText = "";
    while (Date.now() < deadline) {
        // A native Cmd+V may occur between Computer Use polls. Request a full
        // AX snapshot so a prior diff baseline cannot hide the new value.
        const state = await sky.get_app_state({ app, disableDiff: true });
        lastText = state.text;
        if (state.text.includes(expected)) return { state, lastText };
        await sleep(250);
    }
    throw new Error(`paste result not observed; element=${elementIndex}; AX=${lastText.slice(0, 1200)}`);
}

export async function run({
    sky,
    implementation = "rust",
    appPath = implementation === "swift"
        ? path.resolve("build/VoicePasteFn.app")
        : "/Applications/VoicePaste.app",
    targetApp = "com.apple.TextEdit",
    keyInjector = path.resolve("VoicePasteTauri/src-tauri/scripts/key_injector.swift"),
    pasteHelper = implementation === "rust"
        ? "/Applications/VoicePaste.app/Contents/MacOS/modifier_monitor"
        : null,
    hotkey = "fn",
    keyCode = hotkey === "f13" ? 105 : 63,
    fixtureWav,
    verifyRealtimePipeline = false,
    emitEvidenceImage = true,
} = {}) {
    if (!sky) throw new Error("run({ sky }) requires the Computer Use sky surface");
    if (targetApp !== "com.apple.TextEdit") {
        throw new Error("the deterministic canary currently supports only com.apple.TextEdit");
    }

    const executable = path.join(
        appPath,
        "Contents/MacOS",
        implementation === "swift" ? "voicepaste-fn" : "voicepaste",
    );
    const helper = implementation === "rust" ? path.join(appPath, "Contents/MacOS/modifier_monitor") : null;
    const expected = `VoicePaste Computer Use ${Date.now()}`;
    const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "voicepaste-cu-"));
    const evidenceDir = path.resolve(".agents/evidence/computer-use", `${Date.now()}-${implementation}`);
    await fs.mkdir(evidenceDir, { recursive: true });
    fixtureWav ||= verifyRealtimePipeline
        ? await writeRealtimeFixtureWav(evidenceDir)
        : await writeFixtureWav(evidenceDir);
    const configPath = path.join(tempDir, "settings.json");
    const config = {
        base_url: "http://127.0.0.1:0/v1",
        api_key: "computer-use-fixture",
        model: "whisper-1",
        remote_models: [],
        local_model: "whisper-base",
        local_command: null,
        remote_provider: "openai",
        language: "auto",
        realtime_preview: verifyRealtimePipeline,
        vad_sensitivity: 0.65,
        vad_silence_ms: verifyRealtimePipeline ? 300 : 500,
        recording_delay: 0.2,
        hide_delay: 0.8,
        hotkey,
        activation_mode: "hold",
        overlay_centered: false,
        wake_server_on_start: false,
        realtime_chunk_interval: 5,
        local_fallback: false,
        autostart: false,
        history_retention_days: 1,
        engine_order: ["remote"],
        ui_language: "en",
    };

    const { server, port, requests } = await startFixtureServer(expected, verifyRealtimePipeline);
    config.base_url = `http://127.0.0.1:${port}/v1`;
    await fs.writeFile(configPath, JSON.stringify(config, null, 2));
    const targetPidResult = await command("pgrep", ["-x", "TextEdit"]);
    const targetPid = targetPidResult.stdout.trim().split(/\s+/)[0];
    if (!/^\d+$/.test(targetPid)) throw new Error(`TextEdit PID unavailable: ${targetPidResult.stdout}`);

    let appProcess;
    let appStdout = "";
    let appStderr = "";
    const evidence = {
        app_path: appPath,
        executable,
        target_app: targetApp,
        target_element_role: "editable text",
        expected,
        fixture_wav: fixtureWav,
        implementation,
        evidence_dir: evidenceDir,
        actions: [],
        blocker: null,
    };

    try {
        appProcess = spawn(executable, [], {
            cwd: path.dirname(executable),
            env: {
                PATH: "/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin",
                ...(implementation === "rust"
                    ? {
                        VOICEPASTE_CONFIG: configPath,
                        VOICEPASTE_MODIFIER_MONITOR: pasteHelper,
                        VOICEPASTE_TEST_OVERLAY_LOG: path.join(evidenceDir, "overlay.log"),
                    }
                    : {
                        OPENAI_BASE_URL: `http://127.0.0.1:${port}/v1`,
                        OPENAI_API_KEY: "computer-use-fixture",
                        TRANSCRIBE_MODEL: "whisper-1",
                    }),
                VOICEPASTE_TEST_AUDIO: fixtureWav,
                ...(verifyRealtimePipeline ? { VOICEPASTE_TEST_LIVE_AUDIO: fixtureWav } : {}),
                VOICEPASTE_TEST_TARGET_PID: targetPid,
                VOICEPASTE_TEST_OVERLAY_LOG: path.join(evidenceDir, "overlay.log"),
            },
            stdio: ["ignore", "pipe", "pipe"],
        });
        appProcess.stdout.on("data", (chunk) => { appStdout += chunk; });
        appProcess.stderr.on("data", (chunk) => { appStderr += chunk; });
        evidence.actions.push({ tool: "spawn", app: appPath });
        evidence.target_pid = Number(targetPid);
        evidence.paste_helper = pasteHelper;
        evidence.app_pid = appProcess.pid;
        evidence.app_identity = await binaryIdentity(appPath);
        evidence.executable_identity = await binaryIdentity(executable);
        if (helper && pasteHelper) evidence.helper_identity = await binaryIdentity(pasteHelper);
        await sleep(1_500);
        evidence.app_exit_code = appProcess.exitCode;
        evidence.app_stderr = appStderr.slice(-2000);
        if (appProcess.exitCode !== null) {
            throw new Error(`Rust app exited early with code ${appProcess.exitCode}: ${appStderr}`);
        }

        const { state: targetState, elementIndex } = await findTextElement(sky, targetApp);
        evidence.initial_ax = targetState.text.slice(0, 1200);
        evidence.element_index = elementIndex;
        await sky.set_value({ app: targetApp, element_index: elementIndex, value: "" });
        // `set_value` updates AXValue but does not reliably make the editor's
        // text view the native first responder. The real paste path sends
        // Cmd+V, so explicitly focus the field before the global hotkey.
        await sky.click({ app: targetApp, element_index: elementIndex });
        evidence.actions.push({ tool: "sky.set_value", app: targetApp, element_index: elementIndex });
        evidence.actions.push({ tool: "sky.click", app: targetApp, element_index: elementIndex });

        const down = await command("swift", [keyInjector, String(keyCode), "down"]);
        evidence.actions.push({ tool: "key_injector", event: "down", key_code: keyCode, exit_code: down.code });
        if (down.code !== 0) throw new Error(`Hotkey down failed: ${down.stderr}`);
        if (verifyRealtimePipeline) {
            await waitForRequestCount(requests, 2);
            const clipboard = await waitForClipboard("preview-one preview-two");
            evidence.preview_clipboard = clipboard;
            evidence.preview_requests = requests.map((request) => ({ ...request }));
            const beforeFinal = await sky.get_app_state({ app: targetApp, disableDiff: true });
            evidence.before_final_ax = beforeFinal.text.slice(0, 1600);
            if (beforeFinal.text.includes("preview-one") || beforeFinal.text.includes("preview-two")) {
                throw new Error("preview draft was inserted before the full-file pass");
            }
        } else {
            await sleep(700);
        }

        evidence.overlay_recording = await probeOverlay(implementation === "swift" ? "VoicePasteFn" : "VoicePaste");
        const overlayLog = implementation === "rust"
            ? await fs.readFile(path.join(evidenceDir, "overlay.log"), "utf8").catch(() => "")
            : "";
        evidence.overlay_recording_log = `${overlayLog}${appStderr.slice(-4000)}`;
        const compactState = implementation === "rust"
            ? overlayLog.includes("recording 58x38")
            : evidence.overlay_recording.some((window) => window.onscreen && window.bounds?.width === 58 && window.bounds?.height === 38);
        if (!compactState) throw new Error(`compact recording overlay state was not observed: ${JSON.stringify(evidence.overlay_recording)}`);

        if (emitEvidenceImage) {
            const imagePath = path.join(evidenceDir, "recording.png");
            const screenshot = await command("screencapture", ["-x", imagePath]);
            evidence.recording_screenshot = imagePath;
            evidence.recording_screenshot_exit = screenshot.code;
            if (screenshot.code === 0) {
                const image = await fs.readFile(imagePath);
                await nodeRepl.emitImage({ bytes: image, mimeType: "image/png" });
            }
        }

        const up = await command("swift", [keyInjector, String(keyCode), "up"]);
        evidence.actions.push({ tool: "key_injector", event: "up", key_code: keyCode, exit_code: up.code });
        if (up.code !== 0) throw new Error(`Hotkey up failed: ${up.stderr}`);

        if (verifyRealtimePipeline) {
            await waitForRequestCount(requests, 3);
            evidence.transcription_requests = requests.map((request) => ({ ...request }));
            if (requests.length !== 3) {
                throw new Error(`expected exactly 2 chunks plus 1 full pass, got ${requests.length}`);
            }
            const [first, second, full] = requests;
            if (!first.has_first || first.has_second || second.has_first || !second.has_second) {
                throw new Error(`preview requests overlap or are out of order: ${JSON.stringify(requests)}`);
            }
            if (!full.has_first || !full.has_second || full.bytes <= first.bytes || full.bytes <= second.bytes) {
                throw new Error(`final request is not the complete WAV: ${JSON.stringify(requests)}`);
            }
        }
        const finalState = await waitForText(sky, targetApp, elementIndex, expected);
        evidence.final_ax = finalState.state.text.slice(0, 1600);
        evidence.overlay_result = await probeOverlay(implementation === "swift" ? "VoicePasteFn" : "VoicePaste");
        const finalOverlayLog = implementation === "rust"
            ? await fs.readFile(path.join(evidenceDir, "overlay.log"), "utf8").catch(() => "")
            : appStderr;
        evidence.overlay_result_log = finalOverlayLog.slice(-4000);
        const previewObserved = implementation === "rust"
            ? /preview (?!360x100)\d+x\d+/.test(finalOverlayLog)
            : evidence.overlay_result.some((window) => window.onscreen && window.bounds?.width > 58);
        if (!previewObserved) {
            throw new Error(`preview overlay state was not observed: ${finalOverlayLog}`);
        }
        evidence.result = "PASS";
        await fs.writeFile(path.join(evidenceDir, "evidence.json"), JSON.stringify(evidence, null, 2));
        return evidence;
    } catch (error) {
        evidence.app_exit_code = appProcess?.exitCode ?? null;
        evidence.app_stdout = appStdout.slice(-2000);
        evidence.app_stderr = appStderr.slice(-2000);
        evidence.blocker = String(error?.stack || error);
        evidence.result = "FAIL";
        await fs.writeFile(path.join(evidenceDir, "evidence.json"), JSON.stringify(evidence, null, 2));
        throw Object.assign(new Error(evidence.blocker), { evidence });
    } finally {
        server.close();
        if (appProcess && appProcess.exitCode === null) appProcess.kill("SIGTERM");
        await fs.rm(tempDir, { recursive: true, force: true });
    }
}
