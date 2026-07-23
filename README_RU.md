# VoicePaste Fn — минималистичный голосовой транскрайбер для macOS

> 📖 [English documentation](README.md)

Утилита для голосовых заметок: держите клавишу → говорите → отпустите → текст уже в буфере обмена. Вставка автоматически через `Cmd+V` в активное окно.

## Возможности

### Диктовка
- **Настраиваемая клавиша**: Fn (Globe), Right ⌥/⌃/⌘/⇧, Caps Lock, F13/F14/F15.
- **Hold или Toggle активация**: удерживать (default) или нажал/нажал.
- **Задержка старта записи** (0.10 – 2.00 с): чтобы случайное нажатие не триггерило запись.

### Обратная связь
- Оверлей у курсора или **по центру экрана** (toggle).
- Realtime preview: превью текста во время записи (toggle).
- Если расшифровка упала — оверлей показывает ↩ ; клик повторяет запрос с тем же аудио.

### Whisper endpoint
- Endpoint + API key редактируются из menu bar → **Settings ▶**.
- **API key хранится в macOS Keychain** (зашифровано, доступ только у этого приложения).
- `OPENAI_BASE_URL` / `OPENAI_API_KEY` env vars переопределяют для shell-тестов.
- **Wake server on dictation start**: при старте записи в фоне отправляется 1-секундный silence-файл через `/audio/transcriptions` — модель сервера прогревается до того, как придёт реальное аудио. Ошибки проглатываются.

### Очистка текста
- Авто-удаление сабтайтр-бойлерплейта в конце транскрипта: «Продолжение следует», «Субтитры сделал DimaTorzok», «Субтитры сделаны DimaTorzok», «Subtitles by DimaTorzok», «Subtitles made by DimaTorzok», «to be continued», «Thanks for watching». Опциональная trailing пунктуация вытерпится.

### Совместимость
- OpenAI (`https://api.openai.com/v1`)
- Собственные Whisper-серверы
- Любой OpenAI-совместимый endpoint

## Установка

### Требования
- macOS 13+
- Swift 5.9+
- OpenAI API key (или совместимый Whisper endpoint)

### Сборка

```bash
git clone https://github.com/yourusername/voicepaste-fn.git
cd voicepaste-fn
chmod +x run.sh
./run.sh
```

Собранный bundle лежит в `build/VoicePasteFn.app`. macOS попросит следующие разрешения при первом запуске:

```
System Settings → Privacy & Security → Microphone
System Settings → Privacy & Security → Accessibility
```

Разрешите, потом кликните иконку микрофона в menu bar → **Settings ▶ → API Key ▶ → Edit…** и вставьте ключ. macOS попросит Keychain-доступ один раз (потом не спрашивает).

## Меню-бар

Всё настраивается из menu bar — никакого редактирования конфигов.

```
VoicePaste Fn
─────────────
Settings ▶
   Endpoint:  api.openai.com
   API Key:   sk-•••1234 (24)
─────────────
Recording delay: 0.20s   ▶  [0.10 … 2.00 с]
Preview hide:    0.80s   ▶  [Manual / 0.4 … 5.0 с]
Language:        ru       ▶
Model:           auto     ▶
Realtime preview           (toggle)
Realtime every: 5.00s    ▶  [1 … 30 с, эффективно только при включённом Realtime]
Autostart                 (toggle)
─────────────
Hotkey:     Fn (Globe)   ▶   [Fn / Right ⌥ ⌃ ⌘ ⇧ / Caps / F13 F14 F15]
Activation: Hold         ▶   [Hold / Toggle]
Centre overlay on screen (toggle — оверлей по центру экрана или у курсора)
Wake server on dictation start (toggle — POST silence-clip для прогрева)
─────────────
Permissions: ✓ Mic  ✓ Accessibility
Quit
```

### Hotkey changes

