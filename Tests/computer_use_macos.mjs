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
    const bundleID = await command("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleIdentifier", path.join(bundleRoot, "Info.plist")]);
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

async function startFixtureServer(expectedText) {
    const server = http.createServer((request, response) => {
        if (request.method !== "POST" || !request.url.endsWith("/audio/transcriptions")) {
            response.writeHead(404).end();
            return;
        }
        request.resume();
        request.on("end", () => {
            const payload = JSON.stringify({ text: expectedText });
            response.writeHead(200, { "content-type": "application/json" });
            response.end(payload);
        });
    });
    await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });
    return { server, port: server.address().port };
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
        const state = await sky.get_app_state({ app });
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
    fixtureWav,
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
    fixtureWav ||= await writeFixtureWav(evidenceDir);
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
        realtime_preview: false,
        recording_delay: 0.2,
        hide_delay: 0.8,
        hotkey: "fn",
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

    const { server, port } = await startFixtureServer(expected);
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
        evidence.actions.push({ tool: "sky.set_value", app: targetApp, element_index: elementIndex });

        const down = await command("swift", [keyInjector, "63", "down"]);
        evidence.actions.push({ tool: "key_injector", event: "down", exit_code: down.code });
        if (down.code !== 0) throw new Error(`Fn down failed: ${down.stderr}`);
        await sleep(700);

        evidence.overlay_recording = await probeOverlay(implementation === "swift" ? "VoicePasteFn" : "VoicePaste");
        const overlayLog = implementation === "rust"
            ? await fs.readFile(path.join(evidenceDir, "overlay.log"), "utf8").catch(() => "")
            : "";
        evidence.overlay_recording_log = `${overlayLog}${appStderr.slice(-4000)}`;
        const compactState = implementation === "rust"
            ? overlayLog.includes("recording 148x48")
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

        const up = await command("swift", [keyInjector, "63", "up"]);
        evidence.actions.push({ tool: "key_injector", event: "up", exit_code: up.code });
        if (up.code !== 0) throw new Error(`Fn up failed: ${up.stderr}`);

        const finalState = await waitForText(sky, targetApp, elementIndex, expected);
        evidence.final_ax = finalState.state.text.slice(0, 1600);
        evidence.overlay_result = await probeOverlay(implementation === "swift" ? "VoicePasteFn" : "VoicePaste");
        const finalOverlayLog = implementation === "rust"
            ? await fs.readFile(path.join(evidenceDir, "overlay.log"), "utf8").catch(() => "")
            : appStderr;
        evidence.overlay_result_log = finalOverlayLog.slice(-4000);
        const previewObserved = implementation === "rust"
            ? finalOverlayLog.includes("preview 360x100")
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
