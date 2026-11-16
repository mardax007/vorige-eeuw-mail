# Sources

The pages the original plan and its reviews relied on. "Full" means the page was opened and read in
full during planning or review; the rest were read as search-result excerpts. Re-check a page before
a decision rests on it.

The research pass of 16 November 2026 added many more, grouped by topic in `research.md`. Where the
two disagree, `research.md` is newer.

## Tauri and the Rust stack

- Updater plugin (full): https://v2.tauri.app/plugin/updater/
- Webview versions: https://v2.tauri.app/es/reference/webview-versions/
- chatmail core, Delta Chat's Rust core on async-imap: https://github.com/chatmail/core
- RAVN, a Tauri mail client (full; no releases): https://github.com/ravnmail/ravn
- outlook-pst crate (full): https://docs.rs/outlook-pst
- Electron vs Tauri, Capawesome: https://capawesome.io/docs/blog/electron-vs-tauri-for-capacitor-apps/

## Google

- OAuth 2.0 for desktop apps (full): https://developers.google.com/accounts/docs/OAuth2InstalledApp
- OAuth production readiness (full): https://developers.google.com/identity/protocols/oauth2/production-readiness/overview
- Gmail API user data and developer policy (full): https://developers.google.com/gmail/api/policy
- Gmail API scopes: https://developers.google.com/workspace/gmail/api/auth/scopes
- Gmail bandwidth limits (full): https://knowledge.workspace.google.com/admin/gmail/gmail-bandwidth-limits
- OAuth consent screen, testing status: https://support.google.com/cloud/answer/10311615
- Custom URI scheme restrictions on Android: https://developers.googleblog.com/en/improving-user-safety-in-oauth-flows-through-new-oauth-custom-uri-scheme-restrictions/
- Android developer verification (full): https://android-developers.googleblog.com/2026/03/android-developer-verification-rolling-out-to-all-developers.html

## Microsoft

- OAuth for IMAP, POP and SMTP (full): https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth
- App registration breaking changes (full): https://learn.microsoft.com/en-us/entra/identity-platform/reference-breaking-changes
- Smart App Control (full): https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003
- Code signing options (full): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- Basic Auth ended for personal Outlook accounts, heise: https://heise.de/-9767989

## Finding server settings

- Thunderbird ISPDB: https://autoconfig.thunderbird.net/v1.1/
- Autoconfiguration lookup order: https://hlfh.github.io/autoconfiguration

## Import

- pst-extractor (Node only; not usable inside Tauri): https://npmjs.com/pst-extractor
- DbxConv, DBX to mbox or EML: https://pendriveapps.com/dbxconv-dbx-to-mbox-or-eml-mail-converter/

## Optional relay (phase 6)

- Cloudflare minimum TLS version (full): https://developers.cloudflare.com/ssl/edge-certificates/additional-options/minimum-tls/
- Cloudflare Workers limits: https://developers.cloudflare.com/workers/platform/limits
- Cloudflare R2 pricing: https://developers.cloudflare.com/r2/pricing/index.md
- Go minimum requirements per Windows version: https://go.googlesource.com/wiki/+/e391dcb65f9a8ecc0967515739e4125ae67133ff/MinimumRequirements.md

## Other providers

- Yahoo app passwords: https://help.yahoo.com/kb/sln3792.html
