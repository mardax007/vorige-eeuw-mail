# Phase 4 — Hand it out

**Proves:** a family member can install the app once, and from then on every update reaches that
PC by itself, without anyone touching it.

**Before starting:** decisions 4 and 14 were answered on 16 November 2026 (public repository,
GPL-3.0, per-user install, unsigned for now). Read `../PLAN.md` section 6.

## Tasks

1. **Installer.** A Windows installer: NSIS, per user, no admin rights. Unsigned for now
   (decision 14); per user so that updates stay silent.
2. **GitHub Actions.** A workflow on a Windows runner that builds the installer with Tauri's own
   GitHub action, signs the update with the updater key and creates a *draft* release with the
   installer and `latest.json`. The updater key, its password and the report address are GitHub
   Actions secrets. Check the current docs of the action
   and the updater first.
3. **Silent updates.** The updater checks at start and every few hours, downloads in the background
   and installs in `quiet` mode only when the app is idle (no message being written, no import or
   send running). The app restarts where it was and shows "Bijgewerkt" for a day,
   with a "Wat is er nieuw?" link to two or three sentences Martijn writes in the release notes. A failed update is
   reported to Martijn (`VEM-UPDATE-…`).
4. **Log requests.** Move `log-requests.json` from the test repository to the public repository.
5. **Release steps.** Write `../release.md`: version number, run the workflow, install the draft on
   Martijn's own PC, use it for a day, then publish. Short enough to follow half a year later.
6. **Install guide.** One page with a screenshot of each warning and what to click: the "Meer
   informatie", then "Toch uitvoeren" screen. If unsigned: how to check that Smart App Control is off
   on Windows 11, and that it must stay off while the app is used, or updates will be blocked without
   a word. Windows 10 has no Smart App Control.
7. **Help pages.** One public page per `VEM-…` code, generated from the registry in `../errors.md`.
   One page on remote help: only Martijn ever asks for a Quick Assist code.
8. **Backups. (Martijn)** The updater's private key and its password, and the Google Cloud project
   login, are stored somewhere safe outside this folder and outside GitHub. Tick it off in
   `../PROGRESS.md`.

## Exit criteria

- [ ] A family member installs it from the guide alone
- [ ] A release published by Martijn installs on that PC while nobody touches it, and the app
      reopens where it was
- [ ] No update installs while a message is being written
- [ ] A release is built and signed by GitHub Actions, with no key on any PC except the backup
- [ ] Searching for a `VEM-…` code leads to its help page
- [ ] The key backups exist and have been tested by restoring one
