# Mail kept on the PC, also on the phone (decision 26)

Old mail is imported on Windows (PST, Windows Live Mail, Outlook Express). When it fits, it goes into
the mailbox and the phone sees it through IMAP like any other mail. When it does not fit (a Solcon
mailbox of 2 GB, a PST of 6 GB), it stays on the PC as local folders. Martijn's answer to question 26
(17 November 2026): the heavy work stays on Windows, but the phone must have all mail. So the PC copies
its local folders to the paired phone over the home Wi-Fi, by itself, without a question to anyone.

## What the user sees

- Nothing to do. The phone is paired already ("Telefoon koppelen", decision 19). When the PC and the
  phone are on the same Wi-Fi and the app is open on both, the PC's local folders appear on the phone
  under the same names. While a copy runs, the phone says "Mail van de computer: 120 van 5.000".
- The PC's Telefoon page says when the phone last got its mail.
- On the phone "Oude mail ophalen" offers only "Uit een ander postvak". Files from Outlook and
  Windows Live Mail are brought in on the computer; the page says so.

## How it works

The PC never listens for connections. A Windows program that listens makes Windows (or the
virus scanner) ask about the firewall, and an older user who answers "Annuleren" breaks the feature for
good. Checked on Martijn's PC on 17 November 2026: a program that listened on TCP got firewall rules
made for it; a program that only sent a UDP broadcast and read the reply got none, and the reply came
in (Windows lets a unicast answer to a broadcast through for a few seconds).

1. **Find.** Every 90 seconds, when the PC has local folders and a pairing key, it sends one UDP packet
   to the broadcast address of each local network (and to 255.255.255.255), port 47801. The packet is
   sealed with the pairing key (XChaCha20-Poly1305, as the contacts are) and holds a fresh random
   value, the time and a digest of what the PC has to offer.
2. **Answer.** The phone listens on UDP 47801 while the app runs. It answers only a packet it can open
   with its key, only from a private address, only when the time is within ten minutes. The sealed
   answer echoes the random value, gives the phone's TCP port (47802) and says whether the phone
   still needs anything (its last applied digest differs).
3. **Copy.** If the phone needs something, the PC connects to the phone's TCP port. Both sides send 24
   random bytes; the session key is SHA-256 of a label, the pairing key and both random values, so a
   recorded session cannot be replayed. Every frame is sealed with the session key and a counter per
   direction. A side that cannot open a frame closes the connection.
4. **What is copied.** The PC sends a list of its local messages (local account, message number,
   folder, read and star). The phone answers with the numbers it lacks; the PC sends those messages
   whole (the `.eml`). The phone puts them in local accounts of the same names, in the same folders.

## The phone's copy follows the PC

- A message the PC deletes, or a whole local account the PC removes, is removed on the phone.
- A message moved, read or starred on the PC changes on the phone too, but only when the PC's value
  changed since the last copy; what the user did on the phone is not overwritten for nothing.
- A message deleted on the phone stays deleted on the phone; it is not sent again.
- Unpairing stops the copying. What is on the phone stays.

## Limits

- Only private addresses (10/8, 172.16/12, 192.168/16, 169.254/16) are answered or connected to.
- A single message above 128 MB is skipped and counted.
- When the phone runs out of room, the copy stops with `VEM-LAN-SPACE-01`.
- Android's "local network" permission: Android 16 does not enforce it yet. A later Android that does
  will need `ACCESS_LOCAL_NETWORK` asked once; check when the target SDK goes up.
- Some phones drop broadcast packets while the screen is off; the copy happens when the app is open.
- Tested in one process over 127.0.0.1 (`crates/vem-core/tests/pc_to_phone.rs`) and on the emulator
  through `adb forward` and the emulator's UDP redirect. A real phone on a real home network is
  Martijn's check.
