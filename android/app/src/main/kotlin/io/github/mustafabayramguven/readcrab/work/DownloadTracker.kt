package io.github.mustafabayramguven.readcrab.work

import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.DownloadInfo
import io.github.mustafabayramguven.readcrab.core.DownloadState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

// İndirme satırlarını arayüze taşır; iş sürerken 700 ms'de bir çekirdeği yoklar.
class DownloadTracker(private val app: RcApp, private val scope: CoroutineScope) {
    private val _infos = MutableStateFlow<Map<Long, DownloadInfo>>(emptyMap())
    val infos: StateFlow<Map<Long, DownloadInfo>> = _infos.asStateFlow()

    private val lock = Any()
    private var polling: Job? = null

    private fun isActive(i: DownloadInfo) = i.state == DownloadState.QUEUED || i.state == DownloadState.RUNNING

    private fun fetch(): List<DownloadInfo>? {
        val list = runCatching { app.core.downloadInfos() }.getOrNull() ?: return null
        _infos.value = list.associateBy { it.bookId }
        return list
    }

    private fun startPollingIfActive(list: List<DownloadInfo>) {
        if (list.none { isActive(it) }) return
        synchronized(lock) {
            if (polling != null) return
            polling = scope.launch(Dispatchers.IO) { poll() }
        }
    }

    private suspend fun poll() {
        while (true) {
            delay(700)
            val active = fetch()?.any { isActive(it) } ?: false
            synchronized(lock) {
                if (!active) {
                    polling = null
                    return
                }
            }
        }
    }

    /** Tek seferlik `core.downloadInfos()` (IO); aktif iş varsa yoklama başlar. */
    fun refresh() {
        scope.launch(Dispatchers.IO) { fetch()?.let { startPollingIfActive(it) } }
    }

    fun enqueue(bookIds: List<Long>) {
        scope.launch(Dispatchers.IO) {
            runCatching { app.core.downloadEnqueue(bookIds) }
            Scheduler.scheduleDownload(app, app.prefs.state.value.wifiOnly)
            fetch()?.let { startPollingIfActive(it) }
        }
    }

    fun remove(bookIds: List<Long>) {
        scope.launch(Dispatchers.IO) {
            runCatching { app.core.downloadRemove(bookIds) }
            fetch()?.let { startPollingIfActive(it) }
        }
    }
}
