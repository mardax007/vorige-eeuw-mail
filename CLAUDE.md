# Vorige Eeuw Mail

A free, simple mail app for Martijn's family. Windows desktop first, Android later (shared as an APK).
It is a mail *client* for mailboxes people already have (IMAP/SMTP), not a mail host.
Everything that touches mail runs on the user's own device. No mail passes through a server of ours.

## Read first, every session

1. `docs/PROGRESS.md` — where the build stands and what is next.
2. The current phase file in `docs/phases/` — tasks and exit criteria.
3. `docs/DECISIONS.md` — what is decided and what is still open.

Read when needed: `docs/PLAN.md` (the design and the facts behind it), `docs/features.md` (what is
in, what is a candidate, what is out), `docs/screens.md` (screen rules for older users), `docs/sync.md`
(cache rules), `docs/errors.md` (error rule and codes), `docs/family-inventory.md` (what the family
really uses), `docs/research.md` (the research pass, with sources), `docs/sources.md` (older links).

## How to work here

- One phase at a time, in the order of `docs/phases/`. Do not start a phase until the previous phase's
  exit criteria are met and Martijn has said to go on.
- Open decisions in `docs/DECISIONS.md` belong to Martijn. Ask; never pick silently. If you need one
  answered to continue, stop and ask.
- Steps marked **(Martijn)** in a phase file need a person: an account, a key, a real PC or phone.
  Prepare everything around them, then hand over with exact instructions.
- At the end of every session update `docs/PROGRESS.md`: what was done, what was learned, what is next.
  Record new decisions in `docs/DECISIONS.md` with the date.
- Keep it simple. The users are a handful of family members. No accounts on our side, no telemetry
  service, no server of ours, no plugin system, no settings nobody asked for. Error reports, logs and
  wishes go to Martijn by email only, as `docs/PLAN.md` section 5 describes. If it is not in the current phase file,
  do not build it; note the idea in `docs/PROGRESS.md` instead.
- Check the current documentation before using Tauri, its plugins or the mail crates. Versions and
  APIs move; do not pin versions or copy commands from memory.
- Small commits, one topic each.

## Hard rules

**Mail safety**
- Tests, spikes and scripts never delete, move, flag or otherwise change mail in a real mailbox.
  Anything that writes goes to the local test server or a throwaway test account.
- Import opens its source files (PST, EML folders) read-only and never changes or removes them.

**Secrets**
- Passwords and sign-in tokens never go into the SQLite index, logs, error text, error reports,
  crash output or git.
- On Windows encrypt passwords and tokens with DPAPI (`CryptProtectData`, current user) and keep the
  result in its own file in the app's data folder (decided 16 November 2026; replaces the earlier
  Credential Manager key, whose entries hold at most 2,560 bytes).
- Never commit the updater's private key, the Android keystore, OAuth client files, test account
  passwords, Martijn's report address or real mail. Build-time secrets live in GitHub Actions
  secrets. `.gitignore` covers the usual places; look at `git status` before a commit.

**Network**
- Fetch server settings over HTTPS only. Never log in without TLS; a required STARTTLS that is not
  offered stops the login.
- Hosts from our own provider table, the domain's autoconfig file or the ISPDB entry for the domain
  count as looked up. Any other host (from MX, SRV or a guess) that is not under the user's own
  domain is shown to the user for confirmation before a password goes to it.
- Credentials and mail go only to the user's own mail provider.
- Error reports, logs and wishes are sent as email through the user's own provider to Martijn's
  address only. They never contain mail content, passwords or tokens; the only addresses in them are
  the sender's and Martijn's. The user is told once that this happens.
