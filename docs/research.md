# Research pass, 16 November 2026

Five parallel research runs checked the plan against current sources: the Tauri platform, the Rust
crates, the import formats, the providers and distribution, and sync and screen design. This file
keeps what they found and where. Lines marked **(research)** in `PLAN.md`, `sync.md` and `screens.md`
rest on it.

"Opened" sources were fetched and read. "Excerpt" means only a search-result excerpt was seen;
re-check those before a decision rests on them. Crate versions are as of this date; check again
before use.

## What changed in the plan

| Was | Now | Why |
| --- | --- | --- |
| Settings lookup starts at the domain's autoconfig | Starts at our own provider table, Solcon first | Solcon has no autoconfig, ISPDB entry or SRV records; its SMTP host differs from IMAP; the username can differ from the address |
| Nothing said about certificate checking | TLS through the Windows and Android trust stores (`rustls-platform-verifier`) | Antivirus mail scanning (Kaspersky, Avast, ESET) breaks clients with their own root list |
| "HTML mail in a sandboxed frame" | Four layers: Rust sanitiser, sandboxed `srcdoc` frame without scripts or same-origin, CSP, opener for links | WebView2 injects Tauri's scripts into every frame; Tauri allows same-origin frames on Windows to call commands |
| Any Tauri 2 version | At least 2.11.1 | CVE-2026-42184 |
| "Use `outlook-pst`" | `outlook-pst` plus our own MAPI-to-MIME module, per-message isolation and count checks | The crate gives raw properties only, and has open data-loss and panic bugs |
| "New Outlook has no importable files" | New Outlook can export mail to PST | Microsoft added PST export |
| Windows Live Mail read directly from `.eml` | `.eml` plus the ESE database for read state and folder names | Read flags are not in the `.eml` files |
| "Put in my mailbox" as the natural choice | Check quota first; "keep on this computer" often the only fit; export in version 1 | Solcon mailboxes are 2 or 5 GB |
| Smart App Control must stay off for good | Can be switched back on since April 2026; off only while the unsigned app is in use | KB5083769 |
| No signing at all | Open question: free SignPath signing for open source, or a cheap Certum certificate | Would let Smart App Control stay on |
| Android limited distribution: no ID, 20 devices | No ID, but two-step verification and a payments profile; guide says 50 devices | Google's limited-distribution guide |
| Installer 3–10 MB against 80–150 MB | About 5–20 MB against 100–200 MB | Measured comparisons |
| Gmail limits 2,500 / 500 MB a day | Documented for Workspace only; keep the same throttle for consumer Gmail | Google's bandwidth page |
| Google personal use: at most 100 users | At most 100 *new* users over the project's life, cannot be reset | Google Cloud help |
| Spike does not touch import or HTML | Spike reads a real PST and renders a hostile HTML mail | These are the two riskiest parts |
| No rules for older users | `screens.md` | NN/g and W3C research |
| No sync rules | `sync.md` | RFCs and client authors' experience |

## Tauri

- Current: Tauri 2.12.6, CLI 2.12.5. Opened: https://github.com/tauri-apps/tauri/releases
- Mobile is not at parity with desktop; the team said it overpromised. Android minimum is 7.0
  (SDK 24); `--apk --split-per-abi` for sideloading. Opened: https://v2.tauri.app/blog/tauri-2-0-0-release-candidate/,
  https://v2.tauri.app/distribute/google-play/
- On Android and Linux Tauri cannot tell an iframe's requests from the window's. Opened:
  https://v2.tauri.app/security/capabilities/
- Capabilities are scoped by window or webview label. The page says all app commands are open to
  every window unless limited; check in the spike. Same page.
- GHSA-57fm-592m-34r7 (CVE-2024-35222): iframes could call IPC. Fixed in 2.0.0-beta.20, except on
  Windows for a frame with the window's own origin. Opened: https://github.com/advisories/GHSA-57fm-592m-34r7
