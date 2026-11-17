# ClipForge v0.1 — AI Twitch Highlights Studio (Windows)

**Статус:** открытый исходный код, MVP / developer preview. Не финальный коммерческий релиз; в данной среде не выполнены `cargo build` и `npm install` (отсутствует Rust toolchain и сетевой доступ к npm registry). Сама программа не содержит включённых OAuth-ключей и не подключена к реальным Twitch/TikTok/YouTube аккаунтам. Не называйте её «готовой .exe» до локальной сборки.

## Готовый установщик Windows .exe через GitHub Actions

Проект теперь содержит `.github/workflows/build-windows-exe.yml`. Этот workflow компилирует настоящее приложение **на Windows**, добавляет FFmpeg внутрь NSIS-установщика и создаёт два скачиваемых артефакта:

- `ClipForge-Windows-Installer-EXE` — установщик `.exe`; достаточно запустить его на Windows, установка Rust/Node пользователю не нужна. Microsoft WebView2 может быть установлен или загружен установщиком при необходимости.
- `ClipForge-Windows-Portable` — ZIP с `ClipForge.exe` и папкой `resources/ffmpeg.exe`; не разрывайте их при переносе.

**Запуск сборки:** создайте пустой репозиторий GitHub, загрузите **содержимое** папки `clipforge` в корень, откройте **Actions → ClipForge Windows EXE → Run workflow**. После выполнения скачайте артефакт со страницы запуска. У GitHub могут быть свои ограничения на доступ к Actions.

**При сборке непосредственно в Windows:** запустите `powershell -ExecutionPolicy Bypass -File .\build-windows.ps1` с установленными Rust/Node/MSVC. Сборка использует интернет для пакетов и FFmpeg.

**Важно:** в текущем Linux-контейнере Windows EXE собрать и испытать невозможно (нет Cargo и доступа к registry). Автоматическая Windows-сборка подготовлена, но не проходила реальный запуск. OAuth-ключи, разрешения на видеоконтент и ограничения TikTok/YouTube всё ещё применимы. Сам факт компиляции не означает, что все функции продукта уже реализованы.

## Что работает в коде

1. **Tauri 2 + React + TypeScript** — десктопный интерфейс с обзором, списком стримеров, библиотекой событий, видеостудией, публикацией и настройками.
2. **SQLite** в системном каталоге `ClipForge` — хранение настроек, списка каналов и истории моментов.
3. **Twitch Device Authorization Flow** — OAuth вход без встраивания Client Secret в приложение. Access token остаётся в оперативной памяти текущей сессии.
4. **Twitch EventSub WebSocket** — подписка на `channel.chat.message` для нескольких каналов от имени авторизованного пользователя. Нужный scope: `user:read:chat`.
5. **Chat Spike Detector** — 10-секундные окна, сравнение с историей, эмоциональные реакции, число уникальных зрителей, порог и cooldown. Это эвристический детектор, не гарантия качества клипа.
6. **Twitch Clips** — динамическая длина 35/45/55/60 сек, до 60 сек. Для авторизованных редакторов/владельцев: попытка создания точного окна через `POST /helix/videos/clips`; при недоступности VOD и свежем событии — запасной `POST /helix/clips` (последний фрагмент эфира). Предусмотрено асинхронное ожидание создания клипа.
7. **Twitch Clips Download** — получение MP4 через официальный `GET /helix/clips/downloads` только при наличии прав владельца/редактора. Для остальных видна ссылка на Twitch Clip, если клип создан.
8. **FFmpeg** — локальный экспорт исходного видео в 1080×1920 H.264/AAC, размытый фон, центрированное исходное видео, PNG/JPEG баннер. На вход можно выбрать собственный разрешённый MP4/MOV/MKV/WEBM файл.
9. **Опциональная Vision AI** — извлечение одного кадра готового клипа и оценка через OpenAI-совместимый Chat Completions API. Поддержка локального endpoint или HTTPS; ключ вводится в сессии и не пишется на диск.
10. **Публикация в несколько аккаунтов** — YouTube Resumable Upload с выданными OAuth access tokens и TikTok Content Posting API Inbox Upload (черновики). Пользователь отмечает выбранные аккаунты и подтверждает законность и отправку ролика. В TikTok публикацию черновика нужно завершить внутри TikTok. OAuth-токены YouTube/TikTok для прототипа вводятся вручную; встроенный OAuth flow для этих двух платформ — ещё не реализован.
11. **Browser demo** — `npm run dev` покажет интерфейс без Rust (локально сохранённое тестовое состояние). Для настоящих Twitch, FFmpeg и публикаций нужен `npm run dev:desktop`.

