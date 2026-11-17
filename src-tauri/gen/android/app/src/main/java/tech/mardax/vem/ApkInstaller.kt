package tech.mardax.vem

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import java.io.File

/**
 * "Bijwerken" (phase 5 task 8): opens Android's own install screen for the update APK that the
 * Rust side downloaded and checked against the update key. Called from Rust through JNI
 * (src-tauri/src/updates.rs), so ProGuard keeps it (proguard-rules.pro).
 */
object ApkInstaller {
  @JvmStatic
  fun install(context: Context, path: String) {
    val uri = FileProvider.getUriForFile(context, context.packageName + ".fileprovider", File(path))
    val intent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    context.startActivity(intent)
  }
}
