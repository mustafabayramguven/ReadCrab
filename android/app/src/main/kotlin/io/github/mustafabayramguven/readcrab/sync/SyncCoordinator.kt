package io.github.mustafabayramguven.readcrab.sync

import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.CoreException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

enum class SyncKind { SYNCED, PENDING, OFFLINE, ERROR, SYNCING }

data class SyncIndicator(val kind: SyncKind, val pending: Int, val lastOkAt: Long?, val error: String?)

/**
 * Tek eşitleme motoru. Yalnız uygulama ÖNDEYKEN çalışır; arka planda hiçbir iş zamanlanmaz.
 * Yerel yazımlar çekirdekte kuyruğa girer (`progress` satırı); itme tek tüketicide sıralıdır.
 */
class SyncCoordinator(private val app: RcApp, private val scope: CoroutineScope) {
    @Volatile var foreground: Boolean = false // MainActivity onStart/onStop

    private val _indicator = MutableStateFlow(SyncIndicator(SyncKind.SYNCED, 0, null, null))
    val indicator: StateFlow<SyncIndicator> = _indicator.asStateFlow()

    private val _completed = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val completed: SharedFlow<Unit> = _completed.asSharedFlow() // her başarılı syncNow sonrası

    private val syncing = MutableStateFlow(false)
    private val mutex = Mutex() // tek uçuş: push ve tam eşitleme birbirini bekler
    private val indicatorLock = Any()
    private val pushSignal = Channel<Unit>(Channel.CONFLATED)
    private var debounceJob: Job? = null
    private var wasOffline = !app.connectivity.online.value

    init {
        scope.launch(Dispatchers.IO) {
            for (signal in pushSignal) pushOnce()
        }
        // bağlantı geri gelince (yalnız uygulama öndeyken) tam eşitleme
        scope.launch {
            app.connectivity.online.collect { on ->
                if (on && foreground && wasOffline) syncNow()
                wasOffline = !on
                refresh()
            }
        }
    }

    /** `core.syncStatus()` ve bağlantıdan göstergeyi hesaplar (IO). */
    fun refresh() {
        scope.launch(Dispatchers.IO) { recompute() }
    }

    private fun recompute() {
        synchronized(indicatorLock) {
            val s = runCatching { app.core.syncStatus() }.getOrNull() ?: return
            val kind = when {
                syncing.value -> SyncKind.SYNCING
                !app.connectivity.online.value -> SyncKind.OFFLINE
                s.lastError != null && s.pendingOps > 0 -> SyncKind.ERROR
                s.pendingOps > 0 -> SyncKind.PENDING
                else -> SyncKind.SYNCED
            }
            _indicator.value = SyncIndicator(kind, s.pendingOps, s.lastOkAt, s.lastError)
        }
    }

    /** Yerel yazımdan sonra: gösterge yenilenir ve itme sıraya alınır (0 = hemen). */
    @Synchronized
    fun onLocalChange(pushDelayMs: Long) {
        refresh()
        if (pushDelayMs <= 0) {
            pushSignal.trySend(Unit)
        } else {
            debounceJob?.cancel()
            debounceJob = scope.launch {
                delay(pushDelayMs)
                pushSignal.trySend(Unit)
            }
        }
    }

    private suspend fun pushOnce() {
        try {
            if (app.core.session() == null || !app.connectivity.online.value) return
            mutex.withLock { app.core.syncPush() }
        } catch (e: CancellationException) {
            throw e
        } catch (e: CoreException.Unauthorized) {
            app.prefs.update { it.copy(needsRelogin = true) }
        } catch (e: Exception) {
            // kuyruk çekirdekte durur; sonraki değişiklikte ya da elle eşitlemede yeniden denenir
        } finally {
            recompute()
        }
    }

    /** Tam eşitleme: itme + kütüphane + çekme. İkinci çağrı mutex'te bekler. */
    fun syncNow() {
        scope.launch(Dispatchers.IO) {
            if (app.core.session() == null) return@launch
            syncing.value = true
            recompute()
            try {
                mutex.withLock { app.core.syncNow() }
                _completed.emit(Unit)
            } catch (e: CancellationException) {
                throw e
            } catch (e: CoreException.Unauthorized) {
                app.prefs.update { it.copy(needsRelogin = true) }
            } catch (e: Exception) {
                // hata çekirdekte `sync_last_error` olarak durur ve göstergede görünür
            } finally {
                syncing.value = false
                recompute()
            }
        }
    }

    /** Uygulama öne geldi: oturum varsa ve çevrimiçiyse tam eşitleme. */
    fun onForeground() {
        foreground = true
        scope.launch(Dispatchers.IO) {
            if (app.core.session() != null && app.connectivity.online.value) syncNow() else recompute()
        }
    }
}
