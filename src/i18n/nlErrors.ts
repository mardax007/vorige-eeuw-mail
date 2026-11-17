// Error catalogue: one plain sentence and one next step per fixed code (docs/errors.md).
// The server's reply is never in here; it is shown next to this text exactly as sent.

export interface ErrorText {
  sentence: string;
  next: string;
}

export const nlErrors = {
  "VEM-LOOKUP-FIND-01": {
    sentence: "Voor dit e-mailadres zijn geen instellingen gevonden.",
    next: "Controleer of het adres goed gespeld is. Klopt het wel, vraag het dan aan Martijn.",
  },
  "VEM-IMAP-FIND-01": {
    sentence: "De computer van uw e-mailprovider is niet gevonden.",
    next: "Controleer of uw internet werkt. Werkt dat wel, vraag het dan aan Martijn.",
  },
  "VEM-IMAP-CONNECT-01": {
    sentence: "Er kon geen verbinding gemaakt worden met uw postvak.",
    next: "Controleer of uw internet werkt en probeer het straks opnieuw.",
  },
  "VEM-IMAP-CONNECT-02": {
    sentence: "Uw postvak gaf niet op tijd antwoord.",
    next: "Wacht even; het programma probeert het vanzelf opnieuw.",
  },
  "VEM-IMAP-TLS-01": {
    sentence: "De beveiligde verbinding met uw postvak is niet gelukt.",
    next: "Probeer het straks opnieuw. Blijft dit komen, vraag het dan aan Martijn.",
  },
  "VEM-IMAP-TLS-02": {
    sentence: "Uw postvak biedt geen beveiligde verbinding aan, dus het programma is gestopt om uw wachtwoord te beschermen.",
    next: "Vraag het aan Martijn.",
  },
  "VEM-IMAP-LOGIN-01": {
    sentence: "Inloggen bij uw postvak is niet gelukt.",
    next: "Controleer het wachtwoord en probeer het opnieuw. Hebt u het bij uw provider veranderd, gebruik dan het nieuwe.",
  },
  "VEM-IMAP-LOGIN-02": {
    sentence: "Uw provider laat dit programma niet binnen met het gewone wachtwoord.",
    next: "Gebruik Inloggen met Google, of vraag Martijn om een app-wachtwoord te maken.",
  },
  "VEM-IMAP-CMD-01": {
    sentence: "Uw postvak weigerde wat het programma vroeg.",
    next: "Probeer het nog eens. Blijft dit komen, vraag het dan aan Martijn.",
  },
  "VEM-IMAP-CMD-02": {
    sentence: "De verbinding met uw postvak werd verbroken.",
    next: "U hoeft niets te doen; het programma probeert het vanzelf opnieuw.",
  },
  "VEM-SMTP-FIND-01": {
    sentence: "De computer die uw mail verstuurt is niet gevonden.",
    next: "Controleer of uw internet werkt. Het bericht blijft in Postvak UIT staan.",
  },
  "VEM-SMTP-CONNECT-01": {
    sentence: "Er kon geen verbinding gemaakt worden om uw mail te versturen.",
    next: "Controleer of uw internet werkt. Het bericht blijft in Postvak UIT staan en gaat later.",
  },
  "VEM-SMTP-TLS-01": {
    sentence: "De beveiligde verbinding om uw mail te versturen is niet gelukt.",
    next: "Probeer het straks opnieuw. Het bericht blijft in Postvak UIT staan.",
  },
  "VEM-SMTP-TLS-02": {
    sentence: "Uw provider biedt geen beveiligde verbinding om te versturen, dus het programma is gestopt om uw wachtwoord te beschermen.",
    next: "Vraag het aan Martijn. Het bericht blijft in Postvak UIT staan.",
  },
  "VEM-SMTP-LOGIN-01": {
    sentence: "Inloggen om mail te versturen is niet gelukt.",
    next: "Typ bij Instellingen, bij dit postvak, het nieuwe wachtwoord in en druk op Wachtwoord opslaan. Het bericht blijft in Postvak UIT staan.",
  },
  "VEM-SMTP-CMD-01": {
    sentence: "Een van de adressen werd geweigerd.",
    next: "Controleer het adres van de ontvanger, haal het bericht terug en verstuur het opnieuw.",
  },
  "VEM-SMTP-CMD-02": {
    sentence: "Het bericht is te groot om te versturen.",
    next: "Haal een bijlage weg, of zet een vinkje bij Foto's kleiner maken, en verstuur het opnieuw.",
  },
  "VEM-SMTP-CMD-03": {
    sentence: "Uw provider wilde het bericht niet versturen.",
    next: "Probeer het straks opnieuw. Blijft dit komen, vraag het dan aan Martijn.",
  },
  "VEM-AUTH-LOGIN-01": {
    sentence: "Inloggen met Google of Microsoft is niet afgemaakt.",
    next: "Probeer het opnieuw en maak het inloggen in het venster helemaal af.",
  },
  "VEM-AUTH-LOGIN-02": {
    sentence: "Uw inlog is verlopen.",
    next: "Druk bij Instellingen, bij dit postvak, op Opnieuw inloggen.",
  },
  "VEM-IMPORT-READ-01": {
    sentence: "Het bestand met oude mail kan niet gelezen worden.",
    next: "Kies het bestand opnieuw. Lukt het niet, vraag het dan aan Martijn.",
  },
  "VEM-IMPORT-READ-02": {
    sentence: "Het bestand met oude mail is nog in gebruik.",
    next: "Sluit Outlook en probeer het opnieuw.",
  },
  "VEM-IMPORT-READ-03": {
    sentence: "Dit soort bestand kent het programma niet.",
    next: "Kies een ander bestand, of vraag het aan Martijn.",
  },
  "VEM-IMPORT-READ-04": {
    sentence: "Dit bestand is een kopie die Outlook van een postvak bijhoudt. Die kan het programma niet lezen.",
    next: "Voeg dat postvak hier toe met uw e-mailadres. Dan komt de mail vanzelf binnen.",
  },
  "VEM-IMPORT-READ-05": {
    sentence: "Dit Outlook-bestand heeft een wachtwoord, en dat wachtwoord klopt niet.",
    next: "Typ het wachtwoord van het bestand opnieuw. Weet u het niet meer, vraag het dan aan Martijn.",
  },
  "VEM-IMPORT-SPACE-01": {
    sentence: "Er is niet genoeg ruimte op deze computer om de oude mail op te halen.",
    next: "Maak ruimte vrij, bijvoorbeeld bij Originele bestanden, en probeer het opnieuw.",
  },
  "VEM-IMPORT-SPACE-02": {
    sentence: "Er is niet genoeg ruimte in uw postvak voor de oude mail.",
    next: "Onder Details staat hoe groot de oude mail is en hoeveel ruimte er nog is (oude mail / vrij). Kies bij Waar moet de mail heen? de eerste keuze (op dit apparaat) en druk opnieuw op Ophalen.",
  },
  "VEM-STORE-WRITE-01": {
    sentence: "Het programma kon niets opslaan op deze computer.",
    next: "Misschien is de schijf vol. Maak ruimte vrij en probeer het opnieuw.",
  },
  "VEM-STORE-READ-01": {
    sentence: "Bewaarde mail kon niet gelezen worden.",
    next: "Sluit het programma met Afsluiten (onderaan bij Instellingen) en open het opnieuw. Blijft dit komen, vraag het dan aan Martijn.",
  },
  "VEM-STORE-SECRET-01": {
    sentence: "Het bewaarde wachtwoord kon niet gelezen worden.",
    next: "Typ bij Instellingen, bij dit postvak, het wachtwoord opnieuw in en druk op Wachtwoord opslaan. Logt dit postvak in via Google of Microsoft, druk daar dan op Opnieuw inloggen.",
  },
  "VEM-UPDATE-CMD-01": {
    sentence: "Het bijwerken van het programma is niet gelukt.",
    next: "U hoeft niets te doen; het programma probeert het later opnieuw.",
  },
  "VEM-UPDATE-GET-01": {
    sentence: "De nieuwe versie kon niet opgehaald worden.",
    next: "Probeer het later nog eens met Bijwerken. Lukt het steeds niet, vraag het dan aan Martijn.",
  },
  "VEM-PAIR-READ-01": {
    sentence: "Deze code is geen koppelcode van Vorige Eeuw Mail.",
    next: "Kies op de computer Telefoon koppelen en Code maken, en scan die nieuwe code.",
  },
  "VEM-PAIR-FIND-01": {
    sentence: "Het postvak van deze code staat nog niet op deze telefoon.",
    next: "Voeg eerst op deze telefoon het postvak toe dat bij Details staat, en scan de code daarna opnieuw.",
  },
  "VEM-LAN-COPY-01": {
    sentence: "De mail van de computer kon niet naar de telefoon gekopieerd worden.",
    next: "U hoeft niets te doen; het programma probeert het straks opnieuw als de computer en de telefoon op hetzelfde wifi zitten.",
  },
  "VEM-LAN-SPACE-01": {
    sentence: "Er is niet genoeg ruimte op deze telefoon voor de mail van de computer.",
    next: "Maak ruimte vrij op de telefoon, bijvoorbeeld door oude foto's of apps te verwijderen. Daarna gaat het vanzelf verder.",
  },
  "VEM-REPORT-CMD-01": {
    sentence: "Uw bericht aan Martijn kon niet verstuurd worden.",
    next: "Probeer het straks opnieuw.",
  },
  "VEM-APP-CMD-01": {
    sentence: "Er ging iets onverwachts mis.",
    next: "Probeer het nog eens. Blijft dit komen, vraag het dan aan Martijn.",
  },
} as const satisfies Record<string, ErrorText>;

export type ErrorCode = keyof typeof nlErrors;

/** Every code the screens know, in catalogue order. */
export const ERROR_CODES = Object.keys(nlErrors) as ErrorCode[];
