(function () {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    const copy = {
        en: {
            settingsApp: "Settings", general: "General", models: "Models", remote: "Remote", advanced: "Advanced", history: "History", permissions: "Permissions", appLanguage: "Application language",
            generalSubtitle: "Tune recording and recognition without leaving the app.", modelsSubtitle: "Local models stay on this device. Pick the provider that fits your hardware.", remoteSubtitle: "OpenAI, OpenRouter or any OpenAI-compatible transcription endpoint.", advancedSubtitle: "Rare switches for custom deployments and diagnostics.", permissionsSubtitle: "VoicePaste needs microphone access to record and accessibility access to paste into other apps.",
            saveChanges: "Save changes", discardChanges: "Discard", unsavedChanges: "Unsaved changes", saved: "Saved", saving: "Saving…", saveFailed: "Could not save", engineTitle: "Recognition engine", engineHint: "Choose one or more providers. They run in this order as fallbacks.", remoteEngine: "Remote", remoteEngineHint: "OpenAI-compatible API", localEngine: "Local", localEngineHint: "Whisper / Parakeet", nativeEngine: "Native", nativeEngineHint: "Built-in platform speech",
            activationTitle: "Activation", activationHint: "How the hotkey controls a recording.", activationMode: "Mode", hotkey: "Hotkey template", hold: "Hold", toggle: "Toggle", speechTitle: "Speech", speechHint: "Language and timing for the recognizer.", speechLanguage: "Transcription language", russian: "Russian", english: "English", chinese: "Chinese", automatic: "Automatic", recordingDelay: "Recording delay", previewTitle: "Preview", previewHint: "Keep feedback visible while speech is processed.", realtimePreview: "Realtime preview", realtimeHint: "Transcribe each completed phrase once; paste only the final full pass", previewDelay: "Hide preview after", vadSensitivity: "Speech sensitivity", vadSilence: "Phrase-ending pause", startupTitle: "Startup", startupHint: "Make the small workflow feel automatic.", autostart: "Start with the system", autostartHint: "Launch VoicePaste when you sign in", centerOverlay: "Center recording window", centerHint: "Otherwise show it near the pointer",
            modelsTitle: "Models", whisperDescription: "Reliable whisper.cpp model. Download once and use offline on macOS, Windows or Ubuntu.", parakeetDescription: "Fast local provider. Connect a Parakeet/sherpa CLI below; the model files are managed by that runtime.", parakeetNote: "Best for users who already have a local Parakeet runtime.", downloadModel: "Download", openFolder: "Open folder", useModel: "Use model", modelPage: "Model page", nativeDescription: "Use the platform speech framework. On macOS this is Apple Speech; other platforms may not provide it.", useEngine: "Use engine", ready: "Ready", notDownloaded: "Not downloaded", commandReady: "Command ready", commandMissing: "Command missing", macOnly: "macOS only", downloading: "Downloading…", downloadError: "Download failed",
            remoteTitle: "Remote provider", provider: "Provider template", customEndpoint: "Custom endpoint", endpoint: "API endpoint", remoteModel: "Remote model", remoteModelCustom: "Custom model id…", remoteModelsHint: "Refresh to load model ids from the server; manual ids remain supported.", apiKey: "API key", apiKeySet: "Saved key: ", clearApiKey: "Clear the saved key", proxyTitle: "Proxy respected", proxyHint: "VoicePaste uses the system and environment proxy settings. No proxy password is copied into this UI.", noProxy: "No proxy variables detected; system proxy behavior is still enabled.", refresh: "Refresh", modelsFound: "models found", noModels: "No models returned",
            advancedTitle: "Advanced", warmupTitle: "Warm up remote server at speech start", warmupHint: "Sends an empty audio request. Useful for a custom Whisper server that sleeps between requests.", localCommand: "Parakeet command template", localCommandHint: "The command must write plain text to {output_path}; stdout also works. The downloaded model directory is {model_dir}.", automation: "Automation", automationTitle: "Post-transcription automation", automationSubtitle: "Run one program or write to a file after a recognized command. Use curl for webhooks; no shell is involved.", automationEnabled: "Run an automation", automationEnabledHint: "A matching command is sent instead of pasted into the current app.", automationTrigger: "Trigger", triggerKeyword: "Keyword", triggerFnControl: "Fn + Control (macOS)", automationKeyword: "Keyword", automationPosition: "Find keyword", positionStart: "Only at the beginning", positionAnywhere: "Anywhere", positionEnd: "Only at the end", keywordPayloadHint: "For beginning and anywhere, only the text after the keyword is sent. For end, the text before it is sent.", automationKind: "Action", actionCommand: "Run program / script", actionFile: "Write to file", automationCommand: "Executable", automationCommandHint: "This is an executable path or PATH command, not a shell command.", automationArguments: "Arguments, one per line", automationArgumentsHint: "Text is always passed to stdin. Placeholders: {text}, {text_json}, {text_url}, {secret}. The secret is also provided as VOICEPASTE_ACTION_SECRET.", automationFilePath: "File path", automationFileMode: "Write mode", fileAppend: "Append a line", fileOverwrite: "Replace file", automationSecret: "Optional secret", clearAutomationSecret: "Clear saved secret", curlExampleTitle: "Webhook example", curlExample: "Set executable to curl and arguments to: -X, POST, -H, Content-Type: application/json, -d, {\"text\":{text_json}}, then the URL. The transcription goes to curl stdin too.", historyTitle: "Transcription history", historySubtitle: "Completed dictations stay searchable on this device.", historyRetention: "Keep history", historyRetentionHint: "Old entries are removed automatically. Audio is not stored in history.", retention7: "7 days", retention30: "30 days", retention90: "90 days", retentionForever: "Forever", clearHistory: "Clear history", noHistory: "No transcriptions yet", historyCount: "entries", permissionsTitle: "Permissions", microphone: "Microphone", accessibility: "Accessibility", granted: "Granted", notGranted: "Not granted", refreshPermissions: "Refresh status", openSystemSettings: "Open system settings", settingsFooter: "Changes are saved to your local VoicePaste configuration.",
            generalSection: "General", modelsSection: "Models", remoteSection: "Remote", advancedSection: "Advanced", automationSection: "Automation", historySection: "History", permissionsSection: "Permissions", speechRecognition: "Speech recognition (Native)", notRequired: "Not required", unavailable: "Unavailable"
        },
        ru: {
            settingsApp: "Настройки", general: "Общие", models: "Модели", remote: "Удалённый", advanced: "Расширенные", history: "История", permissions: "Разрешения", appLanguage: "Язык приложения",
            generalSubtitle: "Настройте запись и распознавание прямо в приложении.", modelsSubtitle: "Локальные модели остаются на этом устройстве. Выберите подходящий провайдер.", remoteSubtitle: "OpenAI, OpenRouter или любой OpenAI-совместимый endpoint.", advancedSubtitle: "Редкие параметры для кастомных серверов и диагностики.", permissionsSubtitle: "VoicePaste нужен микрофон для записи и доступность для вставки в другие приложения.",
            saveChanges: "Сохранить изменения", discardChanges: "Отменить", unsavedChanges: "Есть несохранённые изменения", saved: "Сохранено", saving: "Сохранение…", saveFailed: "Не удалось сохранить", engineTitle: "Движок распознавания", engineHint: "Выберите один или несколько провайдеров. Они работают по порядку как fallback.", remoteEngine: "Удалённый", remoteEngineHint: "OpenAI-совместимый API", localEngine: "Локальный", localEngineHint: "Whisper / Parakeet", nativeEngine: "Системный", nativeEngineHint: "Встроенная речь платформы",
            activationTitle: "Активация", activationHint: "Как горячая клавиша управляет записью.", activationMode: "Режим", hotkey: "Шаблон горячей клавиши", hold: "Удержание", toggle: "Переключатель", speechTitle: "Речь", speechHint: "Язык и тайминги распознавания.", speechLanguage: "Язык транскрипции", russian: "Русский", english: "Английский", chinese: "Китайский", automatic: "Авто", recordingDelay: "Задержка записи", previewTitle: "Предпросмотр", previewHint: "Показывать обратную связь во время обработки речи.", realtimePreview: "Предпросмотр в реальном времени", realtimeHint: "Каждую законченную фразу распознавать один раз; вставлять только полный финал", previewDelay: "Скрывать через", vadSensitivity: "Чувствительность речи", vadSilence: "Пауза завершения фразы", startupTitle: "Запуск", startupHint: "Сделайте короткий сценарий полностью автоматическим.", autostart: "Запускать вместе с системой", autostartHint: "Запускать VoicePaste при входе в систему", centerOverlay: "Центрировать окно записи", centerHint: "Иначе показывать его возле указателя",
            modelsTitle: "Модели", whisperDescription: "Надёжная модель whisper.cpp. Скачайте один раз и работайте офлайн на macOS, Windows или Ubuntu.", parakeetDescription: "Быстрый локальный провайдер. Подключите CLI Parakeet/sherpa ниже; файлы модели управляются этим runtime.", parakeetNote: "Подходит, если Parakeet уже установлен локально.", downloadModel: "Скачать", openFolder: "Открыть папку", useModel: "Использовать", modelPage: "Страница модели", nativeDescription: "Использовать системный speech framework. На macOS это Apple Speech; на других платформах может отсутствовать.", useEngine: "Использовать", ready: "Готово", notDownloaded: "Не скачана", commandReady: "Команда настроена", commandMissing: "Нет команды", macOnly: "только macOS", downloading: "Скачивание…", downloadError: "Ошибка скачивания",
            remoteTitle: "Удалённый провайдер", provider: "Шаблон провайдера", customEndpoint: "Свой endpoint", endpoint: "API endpoint", remoteModel: "Удалённая модель", remoteModelCustom: "Своё имя модели…", remoteModelsHint: "Обновите список, чтобы загрузить id моделей с сервера; ручной ввод остаётся доступен.", apiKey: "API-ключ", apiKeySet: "Сохранённый ключ: ", clearApiKey: "Удалить сохранённый ключ", proxyTitle: "Прокси учитывается", proxyHint: "VoicePaste использует системные и env-настройки прокси. Пароль прокси не копируется в этот интерфейс.", noProxy: "Переменные прокси не найдены; системное поведение прокси всё равно включено.", refresh: "Обновить", modelsFound: "моделей найдено", noModels: "Модели не вернулись",
            advancedTitle: "Расширенные", warmupTitle: "Прогревать удалённый сервер в начале речи", warmupHint: "Отправляет пустой запрос аудио. Нужно для кастомного Whisper-сервера, который засыпает между запросами.", localCommand: "Команда Parakeet", localCommandHint: "Команда должна записать обычный текст в {output_path}; stdout тоже поддерживается. Каталог скачанной модели доступен как {model_dir}.", automation: "Автоматизация", automationTitle: "Действие после распознавания", automationSubtitle: "Запустите одну программу или запишите текст в файл после голосовой команды. Для webhook используйте curl; shell не нужен.", automationEnabled: "Запускать действие", automationEnabledHint: "Совпавшая команда отправляется в действие, а не вставляется в текущее приложение.", automationTrigger: "Триггер", triggerKeyword: "Ключевое слово", triggerFnControl: "Fn + Control (macOS)", automationKeyword: "Ключевое слово", automationPosition: "Искать слово", positionStart: "Только в начале", positionAnywhere: "В любом месте", positionEnd: "Только в конце", keywordPayloadHint: "Для начала и любого места отправляется текст после слова. Для конца — текст перед словом.", automationKind: "Действие", actionCommand: "Запустить программу / скрипт", actionFile: "Записать в файл", automationCommand: "Исполняемый файл", automationCommandHint: "Это путь или команда из PATH, а не строка shell.", automationArguments: "Аргументы, по одному на строку", automationArgumentsHint: "Текст всегда передаётся на stdin. Подстановки: {text}, {text_json}, {text_url}, {secret}. Секрет также доступен как VOICEPASTE_ACTION_SECRET.", automationFilePath: "Путь к файлу", automationFileMode: "Режим записи", fileAppend: "Добавить строку", fileOverwrite: "Заменить файл", automationSecret: "Необязательный секрет", clearAutomationSecret: "Удалить сохранённый секрет", curlExampleTitle: "Пример webhook", curlExample: "Укажите curl и аргументы: -X, POST, -H, Content-Type: application/json, -d, {\"text\":{text_json}}, затем URL. Транскрипция также приходит в stdin curl.", historyTitle: "История транскрипций", historySubtitle: "Готовые транскрипции остаются доступными на этом устройстве.", historyRetention: "Хранить историю", historyRetentionHint: "Старые записи удаляются автоматически. Аудио в истории не хранится.", retention7: "7 дней", retention30: "30 дней", retention90: "90 дней", retentionForever: "Навсегда", clearHistory: "Очистить историю", noHistory: "Транскрипций пока нет", historyCount: "записей", permissionsTitle: "Разрешения", microphone: "Микрофон", accessibility: "Доступность", granted: "Разрешено", notGranted: "Не разрешено", refreshPermissions: "Обновить статус", openSystemSettings: "Открыть системные настройки", settingsFooter: "Изменения сохраняются в локальную конфигурацию VoicePaste.",
            generalSection: "Общие", modelsSection: "Модели", remoteSection: "Удалённый", advancedSection: "Расширенные", automationSection: "Автоматизация", historySection: "История", permissionsSection: "Разрешения", speechRecognition: "Распознавание речи (системное)", notRequired: "Не требуется", unavailable: "Недоступно"
        },
        zh: {
            settingsApp: "设置", general: "常规", models: "模型", remote: "远程", advanced: "高级", history: "历史", permissions: "权限", appLanguage: "应用语言",
            generalSubtitle: "直接在应用中调整录音和识别。", modelsSubtitle: "本地模型保存在此设备。选择适合硬件的提供商。", remoteSubtitle: "OpenAI、OpenRouter 或任何兼容 OpenAI 的识别接口。", advancedSubtitle: "用于自定义部署和诊断的少量选项。", permissionsSubtitle: "VoicePaste 需要麦克风录音，并需要辅助功能权限粘贴到其他应用。",
            saveChanges: "保存更改", discardChanges: "放弃", unsavedChanges: "有未保存的更改", saved: "已保存", saving: "保存中…", saveFailed: "保存失败", engineTitle: "识别引擎", engineHint: "可选择多个提供商，按顺序作为回退运行。", remoteEngine: "远程", remoteEngineHint: "兼容 OpenAI 的 API", localEngine: "本地", localEngineHint: "Whisper / Parakeet", nativeEngine: "原生", nativeEngineHint: "平台内置语音",
            activationTitle: "激活", activationHint: "快捷键如何控制录音。", activationMode: "模式", hotkey: "快捷键模板", hold: "按住", toggle: "切换", speechTitle: "语音", speechHint: "识别语言和时间设置。", speechLanguage: "转录语言", russian: "俄语", english: "英语", chinese: "中文", automatic: "自动", recordingDelay: "录音延迟", previewTitle: "预览", previewHint: "处理语音时保持反馈可见。", realtimePreview: "实时预览", realtimeHint: "每个完成的短语只识别一次；仅粘贴完整的最终结果", previewDelay: "预览隐藏延迟", vadSensitivity: "语音灵敏度", vadSilence: "短语结束停顿", startupTitle: "启动", startupHint: "让短流程更自动化。", autostart: "随系统启动", autostartHint: "登录时启动 VoicePaste", centerOverlay: "居中录音窗口", centerHint: "否则显示在指针附近",
            modelsTitle: "模型", whisperDescription: "可靠的 whisper.cpp 模型。下载一次后可在 macOS、Windows 或 Ubuntu 离线使用。", parakeetDescription: "快速本地提供商。连接下方的 Parakeet/sherpa CLI，模型文件由该 runtime 管理。", parakeetNote: "适合已经安装本地 Parakeet runtime 的用户。", downloadModel: "下载", openFolder: "打开文件夹", useModel: "使用模型", modelPage: "模型页面", nativeDescription: "使用平台语音框架。macOS 使用 Apple Speech，其他平台可能不可用。", useEngine: "使用引擎", ready: "就绪", notDownloaded: "未下载", commandReady: "命令已配置", commandMissing: "缺少命令", macOnly: "仅 macOS", downloading: "下载中…", downloadError: "下载失败",
            remoteTitle: "远程提供商", provider: "提供商模板", customEndpoint: "自定义接口", endpoint: "API 接口", remoteModel: "远程模型", remoteModelCustom: "自定义模型 id…", remoteModelsHint: "刷新以加载服务器上的模型 id；仍可手动输入自定义 id。", apiKey: "API 密钥", apiKeySet: "已保存密钥：", clearApiKey: "清除已保存密钥", proxyTitle: "支持代理", proxyHint: "VoicePaste 使用系统和环境代理设置。代理密码不会复制到此界面。", noProxy: "未检测到代理变量；系统代理行为仍然启用。", refresh: "刷新", modelsFound: "个模型", noModels: "没有返回模型",
            advancedTitle: "高级", warmupTitle: "开始讲话时预热远程服务器", warmupHint: "发送空音频请求。适用于请求之间会休眠的自定义 Whisper 服务器。", localCommand: "Parakeet 命令模板", localCommandHint: "命令必须将纯文本写入 {output_path}；也支持 stdout。下载的模型目录为 {model_dir}。", historyTitle: "转录历史", historySubtitle: "完成的听写会保存在此设备上。", historyRetention: "保留历史", historyRetentionHint: "旧记录会自动删除。历史记录不保存音频。", retention7: "7 天", retention30: "30 天", retention90: "90 天", retentionForever: "永久", clearHistory: "清空历史", noHistory: "还没有转录", historyCount: "条记录", permissionsTitle: "权限", microphone: "麦克风", accessibility: "辅助功能", granted: "已允许", notGranted: "未允许", refreshPermissions: "刷新状态", openSystemSettings: "打开系统设置", settingsFooter: "更改保存到本地 VoicePaste 配置。",
            generalSection: "常规", modelsSection: "模型", remoteSection: "远程", advancedSection: "高级", historySection: "历史", permissionsSection: "权限", speechRecognition: "语音识别（原生）", notRequired: "不需要", unavailable: "不可用"
        }
    };

    Object.assign(copy.en, {
        inputMonitoring: "Input Monitoring",
        permissionPurpose: "Input Monitoring reads Fn; Accessibility pastes the transcription into the active app.",
        requestPermissions: "Allow required access"
    });
    Object.assign(copy.ru, {
        inputMonitoring: "Мониторинг ввода",
        permissionPurpose: "Мониторинг ввода считывает Fn; Универсальный доступ вставляет текст в активное приложение.",
        requestPermissions: "Разрешить нужный доступ"
    });
    Object.assign(copy.zh, {
        inputMonitoring: "输入监控",
        permissionPurpose: "输入监控读取 Fn；辅助功能将转写内容粘贴到当前应用。",
        requestPermissions: "允许所需访问权限"
    });

    const hotkeys = [
        ["fn", "Fn (Globe 🌐)"], ["right_option", "Right ⌥ Option"], ["right_control", "Right ⌃ Control"],
        ["right_command", "Right ⌘ Command"], ["right_shift", "Right ⇧ Shift"], ["caps_lock", "Caps Lock"],
        ["f13", "F13"], ["f14", "F14"], ["f15", "F15"]
    ];
    const sectionCopy = {
        general: ["generalSection", "generalSubtitle"], models: ["modelsSection", "modelsSubtitle"], remote: ["remoteSection", "remoteSubtitle"], advanced: ["advancedSection", "advancedSubtitle"], automation: ["automationSection", "automationSubtitle"], history: ["historySection", "historySubtitle"], permissions: ["permissionsSection", "permissionsSubtitle"]
    };
    let language = "en";
    let state = null;
    let dirty = false;

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
        setText($("discard-button"), t("discardChanges"));
        setText($("save-button"), t("saveChanges"));
        setText($("save-status"), dirty ? t("unsavedChanges") : "");
    }

    function setDirty(value) {
        dirty = value;
        $("save-button").disabled = !dirty;
        $("discard-button").disabled = !dirty;
        setText($("save-status"), dirty ? t("unsavedChanges") : "");
    }

    function markDirty() {
        setDirty(true);
    }

    function setLanguage(value) {
        language = ["ru", "zh"].includes(value) ? value : "en";
        if (window.voicePasteI18n) window.voicePasteI18n.setLanguage(language);
        $("ui-language").value = language;
        applyTranslations();
        if (state) {
            if (dirty) {
                renderRemoteModels(state.remote_models || []);
                updateEngineSummary();
                renderLocalStatus();
                renderPermissions();
                renderHistory(state.history || []);
            } else {
                render();
            }
        }
    }

    function setSection(section, focus) {
        document.querySelectorAll(".section").forEach((element) => element.classList.toggle("active", element.id === `section-${section}`));
        document.querySelectorAll(".nav-item").forEach((element) => element.classList.toggle("active", element.dataset.section === section));
        const keys = sectionCopy[section] || sectionCopy.general;
        setText($("page-title"), t(keys[0]));
        setText($("page-subtitle"), t(keys[1]));
        if (section === "history") void loadHistory();
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
        const nativeUnavailable = state.permissions && state.permissions.speech_recognition === false;
        setText($("native-status"), state.native_available ? t("ready") : (nativeUnavailable ? t("unavailable") : t("macOnly")));
        $("download-whisper").hidden = whisperReady;
        $("download-parakeet").hidden = parakeetReady;
        $("download-whisper").disabled = state.downloadInProgress;
        $("download-parakeet").disabled = state.downloadInProgress;
        $("use-whisper").disabled = !whisperReady;
        $("select-parakeet").disabled = !parakeetReady;
    }

    function renderPermissions() {
        const permissions = state.permissions || {};
        const requirements = state.permission_requirements || {};
        setText($("mic-permission"), permissions.microphone ? t("granted") : t("notGranted"));
        setText($("input-monitoring-permission"), requirements.input_monitoring
            ? (permissions.input_monitoring ? t("granted") : t("notGranted"))
            : t("notRequired"));
        setText($("accessibility-permission"), permissions.accessibility ? t("granted") : t("notGranted"));
        setText($("speech-permission"), requirements.speech_recognition
            ? (permissions.speech_recognition ? t("granted") : t("notGranted"))
            : t("notRequired"));
        $("request-permissions").hidden = state.permission_setup_required !== true;
    }

    function updateEngineSummary() {
        setText($("engine-summary"), ["remote", "local", "native"]
            .filter((engine) => $(`engine-${engine}`).checked)
            .map((engine) => t(`${engine}Engine`))
            .join(" → "));
    }

    function renderRemoteModels(models) {
        const select = $("remote-model-select");
        if (!select) return;
        const current = $("remote-model").value.trim();
        const uniqueModels = [...new Set((models || []).filter((model) => typeof model === "string" && model.trim()))];
        select.innerHTML = "";
        const custom = document.createElement("option");
        custom.value = "__custom__";
        custom.textContent = t("remoteModelCustom");
        select.appendChild(custom);
        uniqueModels.forEach((model) => {
            const option = document.createElement("option");
            option.value = model;
            option.textContent = model;
            select.appendChild(option);
        });
        select.value = uniqueModels.includes(current) ? current : "__custom__";
        setText($("remote-model-state"), uniqueModels.length ? `${uniqueModels.length} ${t("modelsFound")}` : t("noModels"));
    }

    function renderAutomation() {
        const automation = state.automation || {};
        $("automation-enabled").checked = automation.enabled === true;
        $("automation-trigger").value = automation.trigger || "keyword";
        $("automation-keyword").value = automation.keyword || "";
        $("automation-position").value = automation.keyword_position || "start";
        $("automation-kind").value = automation.action_kind || "command";
        $("automation-command").value = automation.command || "";
        $("automation-arguments").value = Array.isArray(automation.arguments) ? automation.arguments.join("\n") : "";
        $("automation-file-path").value = automation.file_path || "";
        $("automation-file-mode").value = automation.file_mode || "append";
        setText($("automation-secret-state"), automation.secret_set ? `${t("apiKeySet")}${automation.secret_masked}` : "");
        updateAutomationFields();
    }

    function updateAutomationFields() {
        const keyword = $("automation-trigger").value === "keyword";
        const command = $("automation-kind").value === "command";
        $("keyword-fields").hidden = !keyword;
        $("command-fields").hidden = !command;
        $("file-fields").hidden = command;
    }

    function render() {
        $("activation").value = state.activation_mode;
        $("hotkey").value = state.hotkey;
        $("speech-language").value = state.language;
        $("recording-delay").value = state.recording_delay;
        $("preview-delay").value = state.hide_delay;
        $("vad-sensitivity").value = state.vad_sensitivity ?? 0.65;
        $("vad-silence").value = state.vad_silence_ms ?? 500;
        $("realtime-preview").checked = state.realtime_preview;
        $("autostart").checked = state.autostart;
        $("overlay-centered").checked = state.overlay_centered;
        $("warmup").checked = state.wake_server_on_start;
        $("history-retention").value = String(state.history_retention_days ?? 30);
        $("remote-provider").value = state.remote_provider || "openai";
        $("endpoint").value = state.base_url || "";
        $("remote-model").value = state.model || "whisper-1";
        renderRemoteModels(state.remote_models || []);
        $("local-command").value = state.local_command || "";
        renderAutomation();
        $("ui-language").value = language;
        const selectedLocalStatus = (state.model_statuses || {})[state.local_model] || {};
        const availability = {
            ...(state.engine_availability || {}),
            local: selectedLocalStatus.model_ready === true
                && (state.local_model !== "parakeet-v3" || selectedLocalStatus.runtime_configured === true)
        };
        ["remote", "local", "native"].forEach((engine) => {
            const checkbox = $(`engine-${engine}`);
            const available = availability[engine] !== false;
            checkbox.disabled = !available;
            checkbox.checked = available && (state.engine_order || []).includes(engine);
        });
        setText($("recording-delay-value"), `${Number(state.recording_delay).toFixed(1)} s`);
        setText($("preview-delay-value"), `${Number(state.hide_delay).toFixed(1)} s`);
        setText($("vad-sensitivity-value"), `${Math.round(Number(state.vad_sensitivity ?? 0.65) * 100)}%`);
        setText($("vad-silence-value"), `${Number(state.vad_silence_ms ?? 500).toFixed(0)} ms`);
        setText($("api-key-state"), state.api_key_set ? `${t("apiKeySet")}${state.api_key_masked}` : "");
        setText($("proxy-vars"), state.proxy_env && state.proxy_env.length ? state.proxy_env.join(" · ") : t("noProxy"));
        setText($("config-path"), state.config_path || "");
        updateEngineSummary();
        renderLocalStatus();
        renderPermissions();
        renderHistory(state.history || []);
        $("save-button").disabled = !dirty;
        $("discard-button").disabled = !dirty;
    }

    function updateRangeOutputs() {
        setText($("recording-delay-value"), `${Number($("recording-delay").value).toFixed(1)} s`);
        setText($("preview-delay-value"), `${Number($("preview-delay").value).toFixed(1)} s`);
        setText($("vad-sensitivity-value"), `${Math.round(Number($("vad-sensitivity").value) * 100)}%`);
        setText($("vad-silence-value"), `${Number($("vad-silence").value).toFixed(0)} ms`);
    }

    function collectPatch() {
        const engines = ["remote", "local", "native"].filter((engine) => $(`engine-${engine}`).checked);
        const automation = {
            enabled: $("automation-enabled").checked,
            trigger: $("automation-trigger").value,
            keyword: $("automation-keyword").value.trim(),
            keyword_position: $("automation-position").value,
            action_kind: $("automation-kind").value,
            command: $("automation-command").value.trim(),
            arguments: $("automation-arguments").value.split("\n").map((value) => value.trim()).filter(Boolean),
            file_path: $("automation-file-path").value.trim(),
            file_mode: $("automation-file-mode").value,
            ...(($('automation-secret').value.trim()) ? { secret: $("automation-secret").value.trim() } : {}),
            ...(($('clear-automation-secret').checked) ? { clear_secret: true } : {})
        };
        return {
            base_url: $("endpoint").value.trim(),
            model: $("remote-model").value.trim() || "whisper-1",
            local_model: state.local_model,
            local_command: $("local-command").value,
            remote_provider: $("remote-provider").value,
            language: $("speech-language").value,
            realtime_preview: $("realtime-preview").checked,
            vad_sensitivity: Number($("vad-sensitivity").value),
            vad_silence_ms: Number($("vad-silence").value),
            recording_delay: Number($("recording-delay").value),
            hide_delay: Number($("preview-delay").value),
            hotkey: $("hotkey").value,
            activation_mode: $("activation").value,
            overlay_centered: $("overlay-centered").checked,
            wake_server_on_start: $("warmup").checked,
            autostart: $("autostart").checked,
            history_retention_days: Number($("history-retention").value),
            engine_order: engines.length ? engines : ["remote"],
            ui_language: language,
            automation,
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
            $("automation-secret").value = "";
            $("clear-automation-secret").checked = false;
            setDirty(false);
            setText($("save-status"), t("saved"));
            render();
            await loadHistory();
            setTimeout(() => { if (!dirty) setText($("save-status"), ""); }, 1600);
        } catch (error) {
            setText($("save-status"), `${t("saveFailed")}: ${error}`);
        }
    }

    async function load() {
        state = await invoke("get_settings");
        setDirty(false);
        language = state.ui_language || language;
        applyTranslations();
        render();
        if (state.permission_setup_required) setSection("permissions", true);
    }

    async function selectLocalModel(model) {
        state.local_model = model;
        const order = new Set(state.engine_order || []);
        order.add("local");
        state.engine_order = Array.from(order);
        render();
        markDirty();
    }

    async function selectNative() {
        const order = new Set(state.engine_order || []);
        order.add("native");
        state.engine_order = Array.from(order);
        render();
        markDirty();
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

    async function refreshModels({ silent = false } = {}) {
        if (!silent) setText($("refresh-remote-models"), t("saving"));
        try {
            const models = await invoke("refresh_remote_models");
            state.remote_models = Array.isArray(models) ? models : [];
            renderRemoteModels(state.remote_models);
            if (!silent) setText($("save-status"), state.remote_models.length ? `${state.remote_models.length} ${t("modelsFound")}` : t("noModels"));
        } catch (error) {
            if (!silent) setText($("save-status"), `${t("saveFailed")}: ${error}`);
            setText($("remote-model-state"), `${t("saveFailed")}: ${error}`);
        } finally {
            if (!silent) setText($("refresh-remote-models"), "↻");
        }
    }

    function applyProviderTemplate() {
        const provider = $("remote-provider").value;
        const endpoints = { openai: "https://api.openai.com/v1", openrouter: "https://openrouter.ai/api/v1" };
        if (endpoints[provider]) $("endpoint").value = endpoints[provider];
    }

    async function refreshPermissions() {
        state = await invoke("get_settings");
        render();
    }

    async function requestPermissions() {
        $("request-permissions").disabled = true;
        try {
            await invoke("request_permissions");
            state = await invoke("get_settings");
            render();
        } catch (error) {
            setText($("save-status"), `${t("saveFailed")}: ${error}`);
            try {
                state = await invoke("get_settings");
                render();
            } catch (_) {
                // Preserve the last visible permission state if the helper is unavailable.
            }
        } finally {
            $("request-permissions").disabled = false;
        }
    }

    async function init() {
        fillHotkeys();
        await listen("history-changed", () => loadHistory());
        document.querySelectorAll(".nav-item").forEach((button) => button.addEventListener("click", () => setSection(button.dataset.section, true)));
        $("save-button").addEventListener("click", save);
        $("discard-button").addEventListener("click", load);
        $("ui-language").addEventListener("change", (event) => { setLanguage(event.target.value); markDirty(); });
        $("remote-provider").addEventListener("change", () => { applyProviderTemplate(); markDirty(); });
        $("automation-trigger").addEventListener("change", () => { updateAutomationFields(); markDirty(); });
        $("automation-kind").addEventListener("change", () => { updateAutomationFields(); markDirty(); });
        $("remote-model-select").addEventListener("change", (event) => {
            if (event.target.value !== "__custom__") $("remote-model").value = event.target.value;
            markDirty();
        });
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
        $("request-permissions").addEventListener("click", requestPermissions);
        $("open-permissions").addEventListener("click", () => invoke("open_permissions"));
        document.querySelectorAll(".section input, .section select, .section textarea").forEach((element) => {
            if (element.classList.contains("engine-check")) return;
            const eventName = (element.tagName === "INPUT" && element.type !== "checkbox") || element.tagName === "TEXTAREA" ? "input" : "change";
            element.addEventListener(eventName, () => {
                if (["recording-delay", "preview-delay", "vad-sensitivity", "vad-silence"].includes(element.id)) updateRangeOutputs();
                markDirty();
            });
        });
        document.querySelectorAll(".engine-check").forEach((checkbox) => checkbox.addEventListener("change", () => {
            updateEngineSummary();
            markDirty();
        }));
        await listen("local-model-progress", async (event) => {
            const progress = event.payload || {};
            if (progress.state === "downloading") setText($(progress.model === "parakeet-v3" ? "parakeet-status" : "whisper-status"), `${t("downloading")} ${progress.total ? Math.round((progress.downloaded / progress.total) * 100) : ""}%`);
            if (progress.state === "ready") { state.downloadInProgress = false; await load(); }
            if (progress.state === "error") { state.downloadInProgress = false; setText($("save-status"), `${t("downloadError")}: ${progress.error || ""}`); renderLocalStatus(); }
        });
        await listen("ui-language-changed", (event) => setLanguage(event.payload));
        await listen("permission-setup-required", () => setSection("permissions", true));
        const systemLanguage = await invoke("initialize_ui_language", { locale: navigator.language || "" });
        setLanguage(systemLanguage);
        await load();
        await refreshModels({ silent: true });
        await loadHistory();
    }

    init().catch((error) => setText($("save-status"), `${t("saveFailed")}: ${error}`));
})();
