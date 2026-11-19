package io.github.mustafabayramguven.readcrab.ui.reader.text

import android.app.Application
import android.content.res.Configuration
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.systemGestureExclusion
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SkipNext
import androidx.compose.material.icons.outlined.SkipPrevious
import androidx.compose.material.icons.outlined.Toc
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.TEXT_FONTS
import io.github.mustafabayramguven.readcrab.UiPrefs
import io.github.mustafabayramguven.readcrab.core.BookDetail
import io.github.mustafabayramguven.readcrab.core.BookmarkItem
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.PositionSuggestion
import io.github.mustafabayramguven.readcrab.core.SearchHitItem
import io.github.mustafabayramguven.readcrab.core.TextInfo
import io.github.mustafabayramguven.readcrab.core.TocEntry
import io.github.mustafabayramguven.readcrab.ui.common.EmptyState
import io.github.mustafabayramguven.readcrab.ui.common.SyncIndicatorButton
import io.github.mustafabayramguven.readcrab.ui.common.formatPercent
import io.github.mustafabayramguven.readcrab.ui.reader.BookmarksSheet
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderChoiceRow
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderHint
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSection
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSettingsSheet
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSliderRow
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSwitchRow
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderWindowEffects
import io.github.mustafabayramguven.readcrab.ui.reader.ReadingTracker
import io.github.mustafabayramguven.readcrab.ui.reader.RemotePositionDialog
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.ui.theme.isEffectiveDark
import io.github.mustafabayramguven.readcrab.ui.theme.readerColors
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import java.time.LocalDate
import java.util.Locale
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt

enum class TextSheet { NONE, TOC, BOOKMARKS, SEARCH, SETTINGS }

data class SearchUi(
    val query: String = "", val loading: Boolean = false,
    val hits: List<SearchHitItem> = emptyList(), val truncated: Boolean = false,
    val error: String? = null, val searched: Boolean = false,
)

data class TextState(
    val loading: Boolean = true, val error: String? = null, val title: String = "", val seriesId: Long = 0,
    val chapterCount: Int = 1, val toc: List<TocEntry> = emptyList(),
    val chapter: Int = 0, val ratio: Double = 0.0, val page: Int = 0, val pages: Int = 1, val percent: Double = 0.0,
    val barsVisible: Boolean = false, val sheet: TextSheet = TextSheet.NONE,
    val bookmarks: List<BookmarkItem> = emptyList(), val isBookmarked: Boolean = false,
    val search: SearchUi = SearchUi(), val online: Boolean = true,
    val nextBookId: Long? = null, val nextBookTitle: String? = null, val showNextCard: Boolean = false,
    val prefs: UiPrefs = UiPrefs(),
    val suggestion: PositionSuggestion? = null, val blocks: Int = 0,
)

sealed interface TextEvent {
    data class Load(val bookId: Long) : TextEvent
    data class Web(val json: String) : TextEvent // JS mesajı
    data object ToggleBars : TextEvent
    data class OpenSheet(val sheet: TextSheet) : TextEvent
    data object CloseSheet : TextEvent
    data class GoToc(val index: Int) : TextEvent // ratio 0
    data class GoChapter(val delta: Int) : TextEvent // -1/+1, ratio 0
    data object ToggleBookmark : TextEvent
    data class OpenBookmark(val b: BookmarkItem) : TextEvent
    data class DeleteBookmark(val localId: Long) : TextEvent
    data class SearchQuery(val q: String) : TextEvent // 350 ms bekle, ≥2 karakter, yalnız online
    data class OpenHit(val h: SearchHitItem) : TextEvent
    data class Page(val dir: Int) : TextEvent // ses tuşları / düğme
    data object Retry : TextEvent
    data object AcceptSuggestion : TextEvent
    data object DeclineSuggestion : TextEvent
    data class PatchPrefs(val f: (UiPrefs) -> UiPrefs) : TextEvent
}

/** "auto" temasını açık/koyu/AMOLED durumuna göre light|dark|black'e çözer. */
fun resolveReaderTheme(p: UiPrefs, systemDark: Boolean): String = when {
    p.readerTheme != "auto" -> p.readerTheme
    isEffectiveDark(p, systemDark) -> if (p.amoled) "black" else "dark"
    else -> "light"
}