Смена клавиши в меню вступает в силу **только после перезапуска приложения**. event-tap устанавливается один раз при старте — пересоздавать его на каждое изменение слишком жирно. Activation mode (Hold / Toggle) и все остальные настройки применяются мгновенно.

## Конфигурация

### Env vars (перекрывают UserDefaults / Keychain для одного запуска)

```bash
export OPENAI_BASE_URL="https://api.openai.com/v1"
export OPENAI_API_KEY="sk-your-key-here"
export TRANSCRIBE_MODEL="whisper-1"   # default
./run.sh
```

Полезно для shell-тестов без правки сохранённых credentials.

## Кроссплатформенные black-box-тесты

Тест поднимает настоящий локальный HTTP-сервер, запускает production probe
отдельным процессом и проверяет контракт `GET /v1/models`:

```bash
python3 Tests/blackbox_models.py --all
```

Rust probe запускается на Windows, macOS и Ubuntu. Swift probe запускается на
macOS и автоматически пропускается на остальных системах. Для одного варианта:
`--implementation rust` или `--implementation swift`.

На текущем baseline тест намеренно красный для Rust: Swift проходит сортировку
моделей, а Rust показывает текущий несортированный ответ.

### Persisted (UserDefaults + Keychain)

Хранится в `~/Library/Preferences/com.bezrabotnyi.voicepastefn.plist` и в macOS Keychain (Generic Password, service `com.bezrabotnyi.voicepastefn`, account `openai_api_key`). Редактируется через **Settings ▶** в меню-баре.

## Структура проекта

```
voicepaste-fn/
├── Package.swift
├── README.md                # English
├── README_RU.md             # Russian
├── LICENSE
├── run.sh                   # Build + ad-hoc sign + launch
├── AppIcon.icns             # Иконка приложения
├── Sources/
│   └── VoicePasteFn/
│       ├── main.swift       # Запись и bootstrap
│       ├── VoicePasteApp.swift
│       └── RecordingOverlay.swift
├── build/
│   └── VoicePasteFn.app/
```

## Troubleshooting

| Проблема | Решение |
|----------|---------|
| Клавиша не срабатывает | macOS Settings → Privacy & Security → Accessibility → разрешить VoicePasteFn |
| Не запрашивается API key | macOS Settings → Passwords → проверить Keychain Access для VoicePasteFn |
| Первая расшифровка очень долгая / таймаут | Wake-server уже шлёт прогрев при каждом Fn; если всё равно холодно — посмотрите `~/.config` или консоль сервера |
| Оверлей мешает | Toggle «Centre overlay on screen» или двигайте курсор перед нажатием |

## Releases

Готовые `.app.zip` бандлы публикуются на странице GitHub Releases. Bundle подписан ad-hoc с стабильным identifier (`com.bezrabotnyi.voicepastefn`) — macOS TCC сохраняет разрешения Microphone + Accessibility между переустановками.

В текущей development-ветке выбранные macOS-артефакты лежат в [`artifacts/`](artifacts/): последний Swift archive, Rust/Tauri DMG и два Swift helper-бинарника. Контрольные суммы — в [`artifacts/SHA256SUMS.txt`](artifacts/SHA256SUMS.txt).

```bash
curl -L https://github.com/yourusername/voicepaste-fn/releases/download/v0.3.0/VoicePasteFn.app.zip -o vp.zip
unzip vp.zip
mv VoicePasteFn.app /Applications/
open /Applications/VoicePasteFn.app
```

## Разрешения

VoicePasteFn нужны:
- **Microphone** — для записи.
- **Accessibility** — для global hotkey event tap.
- **Keychain Access** — один раз при первом сохранении API key.

## Лицензия

MIT — см. [LICENSE](LICENSE).

## Contributing

PRs приветствуются. Swift остаётся нативным macOS-клиентом, а Rust/Tauri — cross-platform-клиентом с локальными Whisper/Parakeet. Тесты находятся рядом с клиентами и должны проверять доступность моделей и фоновую обработку новых записей.
