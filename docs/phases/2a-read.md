# Phase 2a — Read

**Proves:** someone can add an account by typing an address, read their mail, search it fast, and
open the app without a connection.

This and 2b are most of the work. Reading comes first.

**Before starting:** the spike is done and decision 6 made. Read `../sync.md`, `../screens.md` and `../features.md`.

## Tasks

1. **Add an account.** Type an address; look up the settings (our provider table first); show the
   password field or the provider's sign-in button; test IMAP and SMTP; say exactly which step failed.
   Offer a username field when the address is refused as a username. Show the server name before a
   password goes to a guessed host. Require TLS; a STARTTLS server that stops offering it is an
   error, never a plain-text login.
2. **Error rule.** Build the error type, the codes and the error box from `../errors.md` now, not
   later. Every failure in this phase goes through it. Include the "Kopiëren voor Martijn" button. Sending the error to Martijn by
   email comes in 2b (so does the line saying Martijn gets a message); here every error is already written to the log with its code.
3. **Message catalogue.** All screen text through one catalogue, in Dutch (decision 5), with the
   Outlook words of `../screens.md`. No string on a screen that is not in the catalogue.
4. **Screens base.** The layout and rules of `../screens.md`: folders, list and message side by
   side as in Outlook; main screen with Postvak IN, Verzonden items, Verwijderde items and "Meer
   mappen" (Nieuwe e-mail comes in 2b), one or several accounts; text-size control; word labels;
   44 px targets; 7:1 contrast.
5. **Local store.** SQLite index plus messages as `.eml` files in the app's local data folder (not the
   roaming one: it can grow to gigabytes). Secrets only through DPAPI (decision 17), in their own file. Design it so a
   local account (imported mail kept on this computer, phase 3) fits in the same tables.
6. **Sync.** By `../sync.md`: cache keyed by UIDVALIDITY and UID, CONDSTORE where offered, deleted
   messages detected, state committed per batch, special folders found by SPECIAL-USE flags, then
   XLIST, then by Dutch and English names. Headers first, then every body in the background (decision 7). Stay within Gmail's limits and
   skip Gmail's All Mail folder. Resume after a dropped connection.
7. **Folders and message list.** Folder list, message list that stays fast with tens of thousands of
   messages, unread state. The sender's real address next to the name.
8. **Read a message.** Plain text and HTML. HTML the four-layer way of `../PLAN.md` section 2: Rust
   sanitiser, sandboxed `srcdoc` frame, CSP, links through the opener. A "show images" button. Find a
   safe way to keep `<style>` blocks (for example parse and filter the CSS in Rust) so newsletters
   stay readable. Warn when a link's text shows a different address than its target.
9. **Attachments.** List, open, save. `winmail.dat` (TNEF) attachments: at least show the files
   inside, or say plainly that they cannot be opened.
10. **Search.** Fast at three levels: all mail, one mailbox, one folder (`../PLAN.md` section 2,
    "Search"). One FTS5 table with account and folder on each row; subject, names, addresses, body
    text and attachment names; diacritics folded; prefix matching so results come while typing;
    newest first, in pages. The scope choice of `../screens.md`. When bodies are still being fetched,
    the results say how far the index is.
11. **Offline.** The app opens instantly and shows what it has without a connection.
12. **New mail.** One IDLE connection per account on the Inbox, renewed at least every 20 minutes;
    other folders checked every few minutes. At most two or three connections per account.
    Then the notifications of `../screens.md`: a Windows notification per new message (grouped when
    several arrive), clicking it opens the message, unread count on the taskbar icon. Closing the
    window keeps the app next to the clock; "Afsluiten" in its menu stops it; the app starts with
    Windows (Tauri's notification and autostart plugins and its tray support; check their current
    docs). Test notifications on an installed build: Windows needs the app's identity for them.
    The settings page gets its first toggles: notifications on or off, start with Windows on or off,
    and the text size. The one-time sentence about the icon next to the clock.
13. **Actions.** The actions of decision 10 (delete, move, mark read or unread), through the
    offline queue of `../sync.md`. Verwijderen moves to Verwijderde items with an "Ongedaan maken"
    bar.
14. **Tags and star.** By `../features.md` and `../sync.md`: tag a message, create a tag, open a
    tag as a list across accounts, filter search by tag. Stored as IMAP keywords where the server
    keeps them, as Gmail labels on Gmail, locally otherwise, and the screen says which. The star is
    `\Flagged`.
15. **Chosen candidates.** Print (C4) and "Alle postvakken" (C7) from `../features.md`; C2 and C12
    are part of task 12.
16. **Log and help report.** The local rotating log of `../PLAN.md` section 5: typed log calls that
    cannot take secrets, mail content or addresses; addresses in server replies masked. A random
    install number made at first start. The "Help" item of `../screens.md` with "Hulprapport
    kopiëren" and the Quick Assist sentence.
17. **Google sign-in** (decision 8): the flow proved in spike task 10, built into
    the add-account screen, with the app password as fallback.
18. **Microsoft sign-in** (decision 9), only if the app registration worked. Otherwise the plain
    sentence of `../errors.md`.
19. **Mailbox space.** "Ruimte in postvak" per account: IMAP `QUOTA` where offered, otherwise the sum
    of message sizes against the limit in the provider table. A warning at 90%.

## Test with

- The local test server for everything that writes. Also a misbehaving variant: no CONDSTORE, a
  changing UIDVALIDITY, English-only folder names.
- Awkward old messages: Windows-1252 text, uuencoded attachments, `winmail.dat`, missing or broken
  headers, very large messages. Keep made-up samples in `testdata/samples/` (no real mail in git; `.gitignore` blocks `.eml` files everywhere else).
- A hostile HTML set (from the spike) kept as a regression test.
- For search speed: 100,000 generated messages spread over three accounts and many folders, with
  Dutch text and accented words. Measure on the slowest family PC, or a machine as slow.

## Exit criteria

- [ ] A new account is added with only an address and a password, Solcon included
- [ ] Every failure on the way shows sentence, next step, exact server reply and code
- [x] A large mailbox lists quickly (`search_perf`: a folder's first page in 0.3 ms with 100,000
      messages in the index; 17 November 2026)
- [x] Search gives first results within 0.2 s on 100,000 messages, for all mail, one mailbox and one
      folder alike, and finds "café" when typing "cafe" (`search_perf`: at most 32 ms on the PC, 46 ms
      on the Android emulator; the accent rule in `store/search.rs` tests; 17 November 2026)
- [ ] HTML mail cannot run scripts or load remote images unasked (the hostile set passes)
- [ ] A tag set here shows in another mail program (where the server keeps keywords), and a tag set
      there shows here
- [ ] The app opens and reads mail with the network off
- [ ] Killing the app during sync loses nothing and duplicates nothing
- [ ] Print, "Alle postvakken", "Belangrijk", the "Ongedaan maken" bar and the text-size control work
- [ ] No password or token in the index, logs, help report or message files
- [ ] A new message shows a Windows notification on an installed build, with the window closed,
      and clicking it opens that message
- [ ] Every word on every screen comes from the Dutch catalogue
- [ ] One family member used the main screen without help while Martijn watched
- [ ] Martijn has used it on his own mailbox for a few days
