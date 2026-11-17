package tech.mardax.vem

import android.content.Context

/**
 * The Rust side of "Nieuwe mail direct melden" (decision 24, src-tauri/src/android_mail.rs).
 * The app's screen says when it comes and goes; the service in the `:mail` process starts the
 * headless mail core and wakes it on the alarm.
 */
object MailCore {
  init {
    System.loadLibrary("vem_lib")
  }

  /** The app's screen is visible: the app takes the mailboxes over from the service. */
  @JvmStatic external fun uiForeground()

  /** The app's screen went away: the service may take over, when it is on. */
  @JvmStatic external fun uiBackground()

  /** Starts the core in the service's process once; returns the shell texts as JSON. */
  @JvmStatic external fun serviceStart(context: Context, dataDir: String): String

  /** Look at the mail server now (the alarm). */
  @JvmStatic external fun serviceWake()
}
