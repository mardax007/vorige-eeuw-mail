# Errors and feedback

The goal: a family member can read what went wrong, and anyone helping can paste the error into a
search engine and find the cause.

## The rule

Every error shows four things:

1. **What happened**, in one plain sentence.
2. **What to do next.**
3. **The server's exact words**, in a box the user can copy. When no server was involved (the local
   store, an import file, a certificate check), the box shows the operating system's or library's
   own error text instead, labelled "Details", not "The server said". If there is no such text, the
   box is left out.
4. **A fixed app code.**

And:

- **Never rewrite the server's reply.** Strings such as
  `550 5.7.30 Basic authentication is not supported for Client Submission` are what search engines
  index. Show them character for character. Do not shorten or translate them.
- **Say which stage failed:** finding the server, connecting, securing the connection, signing in,
  or the command itself.
- **Map known causes to a fix.** A refused Gmail password becomes the Google sign-in button (if
  decision 8 built it), with the app-password steps as a fallback or as the only route. A refused
  Outlook password becomes the Microsoft sign-in button (if decision 9 built it); otherwise a plain
  sentence that this app cannot open Outlook.com mailboxes.
- **Report imports per item:** how many messages came in, how many were skipped and why, as a list
  the user can save.

## Code scheme

    VEM-<AREA>-<STAGE>-<NN>

| Area | Covers |
| --- | --- |
| `LOOKUP` | Finding server settings for an address |
| `IMAP` | Reading and syncing mail |
| `SMTP` | Sending mail |
| `AUTH` | The Google or Microsoft sign-in flow itself (browser, token request, token refresh) |
| `IMPORT` | Reading old mail and copying it in |
| `STORE` | The local index and message files |
| `UPDATE` | Checking for and installing updates |
| `REPORT` | Sending error reports, logs and wishes to Martijn, and reading log requests |

| Stage | Meaning |
| --- | --- |
| `FIND` | Finding the server |
| `CONNECT` | Connecting |
| `TLS` | Securing the connection |
| `LOGIN` | Signing in |
| `CMD` | The command itself |

Areas without a network connection (`IMPORT`, `STORE`) use a short stage name of their own, such as
`READ` or `WRITE`. Add it to the table above when it first appears.

Which code owns a failure: the protocol in use when it happened. A token refused by the IMAP server
is `IMAP-LOGIN`; `AUTH` covers only the sign-in flow before any mail server is involved. A host
name that does not resolve while connecting is `IMAP-FIND` or `SMTP-FIND`; `LOOKUP` covers only
finding the settings for an address. Settle this before the first code ships, because codes never
change afterwards.

Example of what the user sees (the app is in Dutch; the server reply is an illustration and stays
exactly as the server sent it, usually in English):

    Inloggen bij uw postvak is niet gelukt.
    Controleer het wachtwoord en probeer het opnieuw. Hebt u het bij uw provider veranderd, gebruik
    dan het nieuwe.

    De server zei:
    NO [AUTHENTICATIONFAILED] Authentication failed.        [Kopiëren]

    Code: VEM-IMAP-LOGIN-01
    Martijn krijgt hier vanzelf bericht van.                [Kopiëren voor Martijn]

(The line "Martijn krijgt hier vanzelf bericht van" appears only once phase 2b builds the reports.)

The screen labels are "De server zei" and, when no server was involved, "Details"
("Details" is the same word in Dutch).

## How to build it

- One error type in `vem-core` is the single source of the codes. A code is never reused or
  renumbered once a release has shipped it.
- The sentence and the next step live in the message catalogue, once per language. The server reply
  is kept as raw text next to the code and is never passed through the catalogue.
- Capture that raw text from the wire, every line as sent. Library error messages are not the
  server's words: `lettre`, for one, joins multi-line SMTP replies with spaces and drops repeated
  status codes (see `research.md`). Spike task 5 finds how to keep the original.
- The error box has a "Kopiëren voor Martijn" button: sentence, server reply and code in one go,
  ready to paste into a message.
- Every error shown is also reported to Martijn by email (`PLAN.md` section 5), with the same code
  and the same raw reply, with one exception: in the emailed report and in the log, every address
  other than the sender's and Martijn's is replaced by `<adres>`, because the privacy rule in
  `CLAUDE.md` allows no other addresses there. Nothing else in the reply changes. On screen and in
  "Kopiëren voor Martijn" the reply stays exact. (Decided by Martijn on 16 November 2026, P1 in
  `DECISIONS.md`; the hard rule in `CLAUDE.md` says the same.) The report is the same for every language: code, raw reply, versions and
  log lines, so Martijn reads it the same way whatever the screen language.
- `REPORT` failures are not shown to the user (the user did not ask for the report): they are logged
  and retried. The one exception is the wish button, which the user pressed, so a failed wish is
  shown like any other error.
- Passwords and tokens never appear in an error, including inside a quoted command.
- Every code gets a row in the registry below when it is added.
- Later (phase 4): each code gets a public help page, so a search for the code lands on our
  explanation. They live on the public repository's GitHub Pages (decision 4).

## Registry

| Code | Sentence (Dutch) | Next step (Dutch) | Added in |
| --- | --- | --- | --- |
| | | | |