- GHSA-7gmj-67g7-phm9 (CVE-2026-42184): remote hosts treated as local on Windows and Android, fixed in
  2.11.1. Opened: https://github.com/tauri-apps/tauri/security/advisories/GHSA-7gmj-67g7-phm9
- WebView2 adds init scripts to every subframe. Opened:
  https://docs.rs/tauri-runtime/latest/tauri_runtime/webview/struct.WebviewAttributes.html
- The isolation pattern guards against risky frontend code, not mail content. Opened:
  https://v2.tauri.app/concept/inter-process-communication/isolation/
- An empty `sandbox` attribute means no scripts and an opaque origin. Opened:
  https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/iframe
- `srcdoc` frames inherit the parent's CSP in Chromium. Excerpt (web-platform tests).
- Updater: keys from `tauri signer generate`; `TAURI_SIGNING_PRIVATE_KEY` at build time; static JSON
  with `version` and per-platform `url` and `signature`; HTTPS enforced; a lost key means no more
  updates; `passive` install mode by default; the app exits during install; Android not covered.
  Opened: https://v2.tauri.app/plugin/updater/, https://docs.rs/crate/tauri-plugin-updater/latest
- NSIS installs per user by default, no admin. WebView2 bootstrapper by default (needs internet);
  embedded bootstrapper adds about 1.8 MB. Opened: https://v2.tauri.app/distribute/windows-installer/
- Rust needs Windows 10 or newer since 1.78. Opened: https://blog.rust-lang.org/2024/02/26/Windows-7/
- WebView2 updates on Windows 10 22H2 until at least October 2028, without ESU. Opened:
  https://learn.microsoft.com/en-us/deployedge/microsoft-edge-supported-operating-systems
- Stronghold "will be deprecated and removed in v3" (maintainer, 2023). Opened:
  https://github.com/orgs/tauri-apps/discussions/7846
- `keyring` 4.x is built on `keyring-core` with per-platform stores, including an Android store.
  Opened: https://docs.rs/keyring/latest/keyring/,
  https://docs.rs/android-native-keyring-store/latest/android_native_keyring_store/
- Credential Manager: 2,560 bytes per credential. Opened:
  https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentiala
- DPAPI: no stated size limit, bound to the user. Opened:
  https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- Background work on Android: only a community plugin (`tauri-plugin-background-service`, first
  released April 2026). Opened: https://docs.rs/crate/tauri-plugin-background-service/latest
- OAuth: `tauri-plugin-oauth` runs a temporary localhost server; check `state`. Deep links on Windows
  arrive as a new process's argument. Opener plugin limits URLs by scope. Opened:
  https://github.com/FabianLars/tauri-plugin-oauth, https://v2.tauri.app/plugin/deep-linking/,
  https://v2.tauri.app/plugin/opener/
