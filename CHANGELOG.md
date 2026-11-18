# ClipForge v0.4.0

## Новое

- Единый экран подключений: Twitch, YouTube, TikTok.
- Twitch Device Flow без ручного ввода Client ID в UI (публичный ID задаёт разработчик при сборке).
- YouTube Google OAuth в системном браузере: PKCE, loopback callback, выбор канала, подключение нескольких каналов.
- Токены Google хранятся в Rust-процессе, не в React UI.
- Публикация через конкретный подключённый аккаунт и явное подтверждение.
- Раздел обновлений с проверкой GitHub Releases, ссылкой на установщик и проверкой при запуске.
- Обновлённые подсказки, сообщения об ошибках и документация.

## Ограничения

- TikTok OAuth требует серверной части с защищённым client_secret и доступа к TikTok API. В этой сборке кнопка показывает причину недоступности, но не подключает TikTok.
- Кнопки Twitch и YouTube требуют настроенных авторизационных ID разработчика в сборке (GitHub Actions variables CLIPFORGE_TWITCH_CLIENT_ID и CLIPFORGE_YOUTUBE_CLIENT_ID). Пользователь не вводит токены.
- В GitHub private-репозитории автоматическая проверка обновлений без специального публичного канала выдачи релизов недоступна. Есть кнопка открытия страницы Releases после входа в браузер.
- OAuth-сессии сохраняются только до закрытия приложения.
- Не добавлены live Vision, Whisper, завершение публикации в TikTok или автоматическая установка обновлений.

Это preview-сборка. Для полного продакшена потребуются end-to-end проверки API, безопасное долговременное хранение OAuth и пользовательское тестирование на Windows.

## v0.4.1
- Статус интеграций Twitch, YouTube, TikTok и FFmpeg непосредственно в приложении
- Исправлено включение настоящего режима после успешного входа Twitch
- Понятная ошибка при попытке мониторить демонстрационные каналы через EventSub
- GitHub Actions проверяет TypeScript и предупреждает об отсутствующих публичных OAuth Client ID
- Проверка обновлений и открытие GitHub Releases сохраняются

## v0.4.2
- В сборку Windows включены предоставленные владельцем публичные Twitch и Google OAuth Client IDs
- Нормализован Google OAuth Client ID с окончанием `.apps.googleusercontent.com`
- Проверка формата идентификаторов в GitHub Actions
- Исправлен цвет уведомления об успешном подключении YouTube

## v0.4.3
- Shortcut to share an existing Twitch Clip directly using Twitch's own official YouTube Shorts / TikTok sharing UI
- Quick link to Twitch account connections
- Allow creating a Twitch Clip directly from the Publishing tab when one is not ready
- Periodic update checks (startup, every 30 minutes, window focus) and deduplicated in-app notices when the update API is reachable
- Clarifies private repository update limitations without silently installing unsigned executables

## v0.5.0
- Новый лаконичный дизайн graphite/emerald и реальные показатели мониторинга
- Центр обновлений, уведомление внутри приложения, проверка каждые 30 минут и при фокусе
- Загрузка Windows EXE из доверенного GitHub Releases, прогресс и проверка SHA-256 перед запуском установщика
- Установщик запускается только после подтверждения пользователя
- Кнопка публикации в TikTok через официальный Twitch Connections
- SHA-256 установщика прикладывается к новому GitHub Release
- Приватный GitHub по-прежнему требует входа в браузере; без публичного канала приложение не может проверить версию автоматически
