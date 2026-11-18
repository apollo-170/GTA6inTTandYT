# ClipForge — TikTok OAuth Bridge (Node.js)

This optional companion server enables **multiple TikTok accounts** inside the Windows ClipForge app without placing TikTok's Client Secret in the executable. The desktop app sends authorized videos to TikTok's **creator inbox drafts** using TikTok's approved \`video.upload\` permission. The TikTok user must finish publishing in TikTok. Fully automatic public posts are a different feature requiring \`video.publish\`, TikTok Direct Post approval/audit, privacy UI, and implementation of the Direct Post API.

**Not deployed by this repository.** The server source is provided, but the owner's domain, credentials, and running server are required before the ClipForge TikTok button can sign anyone in.

## TikTok Developer setup

1. Create an app at [TikTok for Developers](https://developers.tiktok.com/), subject to TikTok's eligibility and platform requirements.
2. Enable **Login Kit (Web)** and the **Content Posting API → Upload to Inbox** product.
3. Request approved scopes **\`user.info.basic\`** and **\`video.upload\`**. An app that has not received these permissions cannot upload videos.
4. Register your own exact **HTTPS Redirect URI**: \`https://YOUR-BRIDGE-DOMAIN/oauth/tiktok/callback\`. Web Login Kit does not accept an HTTP localhost URL.
5. Keep the TikTok **Client Secret only on your server**, never in source code, the Windows EXE, GitHub Actions Variables, or this chat.

Official docs: [TikTok Login Kit](https://developers.tiktok.com/docs/en/login-kit-web) and [Upload to Inbox](https://developers.tiktok.com/docs/en/content-posting-api-reference-upload-video).

## Deploy on your own HTTPS server

Use Node.js 22+ on a server you control. This process binds to \`127.0.0.1:8787\` and **must** be proxied behind HTTPS (for example, with nginx or Cloudflare Tunnel). Set these environment variables on the **server**, via its secret manager or protected process configuration:

\`\`\`bash
PUBLIC_ORIGIN=https://YOUR-BRIDGE-DOMAIN
CLIENT_KEY=your_public_tiktok_client_key
CLIENT_SECRET=your_private_tiktok_client_secret
TOKEN_ENCRYPTION_KEY=64_random_hex_characters
STORE_PATH=/var/lib/clipforge/tiktok-accounts.json
PORT=8787
\`\`\`

Generate a strong encryption key locally on the server using a trusted secrets manager or secure cryptographic random generator; keep it private. Do not share it. The service encrypts OAuth tokens at rest with AES-256-GCM. **Keep backups of the encryption key**; changing it makes stored sessions unreadable. Restrict the file and service account permissions.

Run from the repository:

\`\`\`bash
node tiktok-bridge/server.mjs
\`\`\`

Configure the reverse proxy to forward \`https://YOUR-BRIDGE-DOMAIN\` to \`http://127.0.0.1:8787\`; require valid TLS and protect the origin from direct Internet access.

You can verify that the server responds at \`https://YOUR-BRIDGE-DOMAIN/health\` (the response does not expose tokens).

## Enable the Windows app

GitHub → repository **Settings → Secrets and variables → Actions → Variables** → add:

\`\`\`
CLIPFORGE_TIKTOK_BRIDGE_URL=https://YOUR-BRIDGE-DOMAIN
\`\`\`

This is the bridge's **public HTTPS address**, not the private Client Secret. Rebuild ClipForge using the Windows EXE workflow. When users click **«Добавить TikTok-аккаунт»**, ClipForge opens TikTok Login Kit in the browser, and the bridge retains each user's tokens and refreshes them server-side. The app's random pairing session controls access to its TikTok profiles. Multiple TikTok profiles can be added by repeating login, then selected in a batch with multiple YouTube channels.

**Actual publishing behavior:** Video Upload places a copy into *each selected TikTok account's inbox*. The account owner must finish the TikTok post. Once approved for \`video.publish\`, Direct Post can be added with platform-required creator-info queries, privacy selection, and explicit consent. The current code deliberately does not pretend that Upload to Inbox equals an already-published TikTok post.

## Security and operational notes

- Never put the TikTok Client Secret or access/refresh tokens into the desktop build or browser frontend.
- Deploy a single bridge instance per encrypted store; implement centralized persistence and locks before running several replicas.
- The bridge issues random pairing tokens that are sent only over HTTPS. Do not publish or log them.
- Tokens and sessions expire; they can be revoked from TikTok account settings. The app currently requires re-pairing after a full desktop restart; tokens may remain server-side until session expiry.
- Restrict connection attempts and monitor failed OAuth requests. For a public service, add strong operational rate limits, abuse monitoring, backups, alerting, and a privacy policy.
- Do not bypass platform age requirements, approval restrictions, or content licensing rules.
