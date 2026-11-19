# Features

What version 1 does, what is waiting for Martijn's choice, and what stays out. Nothing gets built
that is not in the "In" table or chosen from the "Candidates" table. The rules of `screens.md` apply
to every feature.

## In version 1

| Feature | Phase | What it means |
| --- | --- | --- |
| Accounts by address | 2a | Type an address, enter a password or sign in; Solcon built in |
| Read, folders, offline | 2a | `PLAN.md` section 2 |
| Search at three levels | 2a | All mail, one mailbox, one folder; first results within 0.2 s on 100,000 messages (decided 16 November 2026) |
| Tags | 2a | Words with a colour, put on messages. See "Tags" below (decided 16 November 2026) |
| Star | 2a | One "Belangrijk" mark, the standard IMAP `\Flagged`, so it shows in every mail program |
| Delete, move, mark read | 2a | Decision 10; Verwijderen goes to Verwijderde items with "Ongedaan maken" |
| Print (C4) | 2a | One "Afdrukken" button with a clean print layout |
| All inboxes (C7) | 2a | One "Alle postvakken" view over every account, next to each account's own Postvak IN |
| Windows notifications (C2) | 2a | New mail shows a Windows notification; clicking it opens the message; unread count on the taskbar icon. `screens.md` |
| Runs next to the clock, starts with Windows (C12) | 2a | Closing the window keeps the app running so notifications keep coming; "Afsluiten" really stops it. Starting with Windows can be turned off |
| Help report | 2a | "Hulprapport kopiëren", for when email itself does not work |
| Write, reply, forward, attach, drafts, Postvak UIT | 2b | `phases/2b-send.md` |
| Smaller photos (C1) | 2b | One question when attaching big photos: "Foto's kleiner maken?" |
| Undo send (C3) | 2b | Sending waits ten seconds, with "Ongedaan maken" |
| Signature (C6) | 2b | One plain-text signature per account |
| Contacts | 2b, 3 | A simple address book on the device. See "Contacts" below (decided 16 November 2026) |
| Error reports to Martijn | 2b | Errors are emailed to Martijn at once, through the user's own account. `PLAN.md` section 5 |
| Logs on request | 2b | Martijn asks a PC for its log from a distance; it is emailed to him. `PLAN.md` section 5 |
| Wish button | 2b | "Wens of idee": one text box, sent to Martijn by email |
| Mailbox space | 2a | "Ruimte in postvak" per account; a warning at 90% |
| Import old mail | 3 | PST, Windows Live Mail, Outlook Express (DBX), another mailbox |
| Copies of original files | 3 | The original import file is kept; a menu shows and deletes the copies (decision 15) |
| Contacts on the phone | 5 | QR pairing; see "Contacts" below |
| Export local mail | 3 | mbox or EML, because local mail has no other backup |
| Silent updates | 4 | Published on GitHub; every PC updates itself when idle, no question asked. `PLAN.md` section 6 |
| "Ongewenste e-mail" (C8) | 5 | Move to the junk folder and back, with undo (decision 29) |
| Invitations shown readably (C11) | 5 | What, when, where and who invites, above the text (decision 29) |
| Backup to a USB stick (C14) | 5 | Windows only: everything but passwords, and a restore (decision 29) |
| Several addresses in one mailbox | 5 | Other addresses (an own domain at iCloud) with a Postvak IN per domain and a "Van" choice (decision 32) |
| A richer Write screen | 5 | Formatting, BCC, Dutch spellcheck and photos in the text (decision 33) |

Error reports, log requests and the wish button come in 2b because they send email, which 2b builds.
Phase 2a already writes the log and builds the error box.

## Tags

- A tag is a short word with a colour, for example "Family", "Bills", "Keep". The word is always
  shown; colour alone never carries the meaning.
- A message can have several tags. Tagging is one button on the message ("Label"), with the list of
  existing tags and "Nieuw label".
- Search can filter by tag, at all three levels, and each tag can be opened as a list (all mail with
  that tag, across accounts).
- **Stored on the server when it can be.** IMAP keywords (`STORE +FLAGS (keyword)`), which
  Thunderbird and many other programs show as tags. Only when the folder's `PERMANENTFLAGS` includes
  `\*`. Some servers do not keep keywords; the spike records this per account.
