# Screens: rules for the people using it

The users are mostly older and not technical. These rules come from the research pass of
16 November 2026 (sources in `research.md`, section "Screens") and were accepted by Martijn the same
day. Every screen in phases 2a, 2b, 3 and 5 follows them. A rule that blocks something Martijn wants
is his call to change; record it in `DECISIONS.md`.

The app is in Dutch. Screen words in this file are the Dutch ones the app shows; the explanation
around them is in English.

## What the research says

- Nielsen Norman Group tested 123 people aged 65 and over: they were 43% slower than younger adults,
  made more errors, and blamed themselves. Text was too small and too light, click targets too small,
  error messages unclear.
- W3C's work on older users found that the normal accessibility rules (WCAG) cover most of what they
  need; what is missing is adjustability.
- A co-design study with older users: an email program should not look completely different from the
  ones people know. Simplify a familiar layout; do not invent a new one.
- Simple mail apps for older people (eM Client's simple mode, BIG EMAIL, Red Stamp) all keep the
  same few things: inbox, read, reply, write, contacts, photos. Folders, rules and settings are
  dropped or left to a helper.

## Outlook as the reference point

The family knows Outlook, and finds it hard (Martijn, 16 November 2026). So keep what they already
recognise, and leave out what makes Outlook hard.

**Keep**

- The layout: folders on the left, the message list in the middle, the open message on the right.
  On a small or zoomed screen the message opens over the list instead, with a big "Terug" button.
- Outlook's Dutch words for the same things:

  | Thing | Word on screen |
  | --- | --- |
  | Inbox | Postvak IN |
  | Sent | Verzonden items |
  | Deleted | Verwijderde items |
  | Drafts | Concepten |
  | Junk | Ongewenste e-mail |
  | Waiting to be sent | Postvak UIT |
  | Write | Nieuwe e-mail |
  | Reply / reply all / forward | Beantwoorden / Allen beantwoorden / Doorsturen |
  | Delete | Verwijderen |
  | Contacts | Contactpersonen |
  | Attachment | Bijlage |

  The folder names are shown in these words whatever the server calls them (`Trash`, `Deleted
  Messages`, `[Gmail]/Prullenbak`), as long as the folder was recognised as that special folder
  (`sync.md`). Other folders keep the name they have on the server.
- The star is called "Belangrijk" on screen, as a word with the star next to it.
- Unread messages in bold, with the unread count after the folder name, as in Outlook.
- Reply, Reply all and Forward as buttons above the open message, where Outlook has them.

**Leave out**

- The ribbon. One row of a few labelled buttons, only the ones that make sense on this screen.
- Tabs, panes to drag, columns to choose, views, rules, categories with colour codes, the calendar
  and to-do parts, "Focused / Other", the people pane.
- Pop-ups that ask something the user cannot answer. No dialog about accounts, profiles, PST files
  or "Send/Receive groups".
- Technical words on any normal screen: IMAP, SMTP, server, port, SSL, sync, cache, OAuth. Those may
  appear only in error details and in the help report.
- Settings pages full of options. The few settings there are (text size, signature, notifications on
  or off, accounts) fit on one page.

## Rules

**Reading**

- Body text at least 18 px by default, with a clear text-size control in the app itself (not only
  Windows scaling). The chosen size is remembered.
- Text contrast at least 7:1 (WCAG AAA). No light-grey text for anything that matters.
- One font, plain. No text in images.

**Pressing**

- Buttons and list rows at least 44 × 44 px.
- Every button has a word on it. An icon may sit next to the word, never instead of it.
- Nothing only on hover, only by right-click, only by swiping or only by keyboard shortcut. Each of
  those may exist as an extra way, never as the only way. Outlook's common shortcuts (Ctrl+N new,
  Ctrl+R reply, Delete) work as extras.

**Finding things**

- One main screen: Postvak IN, Nieuwe e-mail, Verzonden items, Verwijderde items. Each mailbox's
  other folders sit behind its own "Meer mappen (n)" right under it, with an arrow that turns; they
  open in place, indented, inside that mailbox. Labels, the imported old mail and the links
  "Oude mail ophalen" and "Telefoon koppelen" are always visible below the mailboxes (Martijn, 19
  November 2026: one shared "Meer mappen / Minder mappen" button was not easy to use). The same
  words in the same places on every screen.