private fun isNear(locator: String, ch: Int, p: Double, pages: Int): Boolean {
    val l = Locator.parse(locator) ?: return false
    return l.ch == ch && abs(l.p - p) <= max(0.005, 0.5 / pages.coerceAtLeast(1))
}

private fun prefsJson(p: UiPrefs, systemDark: Boolean): String {
    val c = readerColors(resolveReaderTheme(p, systemDark))
    return JSONObject()
        .put("bg", c.bg).put("fg", c.fg).put("link", c.link)
        .put("font", p.textFont).put("fs", p.textSize)
        .put("lh", (p.lineHeight * 10).roundToInt() / 10.0)
        .put("ws", (p.wordSpacing * 100).roundToInt() / 100.0)
        .put("ls", (p.letterSpacing * 100).roundToInt() / 100.0)
        .put("align", p.align).put("mode", p.textMode)
        .put("mg", p.margin).put("zones", p.tapZones)
        .toString()
}

private class PendingSave(val bookId: Long, val locator: String, val percent: Double, val pushDelayMs: Long)

class TextReaderViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = getApplication<RcApp>()
    private val core = rc.core

    val tracker = ReadingTracker(rc, viewModelScope)

    private val _state = MutableStateFlow(
        TextState(prefs = rc.prefs.state.value, online = rc.connectivity.online.value),
    )
    val state: StateFlow<TextState> = _state.asStateFlow()

    // reader.html henüz hazır değilken gönderilenler de ulaşsın diye son iki komut tekrarlanır
    private val _commands = MutableSharedFlow<WebCommand>(replay = 2, extraBufferCapacity = 64)
    val commands: SharedFlow<WebCommand> = _commands.asSharedFlow()

    private var bookId = 0L
    private var loadedFor = -1L
    @Volatile private var webReady = false
    @Volatile private var dataReady = false
    private var systemDark =
        (rc.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    private var loadJob: Job? = null
    private var chapterJob: Job? = null
    private var searchJob: Job? = null

    // Konum kaydı tek tüketicide sıralıdır; kuyrukta yalnız en son konum kalır.
    private val saveChannel = Channel<PendingSave>(Channel.CONFLATED)
    // yüklenen / en son kaydedilen konum dizgesi; aynısı yeniden yazılmaz
    private var lastSavedLocator: String? = null
    // en son bilinen BLOK indeksi: yeniden düzende ve yeniden yüklemede konumu kesin korur
    private var curBlock = -1
    // başka cihaz konumu sorusu yanıtlanana dek bekleyen kitap bilgisi
    private var pendingLoad: Pair<BookDetail, TextInfo>? = null

    // içeriği yerelde olmayan bir bölüm 401 ile getirilemedi: oturum düşmüş
    private val _sessionExpired = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val sessionExpired: SharedFlow<Unit> = _sessionExpired.asSharedFlow()

    init {
        viewModelScope.launch {
            rc.prefs.state.collect { p ->
                _state.update { it.copy(prefs = p) }
                if (webReady) emitPrefs()
            }
        }
        viewModelScope.launch { rc.connectivity.online.collect { o -> _state.update { it.copy(online = o) } } }
        viewModelScope.launch(Dispatchers.IO) {
            for (s in saveChannel) {
                runCatching { core.saveProgress(s.bookId, s.locator, s.percent, LocalDate.now().toString()) }
                    .onFailure { handleError(it) }
                rc.sync.onLocalChange(s.pushDelayMs) // kayıt bittikten SONRA it: en son konum gider
            }
        }
    }

    /** Sistem koyu/açık geçişi: "Otomatik" okuyucu teması yeniden hesaplanır. */
    fun onSystemDark(dark: Boolean) {
        if (systemDark == dark) return
        systemDark = dark
        if (webReady) emitPrefs()
    }

    fun onEvent(e: TextEvent) {
        when (e) {
            is TextEvent.Load -> if (loadedFor != e.bookId) load(e.bookId)
            is TextEvent.Web -> viewModelScope.launch { handleWeb(e.json) }
            TextEvent.ToggleBars -> _state.update { it.copy(barsVisible = !it.barsVisible) }
            is TextEvent.OpenSheet -> _state.update { it.copy(sheet = e.sheet) }
            TextEvent.CloseSheet -> _state.update { it.copy(sheet = TextSheet.NONE) }
            is TextEvent.GoToc -> {
                _state.update { it.copy(sheet = TextSheet.NONE) }
                loadChapter(e.index, 0.0)
            }
            is TextEvent.GoChapter -> {
                val target = _state.value.chapter + e.delta
                if (target in 0 until _state.value.chapterCount) loadChapter(target, 0.0)
            }
            TextEvent.ToggleBookmark -> toggleBookmark()
            is TextEvent.OpenBookmark -> Locator.parse(e.b.locator)?.let { loc ->
                _state.update { it.copy(sheet = TextSheet.NONE) }
                loadChapter(loc.ch, loc.p)
            }
            is TextEvent.DeleteBookmark -> viewModelScope.launch(Dispatchers.IO) {
                runCatching { core.deleteBookmark(e.localId) }.onFailure { handleError(it) }
                reloadBookmarks()
                rc.sync.onLocalChange(2000)
            }
            is TextEvent.SearchQuery -> searchQuery(e.q)
            is TextEvent.OpenHit -> {
                _state.update { it.copy(sheet = TextSheet.NONE) }
                loadChapter(e.h.ch, e.h.p)
            }
            is TextEvent.Page -> _commands.tryEmit(WebCommand.Page(e.dir))
            TextEvent.Retry ->
                if (dataReady) loadChapter(_state.value.chapter, _state.value.ratio, curBlock) else load(bookId)
            TextEvent.AcceptSuggestion -> resolveSuggestion(true)
            TextEvent.DeclineSuggestion -> resolveSuggestion(false)
            is TextEvent.PatchPrefs -> rc.prefs.update(e.f)
        }
    }

    private fun load(id: Long) {
        bookId = id
        loadedFor = id
        dataReady = false
        pendingLoad = null
        loadJob?.cancel()
        _state.update { it.copy(loading = true, error = null, suggestion = null) }
        loadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val detail = core.getBook(id)
                val info = core.textInfo(id)
                if (rc.connectivity.online.value) {
                    // en çok 4 sn beklenir; engelleyici çağrı arka planda sürer, yerel konumla devam edilir
                    val pull = async { runCatching { core.pullBook(id) } }
                    val pulled = withTimeoutOrNull(4000) { pull.await() }
                    pulled?.exceptionOrNull()?.let { handleError(it) }
                }
                // başka cihazda daha yeni, farklı bir konum varsa web başlatılmadan önce sorulur
                val sug = runCatching { core.positionSuggestion(id) }.getOrNull()
                if (sug != null) {
                    pendingLoad = detail to info
                    _state.update { it.copy(suggestion = sug, loading = true) }
                    return@launch
                }
                finishLoad(id, detail, info)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failLoad(e)
            }
        }
    }

    /** "Oraya git" / "Burada kal": yanıt çekirdeğe işlenir, konum yeniden okunur ve web başlatılır. */
    private fun resolveSuggestion(accept: Boolean) {
        val (detail, info) = pendingLoad ?: return
        val id = bookId
        pendingLoad = null
        _state.update { it.copy(suggestion = null) }
        loadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                if (accept) core.acceptPosition(id) else core.declinePosition(id)
                finishLoad(id, detail, info)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failLoad(e)
            }
        }
    }

    private suspend fun finishLoad(id: Long, detail: BookDetail, info: TextInfo) {
        val loc = Locator.parse(core.getProgress(id).locator)
        val ch = loc?.ch?.takeIf { it in 0 until info.chapterCount } ?: 0
        val here = loc != null && loc.ch == ch
        val ratio = if (here) loc?.p ?: 0.0 else 0.0
        val block = if (here) loc?.b ?: -1 else -1
        val bookmarks = core.listBookmarks(id)
        val percent = ((ch + ratio) / info.chapterCount).coerceIn(0.0, 1.0)
        lastSavedLocator = loc?.let { Locator.build(it.ch, it.p, it.b) }
        curBlock = block
        tracker.start(id, percent)
        _state.update {
            it.copy(
                title = detail.book.title, seriesId = detail.book.seriesId,
                chapterCount = info.chapterCount, toc = info.toc,
                chapter = ch, ratio = ratio, percent = percent,
                bookmarks = bookmarks, isBookmarked = bookmarks.any { b -> isNear(b.locator, ch, ratio, 1) },
                nextBookId = detail.nextBookId, nextBookTitle = detail.nextBookTitle,
            )
        }
        withContext(Dispatchers.Main) {
            dataReady = true
            startWeb()
        }
    }

    private fun failLoad(e: Exception) {
        handleError(e)
        _state.update { it.copy(loading = false, error = e.message ?: rc.getString(R.string.reader_error)) }
    }

    private fun emitPrefs() {
        _commands.tryEmit(WebCommand.SetPrefs(prefsJson(_state.value.prefs, systemDark)))
    }

    /** Kabuk sayfası ve veri hazır olunca (hangisi sonra gelirse) tercihler ve geçerli bölüm gönderilir. */
    private fun startWeb() {
        if (!webReady || !dataReady) return
        emitPrefs()
        loadChapter(_state.value.chapter, _state.value.ratio, curBlock)
    }

    private fun loadChapter(ch: Int, ratio: Double, block: Int = -1) {
        val s = _state.value
        val target = ch.coerceIn(0, max(s.chapterCount - 1, 0))
        val r = ratio.coerceIn(0.0, 1.0)
        val id = bookId
        curBlock = block
        _state.update { it.copy(loading = true, error = null, chapter = target, ratio = r, showNextCard = false) }
        chapterJob?.cancel()
        chapterJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                core.ensureChapter(id, target)
                _commands.emit(WebCommand.Load("/book/$id/c$target.html", target, r, block))
            } catch (e: CancellationException) {
                throw e
            } catch (e: CoreException.Offline) {
                _state.update { it.copy(loading = false, error = rc.getString(R.string.reader_error)) }
            } catch (e: CoreException.Unauthorized) {
                handleError(e)
                _state.update { it.copy(loading = false, error = e.message ?: rc.getString(R.string.reader_error)) }
                _sessionExpired.tryEmit(Unit)
            } catch (e: Exception) {
                handleError(e)
                _state.update { it.copy(loading = false, error = e.message ?: rc.getString(R.string.reader_error)) }
            }
        }
    }

    private fun handleWeb(json: String) {
        val o = runCatching { JSONObject(json) }.getOrNull() ?: return
        when (o.optString("t")) {
            "ready" -> {
                webReady = true
                startWeb()
            }
            "loaded" -> onLoaded(o.optInt("ch"), o.optInt("pages", 1))
            "pos" -> onPos(
                ch = o.optInt("ch"), p = o.optDouble("p", 0.0), b = o.optInt("b", -1),
                nb = o.optInt("nb", 0), page = o.optInt("page"), pages = o.optInt("pages", 1),
                user = o.optInt("u") == 1,
            )
            "tap" -> _state.update { it.copy(barsVisible = !it.barsVisible) }
            "edge" -> onEdge(o.optInt("d"))
            "error" -> _state.update { it.copy(loading = false, error = o.optString("msg").ifEmpty { rc.getString(R.string.reader_error) }) }
        }
    }

    private fun onLoaded(ch: Int, pages: Int) {
        val s = _state.value
        if (ch != s.chapter) return
        _state.update { it.copy(loading = false, error = null, pages = pages.coerceAtLeast(1)) }
        // sonraki bölüm arka planda hazırlanır
        if (ch + 1 < s.chapterCount) {
            val id = bookId
            viewModelScope.launch(Dispatchers.IO) { runCatching { core.ensureChapter(id, ch + 1) } }
        }
    }

    private fun onPos(ch: Int, p: Double, b: Int, nb: Int, page: Int, pages: Int, user: Boolean) {
        val s = _state.value
        if (ch != s.chapter) return // eski bölümden gecikmiş mesaj
        val ratio = p.coerceIn(0.0, 1.0)
        val percent = ((ch + ratio) / s.chapterCount).coerceIn(0.0, 1.0)
        val paged = s.prefs.textMode == "page"
        val pg = pages.coerceAtLeast(1)
        val block = b.takeIf { it >= 0 }
        curBlock = b
        // UI durumu her mesajda güncellenir
        _state.update {
            it.copy(
                ratio = ratio, page = page, pages = pg, percent = percent, blocks = nb,
                isBookmarked = it.bookmarks.any { bm -> isNear(bm.locator, ch, ratio, pg) },
                showNextCard = ch == it.chapterCount - 1 && it.nextBookId != null &&
                    (if (paged) page >= pg - 1 else ratio >= 0.98),
            )
        }
        if (!user) return // yerleşim / yeniden düzen konum kaydı değildir
        tracker.addPage(if (block != null) "$ch:$block" else "$ch:$page")
        tracker.setPercent(percent)
        val loc = Locator.build(ch, ratio, block)
        if (loc == lastSavedLocator) return
        lastSavedLocator = loc
        // sayfa modunda her çevirmede hemen, kaydırmada paragraf değişince ~2 sn sonra it
        saveChannel.trySend(PendingSave(bookId, loc, percent, if (paged) 0L else 2000L))
    }

    private fun onEdge(d: Int) {
        val s = _state.value
        if (d > 0) {
            if (s.chapter + 1 < s.chapterCount) loadChapter(s.chapter + 1, 0.0)
            else _state.update { it.copy(showNextCard = it.nextBookId != null) }
        } else if (d < 0 && s.chapter > 0) {
            loadChapter(s.chapter - 1, 1.0)
        }
    }

    private fun toggleBookmark() {
        val s = _state.value
        val id = bookId
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                val near = core.listBookmarks(id).filter { isNear(it.locator, s.chapter, s.ratio, s.pages) }
                if (near.isNotEmpty()) {
                    near.forEach { core.deleteBookmark(it.localId) }
                } else {
                    val label = rc.getString(R.string.reader_chapter_percent, s.chapter + 1, formatPercent(s.percent))
                    // yer imi b'siz yazılır: web ile birebir
                    core.addBookmark(id, Locator.build(s.chapter, s.ratio, null), label)
                }
            }.onFailure { handleError(it) }
            reloadBookmarks()
            rc.sync.onLocalChange(2000)
        }
    }

    private fun reloadBookmarks() {
        runCatching { core.listBookmarks(bookId) }.onSuccess { list ->
            _state.update { s ->
                s.copy(
                    bookmarks = list,
                    isBookmarked = list.any { isNear(it.locator, s.chapter, s.ratio, s.pages) },
                )
            }
        }
    }

    private fun searchQuery(q: String) {
        searchJob?.cancel()
        _state.update { it.copy(search = it.search.copy(query = q, error = null)) }
        val text = q.trim()
        if (text.length < 2) {
            _state.update {
                it.copy(search = it.search.copy(loading = false, hits = emptyList(), truncated = false, searched = false))
            }
            return
        }
        val id = bookId
        searchJob = viewModelScope.launch {
            delay(350)
            if (!rc.connectivity.online.value) {
                _state.update {
                    it.copy(search = it.search.copy(loading = false, searched = false, error = rc.getString(R.string.reader_search_offline)))
                }
                return@launch
            }
            _state.update { it.copy(search = it.search.copy(loading = true)) }
            try {
                val res = withContext(Dispatchers.IO) { core.searchText(id, text) }
                _state.update {
                    it.copy(search = it.search.copy(loading = false, hits = res.hits, truncated = res.truncated, searched = true, error = null))
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                handleError(e)
                _state.update {
                    it.copy(search = it.search.copy(loading = false, searched = false, error = e.message ?: rc.getString(R.string.reader_error)))
                }
            }
        }
    }

    private fun handleError(e: Throwable) {
        if (e is CoreException.Unauthorized) rc.prefs.update { it.copy(needsRelogin = true) }
    }

    override fun onCleared() {
        tracker.stop()
        // kanalda kalan son konum kapsam iptal edilmeden yazılır
        val s = saveChannel.tryReceive().getOrNull() ?: return
        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch {
            runCatching { core.saveProgress(s.bookId, s.locator, s.percent, LocalDate.now().toString()) }
            rc.sync.onLocalChange(0)
        }
    }
}