- Other Tauri mail clients: RAVN (same crates, GPL-3, no releases, opened:
  https://github.com/ravnmail/ravn), Velo (sandboxed frames, per-sender image allowlist, opened:
  https://github.com/SaschaOnTour/velo).
- Size comparisons: https://v2.tauri.app/start/ (opened), https://dev.to/gethopp/tauri-vs-electron-performance-bundle-size-and-the-real-trade-offs-1el4 (opened).

## Rust crates

- `async-imap` 0.12.0 (30 September 2026), chatmail (Delta Chat). Default runtime is async-std;
  use `default-features = false, features = ["runtime-tokio"]`. Has IDLE (29-minute timeout), MOVE,
  UID EXPUNGE, CONDSTORE select, APPEND, quota, ID, Gmail labels, a pluggable authenticator. No
  QRESYNC, no TLS of its own. Opened: https://raw.githubusercontent.com/chatmail/async-imap/main/CHANGELOG.md,
  https://docs.rs/async-imap/latest/async_imap/struct.Session.html
- Alternatives: `imap-codec` 2.0 alpha; `imap-client` moved into Pimalaya's `io-imap`, which has
  QRESYNC but 17 releases in five months; `email-lib` 0.27; the old `imap` crate stalled. Opened:
  https://github.com/pimalaya/io-imap, https://crates.io/api/v1/crates/io-imap
- Pimalaya discovery (`io-pim-discovery`, `pimconf`): autoconfig, ISPDB, MX, SRV and OAuth metadata,
  MIT/Apache, young. Opened: https://github.com/pimalaya/pimconf. OAuth presets for Google and
  Microsoft in `ortie`: https://github.com/pimalaya/ortie
- `lettre` 0.11.23: PLAIN, LOGIN, XOAUTH2; 465 implicit TLS or 587 STARTTLS; rustls with the platform
  verifier. Its error text joins reply lines with spaces and strips repeated status codes, so it is
  not verbatim. Opened: https://raw.githubusercontent.com/lettre/lettre/master/src/transport/smtp/error.rs,
  https://raw.githubusercontent.com/lettre/lettre/master/src/transport/smtp/response.rs
- `mail-parser` 1.0.1 (2 November 2026), new API, recent Rust needed; Windows code pages built in,
  `full_encoding` for Asian charsets. `mail-builder` 2.0.1. Opened: https://github.com/stalwartlabs/mail-parser
- TNEF (`winmail.dat`): only `tnef` 0.1.1 from 2019. Search: https://docs.rs/tnef
- `compressed-rtf` 1.0.1 (Microsoft, in the outlook-pst-rs repository): decompression only.
  Opened: https://crates.io/api/v1/crates/compressed-rtf
- `rustls-platform-verifier` 0.7.1: Windows store with enterprise roots; Android TrustManager, which
  needs a Maven repository, a Kotlin component, a ProGuard rule and an init call with the JNI context.
  Opened: https://github.com/rustls/rustls-platform-verifier
- Antivirus TLS scanning: Kaspersky puts its root in the Windows store and names Thunderbird as
  breaking (opened: https://support.kaspersky.com/keswin/12.7/en-US/202714.htm); Avast Mail Shield
  with Postbox (opened: https://support.postbox-inc.com/hc/en-us/articles/204602300); ESET with
  Thunderbird (opened: https://forum.eset.com/topic/26517-problems-with-thunderbird-and-imaps/page/2/).
- `ammonia` 4.2.5: can rewrite or drop attributes and limit URL schemes; removes `<style>` blocks.
  Opened: https://docs.rs/ammonia/latest/ammonia/struct.Builder.html
- `rusqlite` 0.43.0: the bundled build always has FTS5. FTS5: `unicode61 remove_diacritics 2`;
  trigram tokenizer for substrings (3+ characters); on a 1.6 GiB mail test the full index was
  743 MiB. Opened: https://raw.githubusercontent.com/rusqlite/rusqlite/master/libsqlite3-sys/build.rs,
  https://www.sqlite.org/fts5.html
- Test servers: GreenMail 2.1.15 standalone jar (Java), Stalwart 1.1.1 native Windows binary
  (AGPL; fine as a dev tool), Dovecot in Docker. Opened: https://greenmail-mail-test.github.io/greenmail/,
  https://stalw.art/docs/install/platform/windows

## Sync

- Disconnected IMAP clients: RFC 4549, https://www.rfc-editor.org/rfc/rfc4549
- CONDSTORE and QRESYNC: RFC 7162, https://www.rfc-editor.org/rfc/rfc7162
- IDLE: RFC 2177, https://www.rfc-editor.org/rfc/rfc2177. NAT timeouts shorter than 29 minutes:
  https://forum.mikrotik.com/t/nat-timeout-imap-idle/168338 (opened)
- MOVE: RFC 6851. SPECIAL-USE: RFC 6154, https://www.rfc-editor.org/rfc/rfc6154. Thunderbird's name
  fallback: https://bugzilla.mozilla.org/show_bug.cgi?id=905869
- Gmail: 15 connections per account, https://support.google.com/mail/answer/97150; IMAP delete and
  expunge settings, https://developers.google.com/workspace/gmail/api/reference/rest/v1/ImapSettings;
  duplicate Sent copies, https://bugzilla.mozilla.org/show_bug.cgi?id=1427619
- Nylas on UIDVALIDITY and IMAP sync: https://www.nylas.com/blog/guide-to-imap-send-and-sync-mail/,
  https://developer.nylas.com/docs/support/troubleshooting/uidvalidity/
- Mailspring sync engine (stateless sync, two connections, Gmail via All Mail plus labels):
  https://github.com/Foundry376/Mailspring-Sync
- Mark Crispin's ten commandments for IMAP clients:
  https://git.iohub.dev/dany/imap/src/branch/master/docs/commndmt.txt

## Screens

- NN/g, usability for people aged 65 and over: https://www.nngroup.com/articles/usability-for-senior-citizens/
- W3C, older users: https://www.w3.org/WAI/older-users/ and https://www.w3.org/WAI/WAI-AGE/conclude.html
- WCAG 2.2: https://www.w3.org/TR/WCAG22/
- Co-designing email with older users: https://ojs.meccsa.org.uk/index.php/netknow/article/view/496
- eM Client simple mode: https://emclient.com/simple-email-app-for-seniors
- Thunderbird scam detection: https://support.mozilla.org/kb/thunderbirds-scam-detection
- Outlook first-contact tip: https://it.cornell.edu/news/new-phishing-protection-outlook-first-contact-safety-tips
- Authentication-Results: RFC 8601, https://www.rfc-editor.org/rfc/rfc8601
- Quick Assist: https://learn.microsoft.com/windows/client-management/client-tools/quick-assist;
  abuse by scammers: https://www.theregister.com/2024/05/16/microsoft_quick_assist_crime/

## Import

- `outlook-pst` 1.2.0 (17 March 2026), read-only, ANSI and Unicode, three encryption modes modelled
  (the "high" one unconfirmed in code), raw MAPI only. Opened: https://docs.rs/crate/outlook-pst/latest,
  https://github.com/microsoft/outlook-pst-rs
- Open issues that matter: #81 (reader stops silently), #82 (embedded messages), #60 (RTF panic),
  #83, #84, #93. Opened: https://github.com/microsoft/outlook-pst-rs/issues
- libpff / pffexport (LGPL-3, alpha, reads PST and OST, recovery mode): https://github.com/libyal/libpff.
  libpst `readpst` (GPL-3): https://www.five-ten-sg.com/libpst/rn01re01.html; Windows build via MSYS2
  (excerpt): https://packages.msys2.org/base/mingw-w64-libpst
- RTF-wrapped HTML: MS-OXRTFEX,
  https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxrtfex/411d0d58-49f7-496c-b8c3-5859b045f6cf
- Message flags: https://learn.microsoft.com/en-us/office/client-developer/outlook/mapi/pidtagmessageflags-canonical-property
- Outlook 2013+ keeps IMAP accounts in OST: https://learn.microsoft.com/en-us/answers/questions/1379920/how-to-force-outlook-2013-to-create-pst-not-ost-wi
- PST password is UI-level only:
  https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-pst/48468b1e-cc81-4e2b-82a7-9bf61adc948e
- PST size limits (ANSI 2 GB, Unicode 50 GB since 2010):
  https://learn.microsoft.com/en-US/outlook/troubleshoot/data-files/file-size-limits-of-pst-and-ost-files-are-larger
- New Outlook PST export:
  https://support.microsoft.com/en-us/outlook/export-emails-contacts-and-calendar-items-to-outlook-using-a-pst-file,
  https://techcommunity.microsoft.com/discussions/outlookgeneral/pst-export-new-functionality-in-new-outlook-for-windows/4555903
- Windows Live Mail: read flags not in the `.eml` files, Mozilla importer closed:
  https://bugzilla.mozilla.org/show_bug.cgi?id=561562. ESE readers: `libesedb` bindings
  (https://docs.rs/crate/libesedb/latest), or Windows' `esent.dll` on a copy.
- UnDBX for Outlook Express (excerpt): https://github.com/ZungBang/undbx
- Gmail bandwidth (Workspace): https://knowledge.workspace.google.com/admin/gmail/gmail-bandwidth-limits.
  APPENDLIMIT: RFC 7889.

## Providers

- Solcon: IMAP `imap.solcon.nl` 993 SSL/TLS, SMTP `mail.solcon.nl` 587 STARTTLS, username is the
  mailbox name (see Mijn Solcon), from Solcon's own Outlook guide of January 2026:
  https://www.solcon.nl/content/uploads/Handleiding_instellen_outlook.pdf (fetched by the research
  run; the PDF could not be re-read as text in this session, so confirm the values in spike task 6).
  ISPDB returns 404 for solcon.nl (checked again in this session). No autoconfig host, no SRV
  records. Packages 2 GB (GO) and 5 GB (Lite): https://www.solcon.nl/solcon-mail/. Webmail:
  https://webmail.solconmail.nl/appsuite/
- KPN Mail en Cloud move, separate mail-program password, no POP:
  https://community.kpn.com/kennisbank-mail-en-cloud-135/kpn-mail-en-cloud-gebruiken-met-een-mailprogramma-658469;
  https://geekie.nl/blog/kpn-mail-instellen-outlook
- Ziggo settings: https://www.ziggo.nl/klantenservice/e-mail/serverinstellingen. ISPDB has ziggo.nl and
  kpnmail.nl (checked in this session).
- Gmail app passwords: https://support.google.com/accounts/answer/185833. Unverified apps and the user
  cap: https://support.google.com/cloud/answer/7454865, https://support.google.com/cloud/answer/15549945,
  https://support.google.com/cloud/answer/13464323. Token expiry rules:
  https://developers.google.com/identity/protocols/oauth2. Gmail ending POP fetching (excerpt):
  https://www.captaindns.com/en/blog/gmail-pop-checkmail-2026
- Outlook.com basic auth ended:
  https://support.microsoft.com/en-us/office/modern-authentication-methods-now-needed-to-continue-syncing-outlook-email-in-non-microsoft-email-apps-c5d65390-9676-4763-b41f-d7986499a90d.
  OAuth scopes: https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth.
  Apps must live in a directory: https://learn.microsoft.com/en-us/answers/questions/1663127/.
  Personal account unable to create a directory: https://learn.microsoft.com/en-us/answers/questions/5964218/
- Yahoo app passwords: https://help.yahoo.com/kb/SLN15241.html. iCloud: https://support.apple.com/en-us/102525

## Distribution

- Smart App Control can be re-enabled without reinstalling since KB5083769 (April 2026):
  https://blog.topedia.com/posts/smart-app-control-in-windows-11-can-now-be-re-enabled-without-reinstalling.
  FAQ: https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions
- Signing for Smart App Control:
  https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control.
  SmartScreen reputation: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- Artifact Signing (individuals US and Canada only): https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart.
  SignPath Foundation terms: https://signpath.org/terms. Certum open-source prices: excerpt only.
- Android developer verification: https://developer.android.com/developer-verification,
  https://developer.android.com/developer-verification/guides/faq,
  https://developer.android.com/developer-verification/guides/limited-distribution
- Windows 10 consumer ESU: https://www.microsoft.com/en-us/windows/extended-security-updates (the
  research run read an end date of 12 October 2027; check). Netherlands share September 2026: about
  21% Windows 10 (Statcounter, excerpt).

## Not checked yet

- Solcon's IMAP capabilities (CONDSTORE, MOVE, SPECIAL-USE, QUOTA, IDLE) and maximum message size
  (spike task 6), and whether it saves sent mail (spike task 16, spare mailbox).
- Whether Gmail honours APPEND's date and read flag. Phase 3.
- Whether a sandboxed `srcdoc` frame in WebView2 really cannot call Tauri commands. Spike task 13.
- Whether the "high" PST encryption is decoded by `outlook-pst`. Spike task 14.
