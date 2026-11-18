# ClipForge v0.4.0 — Windows AI Twitch Highlights Studio

**Статус: предварительная версия.** Windows x64 · Rust / Tauri 2 · React / TypeScript · FFmpeg.

Приложение отслеживает чаты разрешённых Twitch-каналов, выявляет всплески активности, создаёт клипы Twitch длительностью **не более 60 секунд**, обрабатывает имеющиеся разрешённые MP4 в вертикальные ролики и поддерживает отправку на YouTube, а для TikTok пока предусмотрен только модуль загрузки черновиков. **Непрерывный live Vision, распознавание речи, полностью автономная публикация и проверенная поддержка любого Twitch-канала ещё не реализованы.**

## Что нового в v0.4

- **Кнопка «Войти через Twitch»** в настройках. Twitch Device Authorization Flow откроет официальный сайт; Twitch может попросить ввести короткий код. Пользователь не вводит API-токены и Client ID вручную.
- **Кнопка «Войти через YouTube»**. Открывается системный браузер с официальным OAuth Google для desktop-приложения, PKCE и локальным loopback callback. Подключённые YouTube-каналы отображаются отдельно, доступны выбор и отключение.
- **Публикация через выбранный аккаунт**, а не через ввод произвольного access token. OAuth-токены хранятся в оперативной памяти Rust-процесса и не попадают в интерфейс.
- **TikTok**: интерфейс содержит кнопку подключения, которая честно сообщает, что для рабочей авторизации нужен одобренный TikTok Login Kit и сервер OAuth с защищённым client secret. Встраивать секрет в публичный .exe небезопасно. В текущем релизе эта кнопка **не подключает TikTok**.
- **Проверка обновлений** при старте и по кнопке в Настройках. Программа обращается к GitHub Releases, показывает новую версию и предлагает открыть официальный установщик. **Беззвучной/автоматической установки нет**.
- Улучшен статус кнопок, показ ошибок и пустых состояний.

## Скачать и установить

