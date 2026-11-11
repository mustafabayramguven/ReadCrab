package io.github.mustafabayramguven.readcrab

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import io.github.mustafabayramguven.readcrab.core.Core
import io.github.mustafabayramguven.readcrab.image.RcFetcher
import io.github.mustafabayramguven.readcrab.image.RcKeyer
import io.github.mustafabayramguven.readcrab.work.DownloadTracker
import io.github.mustafabayramguven.readcrab.work.Scheduler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import java.io.File

// Tek örnek tutucu (DI yok): çekirdek, tercihler, bağlantı durumu, indirme izleyici.
class RcApp : Application(), SingletonImageLoader.Factory {
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val prefs: Prefs by lazy { Prefs(this) }
    val core: Core by lazy { Core(File(filesDir, "rc").absolutePath) }
    val connectivity: Connectivity by lazy { Connectivity(this) }
    val downloads: DownloadTracker by lazy { DownloadTracker(this, appScope) }

    override fun onCreate() {
        super.onCreate()
        val channel = NotificationChannel(
            "indirmeler",
            getString(R.string.notif_channel_downloads),
            NotificationManager.IMPORTANCE_LOW,
        )
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        if (core.session() != null) Scheduler.schedulePeriodicSync(this)
        downloads.refresh()
    }

    override fun newImageLoader(context: PlatformContext): ImageLoader =
        ImageLoader.Builder(context)
            .components {
                add(RcKeyer())
                add(RcFetcher.Factory(core))
            }
            .diskCache(null)
            .build()
}
