// Small dependency-free UI locale used by both the overlay and record window.
(function () {
    const translations = {
        en: {
            endpointTitle: "Endpoint URL",
            endpointPlaceholder: "https://api.openai.com/v1",
            apiKeyTitle: "API Key",
            apiKeyPlaceholder: "sk-...",
            save: "Save",
            cancel: "Cancel",
            retry: "Retry",
            recording: "Recording…",
            processing: "Processing…",
            transcriptionError: "Transcription error",
            permissionsTitle: "Permissions", hotkeyError: "Hotkey unavailable. Grant Accessibility permission in System Settings.",
            granted: "Granted",
            notGranted: "Not granted",
            permissionsHint: "Open System Settings to grant access.",
            recordTitle: "VoicePaste Record",
            close: "Close",
            transcriptPlaceholder: "Transcription will appear here…",
            clickMic: "Click mic to start",
            copy: "Copy",
            copied: "Copied!",
            errorPrefix: "Error: ",
            couldNotStart: "Could not start recording",
            couldNotStop: "Could not transcribe recording",
        },
        ru: {
            endpointTitle: "URL сервера",
            endpointPlaceholder: "https://api.openai.com/v1",
            apiKeyTitle: "API-ключ",
            apiKeyPlaceholder: "sk-...",
            save: "Сохранить",
            cancel: "Отмена",
            retry: "Повторить",
            recording: "Запись…",
            processing: "Обработка…",
            transcriptionError: "Ошибка распознавания",
            permissionsTitle: "Разрешения", hotkeyError: "Горячая клавиша недоступна. Разрешите VoicePaste в разделе «Доступность» системных настроек.",
            granted: "Разрешено",
            notGranted: "Не разрешено",
            permissionsHint: "Откройте системные настройки и выдайте доступ.",
            recordTitle: "VoicePaste — запись",
            close: "Закрыть",
            transcriptPlaceholder: "Здесь появится расшифровка…",
            clickMic: "Нажмите на микрофон для записи",
            copy: "Копировать",
            copied: "Скопировано!",
            errorPrefix: "Ошибка: ",
            couldNotStart: "Не удалось начать запись",
            couldNotStop: "Не удалось распознать запись",
        },
        zh: {
            endpointTitle: "服务器地址",
            endpointPlaceholder: "https://api.openai.com/v1",
            apiKeyTitle: "API 密钥",
            apiKeyPlaceholder: "sk-...",
            save: "保存",
            cancel: "取消",
            retry: "重试",
            recording: "正在录音…",
            processing: "正在处理…",
            transcriptionError: "识别错误",
            permissionsTitle: "权限", hotkeyError: "快捷键不可用。请在系统设置中授予辅助功能权限。",
            granted: "已允许",
            notGranted: "未允许",
            permissionsHint: "请打开系统设置并授予权限。",
            recordTitle: "VoicePaste — 录音",
            close: "关闭",
            transcriptPlaceholder: "识别结果会显示在这里…",
            clickMic: "点击麦克风开始录音",
            copy: "复制",
            copied: "已复制！",
            errorPrefix: "错误：",
            couldNotStart: "无法开始录音",
            couldNotStop: "无法识别录音",
        },
    };

    function normalize(locale) {
        const value = String(locale || "").toLowerCase();
        if (value.indexOf("ru") === 0) return "ru";
        if (value.indexOf("zh") === 0 || value.indexOf("cn") === 0) return "zh";
        return "en";
    }

    let current = normalize(navigator.language);

    function translate(key) {
        return (translations[current] && translations[current][key]) || translations.en[key] || key;
    }

    function apply() {
        document.documentElement.lang = current;
        document.querySelectorAll("[data-i18n]").forEach((element) => {
            element.textContent = translate(element.dataset.i18n);
        });
        document.querySelectorAll("[data-i18n-placeholder]").forEach((element) => {
            element.placeholder = translate(element.dataset.i18nPlaceholder);
        });
        document.querySelectorAll("[data-i18n-title]").forEach((element) => {
            element.title = translate(element.dataset.i18nTitle);
        });
    }

    function setLanguage(language) {
        current = normalize(language);
        apply();
        window.dispatchEvent(new CustomEvent("voicepaste-language-changed", {
            detail: current,
        }));
    }

    window.voicePasteI18n = {
        normalize,
        setLanguage,
        t: translate,
        language: () => current,
        apply,
    };
    document.addEventListener("DOMContentLoaded", apply);
})();
