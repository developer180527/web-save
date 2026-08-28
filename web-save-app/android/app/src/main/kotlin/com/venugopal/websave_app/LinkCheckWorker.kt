package com.venugopal.websave_app

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Checks due links in the background.
 *
 * Calls straight into Rust rather than standing up a Flutter engine. Cargokit
 * builds the bridge crate as a cdylib, so the .so is already inside the APK,
 * and a background worker has no engine of its own — starting one would cost
 * most of the window before a single link was checked.
 */
class LinkCheckWorker(context: Context, params: WorkerParameters) :
    Worker(context, params) {

    override fun doWork(): Result {
        // Must match the directory Dart opens. main.dart derives it from
        // path_provider's application support directory plus "vault", which on
        // Android is the app's files dir — resolved natively here because the
        // worker has no engine to ask, so the two have to agree by
        // construction.
        val vault = File(applicationContext.filesDir, "vault")

        // Nothing has run yet if the app has never been opened. Creating a
        // vault from here would leave an empty one for the app to adopt.
        if (!vault.exists()) return Result.success()

        return try {
            when (val checked = nativeRunDueChecks(vault.absolutePath)) {
                // Negative codes are failures; see background.rs. Retry rather
                // than failure, so WorkManager backs off and tries again
                // instead of dropping the periodic work.
                in Int.MIN_VALUE..-1 -> {
                    Log.w(TAG, "link check failed with code $checked")
                    Result.retry()
                }
                else -> Result.success()
            }
        } catch (e: UnsatisfiedLinkError) {
            // The JNI name is built from this package, class and method. If
            // any of them move without the Rust export moving too, this is
            // where it surfaces.
            Log.e(TAG, "native link check symbol missing", e)
            Result.failure()
        }
    }

    private external fun nativeRunDueChecks(vaultPath: String): Int

    companion object {
        private const val TAG = "WebSaveLinkCheck"
        private const val WORK_NAME = "websave-link-check"

        init {
            System.loadLibrary("websave_bridge")
        }

        /**
         * Ask for periodic checks.
         *
         * KEEP rather than UPDATE: replacing the request on every launch
         * resets its schedule, so an app opened often would never reach the
         * interval and never run in the background at all.
         *
         * Fifteen minutes is the floor WorkManager enforces; the real cadence
         * is the system's to decide, and doze will stretch it considerably.
         */
        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<LinkCheckWorker>(
                2, TimeUnit.HOURS,
            ).setConstraints(
                Constraints.Builder()
                    // Checking links without a network would mark healthy
                    // links unreachable, so the work waits for one.
                    .setRequiredNetworkType(NetworkType.CONNECTED)
                    .setRequiresBatteryNotLow(true)
                    .build(),
            ).build()

            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request,
            )
        }
    }
}