@Composable
fun TextReaderScreen(
    bookId: Long,
    onBack: (seriesId: Long) -> Unit,
    onNextBook: (Long) -> Unit,
    onSessionExpired: () -> Unit,
    vm: TextReaderViewModel = viewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()
    val systemDark = isSystemInDarkTheme()
    val app = LocalContext.current.applicationContext as RcApp
    LaunchedEffect(bookId) { vm.onEvent(TextEvent.Load(bookId)) }
    LaunchedEffect(systemDark) { vm.onSystemDark(systemDark) }
    LaunchedEffect(Unit) { vm.sessionExpired.collect { onSessionExpired() } }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        vm.tracker.foreground = true
        vm.tracker.touch()
    }
    LifecycleEventEffect(Lifecycle.Event.ON_PAUSE) { vm.tracker.foreground = false }
    DisposableEffect(Unit) { onDispose { vm.tracker.stop() } }
    ReaderWindowEffects(state.prefs, onVolume = { f ->
        vm.onEvent(TextEvent.Page(if (f) 1 else -1))
        true
    })

    val bgInt = remember(state.prefs.readerTheme, state.prefs.themeMode, state.prefs.amoled, systemDark) {
        android.graphics.Color.parseColor(readerColors(resolveReaderTheme(state.prefs, systemDark)).bg)
    }
    Box(Modifier.fillMaxSize().background(Color(bgInt))) {
        // görünüm alanı güvenli çizim alanıdır (kesik, köşeler, sistem çubukları); kenar hareketleri WebView'a bırakılır
        ReaderWebView(
            commands = vm.commands,
            bgColor = bgInt,
            onMessage = { vm.onEvent(TextEvent.Web(it)) },
            modifier = Modifier
                .fillMaxSize()
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .systemGestureExclusion(),
        )

        val error = state.error
        when {
            error != null -> Column(
                Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                EmptyState(Icons.Outlined.ErrorOutline, error)
                Button(onClick = { vm.onEvent(TextEvent.Retry) }) { Text(stringResource(R.string.action_retry)) }
            }
            state.loading -> Box(Modifier.fillMaxSize().background(Color(bgInt)), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        }

        AnimatedVisibility(
            visible = state.barsVisible,
            modifier = Modifier.align(Alignment.TopCenter),
            enter = fadeIn(),
            exit = fadeOut(),
        ) { TextTopBar(state, vm, onBack) }

        AnimatedVisibility(
            visible = state.barsVisible,
            modifier = Modifier.align(Alignment.BottomCenter),
            enter = fadeIn(),
            exit = fadeOut(),
        ) { TextBottomBar(state, vm) }

        val nextId = state.nextBookId
        if (nextId != null && state.showNextCard) {
            Card(
                Modifier
                    .align(Alignment.BottomCenter)
                    .navigationBarsPadding()
                    .padding(start = Dimens.gutter, end = Dimens.gutter, bottom = if (state.barsVisible) 112.dp else Dimens.s5),
            ) {
                Row(Modifier.padding(Dimens.s3), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        stringResource(R.string.reader_next_volume, state.nextBookTitle ?: ""),
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false).padding(end = Dimens.s3),
                    )
                    Button(onClick = { onNextBook(nextId) }) { Text(stringResource(R.string.reader_open)) }
                }
            }
        }
        ReaderHint(state.prefs) { app.prefs.update { it.copy(readerHintSeen = true) } }
    }

    // C5: başka cihazda daha yeni, farklı bir konum var
    state.suggestion?.let { sug ->
        val loc = Locator.parse(sug.locator)
        val pct = (sug.percent * 100).roundToInt()
        val where = if (loc != null) stringResource(R.string.reader_remote_chapter, loc.ch + 1, pct) else "%$pct"
        RemotePositionDialog(
            where = where,
            onGo = { vm.onEvent(TextEvent.AcceptSuggestion) },
            onStay = { vm.onEvent(TextEvent.DeclineSuggestion) },
        )
    }

    when (state.sheet) {
        TextSheet.NONE -> Unit
        TextSheet.TOC -> TocSheet(state, vm)
        TextSheet.BOOKMARKS -> BookmarksSheet(
            items = state.bookmarks,
            onOpen = { vm.onEvent(TextEvent.OpenBookmark(it)) },
            onDelete = { vm.onEvent(TextEvent.DeleteBookmark(it)) },
            onDismiss = { vm.onEvent(TextEvent.CloseSheet) },
        )
        TextSheet.SEARCH -> SearchSheet(state, vm)
        TextSheet.SETTINGS -> TextSettingsSheet(state, vm)
    }
}

