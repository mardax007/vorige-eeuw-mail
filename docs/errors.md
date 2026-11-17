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
| `APP` | The app itself: an unexpected internal error |

| Stage | Meaning |
| --- | --- |
| `FIND` | Finding the server |
| `CONNECT` | Connecting |
| `TLS` | Securing the connection |
| `LOGIN` | Signing in |
| `CMD` | The command itself |

Areas without a network connection (`IMPORT`, `STORE`) use a short stage name of their own:
`READ` (reading a file), `WRITE` (writing the index or a file), `SPACE` (not enough room on the disk
or in the mailbox) and `SECRET` (the encrypted password file). The area `APP` is the app itself
(an unexpected internal error).

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
  status codes (see `research.md`). That is why `vem-core` has its own SMTP client, which keeps
  every reply line as sent; the IMAP client records the raw response lines the same way.
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
| `VEM-LOOKUP-FIND-01` | Voor dit e-mailadres zijn geen instellingen gevonden. | Controleer of het adres goed gespeld is. Klopt het, vraag Martijn dan om de instellingen met de hand in te vullen. | phases 1-4 |
| `VEM-IMAP-FIND-01` | De computer van uw e-mailprovider is niet gevonden. | Controleer of uw internet werkt. Werkt dat wel, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-IMAP-CONNECT-01` | Er kon geen verbinding gemaakt worden met uw postvak. | Controleer of uw internet werkt en probeer het straks opnieuw. | phases 1-4 |
| `VEM-IMAP-CONNECT-02` | Uw postvak gaf niet op tijd antwoord. | Wacht even; het programma probeert het vanzelf opnieuw. | phases 1-4 |
| `VEM-IMAP-TLS-01` | De beveiligde verbinding met uw postvak is niet gelukt. | Probeer het straks opnieuw. Blijft dit komen, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-IMAP-TLS-02` | Uw postvak biedt geen beveiligde verbinding aan, dus het programma is gestopt om uw wachtwoord te beschermen. | Vraag het aan Martijn. | phases 1-4 |
| `VEM-IMAP-LOGIN-01` | Inloggen bij uw postvak is niet gelukt. | Controleer het wachtwoord en probeer het opnieuw. Hebt u het bij uw provider veranderd, gebruik dan het nieuwe. | phases 1-4 |
| `VEM-IMAP-LOGIN-02` | Uw provider laat dit programma niet binnen met het gewone wachtwoord. | Gebruik Inloggen met Google, of vraag Martijn om een app-wachtwoord te maken. | phases 1-4 |
| `VEM-IMAP-CMD-01` | Uw postvak weigerde wat het programma vroeg. | Probeer het nog eens. Blijft dit komen, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-IMAP-CMD-02` | De verbinding met uw postvak werd verbroken. | U hoeft niets te doen; het programma probeert het vanzelf opnieuw. | phases 1-4 |
| `VEM-SMTP-FIND-01` | De computer die uw mail verstuurt is niet gevonden. | Controleer of uw internet werkt. Het bericht blijft in Postvak UIT staan. | phases 1-4 |
| `VEM-SMTP-CONNECT-01` | Er kon geen verbinding gemaakt worden om uw mail te versturen. | Controleer of uw internet werkt. Het bericht blijft in Postvak UIT staan en gaat later. | phases 1-4 |
| `VEM-SMTP-TLS-01` | De beveiligde verbinding om uw mail te versturen is niet gelukt. | Probeer het straks opnieuw. Het bericht blijft in Postvak UIT staan. | phases 1-4 |
| `VEM-SMTP-TLS-02` | Uw provider biedt geen beveiligde verbinding om te versturen, dus het programma is gestopt om uw wachtwoord te beschermen. | Vraag het aan Martijn. Het bericht blijft in Postvak UIT staan. | phases 1-4 |
| `VEM-SMTP-LOGIN-01` | Inloggen om mail te versturen is niet gelukt. | Controleer het wachtwoord van dit postvak bij Instellingen. Het bericht blijft in Postvak UIT staan. | phases 1-4 |
| `VEM-SMTP-CMD-01` | Een van de adressen werd geweigerd. | Controleer het adres van de ontvanger, haal het bericht terug en verstuur het opnieuw. | phases 1-4 |
| `VEM-SMTP-CMD-02` | Het bericht is te groot om te versturen. | Haal een bijlage weg, of zet een vinkje bij Foto's kleiner maken, en verstuur het opnieuw. | phases 1-4 |
| `VEM-SMTP-CMD-03` | Uw provider wilde het bericht niet versturen. | Probeer het straks opnieuw. Blijft dit komen, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-AUTH-LOGIN-01` | Inloggen met Google of Microsoft is niet afgemaakt. | Probeer het opnieuw en maak het inloggen in het venster helemaal af. | phases 1-4 |
| `VEM-AUTH-LOGIN-02` | Uw inlog is verlopen. | Log opnieuw in bij Instellingen, bij dit postvak. | phases 1-4 |
| `VEM-IMPORT-READ-01` | Het bestand met oude mail kan niet gelezen worden. | Kies het bestand opnieuw. Lukt het niet, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-IMPORT-READ-02` | Het bestand met oude mail is nog in gebruik. | Sluit Outlook en probeer het opnieuw. | phases 1-4 |
| `VEM-IMPORT-READ-03` | Dit soort bestand kent het programma niet. | Kies een ander bestand, of vraag het aan Martijn. | phases 1-4 |
| `VEM-IMPORT-READ-04` | Dit bestand is een kopie die Outlook van een postvak bijhoudt. Die kan het programma niet lezen. | Voeg dat postvak hier toe met uw e-mailadres. Dan komt de mail vanzelf binnen. | phases 1-4 |
| `VEM-IMPORT-READ-05` | Dit Outlook-bestand heeft een wachtwoord, en dat wachtwoord klopt niet. | Typ het wachtwoord van het bestand opnieuw. Weet u het niet meer, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-IMPORT-SPACE-01` | Er is niet genoeg ruimte op deze computer om de oude mail op te halen. | Maak ruimte vrij, bijvoorbeeld bij Originele bestanden, en probeer het opnieuw. | phases 1-4 |
| `VEM-IMPORT-SPACE-02` | Er is niet genoeg ruimte in uw postvak voor de oude mail. | Kies Op deze computer bewaren. | phases 1-4 |
| `VEM-STORE-WRITE-01` | Het programma kon niets opslaan op deze computer. | Misschien is de schijf vol. Maak ruimte vrij en probeer het opnieuw. | phases 1-4 |
| `VEM-STORE-READ-01` | Bewaarde mail kon niet gelezen worden. | Sluit het programma met Afsluiten en open het opnieuw. Blijft dit komen, vraag het dan aan Martijn. | phases 1-4 |
| `VEM-STORE-SECRET-01` | Het bewaarde wachtwoord kon niet gelezen worden. | Typ het wachtwoord van dit postvak opnieuw in. | phases 1-4 |
| `VEM-UPDATE-CMD-01` | Het bijwerken van het programma is niet gelukt. | U hoeft niets te doen; het programma probeert het later opnieuw. | phases 1-4 |
| `VEM-UPDATE-GET-01` | De nieuwe versie kon niet opgehaald worden. | Probeer het later nog eens met Bijwerken. Lukt het steeds niet, vraag het dan aan Martijn. | phase 5 (Android) |
| `VEM-REPORT-CMD-01` | Uw bericht aan Martijn kon niet verstuurd worden. | Probeer het straks opnieuw. | phases 1-4 |
| `VEM-APP-CMD-01` | Er ging iets onverwachts mis. | Probeer het nog eens. Blijft dit komen, vraag het dan aan Martijn. | phases 1-4 |
