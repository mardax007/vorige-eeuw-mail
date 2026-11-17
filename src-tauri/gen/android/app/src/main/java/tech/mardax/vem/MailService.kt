package tech.mardax.vem

import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.provider.Settings
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject
import java.io.File

/**
 * "Nieuwe mail direct melden" (decision 24): a foreground service of type `specialUse` in its
 * own process (`:mail`) that keeps the connection to the mail provider open while the app's
 * screen is closed, with a permanent notification. The mail work itself is the Rust core
 * (src-tauri/src/android_mail.rs); this class only keeps Android from stopping it.
 *
 * Rust calls [apply] and [showNewMail] through JNI, so ProGuard keeps this class.
 */
class MailService : Service() {
  companion object {
    private const val TAG = "VemMail"
    private const val WATCHING_ID = 1
    private const val CHANNEL_WATCHING = "watching"
    private const val CHANNEL_NEW_MAIL = "new_mail"
    /** Written and removed by Rust when the switch changes; read here before Rust runs. */
    private const val FLAG_FILE = "background-mail"
    /** The alarm that wakes a sleeping phone to look at the server. Android allows a
     * while-idle alarm about every 9 minutes at most in Doze; 15 is enough for mail. */
    private const val WAKE_EVERY_MS = 15 * 60 * 1000L

    /** True while the service runs in this process (the receivers share the process). */
    @Volatile var running = false
      private set

    /** The shell texts from Rust; the app name until they arrive. */
    @Volatile private var texts = JSONObject()

    /** The same folder Tauri uses for the app's data: app_local_data_dir is the app's own data
     * folder itself on Android (seen on the emulator, 17 November 2026). */
    fun dataDir(context: Context): File = context.dataDir

    fun isOn(context: Context): Boolean = File(dataDir(context), FLAG_FILE).exists()

    /** Starts the service, or makes a running one show itself again. */
    fun start(context: Context) {
      try {
        ContextCompat.startForegroundService(context, Intent(context, MailService::class.java))
      } catch (e: Exception) {
        // Not allowed from the background without an exemption; the next open of the app
        // starts it again.
        Log.w(TAG, "start refused: $e")
      }
    }

    /** The switch in Instellingen changed (from Rust, while the app is on screen). */
    @JvmStatic
    fun apply(context: Context, on: Boolean) {
      if (on) {
        start(context)
        askBatteryExemption(context)
      } else {
        cancelAlarm(context)
        context.stopService(Intent(context, MailService::class.java))
      }
    }

    /** Android's own question whether the app may keep working in the background. Without a
     * yes several makers stop the service anyway (dontkillmyapp.com). */
    private fun askBatteryExemption(context: Context) {
      val power = context.getSystemService(PowerManager::class.java)
      if (power.isIgnoringBatteryOptimizations(context.packageName)) return
      try {
        val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:" + context.packageName))
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
      } catch (e: Exception) {
        Log.w(TAG, "battery question not shown: $e")
      }
    }

    /** New mail found by the service (from Rust). Tapping it opens the app. */
    @JvmStatic
    fun showNewMail(context: Context, title: String, body: String) {
      ensureChannels(context)
      val n = NotificationCompat.Builder(context, CHANNEL_NEW_MAIL)
        .setSmallIcon(android.R.drawable.ic_dialog_email)
        .setContentTitle(title)
        .setContentText(body)
        .setStyle(NotificationCompat.BigTextStyle().bigText(body))
        .setContentIntent(openApp(context))
        .setAutoCancel(true)
        .setCategory(NotificationCompat.CATEGORY_EMAIL)
        .build()
      try {
        NotificationManagerCompat.from(context).notify((SystemClock.elapsedRealtime() % Int.MAX_VALUE).toInt() + 2, n)
      } catch (e: SecurityException) {
        // Notifications are not allowed on this phone; nothing to show.
        Log.w(TAG, "no permission for notifications")
      }
    }

    private fun text(key: String, context: Context): String =
      texts.optString(key).ifEmpty { context.getString(R.string.app_name) }

    private fun ensureChannels(context: Context) {
      val manager = context.getSystemService(NotificationManager::class.java)
      // Creating a channel again only updates its name, so this is safe on every start.
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL_WATCHING, text("watchingChannel", context), NotificationManager.IMPORTANCE_MIN)
      )
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL_NEW_MAIL, text("newMailChannel", context), NotificationManager.IMPORTANCE_DEFAULT)
      )
    }

    private fun openApp(context: Context): PendingIntent {
      val intent = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      return PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    private fun alarmIntent(context: Context): PendingIntent =
      PendingIntent.getBroadcast(
        context,
        0,
        Intent(context, MailAlarm::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
      )

    /** The next wake, also in Doze. Inexact: no exact-alarm permission is needed. */
    fun scheduleAlarm(context: Context) {
      val alarms = context.getSystemService(AlarmManager::class.java)
      alarms.setAndAllowWhileIdle(
        AlarmManager.ELAPSED_REALTIME_WAKEUP,
        SystemClock.elapsedRealtime() + WAKE_EVERY_MS,
        alarmIntent(context)
      )
    }

    private fun cancelAlarm(context: Context) {
      context.getSystemService(AlarmManager::class.java).cancel(alarmIntent(context))
    }
  }

  private val main = Handler(Looper.getMainLooper())

  override fun onCreate() {
    super.onCreate()
    running = true
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    // Android wants startForeground soon after every start; the core starts after it.
    ensureChannels(this)
    showWatching()
    // A restart by Android (START_STICKY) or a start that crossed "uit": the switch decides.
    if (!isOn(this)) {
      stopSelf()
      return START_NOT_STICKY
    }
    val dir = dataDir(this).absolutePath
    val app = applicationContext
    Thread {
      val json = try {
        JSONObject(MailCore.serviceStart(app, dir))
      } catch (e: Throwable) {
        Log.w(TAG, "core not started: $e")
        null
      }
      if (json != null) {
        main.post {
          texts = json
          ensureChannels(this)
          showWatching()
        }
      }
    }.start()
    scheduleAlarm(this)
    return START_STICKY
  }

  private fun showWatching() {
    val n = NotificationCompat.Builder(this, CHANNEL_WATCHING)
      .setSmallIcon(android.R.drawable.ic_dialog_email)
      .setContentTitle(text("watching", this))
      .setContentIntent(openApp(this))
      .setOngoing(true)
      .setSilent(true)
      .setCategory(NotificationCompat.CATEGORY_SERVICE)
      .build()
    val type = if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE else 0
    ServiceCompat.startForeground(this, WATCHING_ID, n, type)
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onDestroy() {
    running = false
    super.onDestroy()
    // The Rust core runs threads of its own; ending the process ends them and releases its
    // turn on the mailboxes for the app.
    android.os.Process.killProcess(android.os.Process.myPid())
  }
}
