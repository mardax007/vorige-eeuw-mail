# Phase 2b — Send

**Proves:** someone can write, reply and attach, and the message ends up where they expect.

**Before starting:** decision 21 (the report address) is answered. Read `../sync.md` (sent
copies), `../screens.md` and `../PLAN.md` section 5.

## Tasks

1. **Contacts.** The address book of `../features.md`: name, addresses, phone, note; filled from
   sent mail (as "suggested" until used twice), by hand, and with "Opslaan als contactpersoon" on a message.
   Export and import `.vcf`. Then the contact-based warnings of `../screens.md`: a known name with a
   strange address, and the first message from a new sender about money or passwords. The
   "Contactpersonen" screen of `../screens.md`.
2. **Write.** The "Nieuwe e-mail" button on the main screen, and `mailto:` links from messages. To, Cc, Bcc,
   subject, plain-text body. Keep the editor simple. Typing suggests
   contacts first, then other addresses from mail. One plain-text signature per account (C6), set on the settings page.
3. **Reply, reply all, forward.** Correct quoting and the right reply headers (`In-Reply-To`,
   `References`), so threads stay together in other mail programs.
4. **Attach.** Add and remove files; show the size; warn before a message the server will refuse
   for size, when the server states its limit (SMTP `SIZE`). Offer "Foto's kleiner maken?" when
   big photos are attached (C1).
5. **Send.** Over SMTP with TLS: implicit TLS on 465 or required STARTTLS on 587, as the provider
   says (Solcon: `mail.solcon.nl` 587, not the IMAP host). Failures follow the error rule with the
   SMTP reply shown exactly, all lines, as the server sent them. Ask before sending to more than
   about ten people. Sending waits ten seconds with "Ongedaan maken" (C3).
6. **Sent folder.** By `../sync.md`: Gmail and Outlook.com save a copy themselves; for other
   providers search Sent for the Message-ID and append only if it is missing. Never zero, never two.
7. **Drafts.** Keep an unsent message when the window closes or sending fails. Save locally at once
   and to the server's Drafts folder when online. Never lose text.
8. **Postvak UIT.** A message sent while offline waits in Postvak UIT and goes when the connection is back.
   The screen says it is waiting.
9. **Error reports.** By `../PLAN.md` section 5: every error is emailed to Martijn at once through
   the user's own account, with code, sentence, exact server reply, versions, install number and the
   last 50 log lines. One per code per day, ten a day at most; queued when sending fails. The
   address comes from the build (a GitHub Actions secret), never from the repository. The one-time
   sentence telling the user. Test against the local test server only.
10. **Logs on request.** Read `log-requests.json` at the same moment as the update check (a test file
    in a test repository until phase 4); when the install number is listed with a new request
    number, email the full log. Only answer each request once.
11. **Wish button.** "Wens of idee": one text box, "Versturen", sent to Martijn with the app version.
    It sits in the "Help" item.

## Exit criteria

- [ ] A message, a reply and a forward arrive correctly in another mail program
- [ ] Attachments arrive intact, including file names with accents
- [ ] Typing part of a name suggests the right contact; a contact survives export and import as `.vcf`
- [ ] One copy in Sent, not zero and not two, for each provider in the inventory, tested on a spare
      mailbox at that provider (never a family mailbox)
- [ ] A failed send keeps the message and shows the server's exact reply, every line
- [ ] A message written offline is sent once, and only once, when the connection returns
- [ ] A forced error reaches the report address on the test server once, with the server reply exact
      except masked addresses, no address other than the sender's and Martijn's, and no password,
      token or mail content
- [ ] A listed log request is answered once; an unlisted install number sends nothing
- [ ] A wish sent from the app arrives with the app version
- [ ] Undo send, the signature and smaller photos work; a message in Postvak UIT is sent once