## Ограничения версии 0.1

- **Полный непрерывный видеомониторинг каждого Twitch-кадра не реализован.** Детектор в реальном времени использует Twitch чат. Vision AI анализирует кадр **после появления исходного видео**. Для live Vision требуются разрешённый видеовход/OBS и контроль GPU/стоимости API.
- Нельзя автоматически скачивать чужие клипы без Twitch-прав; у приложения нет и не должно быть скрытого обхода.
- Нет распознавания речи Whisper, слежения за facecam или автосубтитров. Эти функции требуют отдельного этапа.
- При переключении demo → real нужно удалить demo-каналы и заново добавить их имена в реальном режиме.
- Twitch EventSub подписки на активные каналы ставятся в момент старта мониторинга. При изменении списка каналов остановите/запустите мониторинг.
- Точные границы событий при создании из VOD зависят от доступности VOD и синхронизации timestamp. Если endpoint недоступен, live fallback возвращает только окно, заканчивающееся около времени вызова. При позднем ручном запросе этот fallback отключён, чтобы не нарезать неправильный момент.
- Автопубликация TikTok напрямую без явного согласия пользователя и аудита API недоступна. Эта версия поддерживает официальную отправку в черновики.
- Не реализованы OAuth refresh для Twitch, YouTube, TikTok, мониторинг статуса публикации и очереди восстановления после перезапуска. При истечении токена требуется повторный вход / обновлённый access token.
- Нет общего rate-limit manager для нескольких сотен чатов и smart video encoding GPU. Для десятков каналов нужны более продвинутые backpressure/очереди.
- Экспорт FFmpeg ограничивает ролик 60 секундами. Для длинного локального файла используется его начало; выбор произвольного фрагмента длинного исходника ещё не реализован.

## Как запустить в Windows

**Требования:** Windows 10/11 x64, Node.js 20+, Rust stable (актуальная версия через rustup), Microsoft C++ Build Tools (Desktop development with C++), WebView2 и FFmpeg, добавленный в PATH. В данном выпуске создаётся NSIS-установщик `.exe`, а не MSI.

1. Распакуйте архив в любую папку, например `C:\\Projects\\ClipForge`.
2. Откройте PowerShell или терминал в корне проекта.
3. Запустите:

   ```powershell
   npm install
   npm run dev:desktop
   ```

