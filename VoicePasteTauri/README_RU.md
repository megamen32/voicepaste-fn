# VoicePaste

Кроссплатформенное (macOS / Windows / Linux) приложение для преобразования голоса в текст с вставкой в буфер обмена. Построено на **Rust + Tauri v2**.

Записывайте аудио с микрофона, транскрибируйте через Whisper API (с 3x авто-повтором + локальный fallback на whisper.cpp) и вставляйте результат прямо в буфер обмена.

![Платформа](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue)
![Лицензия](https://img.shields.io/badge/license-MIT-green)

## Возможности

- **Глобальная горячая клавиша** — удержание или переключение для записи (по умолчанию — правый Alt)
- **Транскрипция через Whisper API** — OpenAI-совместимый эндпоинт с 3x авто-повтором
- **Локальный fallback** — whisper.cpp (whisper-rs) для офлайн-транскрипции при сбое сервера
- **Плавающий оверлей** — HUD поверх всех окон, следующий за курсором, показывающий состояние записи и превью транскрипции
- **Системный трей** — полное меню настроек со всеми опциями
- **Очередь записи** — цепочка последовательных записей
- **Кроссплатформенность** — macOS, Windows, Linux из единой кодовой базы
- **Автозапуск** — LaunchAgent (macOS), Registry (Windows), XDG (Linux)
- **Настраиваемость** — эндпоинт, API-ключ, язык, модель, задержки, горячая клавиша, режим активации

## Установка

### Из DMG (macOS)

1. Скачайте актуальный установщик `VoicePaste_2.0.0_*` из [Releases](../../releases)
2. Перетащите `VoicePaste.app` в Applications
3. Запустите и предоставьте разрешения на микрофон + универсальный доступ

### Из исходников

```bash
# Требования: Rust toolchain, cmake
cargo install tauri-cli --version "^2"

cd VoicePasteTauri/src-tauri
cargo tauri build
```

Собранные приложения:
- macOS: `target/release/bundle/macos/VoicePaste.app`
- macOS DMG: `target/release/bundle/dmg/VoicePaste_*.dmg`
- Windows: `target/release/bundle/msi/VoicePaste_*.msi`
- Linux: `target/release/bundle/deb/voicepaste_*.deb`

## Использование

1. **Запустите** приложение — в меню-баре появится иконка в трее
2. **Нажмите и удерживайте** правый Alt (или настроенную горячую клавишу) для начала записи
3. **Отпустите** для остановки и транскрипции
4. Транскрибированный текст автоматически вставляется в позицию курсора

### Режим переключения

В меню трея переключите активацию на **Toggle**:
- Первое нажатие: начать запись
- Второе нажатие: остановить и транскрибировать

### Опции меню трея

| Опция | Описание |
|-------|----------|
| Settings > Endpoint | Базовый URL Whisper API |
| Settings > API Key | Ваш API-ключ |
| Recording delay | Задержка перед началом записи (0.2–2.0с) |
| Preview hide delay | Сколько времени превью остаётся видимым (0–5с) |
| Language | ru / en / auto |
| Model | Выбор модели Whisper |
| Realtime preview | Живая транскрипция с настраиваемым интервалом |
| Autostart | Запуск при старте системы |
| Hotkey | Выбор глобальной горячей клавиши |
| Activation mode | Hold или Toggle |
| Centre overlay | Фиксация оверлея по центру экрана |
| Wake server | Отправка тихого запроса перед записью |
| Local fallback | Использовать whisper.cpp при сбое сервера |

## Конфигурация

Настройки хранятся в JSON в каталоге данных приложения:

- **macOS**: `~/Library/Application Support/com.bezrabotnyi.voicepaste/settings.json`
- **Windows**: `%APPDATA%\com.bezrabotnyi.voicepaste\settings.json`
- **Linux**: `~/.config/com.bezrabotnyi.voicepaste/settings.json`

Переменные окружения переопределяют настройки при запуске:

| Переменная | Описание |
|------------|----------|
| `OPENAI_BASE_URL` | Эндпоинт Whisper API |
| `OPENAI_API_KEY` | API-ключ |
| `TRANSCRIBE_MODEL` | Имя модели |

## Разработка

```bash
cd VoicePasteTauri/src-tauri

# Проверка компиляции
cargo check

# Запуск тестов (30 юнит-тестов)
cargo test

# Запуск в dev-режиме
cargo tauri dev

# Продакшн-сборка
cargo tauri build
```

## Стек технологий

- **Rust** — язык бэкенда
- **Tauri v2** — кроссплатформенный десктоп-фреймворк
- **cpal** — кроссплатформенный аудиоввод/вывод
- **hound** — кодирование WAV
- **whisper-rs** — локальный STT на whisper.cpp
- **reqwest** — HTTP-клиент для Whisper API
- **core-graphics** — нативная позиция курсора (macOS)

## Переводы

- [English](README.md)
- [中文](README_CN.md)

## Лицензия

MIT
