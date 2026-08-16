# Задача: опубликовать единый VoicePaste v2.0.0

Статус: выполнение.

## Пользовательский результат

В GitHub опубликован один нечерновой `v2.0.0` с Windows MSI/NSIS и актуальными
macOS/Ubuntu инсталляторами.

## Canary

GitHub Actions собирает Windows x64, а опубликенный release содержит как минимум
один `.msi` и один Windows `.exe`; release не Draft.

## Причина

Windows Tauri build остановился на отсутствующем `src-tauri/icons/icon.ico`.

## Оценка

20–45 активных минут.