4. Для production-пакета:

   ```powershell
   npm run build:desktop
   ```

   Установщик NSIS / MSI появится в `src-tauri\target\release\bundle\` после успешной сборки.

Для **предпросмотра интерфейса без Rust** запустите `npm run dev` и откройте адрес Vite. Это демо с локальными тестовыми данными.

### Twitch API

1. Создайте приложение в [Twitch Developer Console](https://dev.twitch.tv/console/apps).
2. В настройках ClipForge вставьте **Client ID**; Client Secret не вставляйте и никому не отправляйте.
3. Сохраните, выключите «Демонстрационный режим», нажмите «Войти через Twitch».
4. Программа использует Device Flow: браузер откроет ссылку подтверждения, в интерфейсе появится код.
5. После подтверждения добавьте каналы и включите отслеживание.
6. Для автоматического создания клипов включите опцию и подтвердите права у соответствующих каналов. Twitch API всё равно проверит реальные полномочия и настройки канала.

Документация: [EventSub](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/), [Clips API](https://dev.twitch.tv/docs/api/reference/clips), [OAuth Device Flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/).

### FFmpeg

- Скачайте FFmpeg с его официального сайта, добавьте `ffmpeg.exe` в PATH или укажите абсолютный путь в настройках.
- Установите баннер PNG/JPG в «Видеостудии» или настройках. Сохраните настройки.
- Для Twitch-клипов с правами редактора приложение попытается скачать MP4 автоматически. Или прикрепите разрешённый локальный MP4.
- Нажмите «Сделать вертикальное видео». Файл появится в выбранной папке, по умолчанию `Videos\ClipForge`.

### YouTube и TikTok

**YouTube:** используйте OAuth access token, выданный вашему приложению для scope `https://www.googleapis.com/auth/youtube.upload`. В поле «Access Token» нужен именно токен, **не пароль**. `privacyStatus` выбирается перед отправкой. Встроенный OAuth UI для YouTube появится в следующем этапе. [YouTube upload docs](https://developers.google.com/youtube/v3/docs/videos/insert).

**TikTok:** используйте официальный TikTok Content Posting API с разрешением `video.upload`. Текущая реализация отправляет видео в Inbox Draft, **не публикует ролик публично автоматически**. Завершить публикацию нужно внутри TikTok; приложение должно быть зарегистрировано у TikTok и иметь нужный scope. [TikTok upload docs](https://developers.tiktok.com/doc/content-posting-api-reference-upload-video).

**Права:** публикуйте только собственные ролики или контент стримеров, давших разрешение. Учитывайте ограничения по возрасту и правила Twitch/TikTok/YouTube. Программа не обходит модерацию, авторизацию и права на видео.

## Структура

```
clipforge/
  src/                   React + TS, стили, API bridge
  src-tauri/src/
    lib.rs               Tauri IPC, Twitch EventSub, state machine
    twitch.rs            Twitch OAuth, Helix, clips, download
    scoring.rs           Чат-детектор и unit tests
    db.rs                SQLite persistence
    export.rs            FFmpeg 9:16 pipeline
    vision.rs            Optional Vision Chat Completions
    publisher.rs         YouTube upload, TikTok draft
    models.rs            Типы данных
  src-tauri/icons/        Иконки Windows
  examples/               Место для demo content
```

## Контроль и безопасность

- Секретные токены не лежат в исходном коде, не сохраняются в SQLite и не должны отправляться третьим лицам.
- Twitch и видеообработка идут в Rust backend; UI общается с ним через Tauri IPC.
- Имя выходного файла санитизируется; запуск FFmpeg не происходит через shell-строку.
- Модуль Vision по явному действию может передать скриншот удалённому провайдеру; оцените политику приватности модели перед использованием.
- Для production необходимы аудит OAuth, Windows Credential Manager/DPAPI для постоянных токенов, отложенная очередь и проверка пользовательских разрешений.

## Проверки качества

В проекте есть базовые Rust tests (`cargo test`). После установки зависимостей выполнить `npm run build` и `cargo test --manifest-path src-tauri/Cargo.toml`. В текущем окружении TypeScript-синтаксис проверен с помощью `typescript.transpileModule`, JSON валидирован, медиа-иконки созданы. FFmpeg-пайплайн отдельно проверен на тестовом MP4 с баннером: создан корректный H.264 ролик 1080×1920. **Сборка Windows-EXE и настоящие API-запросы не проверены**, так как здесь нет Windows, Rust toolchain, ключей и активных аккаунтов.

---

**Дальнейшие задачи:** Windows Credential Manager + OAuth YouTube/TikTok, поддержка OBS/RTMP/SRT для live Vision и аудио, Whisper субтитры, умный facecam crop, GPU encoding, надёжная очередь с повторными попытками, автоматизация YouTube по длительному согласию пользователя и TikTok Direct Post после API review.
