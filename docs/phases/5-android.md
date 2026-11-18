# Phase 5 — Android app

**Proves:** the same code runs as an APK on the family's phones.

**Before starting:** decisions 19 and 22 were answered on 16 November 2026. Find out which family
phones are Android. iPhones cannot take an APK. Tauri's
mobile support is behind its desktop support; check the current plugin list for each feature below.

## Tasks

1. **Build the APK** from the same code, signed with our own key (`--apk`, split per ABI for smaller
   files). **(Martijn)** Back up that key: an update signed with another key will not install.
2. **Screens for a phone.** The same screens laid out for a small touch screen, by `../screens.md`.
   Test on a real phone; the web view differs from the one on Windows. On Android Tauri cannot tell
   a frame's requests from the window's, so the hostile HTML set must pass again here.
3. **Sign-in on Android.** Address and password as on Windows. For Gmail either an Android-type
   Google client (tied to package name and signing key, with its own redirect flow) or an app
   password. Pick the simpler one that works and record it.
4. **TLS.** `rustls-platform-verifier`'s Android part: the Kotlin component, the ProGuard rule and
   the init call with the app's context.
5. **Secrets.** Stored through Android's keystore (for example the `keyring` Android store).
6. **Search.** The same three levels and the same speed target as on Windows, measured on the
   slowest family phone.
7. **New mail in the background.** This needs a foreground service and is a separate design. Write
   a short proposal and ask Martijn before building it. Checking when the app opens is the baseline.
   The only Tauri plugin found for it is a young community one; weigh that in the proposal.
8. **Sharing and updates.** Android never installs an app update silently from outside a store.
   Build the "Bijwerken" button of decision 22: the app checks GitHub and opens Android's own
   install screen, and write the steps for the family. Error reports and log requests work as on Windows.
   Google's developer check reaches all countries in 2027; until then sideloading works as today.
   **(Martijn)** Before then, register the free limited-distribution account (two-step verification
   and a Google payments profile; up to 50 devices per Google's guide) and add each family phone.
   Without it, each phone needs Android's advanced flow once; if that is switched off later, updates
   fail.

9. **Contacts on the phone** (decision 19). "Telefoon koppelen" on the PC shows a QR code; the
   phone scans it, keeps the key in Android's keystore, and syncs contacts through the mailbox folder
   of `../sync.md`. "Koppeling verbreken" on the PC makes a new key. Test on a throwaway account.

10. **Mail kept on the PC, also on the phone** (decision 26, `../pc-to-phone.md`). The PC finds the
    paired phone on the home Wi-Fi with a sealed UDP broadcast and copies its local folders to it,
    by itself; the phone's copy follows the PC (moves, read, star, deletions). On the phone "Oude
    mail ophalen" offers only "Uit een ander postvak".

Added on 18 November 2026 (decision 29), for Windows and the phone alike unless said otherwise:

11. **"Ongewenste e-mail" (C8).** The reading-pane button that moves a message to the account's junk
    folder, and "Geen ongewenste e-mail" back to Postvak IN, with "Ongedaan maken".
12. **Invitations shown readably (C11).** What, when, where and who invites, above the text.
13. **Backup to a USB stick (C14), Windows only.** "Reservekopie maken" and "Reservekopie
    terugzetten" in Instellingen, with progress and "Stoppen"; no secrets in the backup.

## Exit criteria

- [ ] The APK installs on each Android phone in the inventory
- [ ] Reading, searching and sending work on a phone
- [x] The hostile HTML set passes on Android (emulator, 17 November 2026: nothing ran, loaded or
      reached the log servers; remote pictures only after "Afbeeldingen tonen"; a web link opens the
      browser, `mailto:` opens Write with address and subject)
- [ ] An update APK installs over the old one and keeps the accounts
- [ ] The Android signing key is backed up
- [ ] A contact added on the PC shows on the paired phone, and one changed on the phone shows on
      the PC
- [ ] Old mail kept only on the PC shows on the paired phone on the same Wi-Fi, without a firewall
      question on the PC (Martijn's PC and a family phone)
- [ ] A message marked "Ongewenste e-mail" lands in the junk folder on the server too, and back
      with "Geen ongewenste e-mail"
- [ ] An invitation from Outlook and one from Google Agenda show the right day, time and place
- [ ] A backup made on one PC is restored on another; the mail, labels, contacts and settings are
      there, and only the passwords are asked again
