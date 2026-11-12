package io.github.mustafabayramguven.readcrab.work

import android.content.Context
import android.content.pm.ServiceInfo
import androidx.core.app.NotificationCompat
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.DownloadState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import java.util.concurrent.TimeUnit

object Scheduler {
    /** Tek "downloads" iş kuyruğu; Wi-Fi tercihi kısıtı belirler. */
    fun scheduleDownload(ctx: Context, wifiOnly: Boolean) {
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED)
            .build()
        val req = OneTimeWorkRequestBuilder<DownloadWorker>()
            .setConstraints(constraints)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(ctx)
            .enqueueUniqueWork("downloads", ExistingWorkPolicy.APPEND_OR_REPLACE, req)
    }

    fun schedulePeriodicSync(ctx: Context) {
        val req = PeriodicWorkRequestBuilder<SyncWorker>(1, TimeUnit.HOURS)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(ctx)
            .enqueueUniquePeriodicWork("sync-periodic", ExistingPeriodicWorkPolicy.KEEP, req)
    }

    fun kickSync(ctx: Context) {
        val req = OneTimeWorkRequestBuilder<SyncWorker>()
            .setInitialDelay(5, TimeUnit.SECONDS)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(ctx)
            .enqueueUniqueWork("sync-kick", ExistingWorkPolicy.REPLACE, req)
    }

    /** Çıkışta tüm işler iptal edilir. */
    fun cancelAll(ctx: Context) {
        WorkManager.getInstance(ctx).cancelAllWork()
    }
}

class DownloadWorker(ctx: Context, p: WorkerParameters) : CoroutineWorker(ctx, p) {
    override suspend fun doWork(): Result {
        val app = applicationContext as RcApp
        val core = app.core
        // arka plandan başlatma yasağında sessiz devam
        try {
            val notification = NotificationCompat.Builder(applicationContext, "indirmeler")
                .setContentTitle(applicationContext.getString(R.string.notif_downloading))
                .setOngoing(true)
                .setSmallIcon(android.R.drawable.stat_sys_download)
                .build()
            setForeground(ForegroundInfo(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC))
        } catch (e: Exception) {
            // bildirim/ön plan kullanılamadı: arka planda sessizce sürer
        }
        return try {
            // runDownloads engelleyicidir: iptal edilince await hemen döner ve çekirdek durdurulur
            coroutineScope {
                val run = async(Dispatchers.IO) { core.runDownloads() }
                try {
                    run.await()
                } catch (e: CancellationException) {
                    core.downloadStop()
                    throw e
                }
            }
            // başka bir çalıştırma sürdüğü için hemen dönmüş ya da durdurulmuş olabilir: kuyrukta iş kaldıysa yeniden dene
            val left = withContext(Dispatchers.IO) { core.downloadInfos() }
                .any { it.state == DownloadState.QUEUED }
            if (left) Result.retry() else Result.success()
        } catch (e: CoreException.Unauthorized) {
            app.prefs.update { it.copy(needsRelogin = true) }
            Result.failure()
        } catch (e: CoreException.Network) {
            Result.retry()
        } catch (e: CoreException.Tls) {
            Result.failure()
        } catch (e: CoreException) {
            Result.failure()
        }
    }
}

class SyncWorker(ctx: Context, p: WorkerParameters) : CoroutineWorker(ctx, p) {
    override suspend fun doWork(): Result {
        val app = applicationContext as RcApp
        val core = app.core
        if (core.session() == null) return Result.success()
        return try {
            withContext(Dispatchers.IO) { core.syncNow() }
            Result.success()
        } catch (e: CoreException.Unauthorized) {
            app.prefs.update { it.copy(needsRelogin = true) }
            Result.failure()
        } catch (e: CoreException.Network) {
            Result.retry()
        } catch (e: CoreException.Server) {
            Result.retry()
        } catch (e: CoreException.Tls) {
            Result.failure()
        } catch (e: CoreException) {
            Result.failure()
        }
    }
}
