# Releasing a new version

Short on purpose: follow it top to bottom, also half a year from now.

## Once, before the first release (Martijn)

1. **Updater key.** On your own PC, in a terminal in this folder:

       npm run tauri signer generate -- -w %USERPROFILE%\.tauri\vem-updater.key

   Choose a password. Keep the key file and the password in your backup (phase 4 task 8), never in
   this folder or in git.
2. **GitHub Actions secrets** (repository > Settings > Secrets and variables > Actions):

   | Secret | Value |
   | --- | --- |
   | `TAURI_SIGNING_PRIVATE_KEY` | the contents of `vem-updater.key` |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |
   | `VEM_UPDATER_PUBKEY` | the contents of `vem-updater.key.pub` |
   | `VEM_SUPPORT_ADDRESS` | the report address (decision 21) |
   | `VEM_GOOGLE_CLIENT_ID`, `VEM_GOOGLE_CLIENT_SECRET` | from the Google Cloud project ("Desktop app" client) |
   | `VEM_MICROSOFT_CLIENT_ID` | from the Microsoft Entra app registration (public client, redirect `http://localhost`) |
   | `ANDROID_KEY_BASE64` | the Android signing key (step 3), as one line of base64 |
   | `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` | its alias and password |

   A secret left out switches that feature off in the build; nothing breaks. The Android signing
   key is the exception: without it the Android job fails, because an unsigned APK cannot be
   installed.
3. **Android signing key.** Every Android update must carry the same key as the first install, or
   the phone refuses it; a lost key means every family phone has to uninstall and set up again.
   On your own PC (Java is installed):

       keytool -genkey -v -keystore %USERPROFILE%\.android\vem-upload.jks -keyalg RSA -keysize 2048 -validity 10000 -alias vem

   Keep the `.jks` file and its password in your backup next to the updater key, never in this
   folder or in git. For the secret, in PowerShell:

       [Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:USERPROFILE\.android\vem-upload.jks"))

   To build a signed APK on your own PC, put a `keystore.properties` in `src-tauri/gen/android`
   with `keyAlias=vem`, `password=…` and `storeFile=C:/Users/…/.android/vem-upload.jks` (forward slashes; git ignores the file).
4. **`log-requests.json`** in the root of the main branch, with `{"requests": []}`. The app reads it
   from `raw.githubusercontent.com` every six hours (`PLAN.md` section 5).

## Every release

1. **Version number: nothing to do.** The workflow numbers every build itself (decision 28): the
   first two parts come from the repository (now 0.1), the last is the workflow's run number, so
   each run is higher than the one before and installed apps always take it as an update. The
   draft is called `v0.1.<run number>`. For a bigger step, raise the first two parts by hand in
   `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `crates/vem-core/Cargo.toml` (`node scripts/set-version.mjs 0.2.0` does all four); never lower
   them. The last part stays under 1000 (Android's version code).
   **What is new:** replace the lines in `release-notes.md` with two or three plain Dutch
   sentences, one per line. They appear in the app behind "Wat is er nieuw?" (on the PC for a day
   after the update, on the phone in the "Bijwerken" bar). Commit and push to `main`. The "Tests"
   workflow must be green.
2. **Run the workflow.** GitHub > Actions > "Release" > "Run workflow". It runs the tests, builds the
   installer and the APK, signs both updates and makes a **draft** release `v<version>` with the
   installer, `latest.json`, `vorige-eeuw-mail.apk` and `android.json`. A draft is invisible to the
   family's apps. Editing the draft's text changes nothing in the apps: they read what is new from
   `latest.json` and `android.json`, which the workflow wrote from `release-notes.md`.
3. **Install the draft on your own PC.** Download the `…-setup.exe` from the draft and run it over
   your installed version. Check that it starts, shows your mail, sends one message, and that
   Instellingen shows the new version.
4. **Install the draft on your own phone.** Download `vorige-eeuw-mail.apk` from the draft on the
   phone and install it over your installed version. The same checks.
5. **Use it for a day.**
6. **Publish.** Edit the draft and press "Publish release". Within six hours every family PC
   downloads it, waits until nobody is writing, importing or sending, installs quietly and reopens
   where it was. Family phones show "Er is een nieuwe versie" with **Bijwerken** within six hours
   of the app being open; Android itself asks them to confirm (`install.md`, phone part).

Never publish a release that was not first installed on your own PC (CLAUDE.md, Updates).

## When something goes wrong

- **A bad version is out.** Release a fixed version with a higher number. Deleting the release only
  stops PCs that have not updated yet.
- **The workflow fails at signing.** Check that both `TAURI_SIGNING_…` secrets are set and that the
  password matches the key.
- **The Android job fails at signing.** Check the three `ANDROID_KEY_…` secrets. The job looks for
  `…-release.apk`; without the key Gradle makes `…-release-unsigned.apk` and the job stops.
- **"Bijwerken" fails on a phone.** The report shows `VEM-UPDATE-GET-01` with the reason: no
  connection, a signature that does not match (`android.json` from another build than the APK), or
  Android's installer could not be opened.
- **An update fails on a family PC.** You get a report with `VEM-UPDATE-CMD-01` and the reason.
- **Smart App Control** on Windows 11 blocks unsigned updates without a word; it must stay off
  (install guide).
