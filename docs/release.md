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

   A secret left out switches that feature off in the build; nothing breaks.
3. **`log-requests.json`** in the root of the main branch, with `{"requests": []}`. The app reads it
   from `raw.githubusercontent.com` every six hours (`PLAN.md` section 5).

## Every release

1. **Version number.** Raise it everywhere, the same number: `package.json`,
   `src-tauri/tauri.conf.json` (`version`), `src-tauri/Cargo.toml` and `crates/vem-core/Cargo.toml`.
   It must go up (0.1.0, 0.1.1, 0.2.0 …): apps never install a lower number. Commit and push to
   `main`. The "Tests" workflow must be green.
2. **Run the workflow.** GitHub > Actions > "Release" > "Run workflow". It runs the tests, builds the
   installer, signs the update and makes a **draft** release `v<version>` with the installer and
   `latest.json`. A draft is invisible to the family's apps.
3. **What is new.** Edit the draft's text: two or three plain Dutch sentences, one per line. They
   appear in the app behind "Wat is er nieuw?" for a day after the update.
4. **Install the draft on your own PC.** Download the `…-setup.exe` from the draft and run it over
   your installed version. Check that it starts, shows your mail, sends one message, and that
   Instellingen shows the new version.
5. **Use it for a day.**
6. **Publish.** Edit the draft and press "Publish release". Within six hours every family PC
   downloads it, waits until nobody is writing, importing or sending, installs quietly and reopens
   where it was.

Never publish a release that was not first installed on your own PC (CLAUDE.md, Updates).

## When something goes wrong

- **A bad version is out.** Release a fixed version with a higher number. Deleting the release only
  stops PCs that have not updated yet.
- **The workflow fails at signing.** Check that both `TAURI_SIGNING_…` secrets are set and that the
  password matches the key.
- **An update fails on a family PC.** You get a report with `VEM-UPDATE-CMD-01` and the reason.
- **Smart App Control** on Windows 11 blocks unsigned updates without a word; it must stay off
  (install guide).
