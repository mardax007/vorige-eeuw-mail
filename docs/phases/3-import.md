# Phase 3 — Import

**Proves:** old mail comes in, complete and with a report, from this computer and from a mailbox.

**Before starting:** decisions 11, 14 and 15 were answered on 16 November 2026. Read `../PLAN.md` section 4.

## Fixtures first

Before writing readers, collect test files (none of them real mail, none in git unless generated):

- PST files: ANSI (Outlook 97–2002), Unicode, "compressible" and "high" encryption, one with a
  password, one with embedded messages, inline images, calendar items and contacts.
- A Windows Live Mail store with local folders and an IMAP account (headers-only messages included),
  ideally from a Dutch installation.
- Martijn's copies of the family's real files stay outside the repository and are used read-only.

## Tasks

1. **Import from a file or folder.** The fallback that always works: the user points at a `.pst`
   file or a folder of `.eml` files (also from a USB stick). Explain that a FAT32 stick cannot hold a
   file of 4 GB or more.
2. **Find old mail on this computer.** Look in the usual places: `Documents\Outlook Files`,
   `%LOCALAPPDATA%\Microsoft\Outlook`, the Outlook profiles in the registry, and the Windows Live Mail
   store root from the registry (default `%LOCALAPPDATA%\Microsoft\Windows Live Mail`). Show what was
   found with message counts; the user ticks what to bring in. Nothing is uploaded.
3. **PST reader.** `outlook-pst` for reading, plus our own MAPI-to-MIME module (`../PLAN.md`
   section 4): body choice and RTF unwrapping, rebuilt headers for sent items, dates, read state,
   code pages, embedded messages, inline images. Outlook categories become tags. Contact items go
   to the address book (name, addresses, phone). Calendar items and tasks are counted as skipped.
   Ask for the password of a protected PST. Detect a locked file and ask the user to close Outlook.
   An `.ost` file is refused with a pointer to the mailbox route. A PST exported from the new
   Outlook app is read like any other.
4. **Isolation and counts.** Each message is converted on its own; a panic or error skips that
   message with a reason. Every folder's written count is checked against the source count, so a
   reader that stops early is caught.
5. **Rescue mode.** If `outlook-pst` cannot read a file, offer a separately shipped rescue tool
   (`readpst` or `pffexport`) and read its `.eml` output. Say that read state may be lost. Licence
   per decision 14.
6. **Windows Live Mail reader.** `.eml` files in folders; folder names and read state from a copy of
   `Mail.MSMessageStore` through Windows' `esent.dll`. If that fails, keep the on-disk folder names,
   mark everything read and say so. Headers-only messages are reported, not imported as complete.
   Flagged messages get the star. Vista's Windows Mail works the same way. Contacts come in through
   Windows Live Mail's own vCard export, explained in one screen.
7. **From a mailbox.** Sign in to the old and the new account and copy folders across over IMAP.
8. **Where it goes.** Each import asks: "Put in my mailbox" (over IMAP) or "Keep on this computer"
   (a local account in the same store). Before "Put in my mailbox", read the quota and say plainly
   when the import will not fit, with the numbers.
9. **Progress and resume.** Show progress per folder. An interrupted import continues where it
   stopped and does not create duplicates (Message-ID plus content hash). APPEND keeps the original
   date and read state. Honour `APPENDLIMIT` and `TOOBIG`. Into Gmail: stay under 500 MB of uploads a
   day and continue the next day.
10. **Search index.** Imported mail goes into the same search index as synced mail, so it is found
    at all three levels the moment it is in.
11. **Report.** How many messages came in, how many were skipped and why, as a list the user can
    save. Each skip reason has a `VEM-IMPORT-…` code.
12. **Export and original copies.** Local folders can be exported to mbox or EML files, because that
    copy has no other backup. Decision 15: before converting, copy the original file (the PST, the
    mail folder) into the app's data folder, after checking the free disk space; the source itself
    is never touched. A menu "Originele bestanden" lists the copies with date and size and deletes
    one after a single confirmation.
13. **Outlook Express (DBX)** (decision 11). Unproven format: test UnDBX or DbxConv on real files
    first (UnDBX is GPL, which our GPL-3.0 licence allows), then read DBX into our store.

## Rules

- Source files are opened read-only and never changed or removed. When a library might write (an
  ESE database replaying its logs), work on a copy.
- Dates, read state, folders and attachments survive the import. Check with real old mail.

## Exit criteria

- [ ] The first case works end to end: old mail in, shown together with the new Solcon account
      ("Keep on this computer", or "Put in my mailbox" into a spare Solcon mailbox; never an
      APPEND into a family mailbox during testing)
- [ ] A real PST of several GB imports with a correct count per folder (same rule)
- [ ] Sent items from a PST show the right sender, recipients and date
- [x] An import that is cut off resumes without duplicates (`an_import_cut_off_halfway_goes_on_without_doubles`; 17 November 2026)
- [x] The report lists every skipped message with a reason (import tests for headers-only, too
      large and damaged mail; the real DBX counts added plus skipped against its index; 17 November 2026)
- [x] An import that does not fit the mailbox is stopped before it starts, with the numbers
      (`mail_that_does_not_fit_the_mailbox_is_refused_before_anything_happens`; 17 November 2026)
- [x] The source files are byte-for-byte unchanged afterwards (`import/corrupt_tests.rs` compares
      every source before and after, real and damaged files; 17 November 2026)
- [x] Imported old mail is found by search at all three levels (`eml_folder_import_keeps_original_and_skips_repeats`; 17 November 2026)
- [x] Contacts and categories from a PST arrive as contacts and tags (contacts from the real public
      PSTs and `outlook_contacts_join_the_address_book_without_overwriting`; categories in
      `outlook_categories_become_tags_with_state_and_accents_kept`, since no public PST with
      categories was found; a family PST will show them for real; 17 November 2026)
- [x] A copy of each imported original file is kept, and deleting it from the menu frees the space
      (`eml_folder_import_keeps_original_and_skips_repeats`, `originals_only_inside_our_folder`; 17 November 2026)
- [x] A DBX folder imports with the right count (`real_dbx_file_imports` and a self-made
      40-message DBX; 17 November 2026)
