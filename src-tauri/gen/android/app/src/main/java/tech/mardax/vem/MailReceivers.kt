package tech.mardax.vem

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.PowerManager

/** How long the phone stays awake after the alarm, so the core can look at the server. */
private const val WAKE_HOLD_MS = 60 * 1000L

/**
 * The alarm of "Nieuwe mail direct melden" (decision 24). A phone asleep in Doze holds back
 * timers and the mailbox connection may have gone quiet; every 15 minutes or so the service's
 * core looks at the server, and a service that Android stopped is started again.
 */
class MailAlarm : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (!MailService.isOn(context)) return
    val power = context.getSystemService(PowerManager::class.java)
    power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "vem:mail-alarm").acquire(WAKE_HOLD_MS)
    // The next alarm first: a start Android refuses must not end the chain.
    MailService.scheduleAlarm(context)
    if (MailService.running) {
      MailCore.serviceWake()
    } else {
      MailService.start(context)
    }
  }
}

/** Starts the service after a restart of the phone and after an update of the app. */
class MailBoot : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    val known = intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED
    if (known && MailService.isOn(context)) {
      MailService.start(context)
    }
  }
}
