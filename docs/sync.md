# Sync rules

How the local cache stays correct against an IMAP server. Written from the research pass of
16 November 2026 (sources in `research.md`, section "Sync"). Phase 2a builds by these rules; phase 1
checks the ones marked **spike**.

The aim: the app can be killed at any moment, the network can drop at any moment, and nothing is lost
or shown twice.

## Identity

- A message on the server is `(account, folder, UIDVALIDITY, UID)`. Key every cached row by that.
- A UID is only valid inside one folder. Moving a message gives it a new UID.
- If a folder's UIDVALIDITY changes, throw away that folder's cache and every queued action on its
  UIDs, then resync it. If it changes again and again, stop after three tries and show a `VEM-IMAP-…`
  error. Some servers do this; do not loop.
- Use `Message-ID` plus a content hash only to spot duplicates (import, Sent copies), never as the key.

## Finding changes

Use the best method the server offers, in this order:

1. **CONDSTORE** (RFC 7162): `UID FETCH 1:* (FLAGS) (CHANGEDSINCE <modseq>)` gives changed flags. It
   does not report deleted messages, so follow it with `UID SEARCH` over the cached UID range and
   drop the UIDs that are gone.
2. **Neither CONDSTORE nor QRESYNC:** compare UIDNEXT and the message count with the cache. If both
   are unchanged, nothing new arrived. Fetch flags for the most recent part of the folder on every
   pass, and for the whole folder now and then.

QRESYNC would do both in one step, but `async-imap` does not support it. Do not build it ourselves
in version 1.

- Store a folder's HIGHESTMODSEQ only after the server's final `OK`, never from single fetch replies:
  they can arrive out of order.
- A folder that answers NOMODSEQ has no modseqs: use method 2 for it.
- Commit sync state per batch, in one SQLite transaction with the rows it describes.

## Bodies

- First sync: headers and structure only. Bodies on demand when a message is opened, and in the
  background, newest first, until every body is here (decision 7: everything).
- Gmail: skip `[Gmail]/All Mail` as a folder (it repeats every message). Use `X-GM-MSGID` to know
  that two folders hold the same message.

## New mail and connections

- One IDLE connection per account, on the Inbox. Other folders are checked every few minutes, and at
  once when the user opens them.
- Renew IDLE at least every 20 minutes. RFC 2177 says 29, but home routers and mobile networks drop
  quiet connections sooner.
- When IDLE reports new mail, end IDLE, fetch, and start again. Do not react more than about twice a
  second.
- At most two or three connections per account per device: one for IDLE, one for everything else.
  Gmail allows 15 per account across all devices; many ISPs allow about 10 per address. A household
  with several devices adds up.
- After a dropped connection: reconnect with growing waits plus a random part.
- After a refused login: stop and ask the user. Never retry a refused password in a loop; servers
  lock accounts for that.

## Actions (mark read, move, delete)

- Every action goes into a queue in SQLite first, then to the server. The screen shows the result at
  once.
- Replay the queue in order, after the folder has been resynced. Each action must be safe to repeat.
  "Message no longer exists" counts as done.
- Always use UID commands. Expunge with `UID EXPUNGE`, so messages another program marked deleted
  are not removed by us.
- Never send `CLOSE` on a folder with deleted-marked messages: it expunges them silently.
- Move: `UID MOVE` when the server has MOVE. Otherwise `UID COPY`, then mark deleted, then
  `UID EXPUNGE`.
- Delete: move to the Trash folder. Only emptying the Trash removes mail for good.
- Gmail: taking a message out of the Inbox only archives it. Deleting means moving it to
  `[Gmail]/Trash`.

## Special folders

- Find Sent, Drafts, Trash and Junk by SPECIAL-USE flags (RFC 6154) first, then XLIST on old servers,
  then by name.
- Names to try, Dutch and English: Verzonden, Verzonden items, Sent, Sent Items, Sent Messages;
  Concepten, Drafts; Prullenbak, Verwijderde items, Verwijderd, Trash, Deleted Items, Deleted
  Messages; Ongewenste e-mail, Spam, Junk. Check this list against the folder names spike task 6
  records.
- Gmail's folder names follow the user's language.
- Never create a special folder that already exists under another name.

## Tags and the star

- The star is `\Flagged`, a standard flag every server keeps.
- Tags are IMAP keywords, set and removed with `UID STORE +FLAGS` / `-FLAGS`, and synced like any
  other flag (CONDSTORE reports keyword changes too).
- Only write a new keyword when the folder's `PERMANENTFLAGS` contains `\*`. If it does not, the tag
  is kept locally and marked "only on this computer". Read keywords other programs set, and show
  Thunderbird's `$label1`–`$label5` with their usual names.
- Server keyword names: letters and digits only, no spaces, case kept as stored. The displayed word
  and colour live in our tag table, keyed by the keyword.
- Gmail: tags are labels, through `X-GM-LABELS`, not keywords. Adding a label does not move the
  message. Spike task 16 checks how Gmail treats keywords as well.
- Local tags travel through the offline queue like every other action.

## Sent copies

- Gmail and Outlook.com put a copy in Sent themselves. Appending another makes two.
- Keep a "server saves sent mail" flag per provider in our provider table. For providers without the
  flag: after sending, wait a moment, search Sent for the message's Message-ID, and append only if it
  is missing.
- Spike task 16 checks Solcon's behaviour, on a spare mailbox only.

## Testing

- Every rule above gets a test against the local test server.
- Also test against a deliberately awkward server: no CONDSTORE, a changing UIDVALIDITY, English-only
  folder names, Gmail-style labels. Mailspring's authors found that small providers break sync in
  ways the big ones never do.
- Spike task 16 checks the connection limit and the Sent behaviour per provider, on throwaway
  accounts only. Gmail throwaway accounts use an app password, never the real Google Cloud project.

## Contacts between devices

- Decision 19: a paired phone and PC share contacts through the user's own mailbox. One folder,
  "Vorige Eeuw Mail", in the account chosen at pairing. Our app does not show this folder in its
  folder list; other mail programs will, so its one message has a plain subject saying what it is
  and not to delete it.
- The message body is the whole contact list, encrypted with the key from the QR code (an AEAD
  cipher from a vetted crate; check its current docs). The server sees only ciphertext and a size.
- Each contact carries a change time and a deleted mark, so two devices merge without losing a
  change: newest change per contact wins, deletions stay as marks.
- Writing: APPEND the new version, then delete the older version this app wrote (UID EXPUNGE on our
  own message only). Reading: the newest message in that folder; ignore any message that does not
  decrypt.
- Spike and tests use a throwaway account (the mail-safety rule).
