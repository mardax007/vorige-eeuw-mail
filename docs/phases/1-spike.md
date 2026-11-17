# Phase 1 — Spike: prove the core

**Proves:** the Rust mail core works on Windows and on Android, HTML mail can be shown without
reaching the core, a real PST can be read, and an installer and an update can reach a family PC. If
the core does not hold, switch to Electron before anything else is built.

**Before starting:** every question in `../DECISIONS.md` is answered (16 November 2026). Decision 6
is made on this phase's report.

Spike code, probe output and copies of real mail live in a scratch folder outside the repository
(for example `F:\Projects\Mail-scratch`), never under `F:\Projects\Mail`.

Spike code may be rough, but it lives in the intended layout so that what works can stay. Read
`../PLAN.md` section 2 (crates) and `../research.md` first.

## Tasks

1. **Toolchain.** Check what is installed against Tauri's current prerequisites page. On
   16 November 2026 this PC had Node 24, git, Docker with WSL 2, Java and the GitHub CLI; Rust was
   missing. Rust needs the MSVC toolchain and the Microsoft C++ build tools. The updater needs a
   recent Rust (1.90 or newer at the time of writing; a minimum, re-check it). For task 12 also the
   Android SDK, NDK and a JDK as Tauri's Android prerequisites page lists them. List what is missing;
   let Martijn install it.
2. **Repository.** `git init`, first commit of the docs. Before that commit, make sure
   `family-inventory.md` uses initials only and holds no addresses: the repository is public
   (decision 4) and git keeps history. Add the GPL-3.0 `LICENSE` (decision 14). Scaffold a bare Tauri 2 app (at least
   2.11.1) with React + TypeScript + Vite and the identifier `tech.mardax.vem`. Add `crates/vem-core`. Record the
   dev, build and test commands in `CLAUDE.md`.
3. **Local test server.** A throwaway IMAP and SMTP server for development. All write tests go here.
   GreenMail's standalone jar runs on the installed Java and suits fast tests. For realistic IDLE,
   MOVE and CONDSTORE use Dovecot in Docker or Stalwart's Windows binary. Note which extensions each
   one offers.
4. **Connect and read.** In `vem-core`: connect to IMAP over TLS with `async-imap` on tokio (default
   features off) and `tokio-rustls` with `rustls-platform-verifier`. Sign in, list folders with their
   special-use flags, fetch the headers of the newest 20 messages, fetch and parse one full message
   with `mail-parser`.
5. **Send.** Send one message over SMTP with `lettre` to the test server. Make the server refuse one
   (for example a recipient it rejects) and check that we can show its reply character for
   character. `lettre`'s own error text is not verbatim; find the smallest way to keep the raw reply.
6. **Real account, read only. (Martijn)** Run step 4 against the accounts the family really uses,
   starting with the Solcon account, with `EXAMINE` instead of `SELECT` and `BODY.PEEK[]` instead of
   `BODY[]`, so nothing is marked read. Note the unread counts before and after; they must match.
   Martijn types the password himself; it is not stored in a file.
   Record per account: the server's CAPABILITY list (CONDSTORE, QRESYNC, MOVE, UIDPLUS, SPECIAL-USE,
   QUOTA, IDLE, APPENDLIMIT), whether `PERMANENTFLAGS` includes `\*` (tags as keywords), the quota
   and how full it is, the folder names, whether the username is the address, whether the provider
   offers CardDAV for contacts, and for Solcon confirm `imap.solcon.nl` 993 and `mail.solcon.nl` 587
   STARTTLS.
7. **Settings lookup.** For a typed address run the lookup order from `../PLAN.md` section 3,
   starting with our own provider table, and print what was found and where. HTTPS only. Try it with
   the family's real domains.
8. **Antivirus check.** On a PC with a mail-scanning antivirus (or with a test root certificate added
   to the Windows store and a local TLS proxy), confirm that the connection works through the
   platform verifier and fails with `webpki-roots`. Write down what the user would see.
9. **Store a secret.** Encrypt a long token (several KB, like a Microsoft refresh token) with DPAPI
   (`CryptProtectData`, current user, decision 17), write it to its own file, read it back after a
   restart. Check that another Windows user on the same PC cannot decrypt it.
10. **Google sign-in. (Martijn for the Google Cloud project)** Decision 8: yes. Desktop
    flow: system browser, local redirect, PKCE, `state` checked; then sign in to IMAP with the token.
    The project must be *published*, not in testing. Use only family accounts: the 100-user cap
    counts every new account forever. Note the date; check again after eight days that it still
    works.
