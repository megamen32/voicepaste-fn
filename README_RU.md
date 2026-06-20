# VoicePaste Fn Minimal

macOS утилита для голосовых заметок:

- держите `Fn` минимум 0.2 сек → запись
- отпустите `Fn` → транскрипция → буфер обмена → Cmd+V
- оверлей рядом с курсором показывает `REC`, превью текста, статус/ошибки
- иконка в меню показывает настройки

Параметры по умолчанию встроены:

```bash
OPENAI_BASE_URL="https://example.com/v1"
OPENAI_API_KEY="***"
TRANSCRIBE_MODEL="whisper-1"
Language: ru
```

## Запуск

```bash
chmod +x run.sh
./run.sh
```

`run.sh` собирает приложение по адресу:

```text
build/VoicePasteFn.app
```

Это сделано намеренно: macOS меню-бары и статус-айтемы работают намного надежнее, когда запущены как реальное `.app`, а не как сырой SwiftPM CLI исполняемый файл.

## Меню-бар

Найдите иконку микрофона или `VP` в верхнем меню-баре macOS.

Пункты меню:

- Language: `ru`, `en`, `auto`
- Realtime preview
- Autostart
- Quit

## Разрешения

Предоставьте разрешения **VoicePasteFn.app**:

```text
System Settings → Privacy & Security → Microphone
System Settings → Privacy & Security → Accessibility
System Settings → Privacy & Security → Input Monitoring
```

После предоставления разрешений закройте приложение из меню или Activity Monitor и запустите:

```bash
./run.sh
```

## Примечания

Оверлей - это основное место для записи и превью текста. Меню-бар используется только для настроек.
