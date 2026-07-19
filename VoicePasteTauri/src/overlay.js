// VoicePaste overlay — listens for Tauri events and updates the UI.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const overlay = document.getElementById("overlay");
const textEl = document.getElementById("text");
const content = document.getElementById("content");

let currentState = "hidden";
let dotInterval = null;
let dotCount = 0;

// Listen for overlay state changes from Rust
listen("overlay-state", (event) => {
    const { state, text } = event.payload;
    updateOverlay(state, text);
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
        const mic = perms.microphone ? "✓ Granted" : "✗ Not granted";
        const ax = perms.accessibility ? "✓ Granted" : "✗ Not granted";
        alert(`Permissions:\n\nMicrophone: ${mic}\nAccessibility: ${ax}\n\nOpen System Settings to grant access.`);
    }).catch(console.error);
});

function updateOverlay(state, text) {
    // Clear previous animation
    if (dotInterval) {
        clearInterval(dotInterval);
        dotInterval = null;
    }

    // Remove old state classes
    overlay.classList.remove("recording", "waiting", "preview", "retry");

    switch (state) {
        case "recording":
            overlay.classList.add("recording");
            overlay.classList.remove("hidden");
            textEl.textContent = "● REC";
            content.style.pointerEvents = "none";
            break;

        case "waiting":
            overlay.classList.add("waiting");
            overlay.classList.remove("hidden");
            dotCount = 0;
            textEl.textContent = "·";
            content.style.pointerEvents = "none";
            dotInterval = setInterval(() => {
                dotCount = (dotCount % 3) + 1;
                textEl.textContent = "·".repeat(dotCount);
            }, 400);
            break;

        case "preview":
            overlay.classList.add("preview");
            overlay.classList.remove("hidden");
            textEl.textContent = text || "";
            content.style.pointerEvents = "none";
            break;

        case "retry":
            overlay.classList.add("retry");
            overlay.classList.remove("hidden");
            textEl.textContent = "↩";
            content.style.pointerEvents = "auto";
            content.onclick = () => {
                invoke("retry_transcription").catch(console.error);
            };
            break;

        default:
            overlay.classList.add("hidden");
            content.style.pointerEvents = "none";
            content.onclick = null;
            break;
    }

    currentState = state;
}

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
                alert("Error: " + e);
            });
    };

    cancelBtn.onclick = close;

    // Enter key saves
    input.onkeydown = (e) => {
        if (e.key === "Enter") saveBtn.click();
        if (e.key === "Escape") cancelBtn.click();
    };
}

// Handle body click for retry
content.addEventListener("click", () => {
    if (currentState === "retry") {
        invoke("retry_transcription").catch(console.error);
    }
});
