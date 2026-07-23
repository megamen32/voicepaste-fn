(function () {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    const copy = {
        en: {
            settingsApp: "Settings", general: "General", models: "Models", remote: "Remote", advanced: "Advanced", history: "History", permissions: "Permissions", appLanguage: "Application language",
            generalSubtitle: "Tune recording and recognition without leaving the app.", modelsSubtitle: "Local models stay on this device. Pick the provider that fits your hardware.", remoteSubtitle: "OpenAI, OpenRouter or any OpenAI-compatible transcription endpoint.", advancedSubtitle: "Rare switches for custom deployments and diagnostics.", permissionsSubtitle: "VoicePaste needs microphone access to record and accessibility access to paste into other apps.",
            saveChanges: "Save changes", saved: "Saved", saving: "Saving…", saveFailed: "Could not save", engineTitle: "Recognition engine", engineHint: "Choose one or more providers. They run in this order as fallbacks.", remoteEngine: "Remote", remoteEngineHint: "OpenAI-compatible API", localEngine: "Local", localEngineHint: "Whisper / Parakeet", nativeEngine: "Native", nativeEngineHint: "Built-in platform speech",
            activationTitle: "Activation", activationHint: "How the hotkey controls a recording.", activationMode: "Mode", hotkey: "Hotkey template", hold: "Hold", toggle: "Toggle", speechTitle: "Speech", speechHint: "Language and timing for the recognizer.", speechLanguage: "Transcription language", russian: "Russian", english: "English", chinese: "Chinese", automatic: "Automatic", recordingDelay: "Recording delay", previewTitle: "Preview", previewHint: "Keep feedback visible while speech is processed.", realtimePreview: "Realtime preview", realtimeHint: "Show partial text during recording", previewDelay: "Hide preview after", previewCadence: "Preview cadence", startupTitle: "Startup", startupHint: "Make the small workflow feel automatic.", autostart: "Start with the system", autostartHint: "Launch VoicePaste when you sign in", centerOverlay: "Center recording window", centerHint: "Otherwise show it near the pointer",
            modelsTitle: "Models", whisperDescription: "Reliable whisper.cpp model. Download once and use offline on macOS, Windows or Ubuntu.", parakeetDescription: "Fast local provider. Connect a Parakeet/sherpa CLI below; the model files are managed by that runtime.", parakeetNote: "Best for users who already have a local Parakeet runtime.", downloadModel: "Download", openFolder: "Open folder", useModel: "Use model", modelPage: "Model page", nativeDescription: "Use the platform speech framework. On macOS this is Apple Speech; other platforms may not provide it.", useEngine: "Use engine", ready: "Ready", notDownloaded: "Not downloaded", commandReady: "Command ready", commandMissing: "Command missing", macOnly: "macOS only", downloading: "Downloading…", downloadError: "Download failed",
            remoteTitle: "Remote provider", provider: "Provider template", customEndpoint: "Custom endpoint", endpoint: "API endpoint", remoteModel: "Remote model", apiKey: "API key", apiKeySet: "Saved key: ", clearApiKey: "Clear the saved key", proxyTitle: "Proxy respected", proxyHint: "VoicePaste uses the system and environment proxy settings. No proxy password is copied into this UI.", noProxy: "No proxy variables detected; system proxy behavior is still enabled.", refresh: "Refresh", modelsFound: "models found", noModels: "No models returned",
            advancedTitle: "Advanced", warmupTitle: "Warm up remote server at speech start", warmupHint: "Sends an empty audio request. Useful for a custom Whisper server that sleeps between requests.", localCommand: "Parakeet command template", localCommandHint: "The command must write plain text to {output_path}; stdout also works. The downloaded model directory is {model_dir}.", historyTitle: "Transcription history", historySubtitle: "Completed dictations stay searchable on this device.", historyRetention: "Keep history", historyRetentionHint: "Old entries are removed automatically. Audio is not stored in history.", retention7: "7 days", retention30: "30 days", retention90: "90 days", retentionForever: "Forever", clearHistory: "Clear history", noHistory: "No transcriptions yet", historyCount: "entries", permissionsTitle: "Permissions", microphone: "Microphone", accessibility: "Accessibility", granted: "Granted", notGranted: "Not granted", refreshPermissions: "Refresh status", openSystemSettings: "Open system settings", settingsFooter: "Changes are saved to your local VoicePaste configuration.",
            generalSection: "General", modelsSection: "Models", remoteSection: "Remote", advancedSection: "Advanced", historySection: "History", permissionsSection: "Permissions", unavailable: "Unavailable"
        },
        ru: {
            settingsApp: "Настройки", general: "Общие", models: "Модели", remote: "Удалённый", advanced: "Расширенные", history: "История", permissions: "Разрешения", appLanguage: "Язык приложения",
            generalSubtitle: "Настройте запись и распознавание прямо в приложении.", modelsSubtitle: "Локальные модели остаются на этом устройстве. Выберите подходящий провайдер.", remoteSubtitle: "OpenAI, OpenRouter или любой OpenAI-совместимый endpoint.", advancedSubtitle: "Редкие параметры для кастомных серверов и диагностики.", permissionsSubtitle: "VoicePaste нужен микрофон для записи и доступность для вставки в другие приложения.",
            saveChanges: "Сохранить изменения", saved: "Сохранено", saving: "Сохранение…", saveFailed: "Не удалось сохранить", engineTitle: "Движок распознавания", engineHint: "Выберите один или несколько провайдеров. Они работают по порядку как fallback.", remoteEngine: "Удалённый", remoteEngineHint: "OpenAI-совместимый API", localEngine: "Локальный", localEngineHint: "Whisper / Parakeet", nativeEngine: "Системный", nativeEngineHint: "Встроенная речь платформы",
            activationTitle: "Активация", activationHint: "Как горячая клавиша управляет записью.", activationMode: "Режим", hotkey: "Шаблон горячей клавиши", hold: "Удержание", toggle: "Переключатель", speechTitle: "Речь", speechHint: "Язык и тайминги распознавания.", speechLanguage: "Язык транскрипции", russian: "Русский", english: "Английский", chinese: "Китайский", automatic: "Авто", recordingDelay: "Задержка записи", previewTitle: "Предпросмотр", previewHint: "Показывать обратную связь во время обработки речи.", realtimePreview: "Предпросмотр в реальном времени", realtimeHint: "Показывать промежуточный текст во время записи", previewDelay: "Скрывать через", previewCadence: "Интервал предпросмотра", startupTitle: "Запуск", startupHint: "Сделайте короткий сценарий полностью автоматическим.", autostart: "Запускать вместе с системой", autostartHint: "Запускать VoicePaste при входе в систему", centerOverlay: "Центрировать окно записи", centerHint: "Иначе показывать его возле указателя",
            modelsTitle: "Модели", whisperDescription: "Надёжная модель whisper.cpp. Скачайте один раз и работайте офлайн на macOS, Windows или Ubuntu.", parakeetDescription: "Быстрый локальный провайдер. Подключите CLI Parakeet/sherpa ниже; файлы модели управляются этим runtime.", parakeetNote: "Подходит, если Parakeet уже установлен локально.", downloadModel: "Скачать", openFolder: "Открыть папку", useModel: "Использовать", modelPage: "Страница модели", nativeDescription: "Использовать системный speech framework. На macOS это Apple Speech; на других платформах может отсутствовать.", useEngine: "Использовать", ready: "Готово", notDownloaded: "Не скачана", commandReady: "Команда настроена", commandMissing: "Нет команды", macOnly: "только macOS", downloading: "Скачивание…", downloadError: "Ошибка скачивания",
            remoteTitle: "Удалённый провайдер", provider: "Шаблон провайдера", customEndpoint: "Свой endpoint", endpoint: "API endpoint", remoteModel: "Удалённая модель", apiKey: "API-ключ", apiKeySet: "Сохранённый ключ: ", clearApiKey: "Удалить сохранённый ключ", proxyTitle: "Прокси учитывается", proxyHint: "VoicePaste использует системные и env-настройки прокси. Пароль прокси не копируется в этот интерфейс.", noProxy: "Переменные прокси не найдены; системное поведение прокси всё равно включено.", refresh: "Обновить", modelsFound: "моделей найдено", noModels: "Модели не вернулись",
            advancedTitle: "Расширенные", warmupTitle: "Прогревать удалённый сервер в начале речи", warmupHint: "Отправляет пустой запрос аудио. Нужно для кастомного Whisper-сервера, который засыпает между запросами.", localCommand: "Команда Parakeet", localCommandHint: "Команда должна записать обычный текст в {output_path}; stdout тоже поддерживается. Каталог скачанной модели доступен как {model_dir}.", historyTitle: "История транскрипций", historySubtitle: "Готовые транскрипции остаются доступными на этом устройстве.", historyRetention: "Хранить историю", historyRetentionHint: "Старые записи удаляются автоматически. Аудио в истории не хранится.", retention7: "7 дней", retention30: "30 дней", retention90: "90 дней", retentionForever: "Навсегда", clearHistory: "Очистить историю", noHistory: "Транскрипций пока нет", historyCount: "записей", permissionsTitle: "Разрешения", microphone: "Микрофон", accessibility: "Доступность", granted: "Разрешено", notGranted: "Не разрешено", refreshPermissions: "Обновить статус", openSystemSettings: "Открыть системные настройки", settingsFooter: "Изменения сохраняются в локальную конфигурацию VoicePaste.",
            generalSection: "Общие", modelsSection: "Модели", remoteSection: "Удалённый", advancedSection: "Расширенные", historySection: "История", permissionsSection: "Разрешения", unavailable: "Недоступно"
        },
        zh: {
            settingsApp: "设置", general: "常规", models: "模型", remote: "远程", advanced: "高级", history: "历史", permissions: "权限", appLanguage: "应用语言",
            generalSubtitle: "直接在应用中调整录音和识别。", modelsSubtitle: "本地模型保存在此设备。选择适合硬件的提供商。", remoteSubtitle: "OpenAI、OpenRouter 或任何兼容 OpenAI 的识别接口。", advancedSubtitle: "用于自定义部署和诊断的少量选项。", permissionsSubtitle: "VoicePaste 需要麦克风录音，并需要辅助功能权限粘贴到其他应用。",
            saveChanges: "保存更改", saved: "已保存", saving: "保存中…", saveFailed: "保存失败", engineTitle: "识别引擎", engineHint: "可选择多个提供商，按顺序作为回退运行。", remoteEngine: "远程", remoteEngineHint: "兼容 OpenAI 的 API", localEngine: "本地", localEngineHint: "Whisper / Parakeet", nativeEngine: "原生", nativeEngineHint: "平台内置语音",
            activationTitle: "激活", activationHint: "快捷键如何控制录音。", activationMode: "模式", hotkey: "快捷键模板", hold: "按住", toggle: "切换", speechTitle: "语音", speechHint: "识别语言和时间设置。", speechLanguage: "转录语言", russian: "俄语", english: "英语", chinese: "中文", automatic: "自动", recordingDelay: "录音延迟", previewTitle: "预览", previewHint: "处理语音时保持反馈可见。", realtimePreview: "实时预览", realtimeHint: "录音时显示部分文本", previewDelay: "预览隐藏延迟", previewCadence: "预览间隔", startupTitle: "启动", startupHint: "让短流程更自动化。", autostart: "随系统启动", autostartHint: "登录时启动 VoicePaste", centerOverlay: "居中录音窗口", centerHint: "否则显示在指针附近",
            modelsTitle: "模型", whisperDescription: "可靠的 whisper.cpp 模型。下载一次后可在 macOS、Windows 或 Ubuntu 离线使用。", parakeetDescription: "快速本地提供商。连接下方的 Parakeet/sherpa CLI，模型文件由该 runtime 管理。", parakeetNote: "适合已经安装本地 Parakeet runtime 的用户。", downloadModel: "下载", openFolder: "打开文件夹", useModel: "使用模型", modelPage: "模型页面", nativeDescription: "使用平台语音框架。macOS 使用 Apple Speech，其他平台可能不可用。", useEngine: "使用引擎", ready: "就绪", notDownloaded: "未下载", commandReady: "命令已配置", commandMissing: "缺少命令", macOnly: "仅 macOS", downloading: "下载中…", downloadError: "下载失败",
            remoteTitle: "远程提供商", provider: "提供商模板", customEndpoint: "自定义接口", endpoint: "API 接口", remoteModel: "远程模型", apiKey: "API 密钥", apiKeySet: "已保存密钥：", clearApiKey: "清除已保存密钥", proxyTitle: "支持代理", proxyHint: "VoicePaste 使用系统和环境代理设置。代理密码不会复制到此界面。", noProxy: "未检测到代理变量；系统代理行为仍然启用。", refresh: "刷新", modelsFound: "个模型", noModels: "没有返回模型",
            advancedTitle: "高级", warmupTitle: "开始讲话时预热远程服务器", warmupHint: "发送空音频请求。适用于请求之间会休眠的自定义 Whisper 服务器。", localCommand: "Parakeet 命令模板", localCommandHint: "命令必须将纯文本写入 {output_path}；也支持 stdout。下载的模型目录为 {model_dir}。", historyTitle: "转录历史", historySubtitle: "完成的听写会保存在此设备上。", historyRetention: "保留历史", historyRetentionHint: "旧记录会自动删除。历史记录不保存音频。", retention7: "7 天", retention30: "30 天", retention90: "90 天", retentionForever: "永久", clearHistory: "清空历史", noHistory: "还没有转录", historyCount: "条记录", permissionsTitle: "权限", microphone: "麦克风", accessibility: "辅助功能", granted: "已允许", notGranted: "未允许", refreshPermissions: "刷新状态", openSystemSettings: "打开系统设置", settingsFooter: "更改保存到本地 VoicePaste 配置。",
            generalSection: "常规", modelsSection: "模型", remoteSection: "远程", advancedSection: "高级", historySection: "历史", permissionsSection: "权限", unavailable: "不可用"
        }
    };

    const hotkeys = [
        ["fn", "Fn (Globe 🌐)"], ["right_option", "Right ⌥ Option"], ["right_control", "Right ⌃ Control"],
        ["right_command", "Right ⌘ Command"], ["right_shift", "Right ⇧ Shift"], ["caps_lock", "Caps Lock"],
        ["f13", "F13"], ["f14", "F14"], ["f15", "F15"]
    ];
    const sectionCopy = {
        general: ["generalSection", "generalSubtitle"], models: ["modelsSection", "modelsSubtitle"], remote: ["remoteSection", "remoteSubtitle"], advanced: ["advancedSection", "advancedSubtitle"], history: ["historySection", "historySubtitle"], permissions: ["permissionsSection", "permissionsSubtitle"]
    };
    let language = "en";
    let state = null;

    const $ = (id) => document.getElementById(id);
    const t = (key) => (copy[language] && copy[language][key]) || copy.en[key] || key;
    const setText = (element, value) => { if (element) element.textContent = value; };

    function applyTranslations() {
        document.documentElement.lang = language;
        document.querySelectorAll("[data-i18n]").forEach((element) => setText(element, t(element.dataset.i18n)));
        const active = document.querySelector(".nav-item.active");
        if (active) setSection(active.dataset.section, false);
        setText($("download-whisper"), t("downloadModel"));
        setText($("refresh-remote-models"), t("refresh"));
        setText($("save-button"), t("saveChanges"));
    }

    function setLanguage(value) {
        language = ["ru", "zh"].includes(value) ? value : "en";
        if (window.voicePasteI18n) window.voicePasteI18n.setLanguage(language);
        $("ui-language").value = language;
        applyTranslations();
        if (state) render();
    }

    function setSection(section, focus) {
        document.querySelectorAll(".section").forEach((element) => element.classList.toggle("active", element.id === `section-${section}`));
        document.querySelectorAll(".nav-item").forEach((element) => element.classList.toggle("active", element.dataset.section === section));
        const keys = sectionCopy[section] || sectionCopy.general;
        setText($("page-title"), t(keys[0]));
        setText($("page-subtitle"), t(keys[1]));
        if (focus) $("page-title").focus();
    }

    function fillHotkeys() {
        const select = $("hotkey");
        select.innerHTML = "";
        hotkeys.forEach(([value, label]) => {
            const option = document.createElement("option");
            option.value = value;
            option.textContent = label;
            select.appendChild(option);
        });
    }

    function formatBytes(bytes) {
        if (!bytes) return "";
        const units = ["B", "KB", "MB", "GB"];
        let value = bytes;
        let index = 0;
        while (value >= 1024 && index < units.length - 1) { value /= 1024; index += 1; }
        return `${value.toFixed(index ? 1 : 0)} ${units[index]}`;
    }

    function renderLocalStatus() {
        const statuses = state.model_statuses || {};
        const whisper = statuses["whisper-base"] || {};
        const parakeet = statuses["parakeet-v3"] || {};
        const whisperReady = whisper.model_ready === true;
        setText($("whisper-status"), whisperReady ? t("ready") : t("notDownloaded"));
        $("whisper-status").className = `badge ${whisperReady ? "good" : ""}`;
        setText($("whisper-path"), whisperReady ? `${whisper.path || ""} · ${formatBytes(whisper.bytes)}` : state.models_dir || "");
        const parakeetReady = parakeet.model_ready === true;
        const parakeetRuntimeReady = parakeet.runtime_configured === true;
        setText($("parakeet-status"), !parakeetReady ? t("notDownloaded") : (parakeetRuntimeReady ? t("ready") : t("commandMissing")));
        $("parakeet-status").className = `badge ${parakeetReady ? "good" : ""}`;
        setText($("parakeet-runtime"), parakeetReady && !parakeetRuntimeReady ? t("commandMissing") : (parakeet.path || t("parakeetNote")));
        setText($("native-status"), state.native_available ? t("ready") : t("macOnly"));
        $("download-whisper").disabled = state.downloadInProgress;
        $("download-parakeet").disabled = state.downloadInProgress;
        $("use-whisper").disabled = !whisperReady;
        $("select-parakeet").disabled = !parakeetReady;
    }

    function renderPermissions() {
        const permissions = state.permissions || {};
        setText($("mic-permission"), permissions.microphone ? t("granted") : t("notGranted"));
        setText($("accessibility-permission"), permissions.accessibility ? t("granted") : t("notGranted"));
    }

    function render() {
        $("activation").value = state.activation_mode;
        $("hotkey").value = state.hotkey;
        $("speech-language").value = state.language;
        $("recording-delay").value = state.recording_delay;
        $("preview-delay").value = state.hide_delay;
        $("preview-cadence").value = state.realtime_chunk_interval;
        $("realtime-preview").checked = state.realtime_preview;
        $("autostart").checked = state.autostart;
        $("overlay-centered").checked = state.overlay_centered;
        $("warmup").checked = state.wake_server_on_start;
        $("history-retention").value = String(state.history_retention_days ?? 30);
        $("remote-provider").value = state.remote_provider || "openai";
        $("endpoint").value = state.base_url || "";
        $("remote-model").value = state.model || "whisper-1";
        $("local-command").value = state.local_command || "";
        $("ui-language").value = language;
        ["remote", "local", "native"].forEach((engine) => { $(`engine-${engine}`).checked = (state.engine_order || []).includes(engine); });
        setText($("recording-delay-value"), `${Number(state.recording_delay).toFixed(1)} s`);
        setText($("preview-delay-value"), `${Number(state.hide_delay).toFixed(1)} s`);
        setText($("preview-cadence-value"), `${Number(state.realtime_chunk_interval).toFixed(0)} s`);
        setText($("api-key-state"), state.api_key_set ? `${t("apiKeySet")}${state.api_key_masked}` : "");
        setText($("proxy-vars"), state.proxy_env && state.proxy_env.length ? state.proxy_env.join(" · ") : t("noProxy"));
        setText($("config-path"), state.config_path || "");
        setText($("engine-summary"), (state.engine_order || []).join(" → "));
        renderLocalStatus();
        renderPermissions();
        renderHistory(state.history || []);
    }

    function updateRangeOutputs() {
        setText($("recording-delay-value"), `${Number($("recording-delay").value).toFixed(1)} s`);
        setText($("preview-delay-value"), `${Number($("preview-delay").value).toFixed(1)} s`);
        setText($("preview-cadence-value"), `${Number($("preview-cadence").value).toFixed(0)} s`);
    }

    function collectPatch() {
        const engines = ["remote", "local", "native"].filter((engine) => $(`engine-${engine}`).checked);
        return {
            base_url: $("endpoint").value.trim(),
            model: $("remote-model").value.trim() || "whisper-1",
            local_model: state.local_model,
            local_command: $("local-command").value,
            remote_provider: $("remote-provider").value,
            language: $("speech-language").value,
            realtime_preview: $("realtime-preview").checked,
            recording_delay: Number($("recording-delay").value),
            hide_delay: Number($("preview-delay").value),
            hotkey: $("hotkey").value,
            activation_mode: $("activation").value,
            overlay_centered: $("overlay-centered").checked,
            wake_server_on_start: $("warmup").checked,
            realtime_chunk_interval: Number($("preview-cadence").value),
            autostart: $("autostart").checked,
            history_retention_days: Number($("history-retention").value),
            engine_order: engines.length ? engines : ["remote"],
            ui_language: language,
            ...(($("api-key").value.trim()) ? { api_key: $("api-key").value.trim() } : {}),
            ...(($("clear-api-key").checked) ? { clear_api_key: true } : {})
        };
    }

    async function save() {
        setText($("save-status"), t("saving"));
        try {
            state = await invoke("save_settings", { patch: collectPatch() });
            $("api-key").value = "";
            $("clear-api-key").checked = false;
            setText($("save-status"), t("saved"));
            render();
            await loadHistory();
            setTimeout(() => setText($("save-status"), ""), 1600);
        } catch (error) {
            setText($("save-status"), `${t("saveFailed")}: ${error}`);
        }
    }

    async function load() {
        state = await invoke("get_settings");
        language = state.ui_language || language;
        applyTranslations();
        render();
    }

    async function selectLocalModel(model) {
        state.local_model = model;
        const order = new Set(state.engine_order || []);
        order.add("local");
        state.engine_order = Array.from(order);
        await save();
    }

    async function selectNative() {
        const order = new Set(state.engine_order || []);
        order.add("native");
        state.engine_order = Array.from(order);
        await save();
    }

    function renderHistory(entries) {
        const list = $("history-list");
        if (!list) return;
        list.innerHTML = "";
        setText($("history-count"), entries.length ? `${entries.length} ${t("historyCount")}` : "");
        if (!entries.length) {
            const empty = document.createElement("p");
            empty.className = "muted";
            empty.textContent = t("noHistory");
            list.appendChild(empty);
            return;
        }
        entries.forEach((entry) => {
            const item = document.createElement("article");
            item.className = "history-item";
            const header = document.createElement("header");
            const date = new Date(Number(entry.created_at) * 1000);
            header.textContent = `${date.toLocaleString(language)} · ${entry.language || "auto"} · ${entry.engine || "cascade"}`;
            const text = document.createElement("p");
            text.textContent = entry.text || "";
            item.append(header, text);
            list.appendChild(item);
        });
    }

    async function loadHistory() {
        try {
            state.history = await invoke("get_history");
            renderHistory(state.history);
        } catch (error) {
            setText($("history-count"), `${t("saveFailed")}: ${error}`);
        }
    }

    async function downloadModel(model) {
        state.downloadInProgress = true;
        renderLocalStatus();
        try {
            await invoke("download_local_model", { model });
        } catch (error) {
            state.downloadInProgress = false;
            setText($("save-status"), `${t("downloadError")}: ${error}`);
            renderLocalStatus();
        }
    }

    async function refreshModels() {
        setText($("refresh-remote-models"), t("saving"));
        try {
            const models = await invoke("refresh_remote_models");
            const list = $("remote-model-options");
            list.innerHTML = "";
            models.forEach((model) => { const option = document.createElement("option"); option.value = model; list.appendChild(option); });
            setText($("save-status"), models.length ? `${models.length} ${t("modelsFound")}` : t("noModels"));
        } catch (error) {
            setText($("save-status"), `${t("saveFailed")}: ${error}`);
        } finally {
            setText($("refresh-remote-models"), t("refresh"));
        }
    }

    function applyProviderTemplate() {
        const provider = $("remote-provider").value;
        const endpoints = { openai: "https://api.openai.com/v1", openrouter: "https://openrouter.ai/api/v1" };
        if (endpoints[provider]) $("endpoint").value = endpoints[provider];
    }

    async function refreshPermissions() {
        state.permissions = await invoke("get_permissions");
        renderPermissions();
    }

    async function init() {
        fillHotkeys();
        document.querySelectorAll(".nav-item").forEach((button) => button.addEventListener("click", () => setSection(button.dataset.section, true)));
        $("save-button").addEventListener("click", save);
        $("ui-language").addEventListener("change", async (event) => { setLanguage(event.target.value); await save(); });
        $("remote-provider").addEventListener("change", applyProviderTemplate);
        $("download-whisper").addEventListener("click", () => downloadModel("whisper-base"));
        $("download-parakeet").addEventListener("click", () => downloadModel("parakeet-v3"));
        $("use-whisper").addEventListener("click", () => selectLocalModel("whisper-base"));
        $("open-models-folder").addEventListener("click", () => invoke("open_models_folder"));
        $("open-parakeet").addEventListener("click", () => invoke("open_model_page"));
        $("select-parakeet").addEventListener("click", () => selectLocalModel("parakeet-v3"));
        $("select-native").addEventListener("click", selectNative);
        $("clear-history").addEventListener("click", async () => { await invoke("clear_history"); await loadHistory(); });
        $("refresh-remote-models").addEventListener("click", refreshModels);
        $("refresh-permissions").addEventListener("click", refreshPermissions);
        $("open-permissions").addEventListener("click", () => invoke("open_permissions"));
        ["recording-delay", "preview-delay", "preview-cadence"].forEach((id) => $(id).addEventListener("input", updateRangeOutputs));
        await listen("local-model-progress", async (event) => {
            const progress = event.payload || {};
            if (progress.state === "downloading") setText($(progress.model === "parakeet-v3" ? "parakeet-status" : "whisper-status"), `${t("downloading")} ${progress.total ? Math.round((progress.downloaded / progress.total) * 100) : ""}%`);
            if (progress.state === "ready") { state.downloadInProgress = false; await load(); }
            if (progress.state === "error") { state.downloadInProgress = false; setText($("save-status"), `${t("downloadError")}: ${progress.error || ""}`); renderLocalStatus(); }
        });
        await listen("ui-language-changed", (event) => setLanguage(event.payload));
        const systemLanguage = await invoke("initialize_ui_language", { locale: navigator.language || "" });
        setLanguage(systemLanguage);
        await load();
        await loadHistory();
    }

    init().catch((error) => setText($("save-status"), `${t("saveFailed")}: ${error}`));
})();
