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

## Exit criteria

- [ ] The APK installs on each Android phone in the inventory
- [ ] Reading, searching and sending work on a phone
- [ ] The hostile HTML set passes on Android
- [ ] An update APK installs over the old one and keeps the accounts
- [ ] The Android signing key is backed up
- [ ] A contact added on the PC shows on the paired phone, and one changed on the phone shows on
      the PC