Открой [Releases](https://github.com/apollo-170/GTA6inTTandYT/releases) и скачай файл с окончанием `_x64-setup.exe`. Для готового EXE **не нужно устанавливать Rust или Node.js**.

**Важно:** на момент разработки репозиторий **private**. Для просмотра и скачивания релизов нужно войти в GitHub с правами доступа. Программа не может автоматически получить список приватных релизов без авторизованного API-доступа — в таком случае в «Обновлениях» есть кнопка «Открыть Releases» для браузера. Превращать репозиторий в публичный без разрешения владельца нельзя.

## Одноразовая настройка разработчика для кнопок входа

Для реального входа владельцу приложения нужно создать приложения у провайдеров и прописать **публичные ID приложения** как GitHub Actions Repository Variables (это не пароли пользователей):

```
CLIPFORGE_TWITCH_CLIENT_ID     # Twitch Developer Console → application Client ID
CLIPFORGE_YOUTUBE_CLIENT_ID    # Google Cloud Console → OAuth client ID (type Desktop app)
```

Путь: GitHub → Settings → Secrets and variables → Actions → Variables → New repository variable. После создания переменных заново запусти Windows Actions и скачай свежий установщик. **Не публикуй client secrets, refresh/access tokens или пароли** в GitHub, чатах или исходном коде.

Google: включи **YouTube Data API v3**, настрой OAuth consent screen и тестовых пользователей либо verification, если требуется. Twitch: зарегистрируй приложение и запроси необходимые разрешения. OAuth Client ID относится к приложению разработчика; без него официальная кнопка входа физически не может получить доступ к Twitch/Google API.

Разрешения YouTube: `youtube.upload`, `youtube.readonly`. Пользователь выбирает канал в браузере. Access/refresh tokens хранятся **только в RAM**, поэтому после перезапуска в текущем MVP вход потребуется повторить. Для полноценной версии нужен защищённый Windows Credential Locker и проверенная обработка обновления токенов.

Для TikTok не добавляй `client_secret` в EXE! Нужен отдельный backend с секретом и соответствующими правами разработчика. Пока такого backend в этом проекте нет.

## Тестирование и локальная разработка

Для Windows-разработки установи Node 20+, Rust, MSVC Build Tools. В папке проекта:

```powershell
npm install
npm run dev:desktop
```

Создание EXE на Windows:

```powershell
npm run build:desktop
```

GitHub Actions workflow: `.github/workflows/build-windows-exe.yml`. В момент сборки скачивается FFmpeg, после чего создаётся NSIS installer, portable ZIP и Release для релизного коммита. Без указанных repository variables EXE **скомпилируется**, но OAuth-кнопки сообщат об отсутствии настройки.

## Текущее поведение

- **Twitch:** chat EventSub, 10-секундный детектор всплесков, динамический выбор длины до 60 секунд, официальные Twitch Clips и загрузка только при наличии необходимых прав.
- **Video:** разрешённый локальный видеофайл, FFmpeg H.264/AAC 1080 × 1920 с размытым фоном и баннером.
- **Vision AI:** анализ одного кадра готового видео после явного действия, не постоянный live-анализ.
- **YouTube:** OAuth browser + PKCE + выбор канала, отправка только после подтверждения пользователя.
- **TikTok:** официальная функция загрузки черновиков присутствует в backend, но пользовательского OAuth-подключения пока нет; реальная публикация недоступна без настроенной интеграции.
- **Обновления:** проверка GitHub Releases, сообщение и ссылка на официальный установщик. Без автоматической установки; для private repo нужен отдельный доступный feed.

Публикуй только собственные материалы или материалы стримеров, давших разрешение на повторное использование. Не обходи авторизацию и ограничения платформ.

## v0.4.1 — диагностика интеграций

В настройках открой **Диагностика подключений → Проверить подключения**. Приложение покажет статус Twitch, YouTube, TikTok, FFmpeg и канала обновлений. Кнопка «Настройка разработчика» открывает GitHub Actions Variables. Разработчику нужно зарегистрировать собственные приложения и установить публичные `CLIPFORGE_TWITCH_CLIENT_ID` и `CLIPFORGE_YOUTUBE_CLIENT_ID`; они передаются в компилятор. Пароли и client secrets нельзя хранить в публичных переменных или в EXE.

После Twitch OAuth программа автоматически выходит из демонстрационного режима. Каналы, добавленные в деморежиме, нужно удалить и заново добавить как реальные Twitch-каналы. TikTok пока требует отдельной серверной OAuth-интеграции, а приватность GitHub-репозитория ограничивает автоматическую проверку обновлений: вкладка «Обновления» открывает Releases в браузере.

## v0.4.2 — OAuth Client IDs включены в сборку

Настроены публичные Twitch Client ID и Google OAuth Desktop Client ID через резервные значения GitHub Actions. Для Google применено окончание `.apps.googleusercontent.com`, поскольку изначально был прислан идентификатор без окончания. При создании Windows EXE компилятор получает оба значения через переменные окружения; пользователь **не должен** вставлять токены или Client ID в форму.

**Проверка:** значения соответствуют ожидаемому синтаксису, но регистрация приложений в Twitch и Google на стороне провайдеров, экран согласия Google, YouTube Data API v3 и тестовые пользователи всё ещё должны быть корректно настроены владельцем. Сборка EXE сама по себе не подтверждает успешный вход.

В уведомлениях успешное подключение YouTube теперь отображается как успех.

## v0.4.3 — публикация напрямую через Twitch

Во вкладке «Публикация» появилась отдельная панель **«Публикация через Twitch»**. После создания настоящего Twitch Clip нажми **«Открыть клип и поделиться»**. Откроется официальный интерфейс Twitch: нажми «Поделиться», затем выбери YouTube Shorts или TikTok, если публикация разрешена владельцем канала. Кнопка **«Связанные аккаунты Twitch»** ведёт на `https://www.twitch.tv/settings/connections`.

Это открывает официальный UI Twitch, но НЕ выполняет автоматическую публикацию от имени пользователя через несуществующий Twitch Export API. Некоторые стримеры запрещают прямую публикацию своих клипов в соцсети: приложение уважает эти ограничения.

## Проверка обновлений и ограничение приватного репозитория

ClipForge проверяет наличие новых версий при запуске, каждые 30 минут при открытом приложении и когда окно получает фокус. При доступном обновлении показывает уведомление и кнопку для загрузки установщика из GitHub Releases.

**Сейчас GitHub-репозиторий приватный**, поэтому без авторизации программа не может получить API-данные релизов, и автоматическое обнаружение обновлений не будет работать. Для установки обновления из приватного репозитория предусмотрена кнопка «Открыть Releases» в системном браузере (в браузере нужен вход в GitHub). Для надёжной **установки внутри приложения в один клик** необходимо создать отдельный публичный репозиторий дистрибутивов/manifest и настроить Tauri v2 updater с постоянным ключом подписи. **Автоматическая установка пока не реализована, чтобы не запускать неподписанные EXE**. После настройки публичного feed и ключа подписи можно включить updater.


## ClipForge v0.5.0 — обновления из приложения

ClipForge проверяет новые версии при запуске, при возвращении в окно и каждые 30 минут. Появляется окно «Центр обновлений», кнопка **«Скачать и установить»**, прогресс скачивания, проверка SHA-256 и запуск обычного Windows NSIS-установщика **только после действия пользователя**. Исходный релиз выбирается исключительно из заданного GitHub-репозитория; пользовательский URL нельзя передать в команду установки. SHA-256 сверяется с неизменённым полем digest из GitHub Release API; при несовпадении приложение отменяет запуск и удаляет повреждённый файл.

**Текущий блокер:** исходный репозиторий \`apollo-170/GTA6inTTandYT\` приватный. GitHub Releases API отвечает 404 для неавторизованных приложений, поэтому ClipForge не сможет обнаружить и установить новый релиз автоматически, пока не будет открыт публичный канал релизов. В приложении предусмотрена резервная кнопка «Открыть GitHub Releases» для ручной установки с авторизацией в браузере. Не вставляйте персональный GitHub Token в EXE. Варианты: сделать репозиторий публичным или создать отдельный публичный репозиторий только для EXE релизов и организовать их публикацию.

**Статус безопасности:** автоматический установщик проверяет SHA-256, но пока не использует подписанные пакеты Tauri Updater. Для стабильного публичного распространения рекомендуется добавить постоянную подпись обновлений, хранить приватный ключ только в GitHub Actions Secrets и обеспечить публичный канал релизов.

**TikTok:** самый простой рабочий способ — привязать TikTok в \`https://www.twitch.tv/settings/connections\`, открыть свой Twitch Clip в ClipForge и опубликовать его через официальный Twitch интерфейс «Поделиться». Это ручная публикация, а не автоматическая отправка через TikTok API. Для отправки уже смонтированных MP4 из ClipForge нужен отдельный TikTok Developer app, Login Kit, разрешения Content Posting API, сервер OAuth и проверка платформой.

## v0.6.0 — мультипубликация

Один готовый вертикальный MP4 можно отправить на **несколько подключённых YouTube-каналов и TikTok-профилей** за один запуск (до 20 аккаунтов). В разделе «Публикация» доступны выбор всех аккаунтов, выбор платформы, выбор отдельных профилей, правообладательское подтверждение и отдельные статусы загрузки каждого аккаунта. Новые аккаунты не выбираются автоматически — пользователь должен отметить их самостоятельно.

**YouTube:** нажимай «Войти через YouTube» отдельно для каждого канала, который можешь администрировать. После OAuth подтверждения каждый канал появится в списке. YouTube API загружает MP4 непосредственно на каждый выбранный канал, используя принадлежащий ему токен. Каналы, не указанные среди Google OAuth Test Users при Testing, могут получить ошибку 403; исправить это можно в Google Auth Platform → Audience.

**TikTok:** доступен отдельный `tiktok-bridge/server.mjs` и инструкция `tiktok-bridge/README.md`, чтобы подключать несколько TikTok-профилей без Client Secret в EXE. Для реального подключения владелец должен зарегистрировать TikTok Login Kit Web app, получить разрешения `user.info.basic` и `video.upload`, запустить HTTPS bridge со своим Client Secret в защищённых переменных сервера и указать в GitHub Actions Variables **`CLIPFORGE_TIKTOK_BRIDGE_URL`**. OAuth bridge шифрует токены и возвращает программе временные разрешения для каждого аккаунта. Пока bridge не развёрнут, приложение показывает понятную ошибку вместо неработающей авторизации. Загруженные ролики попадают в TikTok Inbox как черновики; пользователю нужно завершить публикацию в TikTok. Настоящий Direct Post требует отдельной проверки и разрешения `video.publish` и здесь не включён.

Результаты каждой попытки видны сразу; ошибка одного аккаунта не блокирует остальные. Публикация не начинается без явного подтверждения пользователя. Загружай только принадлежащие тебе материалы или ролики, на повторное использование которых есть разрешение.


## ClipForge v0.7.0 — локальная автоматизация без собственного сервера

**Основной рекомендуемый путь:** официальная авторизация Twitch + Upload-Post Cloud для нескольких TikTok/YouTube + Windows Credential Manager + GitHub Releases. Код \`tiktok-bridge/\` остался экспериментальной альтернативой и **не нужен** для основного сценария.

### Подключение Upload-Post без своего сервера

1. Создай аккаунт в [Upload-Post](https://www.upload-post.com/) и при необходимости выбери тариф, который поддерживает TikTok. **На бесплатном плане Upload-Post TikTok-загрузки ограничены/недоступны; проверь свой тариф.**
2. Открой Upload-Post Dashboard → API Keys, создай ключ и **один раз вставь его непосредственно в ClipForge → Публикация → Подключение соцсетей → Сохранить ключ**. Не отправляй ключ в чат и не вставляй его в GitHub Actions Variables.
3. В ClipForge укажи название профиля \`clipforge_1\`, нажми «Подключить TikTok / YouTube» и авторизуй нужный аккаунт на официальной странице Upload-Post.
4. Для второго TikTok-аккаунта создай профиль \`clipforge_2\` и повтори авторизацию. Для нескольких YouTube-каналов — так же. Нажми «Обновить список аккаунтов».
5. На вкладке «Публикация» отмечай несколько выбранных профилей для **ручной пакетной отправки** или выбирай их в панели «Полный автоматический конвейер».

Ключ Upload-Post хранится **только** в Windows Credential Manager для данного пользователя Windows. ClipForge не встраивает его в EXE, не пишет в настройки SQLite и не коммитит на GitHub. Используй только свои соцсети или аккаунты, где тебе разрешено публиковать.

### Включение полного автоматического конвейера

1. Войди в Twitch и подключи канал, на который имеешь права. В «Стримеры» отметь «Есть разрешение на клипы».
2. В «Публикация → Полный автоматический конвейер» включи \`Автоматически создавать клипы\` и \`Автоматически монтировать Shorts\`, затем \`Автоматически загружать на соцсети\`.
3. Отметь профили Upload-Post, выбери **TikTok: MEDIA_UPLOAD** (черновики) или **DIRECT_POST** (прямая публикация, только если она разрешена платформой и доступна в твоём тарифе), настрой приватность YouTube и ограничение клипов за 24 часа. Подтверди права и нажми «Сохранить настройки автоматики».
4. Запусти мониторинг Twitch. Когда детектор заметит сильный всплеск чата, ClipForge создаст официальный Twitch Clip. Если Twitch даст разрешённую ссылку \`GET /helix/clips/downloads\`, программа скачает MP4, создаст 9:16 Shorts через встроенный FFmpeg и загрузит в выбранные профили через Upload-Post. По каждому профилю отображается статус, в том числе «в очереди», «успешно» или «ошибка».

**Ограничения:** Этот конвейер работает **только пока Windows-приложение открыто и ПК подключён к интернету**. Он обнаруживает события на основе чата, а не анализирует непрерывный поток видео/аудио AI. Для скачивания Twitch Clip нужны реальные OAuth-права владельца/редактора (\`editor:manage:clips\` или \`channel:manage:clips\`). Если Twitch не разрешает скачать клип, программа не обходит ограничение. Для TikTok \`MEDIA_UPLOAD\` означает черновик, который пользователь завершает в TikTok; \`DIRECT_POST\` зависит от разрешений сервиса и платформы. Не обещайте мгновенную публичную публикацию, если Upload-Post вернул только \`request_id\` очереди.

По умолчанию \`auto_publish=false\`, \`auto_daily_limit=5\`, \`auto_tiktok_mode=MEDIA_UPLOAD\`, \`auto_youtube_privacy=private\`. Запуск автопубликации невозможен без явного подтверждения прав, выбранных профилей, включённого экспорта и защищённого Upload-Post ключа.

### Serverless OTA из приватного GitHub

ClipForge v0.7.0 теперь может проверять и устанавливать версии **из приватного** \`apollo-170/GTA6inTTandYT\` прямо в Windows, без публичного репозитория и без собственного сервера, после одноразовой настройки доступа:

1. В [GitHub Fine-grained Personal Access Tokens](https://github.com/settings/personal-access-tokens/new) создай токен **только для репозитория** \`apollo-170/GTA6inTTandYT\` с **Contents: Read-only**. Не добавляй write permissions.
2. Открой ClipForge → Настройки → Обновления → «Подключить обновления без публичного репозитория», вставь токен и нажми «Подключить OTA».
3. ClipForge сохранит токен в Windows Credential Manager. Далее он проверяет GitHub Releases при старте, по возвращении в окно и каждые 30 минут. При новой версии покажет окно. Нажми «Скачать и установить», после чего программа загрузит asset через GitHub Releases API, сверит SHA-256 с GitHub release metadata и запустит проверенный установщик с обычным подтверждением Windows.

**Это consent-based обновления установщика, а не полностью бесшумные Tauri signed differential OTA.** SHA-256 защищает от повреждённых файлов в рамках доверия к GitHub API, но не заменяет отдельную криптографическую подпись разработчика. Публичный Tauri подписанный updater позже можно внедрить без собственного сервера через отдельный публичный репозиторий релизов и постоянный ключ подписи (приватный ключ должен храниться в GitHub Actions Secrets). **Первую v0.7.0 нужно установить вручную** из GitHub; последующие обновления доступны в приложении после подключения токена. Не раскрывай GitHub-токен и не встраивай его в EXE.


## v0.7.1 — основной режим бесплатно

Вкладка «Публикация» теперь открывается с бесплатными вариантами: официальный Twitch, YouTube Data API и локальный FFmpeg. Режим автоматической отправки принимает **прямые YouTube OAuth аккаунты** с идентификаторами \`youtube:...\`, не требует Upload-Post API-ключа, облачной подписки и собственного сервера. Новый токен/refresh token YouTube сохраняется в защищённом **Windows Credential Manager**, отдельно для каждого канала, восстанавливается при перезапуске и обновляется по мере необходимости. Уже подключённые до этого обновления каналы нужно авторизовать ещё раз, чтобы сохранить разрешение на дальнейшие запуски.

Для автоматического YouTube: войди в YouTube для каждого канала в ClipForge, включи авто-клип, авто-экспорт и автопубликацию, отметь каналы, поставь видимость \`private\` для проверки и подтверди право на использование видео. Мониторинг должен быть запущен, а Twitch — предоставлять разрешённый клип. При поступлении сигнала из чата ClipForge создаст клип, скачает его при наличии прав, смонтирует и загрузит на выбранные YouTube-каналы.

**TikTok:** бесплатный официальный способ из ClipForge — открыть разрешённый Twitch Clip и использовать официальную кнопку «Поделиться» TikTok на Twitch. Это **ручное подтверждение**, а не скрытая или гарантированная автоматическая публикация. Для настоящего TikTok Direct Post нужны собственная одобренная интеграция Content Posting API и разрешения \`video.publish\`; API не тарифицируется Upload-Post, но регистрация, аудит и разрешения остаются требованиями TikTok. Нельзя обещать TikTok-постинг на много аккаунтов в фоне без таких разрешений. Платный Upload-Post оставлен только в свёрнутой необязательной секции для тех, кто уже использует сервис.

Старые возможности OTA для закрытого GitHub сохранены: токен только для чтения релизов в Windows Credential Manager, проверка версий и SHA-256 установщика, обновление после нажатия пользователя.

**Важное ограничение:** это исходники и успешно собранный Windows preview, а не сквозной тест на реальных аккаунтах. Google OAuth может потребовать добавления пользователей в Testing; TikTok может ограничивать публикацию исходя из прав канала и возраста пользователя.


## ClipForge v0.8.0 — native multi-account TikTok publishing (no paid services)

ClipForge v0.8.0 includes a Windows-native integration with TikTok's official Desktop Login Kit and Content Posting API. This is **not** browser automation, cookie scraping, CAPTCHA bypass, or a way around TikTok's verification requirements. It is provided for authorized personal developer use; **public automatic TikTok posts require TikTok Developer approval and an audited client**.

### Setup

1. In [TikTok for Developers](https://developers.tiktok.com/), register your own application (subject to TikTok's eligibility requirements).
2. Enable **Login Kit → Desktop**. Add this **exact Redirect URI**:
   \`http://127.0.0.1:34865/callback/\`
3. Enable **Content Posting API**. Apply for the scopes \`user.info.basic\` and either:
   - \`video.upload\` to send videos to TikTok Inbox (user completes the post), or
   - \`video.publish\` for Direct Post (subject to approval and audit for public visibility).
4. In ClipForge → Публикация → TikTok, enter **Client Key** and **Client Secret** from your own Developer app once. They are saved only to **Windows Credential Manager**, not the frontend, SQLite, source files or releases. This personal setup is not an appropriate way to distribute one application's confidential Client Secret to many unrelated users.
5. Select either **Direct Post** or **Черновики**, then click **Добавить TikTok-аккаунт**. TikTok authorization opens in your browser and returns to a temporary listener on \`127.0.0.1:34865\`. ClipForge uses OAuth \`state\` plus **PKCE** (TikTok's SHA256 hex challenge) to validate the callback.
6. Repeat step 5 for each authorized TikTok account. Refresh/access tokens are stored individually in Windows Credential Manager and refreshed with rotated tokens when necessary. You may disconnect an account from ClipForge.
7. Click **Права** for a connected TikTok profile to retrieve its latest creator privacy options. TikTok is the source of truth for permitted privacy and whether a public post is possible.

### Publishing and automatic queue

On **Публикация**, select any combination of TikTok and YouTube accounts (up to 20) and confirm the rights to the MP4, caption and audience settings. For TikTok, choose the audience (default: **SELF_ONLY**). ClipForge requests Creator Info from TikTok for each Direct Post attempt, verifies the selected audience is available and uses the official video/init endpoint. Files go to TikTok's own \`open-upload.tiktokapis.com\` upload URL; ClipForge queries publish status before reporting success, and preserves queued/draft/failed statuses instead of guessing.

Automatic publishing uses the existing **Twitch → clip → FFmpeg Shorts → platform** pipeline when enabled by the owner, with explicit prior rights confirmation, selected target accounts and daily caps. **For native TikTok accounts background automation is limited to Inbox drafts (video.upload)**; Direct Post requires opening the content in the publishing screen, reviewing the chosen audience and giving explicit per-video consent, as required by TikTok's sharing UX guidelines. Exact permissions and user approval requirements imposed by TikTok remain authoritative and may require interactive approval for a given post. The application never fakes authorization, bypasses platform controls or silently substitutes a public audience for a private one. If TikTok blocks or delays a post, the per-account status explains the issue. TikTok may also restrict posting based on creator age, account status or platform eligibility; respect these restrictions.

If the app has only \`video.upload\`, ClipForge sends the file to **Inbox as a draft**; the TikTok user must complete posting manually. If your app is **unaudited**, TikTok Direct Post is restricted to **private** visibility. Approved \`video.publish\` + an audited client are prerequisites for normal public direct posting. The app will not work until you provide **your own TikTok Developer credentials and receive the required permissions**.

### Known limitations

- No TikTok Client Key or Secret is provided in the prebuilt EXE.
- The TikTok native integration has been compiled in CI, but not verified against real TikTok accounts; an end-to-end authorization and posting test must be performed with an approved app.
- TikTok public publishing and per-post consent must follow the current platform rules.
- This uses your local computer, **no paid third-party publishing service**, but it does not imply that approval, account eligibility or TikTok limits can be skipped.
- The optional legacy TikTok OAuth Bridge and paid Upload-Post connector remain in the repository but are not necessary for native personal TikTok publishing.