- Besides mail servers, the app contacts only GitHub (updates and Martijn's log requests), the
  settings lookup (the domain's autoconfig host, Thunderbird's ISPDB, DNS), Google's and Microsoft's
  sign-in pages when the user signs in that way, Microsoft's WebView2 installer, and, when the user
  asks, a remote image's server. Anything else is added here first.

**Showing mail**
- HTML mail renders in a sandboxed frame: scripts off, no access to the Rust core (no Tauri IPC),
  remote images blocked until the user allows them. Links open in the system browser, caught by the
  shell's navigation handler; `mailto:` opens our own Write screen.

**Updates**
- Updates install by themselves, with no question to the user, only when the app is idle (no message
  being written, no import or send running), and the app reopens where it was. Every update is signed with
  the updater key; never publish a release that was not first installed on Martijn's own PC.

**Errors**
- Every error a user sees follows `docs/errors.md`: a plain sentence, the next step, the server's
  exact reply (copyable, character for character) and a fixed `VEM-…` code.
- Never rewrite, shorten or translate a server reply. On screen and in "Kopiëren voor Martijn" the
  reply is exact. In the emailed report and in logs every address other than the sender's and
  Martijn's is replaced by `<adres>`; nothing else in the reply changes.

**Screen text**
- No hard-coded screen text. Every user-facing string goes through one message catalogue. The app is
  in Dutch (decided 16 November 2026); the catalogue keeps English possible later.
- Where Dutch Outlook has a word the family knows ("Postvak IN", "Verwijderde items"), use that word.
  See `docs/screens.md`.

## Stack (confirmed by the build, 16 November 2026)

- Tauri 2, at least 2.11.1 (security fix): screens in React + TypeScript + Vite (decision 2), a Rust core for IMAP, SMTP,
  sync, import and sign-in.
- Rust crates: `async-imap` (tokio runtime), an own small SMTP client in `vem-core/src/smtp.rs`
  (instead of `lettre`, see `docs/DECISIONS.md`), `mail-parser`, `mail-builder`, `rusqlite` with FTS5,
  `ammonia`, `outlook-pst`. SQLite index, messages as `.eml` files, secrets through DPAPI (Android: the keystore).
  The full list with reasons is in `docs/PLAN.md` section 2.
- TLS through rustls with `rustls-platform-verifier` (the operating system's certificate store),
  never a built-in root list: antivirus mail scanning on family PCs needs the system store.
- Fallback if the spike fails: Electron + ImapFlow + Nodemailer. Switching is Martijn's call, made on
  the spike report.
- Targets: Windows 10/11 and Android. No macOS, Linux or iPhone.
- App identifier `tech.mardax.vem` (Windows and Android; permanent). One public GitHub repository,
  licence GPL-3.0. Installed per user, no admin rights, so updates stay silent.

Layout:

    crates/vem-core/   mail logic without any Tauri dependency: testable headless, reused on Android
    src-tauri/         thin Tauri shell: commands that call vem-core
    src/               screens
    docs/              plan, phases, decisions, progress

## Environment

- Development is on Windows, in `F:\Projects\Mail`. Use commands that work in PowerShell.
- Installed: Node 24, git, Docker with WSL 2, Java, GitHub CLI, Rust (stable, MSVC), Build Tools 2022,
  MSYS2 in `C:\msys64` with `mingw-w64-ucrt-x86_64-libpst` (the rescue tool; `scripts/rescue-tool.mjs`
  copies it into `src-tauri/rescue/` before every `tauri dev` and `tauri build`).
- Visual Studio 18 on this PC lacks the x64 libraries, so every cargo command runs through
  `scripts\vcenv.cmd`, which loads the Build Tools 2022 x64 environment first. From Git Bash:
  `cmd //c "scripts\vcenv.cmd cargo …"`.

## Commands

| What | Command |
| --- | --- |
| Screens in a browser, with the mock backend | `npm run dev` (http://localhost:1420) |
| Screens: types, tests, build | `npm run typecheck`, `npm test`, `npm run build` |
| Rust core unit tests | `scripts\vcenv.cmd cargo test -p vem-core --lib` |
| Rust core with the local test mail server | start GreenMail (below), then `scripts\vcenv.cmd cargo test -p vem-core --features test-support` |
| Lint | `scripts\vcenv.cmd cargo clippy -p vem-core -p vem --all-targets --features vem-core/test-support` |
| Search speed on 100,000 messages | `scripts\vcenv.cmd cargo test -p vem-core --release --test search_perf -- --ignored --nocapture` |
| The app with live reload | `scripts\vcenv.cmd npm run tauri dev` |
| Installer (NSIS, per user) | `scripts\vcenv.cmd npm run tauri build` (output in `target\release\bundle\nsis`) |

GreenMail is the local test mail server (IMAPS 3993, SMTPS 3465, users `jan@example.test` and
`piet@example.test`, password `geheim`; made-up test accounts, not secrets). The jar lives outside the
repository, in `F:\Projects\Mail-scratch\greenmail`:

    java -Dgreenmail.setup.test.all -Dgreenmail.hostname=127.0.0.1 -Dgreenmail.users=jan:geheim@example.test,piet:geheim@example.test -Dgreenmail.users.login=email -jar greenmail-standalone-2.1.3.jar

Without it the GreenMail tests print a note and pass. CI (`.github/workflows/ci.yml`) starts it itself.

Build-time settings, read by `option_env!` and set only in the release workflow from GitHub Actions
secrets: `VEM_SUPPORT_ADDRESS`, `VEM_UPDATER_PUBKEY`, `VEM_GOOGLE_CLIENT_ID`,
`VEM_GOOGLE_CLIENT_SECRET`, `VEM_MICROSOFT_CLIENT_ID`, plus `VEM_UPDATE_URL` and
`VEM_LOG_REQUESTS_URL`, which the workflow derives from the repository name. A local build without
them has no reports, no updates and no Google or Microsoft sign-in.
