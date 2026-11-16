# Progress

Claude Code updates this file at the end of every session.

## Now

- **Phase:** 0 done: every question in `DECISIONS.md` is answered (decision 6 is made on the spike
  report).
- **Next:** phase 1, `phases/1-spike.md`. Martijn said to go on with it on 16 November 2026. Start
  with task 1, the toolchain check (Rust is not installed yet), then task 2 (git and the scaffold).
- **Martijn, when convenient:** make sure the report address receives mail at his domain, and put
  it in the GitHub Actions secret `VEM_SUPPORT_ADDRESS` when the repository exists (spike task 2).
  Fill in `family-inventory.md` where people know the answers; it no longer blocks anything.

## State of the folder

- Docs only. No code, no git repository yet.
- Toolchain seen on 16 November 2026: Node 24, git, Docker with WSL 2, Java, GitHub CLI. Rust not
  installed.

## Session log

| Date | What was done | What was learned | Next |
| --- | --- | --- | --- |
| 2026-11-16 | Folder set up: CLAUDE.md, plan, decisions, phase files | — | Inventory, then the spike |
| 2026-11-16 | Research pass over the whole plan (five parallel research runs). Rewrote `PLAN.md` and every phase file except 6; added `research.md`, `sync.md`, `screens.md`; decisions 14–18; Martijn's search requirement added; then tags and contacts added to version 1 with `features.md` (candidate list C1–C14); a consistency review then fixed 30 issues, among them read-only spike access to real mailboxes (EXAMINE, BODY.PEEK), a throwaway-account spike task for anything that writes, and link handling without IPC | Solcon has no autoconfig, and its SMTP host and username differ; its mailboxes are 2–5 GB. Antivirus TLS scanning breaks clients with their own root list. `outlook-pst` gives raw MAPI only and has data-loss bugs; MAPI-to-MIME is ours to build. WebView2 puts Tauri's scripts in every frame. New Outlook can export PST. Smart App Control can be switched back on since April 2026. KPN is moving mailboxes from today | Inventory, decisions, then the spike |
| 2026-11-16 | Martijn accepted all proposals. Recorded decisions 2 (React + TypeScript + Vite), 5 (Dutch), 10, 13, 17 (DPAPI), 20 (C1–C4, C6, C7, plus C12). Designed and added: error reports by email, remote log requests through a `log-requests.json` file next to the releases, the "Wens of idee" button (`PLAN.md` section 5); silent updates through GitHub Actions and Releases with a draft tested on Martijn's PC (section 6); Windows notifications, the icon next to the clock and starting with Windows; Outlook as reference point with its Dutch words (`screens.md`). Updated `CLAUDE.md` hard rules (secrets, network, updates, Dutch), `features.md`, `errors.md` (Dutch, `REPORT` area) and phases 1, 2a, 2b, 4, 5. New questions 21 (report address) and 22 (Android updates) | Releases of a private GitHub repository need a login, so silent updates need a public repository for the release files. A silent update of an unsigned app is blocked where Smart App Control is on, which makes signing (decision 14) worth more. Windows shows notifications properly only for an installed app. Android cannot update an app silently outside a store | Inventory, questions 1, 3, 4, 8, then the spike |
| 2026-11-16 | Martijn answered every open question: support as many providers as possible, identifier `tech.mardax.vem`, public repository, GPL-3.0, per-user install, Google sign-in yes, full offline copy, DBX import yes, no import over the internet (phase 6 dropped), old mail kept locally with a copy of the original file and a menu to delete it, mailbox space shown, contacts on the phone through QR pairing and the user's own mailbox (Firebase considered, not chosen), report address set (kept out of the repository), Android "Bijwerken" button. Applied hard-rule changes P1–P3. Updated DECISIONS, PLAN, features, sync, errors, phases 1–6, CLAUDE.md | An admin install would break silent updates (UAC at every update). The report address must stay out of the public repository but is readable from the installer | Martijn's go for phase 1 |

## Ideas parked

Things that came up but are not in the current phase. Do not build them without asking.

- A per-sender "always show images" list, as Velo does.
- Substring search (trigram index) on subject and sender, if word search turns out not to be enough.
- QRESYNC through Pimalaya's `io-imap`, once its API settles.
- A KPN entry in the provider table, once KPN publishes the Mail en Cloud server names.
- Automatic tagging rules (left out of version 1 on purpose; see `features.md`, "Out").
- Read aloud (C5), "This is spam" (C8), photo view (C9), conversations (C10), readable invites
  (C11), backup of everything (C14): candidates not chosen yet.
