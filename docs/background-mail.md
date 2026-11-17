# New mail in the background on Android (proposal, phase 5 task 7)

Written 17 November 2026 for Martijn. Nothing here is built yet; open question 24 in
`DECISIONS.md` asks which option to build.

## What the app does today

On Windows the app keeps IMAP IDLE open on Postvak IN and shows new mail within seconds, also with
the window closed (icon next to the clock). On Android the same code runs only while the app is on
screen or was just left: Android stops a background app's network within minutes and kills the
process soon after. When the app is opened again it fetches at once. That is the baseline the phase
file names: **new mail shows when the app is opened**, not before. There is no notification.

## What Android allows (checked 17 November 2026)

- **A foreground service** keeps the process and its network alive, with a fixed notification
  ("Vorige Eeuw Mail kijkt naar nieuwe mail") that stays as long as the service runs. Every
  foreground service needs a *type*:
  - `dataSync`: since Android 15 limited to **6 hours per 24 hours**, and it may not start at boot.
    Not usable for "always".
  - `remoteMessaging`: meant for chat apps that relay messages; a mail client is a poor fit for
    Google Play's review, though we are not on Play.
  - `specialUse`: no time limit; only Google Play asks for a written reason. FairEmail (a widely
    used open-source mail app) uses `specialUse` for exactly this; Thunderbird for Android uses
    `dataSync` plus `specialUse`.
- **IDLE needs renewing** every 29 minutes at most (RFC 2177). A phone asleep in Doze delays
  ordinary timers for hours, so renewing needs an **exact alarm** (`SCHEDULE_EXACT_ALARM`, which the
  person allows once in settings, or `USE_EXACT_ALARM`, which Google restricts to alarm and
  calendar apps on Play).
- **Battery optimisation**: several makers (Samsung, Xiaomi, Huawei, OnePlus) stop foreground
  services anyway unless the app is excluded from battery optimisation; the app can open that
  setting (`REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`), the person has to say yes. dontkillmyapp.com
  lists the extra steps per maker.
- **WorkManager** (periodic check without a service): at least every 15 minutes, and Android
  decides when. An app used rarely lands in the "rare" bucket: about 10 minutes of work per day and
  sometimes no network. Fine as "check now and then", not for "within a minute".
- **Push from a server** (Firebase) would be the normal route, but needs a server of ours that
  watches the mailbox; that breaks "no server of ours" and "mail goes only to the user's provider".
  Not an option.

## The Tauri side

- Tauri 2 has no official plugin for background work. The one community plugin found
  (`tauri-plugin-background-service` 1.0.1) is young, has one maintainer, defaults to the
  `remoteMessaging` type and cannot restart without the app's screen. Not recommended.
- Known Tauri issue (#15671): when a foreground service keeps the process alive after the screen is
  closed, reopening the app can show a white page. Our app already keeps the process on purpose
  (`prevent_exit`), so this must be tested; a fix may need the service in its own process
  (`android:process=":sync"`) with the SQLite index in WAL mode, which is untested.

## Options

1. **Keep the baseline, add a periodic check (WorkManager, about every 15 minutes, as Android
   allows).** A notification "Nieuwe mail van …" when something arrived. No fixed notification, no
   battery settings. Mail can be late by 15 minutes to hours on a phone that is rarely used.
   About **2 days**.
2. **An own `specialUse` foreground service that keeps IDLE open, switched on per phone**
   ("Nieuwe mail direct melden" in Instellingen, off by default), with exact-alarm renewal, start
   after reboot (`RECEIVE_BOOT_COMPLETED`) and the battery-optimisation question once. New mail within
   a minute, like on Windows, at the cost of a permanent notification and some battery. Written in
   Kotlin next to `ApkInstaller.kt`, calling the Rust core through the existing JNI route.
   About **4 to 6 days**, mostly testing on real phones of different makers.
3. **Nothing extra**: new mail shows when the app is opened, as today. The family opens the app to
   look anyway; nothing to explain or to break.

## Recommendation

Option 2 if anyone in the family expects the phone to ping on new mail; otherwise option 3 now and
option 1 later if it is missed. Option 2 must be tried on each family phone's make before it is
handed out, because maker battery rules decide whether it works.

Permissions option 2 adds: `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE`,
`POST_NOTIFICATIONS`, `RECEIVE_BOOT_COMPLETED`, `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`,
`SCHEDULE_EXACT_ALARM`. Option 1 adds only `POST_NOTIFICATIONS`.

## Sources

- developer.android.com: foreground service types; foreground service timeouts; behavior changes
  in Android 14, 15 and 17; power management (Doze, app standby buckets); exact alarms.
- FairEmail's and Thunderbird for Android's `AndroidManifest.xml` (GitHub).
- Tauri issues #15671 and #11609; crates.io `tauri-plugin-background-service`.
- dontkillmyapp.com.