@Composable
private fun TextTopBar(state: TextState, vm: TextReaderViewModel, onBack: (Long) -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surface.copy(alpha = 0.94f), modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal)).height(Dimens.barHeight),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { onBack(state.seriesId) }) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
            }
            Text(
                state.title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            SyncIndicatorButton()
            IconButton(onClick = { vm.onEvent(TextEvent.ToggleBookmark) }) {
                Icon(
                    if (state.isBookmarked) Icons.Filled.Bookmark else Icons.Filled.BookmarkBorder,
                    contentDescription = stringResource(
                        if (state.isBookmarked) R.string.reader_bookmark_remove else R.string.reader_bookmark_add,
                    ),
                )
            }
            IconButton(onClick = { vm.onEvent(TextEvent.OpenSheet(TextSheet.SEARCH)) }) {
                Icon(Icons.Outlined.Search, contentDescription = stringResource(R.string.reader_search))
            }
            IconButton(onClick = { vm.onEvent(TextEvent.OpenSheet(TextSheet.TOC)) }) {
                Icon(Icons.Outlined.Toc, contentDescription = stringResource(R.string.reader_toc))
            }
        }
    }
}

@Composable
private fun TextBottomBar(state: TextState, vm: TextReaderViewModel) {
    Surface(color = MaterialTheme.colorScheme.surface.copy(alpha = 0.94f), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal)).padding(horizontal = Dimens.gutter, vertical = Dimens.s2)) {
            LinearProgressIndicator(
                progress = { state.percent.toFloat().coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth(),
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = { vm.onEvent(TextEvent.GoChapter(-1)) }, enabled = state.chapter > 0) {
                    Icon(Icons.Outlined.SkipPrevious, contentDescription = null)
                }
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        stringResource(R.string.reader_chapter_percent, state.chapter + 1, formatPercent(state.percent)),
                        style = MaterialTheme.typography.labelLarge,
                    )
                    if (state.prefs.textMode == "page") {
                        Text(
                            stringResource(R.string.reader_page_of, state.page + 1, state.pages),
                            style = MaterialTheme.typography.labelSmall,
                        )
                    }
                }
                IconButton(
                    onClick = { vm.onEvent(TextEvent.GoChapter(1)) },
                    enabled = state.chapter + 1 < state.chapterCount,
                ) {
                    Icon(Icons.Outlined.SkipNext, contentDescription = null)
                }
                TextButton(onClick = { vm.onEvent(TextEvent.OpenSheet(TextSheet.SETTINGS)) }) {
                    Icon(Icons.Outlined.Settings, contentDescription = null)
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(R.string.reader_settings_label))
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TocSheet(state: TextState, vm: TextReaderViewModel) {
    ModalBottomSheet(onDismissRequest = { vm.onEvent(TextEvent.CloseSheet) }) {
        LazyColumn {
            items(state.toc, key = { it.index }) { entry ->
                val current = entry.index == state.chapter
                Text(
                    entry.title,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (current) FontWeight.Bold else FontWeight.Normal,
                    color = if (current) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { vm.onEvent(TextEvent.GoToc(entry.index)) }
                        .padding(horizontal = Dimens.gutter, vertical = Dimens.s3),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SearchSheet(state: TextState, vm: TextReaderViewModel) {
    val s = state.search
    ModalBottomSheet(onDismissRequest = { vm.onEvent(TextEvent.CloseSheet) }) {
        OutlinedTextField(
            value = s.query,
            onValueChange = { vm.onEvent(TextEvent.SearchQuery(it)) },
            placeholder = { Text(stringResource(R.string.reader_search_hint)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = Dimens.gutter),
        )
        when {
            s.loading -> Box(Modifier.fillMaxWidth().padding(Dimens.s4), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            s.error != null -> Text(
                s.error,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(Dimens.gutter),
            )
            s.searched && s.hits.isEmpty() -> Text(
                stringResource(R.string.reader_search_none),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(Dimens.gutter),
            )
        }
        if (s.truncated) {
            Text(
                stringResource(R.string.reader_search_truncated),
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(horizontal = Dimens.gutter),
            )
        }
        LazyColumn {
            items(s.hits) { h ->
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clickable { vm.onEvent(TextEvent.OpenHit(h)) }
                        .padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
                ) {
                    Text(
                        stringResource(R.string.reader_chapter_percent, h.ch + 1, formatPercent(h.p)),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        buildAnnotatedString {
                            append(h.before)
                            withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)) {
                                append(h.hit)
                            }
                            append(h.after)
                        },
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
        }
    }
}

@Composable
private fun TextSettingsSheet(state: TextState, vm: TextReaderViewModel) {
    val p = state.prefs
    fun patch(f: (UiPrefs) -> UiPrefs) = vm.onEvent(TextEvent.PatchPrefs(f))
    ReaderSettingsSheet(
        title = stringResource(R.string.reader_settings),
        onDismiss = { vm.onEvent(TextEvent.CloseSheet) },
    ) {
        ReaderSection(stringResource(R.string.reader_theme), first = true) {
            ReaderChoiceRow(
                options = listOf(
                    "auto" to stringResource(R.string.reader_theme_auto),
                    "light" to stringResource(R.string.reader_theme_light),
                    "dark" to stringResource(R.string.reader_theme_dark),
                    "sepia" to stringResource(R.string.reader_theme_sepia),
                    "black" to stringResource(R.string.reader_theme_black),
                ),
                selected = p.readerTheme,
                onSelect = { k -> patch { it.copy(readerTheme = k) } },
            )
        }
        ReaderSection(stringResource(R.string.reader_font)) {
            ReaderChoiceRow(
                options = TEXT_FONTS,
                selected = p.textFont,
                onSelect = { k -> patch { it.copy(textFont = k) } },
            )
        }
        ReaderSection(stringResource(R.string.reader_section_text)) {
            ReaderSliderRow(
                label = stringResource(R.string.reader_font_size),
                valueText = "${p.textSize} px",
                value = p.textSize.toFloat(),
                range = 14f..28f,
                steps = 6,
            ) { v -> patch { it.copy(textSize = (v / 2f).roundToInt() * 2) } }
            ReaderSliderRow(
                label = stringResource(R.string.reader_line_height),
                valueText = "%.1f".format(Locale.ROOT, p.lineHeight),
                value = p.lineHeight,
                range = 1.2f..2.2f,
                steps = 9,
            ) { v -> patch { it.copy(lineHeight = (v * 10).roundToInt() / 10f) } }
            ReaderSliderRow(
                label = stringResource(R.string.reader_word_spacing),
                valueText = "%.2f em".format(Locale.ROOT, p.wordSpacing),
                value = p.wordSpacing,
                range = 0f..0.5f,
                steps = 9,
            ) { v -> patch { it.copy(wordSpacing = (v / 0.05f).roundToInt() * 0.05f) } }
            ReaderSliderRow(
                label = stringResource(R.string.reader_letter_spacing),
                valueText = "%.2f em".format(Locale.ROOT, p.letterSpacing),
                value = p.letterSpacing,
                range = 0f..0.1f,
                steps = 9,
            ) { v -> patch { it.copy(letterSpacing = (v / 0.01f).roundToInt() * 0.01f) } }
            ReaderSliderRow(
                label = stringResource(R.string.reader_margin),
                valueText = "${p.margin}%",
                value = p.margin.toFloat(),
                range = 0f..20f,
                steps = 19,
            ) { v -> patch { it.copy(margin = v.roundToInt()) } }
        }
        ReaderSection(stringResource(R.string.reader_section_page)) {
            Text(stringResource(R.string.reader_align), style = MaterialTheme.typography.bodyMedium)
            ReaderChoiceRow(
                options = listOf(
                    "left" to stringResource(R.string.reader_align_left),
                    "justify" to stringResource(R.string.reader_align_justify),
                ),
                selected = p.align,
                onSelect = { k -> patch { it.copy(align = k) } },
            )
            Spacer(Modifier.height(Dimens.s3))
            Text(stringResource(R.string.reader_mode), style = MaterialTheme.typography.bodyMedium)
            ReaderChoiceRow(
                options = listOf(
                    "scroll" to stringResource(R.string.reader_mode_scroll),
                    "page" to stringResource(R.string.reader_mode_page),
                ),
                selected = p.textMode,
                onSelect = { k -> patch { it.copy(textMode = k) } },
            )
        }
        ReaderSection(stringResource(R.string.reader_section_behavior)) {
            ReaderSwitchRow(stringResource(R.string.settings_tap_zones), p.tapZones) { v ->
                patch { it.copy(tapZones = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_volume_keys), p.volumeKeys) { v ->
                patch { it.copy(volumeKeys = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_keep_awake), p.keepAwake) { v ->
                patch { it.copy(keepAwake = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_immersive), p.immersive) { v ->
                patch { it.copy(immersive = v) }
            }
        }
    }
}
