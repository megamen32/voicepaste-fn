# Задача: VAD preview без повторной расшифровки старого аудио

Статус: завершено

## Пользовательский результат

Realtime-preview режет речь по паузам и отправляет на распознавание только новый
необработанный аудиофрагмент. Распознанные chunks собираются в черновик и сразу
копируются в clipboard, но не вставляются. После остановки записи весь исходный
WAV распознаётся заново; только этот финальный результат вставляется в целевое
приложение.

## Canary

Loopback endpoint получает непересекающиеся preview WAV, затем один полный WAV.
До финала целевое поле пустое, clipboard содержит сборку chunks; после финала в
поле ровно результат полного прогона и Cmd/Ctrl+V вызван один раз.

## Решение

- абсолютный sample cursor в recorder запрещает повторную отправку старого
  аудио;
- адаптивный RMS VAD: noise floor, гистерезис, pre-roll, min speech и
  настраиваемая пауза;
- preview пишет только clipboard, финальная вставка остаётся в
  `deliver_transcript`;
- preview draft больше не используется как fallback полного распознавания.

## Проверка

- `cargo test --lib`: 94 passed, 0 failed.
- macOS Computer Use canary на F14: PASS.
- Preview requests: `first-only` 69120 bytes, затем `second-only` 69120 bytes.
- Clipboard до финала: `preview-one preview-two`; целевое поле пустое.
- Финальный request: 83200 bytes, содержит обе фразы; только его результат
  появился в TextEdit.
- Evidence: `.agents/evidence/computer-use/1786568056212-rust/evidence.json`.

## Оценка

25–60 активных минут.
