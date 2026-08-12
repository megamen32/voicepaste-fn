// VoicePaste overlay — listens for Tauri events and updates the UI.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const overlay = document.getElementById("overlay");
const textEl = document.getElementById("text");
const content = document.getElementById("content");
const retryActions = document.getElementById("retry-actions");
const retryButtons = [...retryActions.querySelectorAll("[data-engine]")];
const closeErrorButton = document.getElementById("close-error-button");

const t = (key) => window.voicePasteI18n.t(key);

async function initializeUiLanguage() {
    try {
        const language = await invoke("initialize_ui_language", {
            locale: navigator.language || "",
        });
        window.voicePasteI18n.setLanguage(language);
    } catch (error) {
        // The overlay still works if an old binary does not have the command.
        window.voicePasteI18n.setLanguage(navigator.language || "en");
        console.error("UI language initialization failed:", error);
    }
}

initializeUiLanguage();
listen("ui-language-changed", (event) => {
    window.voicePasteI18n.setLanguage(event.payload);
});

let currentState = "hidden";
// Listen for overlay state changes from Rust
listen("overlay-state", (event) => {
    const { state, text } = event.payload;
    updateOverlay(state, text);
});

listen("hotkey-error", (event) => {
    const message = typeof event.payload === "string" ? event.payload : t("hotkeyError");
    updateOverlay("hotkey-error", message);
    setTimeout(() => {
        if (currentState === "hotkey-error") {
            invoke("dismiss_overlay").catch(console.error);
            updateOverlay("hidden");
        }
    }, 8000);
});

listen("paste-error", (event) => {
    const message = typeof event.payload === "string" ? event.payload : t("transcriptionError");
    updateOverlay("paste-error", message);
    setTimeout(() => {
        if (currentState === "paste-error") {
            invoke("dismiss_overlay").catch(console.error);
            updateOverlay("hidden");
        }
    }, 8000);
});

// Listen for dialog events — resize window then show dialog
listen("dialog-endpoint", () => {
    invoke("show_dialog").then(() => {
        showDialog("dialog-endpoint", "endpoint-input", "save_endpoint", "url");
    }).catch(console.error);
});

listen("dialog-api-key", () => {
    invoke("show_dialog").then(() => {
        showDialog("dialog-api-key", "api-key-input", "save_api_key", "key");
    }).catch(console.error);
});

listen("dialog-permissions", () => {
    invoke("get_permissions").then((perms) => {
        const mic = perms.microphone ? `✓ ${t("granted")}` : `✗ ${t("notGranted")}`;
        const ax = perms.accessibility ? `✓ ${t("granted")}` : `✗ ${t("notGranted")}`;
        alert(`${t("permissionsTitle")}\n\nMicrophone: ${mic}\nAccessibility: ${ax}\n\n${t("permissionsHint")}`);
    }).catch(console.error);
});

function updateOverlay(state, text) {
    // Remove old state classes
    overlay.classList.remove("recording", "waiting", "preview", "retry", "error");
    retryActions.classList.add("hidden");
    closeErrorButton.classList.add("hidden");
    content.title = "";

    switch (state) {
        case "recording":
            overlay.classList.add("recording");
            overlay.classList.remove("hidden");
            textEl.textContent = t("recording");
            content.style.pointerEvents = "none";
            break;

        case "waiting":
            overlay.classList.add("waiting");
            overlay.classList.remove("hidden");
            textEl.textContent = t("processing");
            content.style.pointerEvents = "none";
            break;

        case "preview":
            overlay.classList.add("preview");
            overlay.classList.remove("hidden");
            textEl.textContent = text || "";
            content.style.pointerEvents = "none";
            break;

        case "retry":
        case "error":
            overlay.classList.add("error");
            overlay.classList.remove("hidden");
            textEl.textContent = text || t("transcriptionError");
            content.title = "";
            content.style.pointerEvents = "auto";
            overlay.classList.add("retry");
            retryActions.classList.remove("hidden");
            closeErrorButton.classList.remove("hidden");
            break;

        case "hotkey-error":
        case "paste-error":
            overlay.classList.add("error");
            overlay.classList.remove("hidden");
            textEl.textContent = text || t("hotkeyError");
            content.title = "";
            content.style.pointerEvents = "auto";
            closeErrorButton.classList.remove("hidden");
            break;

        default:
            overlay.classList.add("hidden");
            content.style.pointerEvents = "none";
            content.onclick = null;
            break;
    }

    currentState = state;
}

retryButtons.forEach((button) => {
    button.addEventListener("click", () => {
        invoke("retry_transcription", { engine: button.dataset.engine }).catch(console.error);
    });
});

closeErrorButton.addEventListener("click", () => {
    const command = currentState === "retry" || currentState === "error"
        ? "dismiss_transcription_error"
        : "dismiss_overlay";
    invoke(command).catch(console.error);
    updateOverlay("hidden");
});

function showDialog(dialogId, inputId, command, paramName) {
    // Hide all dialogs first
    document.querySelectorAll(".dialog").forEach(d => d.classList.add("hidden"));

    const dialog = document.getElementById(dialogId);
    const input = document.getElementById(inputId);
    dialog.classList.remove("hidden");

    // Move dialog into view
    overlay.classList.remove("hidden");
    overlay.classList.add("dialog-active");

    setTimeout(() => input.focus(), 100);

    // Find buttons by ID pattern
    const buttons = dialog.querySelectorAll("button");
    const saveBtn = buttons[0];
    const cancelBtn = buttons[1];

    const close = () => {
        dialog.classList.add("hidden");
        overlay.classList.remove("dialog-active");
        input.value = "";
        // Restore small overlay size
        invoke("hide_dialog").catch(console.error);
    };

    saveBtn.onclick = () => {
        const value = input.value;
        invoke(command, { [paramName]: value })
            .then(() => {
                close();
            })
            .catch((e) => {
                console.error("Save error:", e);
                alert(t("errorPrefix") + e);
            });
    };

    cancelBtn.onclick = close;

    // Enter key saves
    input.onkeydown = (e) => {
        if (e.key === "Enter") saveBtn.click();
        if (e.key === "Escape") cancelBtn.click();
    };
}
