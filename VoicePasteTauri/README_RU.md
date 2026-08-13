# VoicePaste

Голосовая транскрипция в любом приложении: нажмите горячую клавишу, говорите, отпустите — текст появится в активном поле.

## Установка

Откройте [релиз v2.0.0](https://github.com/megamen32/voicepaste-fn/releases/tag/v2.0.0) и скачайте установщик для своей системы:

- macOS — `.dmg`
- Windows — `.msi` или `.exe`
- Linux — `.deb` или `.AppImage`

На macOS после первого запуска разрешите микрофон и Accessibility.

## Использование

1. Запустите VoicePaste.
2. Нажмите и удерживайте горячую клавишу.
3. Продиктуйте текст и отпустите клавишу.

Готовый текст будет вставлен в активное приложение. Endpoint и API key задаются в Settings.

## Проверено

- macOS — Rust/Tauri production canary: PASS.
- Windows — не тестировано в этой сессии: удалённый SSH/RDP-сеанс недоступен.
- Mac Mini — не тестировано в этой сессии: SSH недоступен.

Исходный код: [github.com/megamen32/voicepaste-fn](https://github.com/megamen32/voicepaste-fn).
