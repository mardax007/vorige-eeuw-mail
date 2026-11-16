# Vorige Eeuw Mail — build plan

Source: the plan doc "Vorige Eeuw Mail — Research & Plan" of 16 November 2026
(https://claude.ai/artifact/VqLNfDZtG79Y72HohP1CZN, private to Martijn), its two reviews of the same
day, and a deeper research pass the same evening (`research.md`).

Two kinds of marked lines:

- **(review)** comes from the two reviews of the first plan doc.
- **(research)** comes from the research pass of 16 November 2026. Each one has a source in
  `research.md`.

Both were checked against sources. Martijn accepted all of them on 16 November 2026, together with
the proposals in `sync.md`, `screens.md` and `features.md`. Where a research line corrects an older
line, the older line is gone and `research.md` says what changed.

## 1. What it is

- A free mail app for the family. Simple to use and simple to set up.
- A Windows desktop app first. An Android app later, shared as an APK. No macOS, Linux or iPhone.
- A mail client for mailboxes people already have: an ISP, Gmail, their own domain. Not a mail host.
  "Works with anything" means anything that speaks IMAP and SMTP.
- No stores, no paid signing certificates, no reviews at Google or Microsoft. Unsigned installers
  are fine.
- The first real case: bring in someone's old mail and connect a new account at Solcon for new mail,
  so old and new mail sit together.
- The people using it are mostly older and not technical. The screens follow `screens.md`.

**What it does better than Thunderbird** (decision 13, accepted by Martijn on 16 November 2026, with
evidence) **(research)**:

- **Solcon works by typing the address.** Solcon publishes no autoconfig file, no ISPDB entry and no
  SRV records. Its SMTP host is not its IMAP host, and the username is not always the address.
  Thunderbird and the new Outlook guess wrong. We ship the right answer.
- **It connects behind antivirus.** Kaspersky, Avast and ESET scan mail by putting their own
  certificate in the Windows store. Thunderbird uses its own store and then fails to connect; we use
  the Windows store.
- **Windows Live Mail import exists.** Thunderbird closed its Windows Live Mail importer as "won't
  fix".
- **Errors can be searched**, and the screens are made for older eyes and hands.

## 2. Architecture

Everything that touches mail runs inside the app on the user's computer.

- **Rust core.** Holds the mail connections, sync, the import readers and the sign-in flows. The
  screens only ask it for data.
- **Local store.** A SQLite index with full-text search (FTS5) for fast lists and search, messages as
  `.eml` files, secrets encrypted by the operating system (DPAPI on
  Windows, the keystore on Android) in their own file. The app opens instantly and works
  offline. Imported mail kept "on this computer" lives in the same store as a local account, so it
  sits in the same list as the new mail.
- **Search.** (Martijn, 16 November 2026) Search must be fast at three levels: all mail (every
  account and the imported old mail), one mailbox, and one folder. It runs on the local index only,
  never waits for a server, and gives first results while typing. How:
  - One FTS5 table over subject, sender and recipients (names and addresses), body text,
    attachment names and tags, with `unicode61 remove_diacritics 2`, so "cafe" finds "café". Prefix matching
    so results come while typing.
  - Each indexed row carries its account and folder, and the message table has indexes on
    (account, folder, date). The three levels are one query with a different filter, newest first,
    in pages, so a scope costs no more than global search.
  - Body text is indexed when the body arrives. A search can only find words in bodies the app has
    downloaded, so the app downloads every body in the background (decision 7: everything). Until a
    folder is complete, the results say so ("still fetching older mail: 3,200 of 18,000").
  - Target: first results within 0.2 s on 100,000 messages on the slowest family PC, at all three
    levels. Phase 2a measures it on generated test mail.
- **Tags and contacts.** (Martijn, 16 November 2026) Messages can carry tags, and the app keeps a
  simple address book. Tags are stored on the server as IMAP keywords (Gmail: labels) where the
  server keeps them, otherwise only locally. Contacts live in the local store, filled from sent mail,
  by hand and by import. A paired phone gets the same contacts, encrypted, through a folder in the
  user's own mailbox (decision 19). Details in `features.md`.
- **Mailbox space.** (Martijn, 16 November 2026) Each account shows how full the mailbox is: IMAP
  `QUOTA` where the server offers it, otherwise the sum of the message sizes against the limit in
  our provider table when known. A calm warning at 90%.
- **Sync.** **(review)** First sync fetches headers, then bodies on demand and in the background.
  Skip Gmail's All Mail folder, which repeats every message. Whether version 1 keeps a full offline
  copy is an open decision. **(research)** The rules a correct cache needs (UIDVALIDITY, CONDSTORE,
  deleted messages, special folders, sent copies, connection limits) are in `sync.md`.
- **New mail.** **(research)** While the app runs it keeps one live (IDLE) connection to each
  mailbox's Inbox and checks the other folders every few minutes. No polling service.
- **Windows notifications and running in the background.** (Martijn, 16 November 2026) New mail
  shows a Windows notification (sender and subject) through Tauri's notification plugin; clicking
  it opens that message. So that notifications keep coming, closing the window does not stop the
  app: it stays as an icon next to the clock, says so once in plain words, and "Afsluiten" in that
  icon's menu really closes it. The app starts with Windows (per user, no admin rights). Windows
  only shows notifications properly for an installed app with its own identity, so test them on an
  installed build, not in development mode.
- **Secure connections.** **(research)** TLS through rustls with `rustls-platform-verifier`, so the
  operating system's certificate store decides. Never a built-in root list such as `webpki-roots`:
  antivirus mail scanning on family PCs would break every connection.
- **Secrets on Windows.** (decision 17, 16 November 2026) DPAPI (`CryptProtectData` for the current
  user) encrypts passwords and tokens; the result goes in its own file in the app's data folder, never
  in the SQLite index, and nothing secret is ever written in plain text. DPAPI has no size limit, so
  the 2,560-byte limit of a Credential Manager entry no longer matters. **(research)** Do not use
  Tauri's Stronghold plugin: its maintainer said it will be removed in Tauri 3.
- **Showing mail.** **(review)** HTML mail cannot reach the Rust core, scripts are off, remote images
  stay blocked until the user allows them. **(research)** How, in four layers, because each layer
  alone has known holes:
  1. Sanitise in Rust (`ammonia`): drop scripts, event handlers, `<meta>`, `<link>`, `<base>` and
     forms; rewrite remote image and CSS addresses to a blocked placeholder; give every link an
     explicit target so a click leaves the frame as a navigation.
  2. Render in an `<iframe srcdoc>` with an empty `sandbox` attribute: no `allow-scripts`, no
     `allow-same-origin`. On Windows, WebView2 injects Tauri's scripts into every frame, and Tauri
     still allows commands from a same-origin frame on Windows. An opaque origin with scripts off is
     what makes them unusable.
  3. A Content-Security-Policy in the frame: `default-src 'none'`, images only from `data:`, `cid:`
     and our own scheme, inline styles allowed.
  4. The frame never calls the core. The Tauri shell's navigation and new-window handlers catch a
     link click, cancel it, and hand `http:` and `https:` addresses to the system browser through
     the opener plugin. A `mailto:` link opens our own Write screen. Every other scheme is dropped.

  "Show images" does not loosen the CSP: the core fetches the remote images and serves them to the
  frame through our own scheme. The sender's server still sees the user's internet address, which is
  why images stay blocked by default.

  `ammonia` removes `<style>` blocks, which makes many newsletters look plain. Keeping them safely is
  a phase 2a task, not a reason to drop the sanitiser.
- **Tauri version.** **(research)** At least 2.11.1, a security minimum and not a pin: older
  versions on Windows and Android treated some remote sites as local (CVE-2026-42184). Use the
  current release and re-check its advisories.
- **Servers of ours.** None. Installers, the update file and `log-requests.json` sit in the public
  GitHub repository (decision 4). Import over the internet is dropped (decision 12), and with it the
  relay.

Why Tauri and not Electron: the same code can later become the Android app, and the installer is
small (about 5–20 MB against 100–200 MB **(research)**). The cost is a Rust core instead of
TypeScript. Tauri needs WebView2. Windows 11 has it, Windows 10 since April 2018 normally has it, and
Tauri's installer adds it when missing (that default needs an internet connection). **(research)**
Microsoft keeps updating WebView2 on Windows 10 22H2 until at least October 2028. Screens can render
slightly differently on Windows and Android, so test on both.

**Windows versions.** **(research)** Rust itself needs Windows 10 or newer since Rust 1.78, so the
app runs on Windows 10 and 11 only. A Windows 7, 8 or XP machine can only be a source of old mail,
brought over on a USB stick.

### Crates **(research)**

The set below is what the research found most solid in November 2026. Check each crate's current
docs before use; do not pin versions from this list.

| Job | Crate | Why, and what to watch |
| --- | --- | --- |
| IMAP | `async-imap`, feature `runtime-tokio` (default features off) | In production in Delta Chat on Android. Has IDLE, MOVE, UIDPLUS, CONDSTORE, APPEND, quota, Gmail labels. No QRESYNC, no built-in TLS |
| TLS | `rustls`, `tokio-rustls`, `rustls-platform-verifier` | Windows store and Android trust store. Android needs a Kotlin part and an init call |
| SMTP | `lettre` with rustls and the platform verifier | Its error text joins and trims the server's reply lines, so it is not verbatim. Capture the raw reply ourselves |
| Parse and build | `mail-parser` (with `full_encoding`), `mail-builder` | Version 1.0 came out in October 2026 and needs a recent Rust |
| Index and search | `rusqlite` (bundled), FTS5 with `unicode61 remove_diacritics 2` | One file, transactional, enough for 100,000 messages on a laptop and a phone |
| HTML | `ammonia` | Plus the frame and the CSP above |
| Sign-in | `oauth2` | PKCE and a loopback redirect. Our own few-line XOAUTH2 authenticator for `async-imap` |
| Settings lookup | our own module | Pimalaya's `pimconf` is a good reference, but its API changes weekly |
| PST | `outlook-pst`, `compressed-rtf` | Gives MAPI properties, not mail files. See section 4 |

Watched, not used yet: Pimalaya's `io-imap` has QRESYNC and OAUTHBEARER, but it had 17 releases
between June and November 2026.

RAVN (a Tauri mail client with almost this exact stack, GPL-3) and Velo (a Tauri mail client with
sandboxed frames) are worth reading for ideas. Do not copy GPL code into this project without
Martijn's decision on the licence (decision 14).

## 3. Accounts

| Provider | How sign-in works | What the app does | Build it when |
| --- | --- | --- | --- |
| Solcon | **(research)** Mailbox name and password. IMAP `imap.solcon.nl` 993 TLS, SMTP `mail.solcon.nl` 587 STARTTLS | Built-in entry, because nothing can be looked up. Username field, because the mailbox name is not always the address | Version 1 |
| Other ISP, web host, own domain | Address and password over IMAP and SMTP | Find the settings automatically, test the login | Version 1 |
| KPN (kpnmail, planet, hetnet, xs4all) | **(research)** Moving to "KPN Mail en Cloud" from 16 November 2026: a separate password for mail programs, no POP. New server names not published yet | Address and password; point to KPN's mail-program password when the login fails | If the inventory has KPN |
| Ziggo (also home.nl, casema, quicknet, chello, upcmail) | **(research)** Address and mailbox password; in ISPDB | Normal lookup | If the inventory has Ziggo |
| Gmail | Normal password refused. OAuth, or an app password (needs two-step verification) | "Sign in with Google" in personal-use mode; app password as fallback | The inventory lists a Gmail address |
| Outlook.com, Hotmail, Live | OAuth only since 16 September 2024. **(research)** App passwords no longer work for IMAP | One Microsoft Entra app registration with the IMAP, SMTP and offline_access scopes | Only if someone uses these; see the risk below |
| Yahoo, AOL | App password. **(research)** Yahoo only lets you make one after several days of normal browser sign-ins | Walk the user through making one | Only if someone uses these |
| iCloud | App-specific password. **(research)** IMAP username without the domain, SMTP with it | Walk the user through making one | Only if someone uses these |
| Microsoft 365 work or school | OAuth, admin approval in some tenants | Ask before building: this row predates the family-only decision | Probably never |

Services with no IMAP access are out of scope.

**(research)** Gmail is ending "check mail from other accounts" (POP fetching) and Gmailify. A family
member who reads an ISP mailbox inside Gmail loses that, and may want this app for it.

**Google sign-in.** A desktop app is a public client: sign-in opens the system browser, returns to a
local address and uses PKCE. No server of ours. Personal-use mode means published but not reviewed,
one warning screen to click through. **(research)** The cap is 100 *new* users over the project's
whole life and cannot be reset, so never test with throwaway Google accounts on the real project.
Never leave the Google project in *testing* status: sign-in then expires every seven days. A Gmail
sign-in also ends when the user changes their Google password, or after six months unused; the app
must then ask again, not fail silently. Mail stays on the device, so Google's paid security
assessment should not apply; in personal-use mode there is no review at all **(review)**.

**Google sign-in on Android.** **(review)** Needs its own Android-type client, tied to the package
name and the signing key, and a different flow: custom-scheme redirects are off by default for new
Android clients. An app password may be the simpler route on phones.

**Microsoft.** **(review)** The app registration has to live in a Microsoft Entra directory.
**(research)** For a personal account that means an Azure sign-up with a card check (no charge;
prepaid cards refused), and a public thread from August 2026 shows a personal account that could not
create a directory at all. If nobody in the inventory uses Outlook.com, skip it. If someone does, try
the registration first, before any code.

### Setup flow

1. The user types an email address.
2. Look up the settings, in this order:
   1. Our own small provider table (Solcon first) **(research)**.
   2. The domain's own autoconfig file (`autoconfig.<domain>` and `<domain>/.well-known/autoconfig/`).
   3. Thunderbird's ISPDB (`https://autoconfig.thunderbird.net/v1.1/<domain>`).
   4. The ISPDB entry for the MX host's domain.
   5. SRV records (RFC 6186).
   6. A guess (`imap.<domain>`, `mail.<domain>`, `smtp.<domain>`).
3. For a provider that only allows OAuth, show that provider's sign-in button, not a password field.
4. Test the IMAP and SMTP login and report exactly which step failed. **(research)** If the address
   is refused as a username, offer a username field.

**(review)** Fetch settings over HTTPS only, refuse a login without TLS, and show the server name
before a password goes to a guessed host. **(research)** SRV and MX answers come from unsigned DNS,
so a host from them that is not under the user's own domain counts as guessed too. Our own provider
table counts as looked up, like autoconfig and ISPDB. HTTPS applies to the web lookups; the DNS steps
(MX, SRV) cannot use it, which is why their answers need the user's confirmation. When STARTTLS is
used it is required: if the server does not offer it, stop; never continue in plain text. This is
the Network rule in `CLAUDE.md`.

## 4. Import

Three routes. A USB stick plus "import from a file or folder" is the fallback that always works.

1. **From this computer.** The app looks in the usual places for Outlook and Windows Live Mail,
   shows what it found with message counts, and the user ticks what to bring in. Nothing is uploaded.
2. **From a mailbox.** The old mail still sits on a server. The app signs in to both accounts and
   copies the folders across.
3. Over the internet from another computer: not built (decision 12). Old mail from another computer
   comes on a USB stick (exFAT or NTFS for files of 4 GB or more).

| Old client | Where the mail is | How to read it |
| --- | --- | --- |
| Outlook for Windows, POP account | One `.pst` file | **(review)** Microsoft's Rust crate `outlook-pst` in the core first. **(research)** It gives raw MAPI properties; turning them into mail files is our own module (below). Backup: a separate rescue tool (`readpst`, GPL-3, or `pffexport`, LGPL-3) |
| Outlook 2013 and later, IMAP or Exchange account | **(research)** An `.ost` file | **(review)** Do not read the file; copy from the server (route 2) |
| New Outlook app | **(research)** Can now export mail to a `.pst` file (Settings, Files, Export) | Export to PST, then read it as above. Or route 2 |
| Windows Mail and Calendar app | Retired 31 December 2024 | Route 2 only |
| Windows Live Mail 2011/2012, and Vista's Windows Mail | `.eml` files in folders, plus a database (`Mail.MSMessageStore`) | **(research)** The messages are the `.eml` files. Read state and the folder names shown on screen live in the database (ESE format). Read it from a copy with Windows' own `esent.dll`; if that fails, import with every message marked read and say so in the report |
| Outlook Express 5–6 | One `.dbx` file per folder | Unproven. UnDBX (GPL-3) and DbxConv convert DBX to EML. Test before promising. In version 1 (decision 11) |

Excluded as import sources (decided 16 November 2026): Thunderbird, Eudora, Apple Mail.

**From MAPI to mail files.** **(research)** This is the largest piece of import work, and no crate
does it:

- Body: HTML if present, else plain text, else the compressed RTF body. RTF often wraps the original
  HTML (`\fromhtml1`); unwrapping it is our own code.
- Headers: received POP mail usually keeps its original headers (`PR_TRANSPORT_MESSAGE_HEADERS`).
  Sent items and drafts do not; build From, To, Cc, Date, Subject and Message-ID from properties and
  the recipient table. Never reuse an old Content-Type or boundary.
- Dates: delivery time for received mail, submit time for sent mail, creation time as fallback.
- Read state from `PR_MESSAGE_FLAGS`; drafts from the unsent flag.
- Old (ANSI) PST files store text in a code page: use the message's code page, fall back to
  Windows-1252.
- Attachments: embedded messages become `message/rfc822` parts; inline images need their Content-ID.
- Only mail items come in as mail. Contact items go to the address book; Outlook categories become
  tags. Calendar items and tasks are skipped and counted in the report.

**Known risks with `outlook-pst`** **(research)**: open issues include a reader that silently stops
at the first record it cannot parse, embedded messages read from the wrong place, and a panic on some
RTF bodies. So each message is converted in isolation (a panic skips that message, it does not stop
the import), and every folder's count is checked against the source.

**Practical facts** **(research)**:

- Outlook locks its PST file while running. Detect that and ask the user to close Outlook.
- A PST password is only checked by the program, not used to encrypt; the format still says a reader
  must ask for it. So ask for it.
- A USB stick formatted FAT32 cannot hold a file of 4 GB or more. Large PST files need an exFAT or
  NTFS stick, or a copy over the network. The import guide says so.
- Windows Live Mail folders of an IMAP account can hold headers-only `.eml` files. Count and report
  them; they are not complete messages.

**Where imported mail goes.** Each import asks:

- "Put in my mailbox": add it over IMAP, so it shows up in every mail program and later on the phone.
  **(research)** Check the free space first (IMAP QUOTA). Solcon mailboxes hold 2 or 5 GB, KPN's basic
  Mail en Cloud package 5 GB (per a Dutch help site, not KPN itself); a PST of several GB will often
  not fit. Say so before starting, with the numbers.
  **(review)** Gmail limits IMAP uploads (Google documents 500 MB a day for Workspace; treat consumer
  accounts the same), so a large import into Gmail takes days and must resume.
- "Keep on this computer": local folders next to the account. Also works without IMAP.
  **(review)** That copy has no backup, so the app needs an export to mbox or EML files.

**(research)** For the first case (old mail plus a new Solcon mailbox) "keep on this computer" will
often be the only choice that fits. That makes the export part of version 1, not a later extra. How
that mail is kept is decision 15: it stays on the computer as a local account in our own store
(`.eml` files plus the index), converted on import. A copy of the original file is kept in the app's
data folder so nothing is lost if the conversion missed something, and a menu lists these copies with
their size and deletes them on request. That copy is on the same disk; the export is the backup.

**Report per item.** State how many messages came in and how many were skipped and why, as a list
the user can save.

## 5. Errors and support

See `errors.md`. In short: a plain sentence, the next step, the server's exact words, a fixed code.

Martijn supports everyone from a distance, so the app reports to him by itself (Martijn,
16 November 2026). There is still no server of ours: everything goes as an ordinary email, sent
through the user's own mail account to Martijn's address. That address is set at build time from a
GitHub Actions secret, never written in the repository. It is not secret once released: anyone can
read it out of the installer, so it should be an address used only for this (decision 21).

**Logs.** The app keeps a local log that rotates (a few days, a few MB). Logs are written so they
never contain secrets, mail content or addresses: the log functions take typed values (account
number, host, error code, server reply), not free text. Server replies can quote an address; those
are masked before writing.

**Errors go to Martijn at once.** When an error is shown (or a background task fails), the app
sends Martijn an email with the error code, the sentence, the server's exact reply, the app version,
the Windows version, an install number and the last 50 log lines. Rules:
- Same code on the same PC: one email, then a count in the next one, at most one per code per day
  and at most ten emails a day in total.
- If sending fails (the error may be about sending), the report waits and goes later.
- These emails get a fixed subject start, `[Vorige Eeuw Mail]`, so Martijn can filter them. Where
  the provider does not save sent mail itself, the app does not add them to Sent; where it does
  (Gmail), they show there with that clear subject.
- The first time, the app says in one sentence that Martijn gets a message when something goes
  wrong, and that no mail content goes with it.

**Logs on request.** Martijn can ask any PC for its full log without anyone doing anything there.
The app already checks GitHub for updates; at the same moment it reads a small file in the public
release repository, `log-requests.json`, that lists install numbers and a request number. If its own
install number is listed with a request it has not answered yet, it emails its full log (compressed,
secrets were never in it) to Martijn. Only Martijn can change that file, so nobody else can ask. The
install number is a random number made at first start; it is in every error report and help report.
(Rejected: a special email that the app obeys. A From address is easy to fake, and it would mean
reading the user's mailbox for commands.)

**Wishes.** A "Wens of idee" button opens one text box and "Versturen". It sends Martijn an email
from the user's own account, with the app version added. It is a normal message the user wrote, so it
shows in Sent like any other.

**Help report and remote help.** **(research)** "Hulprapport kopiëren" puts the same fields as an
error report on the clipboard (app version, Windows version, install number, account hosts but not
addresses, server capabilities, the last sync state per folder, the last 200 log lines), with the same
address masking, for when email itself does not work. For remote help, Windows Quick Assist works;
scammers abuse it, so the app's help page says plainly that only Martijn ever asks for its code.

## 6. Handing it out

- **Windows.** Share the unsigned installer directly. Windows warns once: "Meer informatie", then
  "Toch uitvoeren" (Dutch Windows). **(research)** The installer installs for the current user only, with no admin
  rights.
- **Smart App Control.** **(review)** A PC with Smart App Control switched on blocks unsigned
  programs with no per-app override, every time they start, and every update is a new unsigned file.
  **(research)** Since the April 2026 update it can be switched back on without reinstalling Windows,
  but it has to stay off for as long as the app is used unsigned. Windows 10 does not have Smart App
  Control.
- **Code signing (decision 14: unsigned for now; licence GPL-3.0).** **(research)** Free signing exists for open-source projects:
  SignPath Foundation, if the repository is public, OSI-licensed and built automatically; the
  publisher then shows as "SignPath Foundation". Certum sells a cheap open-source certificate. Either
  would let Smart App Control stay on. Microsoft's own Artifact Signing is not open to individuals in
  the Netherlands. Signing does not remove the SmartScreen warning at once: a new file still has to
  build reputation.
- **Updates: automatic and silent.** (Martijn, 16 November 2026) Martijn publishes a release on
  GitHub; every family PC updates itself, with no question to the user.
  - **Where:** GitHub Releases. Tauri's updater reads one `latest.json` from the newest release.
    **(review)** Releases in a private repository cannot be downloaded without a login, so the
    release files must be in a public repository: either the source repository itself (if it is made
    public, decision 4) or a separate public repository that holds only release files, while the
    source stays private.
  - **Build:** a GitHub Actions workflow on a Windows runner builds the installer with Tauri's own
    action, signs the update with the updater key (a GitHub Actions secret) and creates a *draft*
    release. Martijn installs the draft on his own PC first; only then does he publish it, which
    makes it the one every PC sees.
  - **On the PC:** the app checks at start and every few hours. It downloads in the background and
    installs in `quiet` mode, which Tauri allows for a per-user install, so no window and no click.
    It only installs when the app is idle: no message being written, no import or send running,
    ideally while the window is closed. The app closes for a moment and starts again where it was.
    A failed update sends an error report to Martijn.
  - **Never a step back:** a bad release is fixed by a newer one; the updater does not downgrade.
  - **(review)** The updater only accepts updates signed with Tauri's own key pair (free, made once).
    **(research)** Losing that key means no installed copy can ever be updated again.
  - **Smart App Control:** a silent update of an unsigned app is blocked on a PC where Smart App
    Control is on, and nobody would see why. Automatic updates make code signing (decision 14) more
    valuable than before.
- **Android.** Share the APK. **(research)** Google's developer check for sideloaded apps starts
  worldwide in 2027 (no date yet); until then a sideloaded APK installs as today. Installing over ADB
  stays exempt. The practical route is the free limited-distribution account: no government ID, but
  two-step verification and a Google payments profile with name and address. Google's guide says up
  to 50 devices (an older page says 20). Without it, each phone needs Android's advanced flow once:
  developer options, an "are you being coached" check, a restart, a one-day wait, a fingerprint or
  PIN, then "allow unverified apps". If that is switched off later, updates fail. Tauri's updater
  does not cover Android, and Android never installs an update silently from outside a store: an
  update is at best one "Bijwerken" button that opens Android's install screen (decision 22). Every APK is signed with our own key;
  an update signed with another key will not install.
- **Android background mail.** **(review)** Checking for new mail in the background needs a
  foreground service. That is a separate design; do not improvise it. **(research)** There is no
  official Tauri plugin for it; one community plugin exists (one maintainer, first released April
  2026).

**Back up three things** **(review)**: the updater's private key and its password, the Android
signing key, and the login to the Google Cloud project that holds the sign-in client. Losing one
strands every installed copy.

## 7. Cost

Nothing in this plan costs money for family use. Things that ask for a payment card or personal
details **(review)**: the Azure sign-up behind a Microsoft app registration, and **(research)** the Google payments profile behind the Android limited-distribution
account. Optional and paid: a Certum code-signing certificate (decision 14).

## 8. Build order

Each step proves one thing before the next starts. Details and exit criteria are in `phases/`.

| Step | File | Proves |
| --- | --- | --- |
| 0 | `family-inventory.md` | What the family really needs. Whatever the list does not need drops out |
| 1 | `phases/1-spike.md` | The Rust core works on Windows and Android, mail renders safely, a real PST can be read, installer and update work |
| 2a | `phases/2a-read.md` | Add an account, read mail, search, work offline |
| 2b | `phases/2b-send.md` | Write, reply, attach |
| 3 | `phases/3-import.md` | Old mail comes in, from this computer and from a mailbox |
| 4 | `phases/4-hand-out.md` | A family member can install and update it alone |
| 5 | `phases/5-android.md` | The same code runs as an APK |
| 6 | `phases/6-relay-optional.md` | Dropped (decision 12): no import over the internet |

**(research)** The spike now also reads one real PST and renders one hostile HTML mail. The first
real case depends on import, and these are the two riskiest parts of the plan; finding out in phase 3
would be late.