- **Gmail:** tags become Gmail labels (`X-GM-LABELS`), so they show in Gmail on the web too. A label
  also shows as a folder in other programs; spike task 16 tries it on a throwaway account so Martijn
  can judge whether that is acceptable.
- **Where the server cannot keep them** (and for mail kept on this computer): tags live in the local
  index only, and the tag screen says "only on this computer".
- **Import:** Outlook categories on PST messages become tags. Windows Live Mail flags become a star.
- Keyword names are kept simple (letters and digits, no spaces) on the server; the word the user sees
  is kept in our tag table. Use Thunderbird's `$label1`–`$label5` meaning (Important, Work, Personal,
  To do, Later) when they are found, so old Thunderbird tags show with a name.
- Not in version 1: rules that tag automatically.

## Contacts

- A list of people: name, one or more addresses, optionally a phone number and a note. Nothing more
  in version 1.
- **Filled automatically and by hand.** Everyone the user writes to is added (as "suggested" until
  used twice, so one-off addresses do not clutter the list). An "Opslaan als contactpersoon" button on any message.
- **Used when writing:** typing a name or part of an address suggests contacts first, then other
  addresses from mail.
- **Used for safety:** the warnings in `screens.md` (a known name with a strange address, the first
  message from a new sender) read the contact list.
- **Import:** contacts in a PST file (Outlook's contact items) and `.vcf` files. Windows Live Mail
  keeps its contacts in a separate database; its "export to vCard" route works, reading the database
  directly is not planned.
- **Export:** `.vcf`, so the list is never trapped in the app.
- **On several devices** (decision 19, chosen as C13; phase 5): pair the phone once by scanning a
  QR code on the PC ("Telefoon koppelen"). The QR code holds a random secret key and which account to
  use, never a password; the phone adds that account itself with its own sign-in. The contacts then
  travel encrypted with that key, as one message in a "Vorige Eeuw Mail" folder in the user's own
  mailbox (`sync.md`). No server of ours. Firebase was considered: it would be a server of ours to
  maintain and would change the hard rules, so it was not chosen. A lost phone: pair a new one;
  "Koppeling verbreken" on the PC makes a new key.

## Candidates

Each needs Martijn's yes before it is built. Ordered by how much it helps an older user for its cost.
"Cost" is a rough guess of the work. C1, C2, C3, C4, C6, C7 and C12 were chosen on 16 November 2026
(decision 20), C8, C11 and C14 on 18 November 2026 (decision 29); all are now in the table above. They stay listed here so the numbers keep their
meaning.

| # | Feature | Why | Cost |
| --- | --- | --- | --- |
| C1 | Make photos smaller when attaching (chosen) | Phone photos are 3–10 MB each; five of them hit many providers' size limit and fill the mailbox. One question: "Foto's kleiner maken?" | Small |
| C2 | New-mail notice (chosen) | A Windows notification and an unread count on the taskbar icon, while the app runs | Small |
| C3 | Undo send (chosen) | Wait ten seconds before sending, with an "Ongedaan maken" button. Catches the most common mistake | Small |
| C4 | Print (chosen) | One "Afdrukken" button with a clean print layout. Older users print mail (decision 18) | Small |
| C5 | Read aloud | Windows' own voices read a message out loud. Helps tired or weak eyes | Small |
| C6 | Signature (chosen) | One plain-text signature per account | Small |
| C7 | All inboxes (chosen) | One "Alle postvakken" view over every account, next to each account's own Postvak IN | Medium |
| C8 | "This is spam" (chosen) | Move to the Junk folder; tells the server where it supports that | Small |
| C9 | Photo view | Pictures in a message shown large, with "Save all photos" | Medium |
| C10 | Conversations | Replies grouped under the first message. Off by default; many older users find it confusing | Medium |
| C11 | Calendar invites shown readably (chosen) | A received invitation shows date, time and place in plain words. No calendar in the app | Small |
| C12 | Start with Windows (chosen) | The app starts hidden at sign-in so new mail is noticed | Small |
| C13 | Contacts on several devices (chosen) | QR pairing, encrypted through the user's own mailbox (decision 19) | Medium |
| C14 | Backup of everything (chosen) | One button that copies the whole store (mail, tags, contacts, settings, no passwords) to a USB disk, and a restore | Medium |

## Out

Not planned, to keep it simple: rules and filters, a calendar, snooze and reminders, mail merge,
encryption (PGP, S/MIME), more than one window, themes beyond the text-size control, chat, a
built-in web browser.