- A mailbox with other addresses on another domain (decision 32) shows "Postvak IN <domain>" for
  each, indented right under its own Postvak IN, with its own unread count.
- With one account the main screen is that account. With two or more, "Alle postvakken" (C7) comes
  first, then each account with its own Postvak IN, Verzonden items and Verwijderde items.
- The "Nieuwe e-mail", "Beantwoorden", "Allen beantwoorden" and "Doorsturen" buttons appear in
  phase 2b, when writing exists; phase 2a shows no button that does nothing.
- Imported old mail shows as its own clearly named group (for example "Oude mail uit Outlook"), not
  mixed invisibly into Postvak IN.
- Search is one box at the top ("Zoeken"). Next to it, a plain choice of where to look: "Alle mail"
  (every account and the old mail), "Dit postvak", or "Deze map". It starts on the folder or mailbox
  the user is looking at, with "Alle mail" one press away. Results appear while typing, newest first,
  each with its mailbox and folder named.

**Tags and contacts**

- A tag shows as its word on a coloured background, with enough contrast; colour never carries the
  meaning alone. One "Label" button on a message; no drag and drop needed.
- Tags appear under "Labels" in the folder pane as lists ("Rekeningen", "Familie"), each across all accounts.
- Contactpersonen: one simple list, sorted by name, with a search box. Opening a contact shows its
  addresses and the recent mail with that person. No columns, no groups in version 1.

**Mistakes**

- Verwijderen moves to Verwijderde items and shows an "Ongedaan maken" bar for about ten seconds. No
  "are you sure?" dialog.
- Ask for confirmation only for what cannot be undone: emptying Verwijderde items, removing an
  account, sending to more than about ten people.
- A message being written is never lost: closing the window keeps it in Concepten.

**Safety**

- Always show the sender's real address next to the name.
- Remote images stay blocked until the user presses one clearly labelled button ("Afbeeldingen
  tonen").
- Warn, calmly and in one sentence, when:
  - a link's visible text shows one web address and the link goes to another;
  - the sender's name matches a known contact but the address does not;
  - the first message from a new sender mentions money, a bank, a password or urgency.
  Thunderbird's link warning fires on harmless marketing mail; keep ours narrow so people do not learn
  to ignore it.
- Do not judge a message by an `Authentication-Results` header unless it carries the user's own
  provider's name, and only the top one. Anyone can add such a header. If in doubt, show nothing.

**Notifications and the icon next to the clock**

- New mail: a Windows notification with the sender's name and the subject. Clicking it opens that
  message. Several at once become one notification ("3 nieuwe berichten"). Notifications can be
  turned off on the settings page; Windows' own "do not disturb" is respected.
- Closing the window does not stop the app. The first time, one sentence says so: the app keeps
  checking for mail and lives next to the clock. Clicking that icon opens the window again; its menu
  has "Openen" and "Afsluiten".
- The app starts with Windows, quietly, next to the clock. This can be turned off on the settings
  page.

**Help, wishes and updates**

- One "Help" item in the main screen holds: "Wens of idee" (one text box, "Versturen"), "Hulprapport
  kopiëren", and the Quick Assist note (`PLAN.md` section 5).
- The first time something is reported to Martijn, one plain sentence says that Martijn gets a message
  when something goes wrong, and that no mail content goes with it.
- Updates are never announced with a question. After an update, a small line for one day says that
  the app was updated, with one "Wat is er nieuw?" link that shows two or three plain sentences.

**Errors**

- Every error follows `errors.md`, plus a "Kopiëren voor Martijn" button that copies the sentence,
  the server's words and the code. The error is also reported to Martijn by itself (`PLAN.md`
  section 5); the screen says so in one line ("Martijn krijgt hier vanzelf bericht van"). That line
  appears only once the reporting of phase 2b is built.

## What this means for the build

- Phase 2a: the three-pane layout, the Outlook words, the text-size control and the Undo bar come
  with the first screens, not at the end.
- Phase 2a: the address-next-to-name rule and the link warning come with the message view.
- Phase 2a: notifications, the icon next to the clock and starting with Windows come with new mail.
- The first-contact and contact-spoofing warnings read the contact list (`features.md`), so they
  come with phase 2b, when contacts exist.
- Test each screen with one family member before calling a phase done. Watch, do not explain.