11. **Installer, silent update, notification. (Martijn for the key, the GitHub repository and the
    family PC)** Build an unsigned per-user installer. Make the updater key pair and back up the
    private key and its password outside the repository. Install on one family PC. Publish one
    update as a GitHub release in a public repository (a throwaway one is fine if decision 4 is not
    settled) and see it install in `quiet` mode with nobody touching the PC, and the app start again.
    Write down every warning Windows shows, and whether Smart App Control is on; if it can be
    switched on for a test, see whether it blocks the unsigned update. On the installed build, show
    one Windows notification and check that clicking it opens the app, also with the window closed
    to the icon next to the clock.
12. **Android. (Martijn for the phone)** Build the same core once for Android. Check that it
    connects (with `rustls-platform-verifier`'s Android setup), signs in and stores a secret. No
    screens needed beyond a status line.
13. **Hostile HTML.** Show one deliberately hostile HTML mail the way `../PLAN.md` section 2
    describes: sanitised, in a sandboxed `srcdoc` frame, with the CSP. The sample tries scripts,
    event handlers, a remote image, a CSS `url()`, `@import`, `<meta http-equiv=refresh>`, a form,
    a link without a target, and a direct call to Tauri's `invoke`. Nothing may load or run. A link
    click is caught by the shell's navigation handler (not by a call from the frame) and reaches the
    system browser; a `mailto:` link opens our own Write screen. Also check what Tauri's capability
    defaults allow for a window or webview with no capability of its own. Do it on Windows; on
    Android if task 12 got that far.
14. **PST probe. (Martijn for the file)** With `outlook-pst`, open a copy of one real family PST
    read-only: walk all folders, count messages per folder, compare with the counts Outlook shows,
    and turn three messages (one received, one sent, one with an attachment) into `.eml` files that
    another mail program opens correctly. Note the PST's format (ANSI or Unicode) and encryption.
    The file stays outside the repository.
15. **Search probe.** Generate 100,000 test messages into the SQLite index with FTS5 and time a
    search over all mail, over one account and over one folder. Record the numbers; the 0.2 s target
    is checked in phase 2a on the slowest family PC.
16. **Throwaway accounts. (Martijn for the accounts)** Everything that writes to a provider runs on a
    spare mailbox there, never on a family mailbox: for Solcon a spare mailbox in the same package
    (Lite has five), for Gmail a spare account with an app password (never the real Google Cloud
    project, whose user cap counts every account forever). On each: send one message and see whether
    the server puts it in Sent itself; set and read back a keyword tag (and on Gmail a label); open
    connections until the server refuses and note the number. For Outlook.com too, if a spare account
    is at hand (decision 9). Also try the Microsoft app registration here, so 2a knows whether
    Outlook.com can be supported.
17. **Spike report.** Write `../spike-report.md`: each check below with yes or no and what was seen,
    how the Rust crates held up, an honest note on the pace of working in Rust, and a
    recommendation: stay with Tauri or switch to Electron.

## Exit criteria

- [ ] IMAP: connect, sign in, list, fetch and parse work against the test server and one real account
- [x] SMTP: a message sent and received on the test server; a refused send shows the reply verbatim
      (`engine_greenmail.rs`, `smtp.rs` fake-server tests; 17 November 2026)
- [ ] Settings lookup finds the right servers for the family's real domains, Solcon included
- [ ] Connections work behind antivirus mail scanning (or the test root), through the Windows store
- [ ] Unread counts on the real accounts are the same before and after the spike
- [x] A secret survives a restart and never appears in plain text in a file or a log
      (`after_a_restart_mail_reads_offline_and_no_file_holds_the_password`; 17 November 2026)
- [ ] Google sign-in works and still works after eight days (or: not needed, per decision 8)
- [ ] An unsigned installer runs on a family PC, and one update from a GitHub release installs
      there with nobody touching it
- [ ] A Windows notification shows on the installed build and opens the app when clicked
- [ ] The core connects, signs in and stores a secret on an Android phone
- [x] The hostile HTML sample loads nothing, runs nothing and cannot reach the core
      (`tests/hostile_html.rs`, by hand on Windows and the Android emulator; 17 November 2026)
- [ ] A real PST is read with the right counts, and three messages open as `.eml` elsewhere
- [x] Search times over 100,000 messages are recorded for all three levels (`search_perf`, PC and
      Android emulator, 17 November 2026; docs/PROGRESS.md)
- [ ] Capabilities, quota and folder names of each real account are written down
- [ ] Sent-copy behaviour, keyword support and connection limit are known per provider, from
      throwaway accounts only
- [x] `spike-report.md` written; Martijn has decided Tauri or Electron (decision 6: left to Claude
      Code, Tauri stays; `../spike-report.md`, 17 November 2026)
