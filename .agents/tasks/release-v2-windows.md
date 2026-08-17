# Задача: опубликовать единый VoicePaste v2.0.0

Статус: завершено.

## Пользовательский результат

В GitHub опубликован один нечерновой `v2.0.0` с macOS Apple Silicon DMG,
Intel macOS `.app.tar.gz`, Windows MSI/NSIS, Ubuntu `.deb` и AppImage.

## Canary

Опубликованный release содержит шесть проверенных ассетов, включая `.msi`,
Windows `.exe`, Intel macOS архив, `.deb` и AppImage; release не Draft.

## Причина

Windows Tauri build остановился на отсутствующем `src-tauri/icons/icon.ico`.

## Оценка

Фактически: Windows/Apple Silicon через GitHub Actions; Linux нативно на
Linux-хосте; Intel macOS нативно на Mac Mini. DMG Intel не создался из-за
устаревшего `bundle_dmg.sh`, поэтому опубликован валидный `.app.tar.gz`.
