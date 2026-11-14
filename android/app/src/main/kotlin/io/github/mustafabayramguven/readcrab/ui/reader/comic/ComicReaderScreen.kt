package io.github.mustafabayramguven.readcrab.ui.reader.comic

import android.app.Application
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.outlined.Bookmarks
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import coil3.SingletonImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.UiPrefs
import io.github.mustafabayramguven.readcrab.core.BookDetail
import io.github.mustafabayramguven.readcrab.core.BookmarkItem
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.PositionSuggestion
import io.github.mustafabayramguven.readcrab.image.RcImage
import io.github.mustafabayramguven.readcrab.ui.common.EmptyState
import io.github.mustafabayramguven.readcrab.ui.common.SyncIndicatorButton
import io.github.mustafabayramguven.readcrab.ui.reader.BookmarksSheet
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderChoiceRow
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderHint
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSection
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSettingsSheet
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderSwitchRow
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderWindowEffects
import io.github.mustafabayramguven.readcrab.ui.reader.ReadingTracker
import io.github.mustafabayramguven.readcrab.ui.reader.RemotePositionDialog
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.ui.theme.comicBgColor
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import java.time.LocalDate
import kotlin.math.roundToInt

data class ComicState(
    val loading: Boolean = true, val error: String? = null, val title: String = "", val seriesId: Long = 0,
    val pageCount: Int = 0, val page: Int = 0, val mode: String = "ltr", // ltr | rtl | webtoon
    val barsVisible: Boolean = false, val showBookmarks: Boolean = false,
    val bookmarks: List<BookmarkItem> = emptyList(),
    val nextBookId: Long? = null, val nextBookTitle: String? = null, val prefs: UiPrefs = UiPrefs(),
    val suggestion: PositionSuggestion? = null, val settingsOpen: Boolean = false,
)

sealed interface ComicEvent {
    data class Load(val bookId: Long) : ComicEvent
    data class PageChanged(val page: Int) : ComicEvent // pager/webtoon yerleşince
    data class SeekTo(val page: Int) : ComicEvent // kaydırıcı bırakıldı / yer imi
    data object ToggleBars : ComicEvent
    data class SetMode(val mode: String) : ComicEvent
    data object ToggleBookmark : ComicEvent
    data object ShowBookmarks : ComicEvent
    data object HideBookmarks : ComicEvent
    data class OpenBookmark(val b: BookmarkItem) : ComicEvent
    data class DeleteBookmark(val localId: Long) : ComicEvent
    data object Retry : ComicEvent
    data object OpenSettings : ComicEvent
    data object CloseSettings : ComicEvent
    data object AcceptSuggestion : ComicEvent
    data object DeclineSuggestion : ComicEvent
    data class PatchPrefs(val f: (UiPrefs) -> UiPrefs) : ComicEvent
}

sealed interface ComicEffect { data class ScrollTo(val page: Int) : ComicEffect }

class ComicReaderViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = getApplication<RcApp>()
    private val core = rc.core

    val tracker = ReadingTracker(rc, viewModelScope)

    private val _state = MutableStateFlow(ComicState(prefs = rc.prefs.state.value))
    val state: StateFlow<ComicState> = _state.asStateFlow()

    private val _effects = MutableSharedFlow<ComicEffect>(extraBufferCapacity = 4)
    val effects: SharedFlow<ComicEffect> = _effects.asSharedFlow()

    private var bookId = 0L
    private var loadedFor = -1L
    private var loadJob: Job? = null

    // Konum yazımı tek tüketicide sıralıdır: son yazan her zaman en son sayfadır.
    private val saveChannel = Channel<Int>(Channel.CONFLATED)
    // yüklenen / en son kaydedilmesi istenen sayfa; kitabı yalnız açmak konum yazmaz
    private var savedPage = -1
    // başka cihaz konumu sorusu yanıtlanana dek bekleyen kitap bilgisi
    private var pendingDetail: BookDetail? = null

    init {
        viewModelScope.launch { rc.prefs.state.collect { p -> _state.update { it.copy(prefs = p) } } }
        viewModelScope.launch(Dispatchers.IO) {
            for (page in saveChannel) {
                val count = _state.value.pageCount.coerceAtLeast(1)
                val percent = ((page + 1).toDouble() / count).coerceIn(0.0, 1.0)
                runCatching { core.saveProgress(bookId, page.toString(), percent, LocalDate.now().toString()) }
                    .onFailure { handleError(it) }
                rc.sync.onLocalChange(0) // her sayfa çevirmede hemen it
            }
        }
    }

    /** İçeriği yerelde olmayan bir sayfa 401 ile getirilemedi: oturum düşmüş. */
    fun markSessionExpired() {
        rc.prefs.update { it.copy(needsRelogin = true) }
    }

    fun onEvent(e: ComicEvent) {
        when (e) {
            is ComicEvent.Load -> if (loadedFor != e.bookId) load(e.bookId)
            is ComicEvent.PageChanged -> pageChanged(e.page)
            is ComicEvent.SeekTo -> {
                _state.update { it.copy(page = e.page) }
                _effects.tryEmit(ComicEffect.ScrollTo(e.page))
            }
            ComicEvent.ToggleBars -> _state.update { it.copy(barsVisible = !it.barsVisible) }
            is ComicEvent.SetMode -> {
                rc.prefs.setComicModeFor(bookId, e.mode)
                _state.update { it.copy(mode = e.mode) }
                _effects.tryEmit(ComicEffect.ScrollTo(_state.value.page))
            }
            ComicEvent.ToggleBookmark -> toggleBookmark()
            ComicEvent.ShowBookmarks -> _state.update { it.copy(showBookmarks = true) }
            ComicEvent.HideBookmarks -> _state.update { it.copy(showBookmarks = false) }
            is ComicEvent.OpenBookmark -> {
                val p = e.b.locator.toIntOrNull()
                _state.update { it.copy(showBookmarks = false) }
                if (p != null && p in 0 until _state.value.pageCount) onEvent(ComicEvent.SeekTo(p))
            }
            is ComicEvent.DeleteBookmark -> viewModelScope.launch(Dispatchers.IO) {
                runCatching { core.deleteBookmark(e.localId) }.onFailure { handleError(it) }
                reloadBookmarks()
                rc.sync.onLocalChange(2000)
            }
            ComicEvent.Retry -> load(bookId)
            ComicEvent.OpenSettings -> _state.update { it.copy(settingsOpen = true) }
            ComicEvent.CloseSettings -> _state.update { it.copy(settingsOpen = false) }
            ComicEvent.AcceptSuggestion -> resolveSuggestion(true)
            ComicEvent.DeclineSuggestion -> resolveSuggestion(false)
            is ComicEvent.PatchPrefs -> rc.prefs.update(e.f)
        }
    }

    private fun load(id: Long) {
        bookId = id
        loadedFor = id
        loadJob?.cancel()
        pendingDetail = null
        _state.update { it.copy(loading = true, error = null, suggestion = null) }
        loadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val detail = core.getBook(id)
                val count = detail.book.pageCount
                if (count <= 0) {
                    _state.update { it.copy(loading = false, error = rc.getString(R.string.reader_error)) }
                    return@launch
                }
                if (rc.connectivity.online.value) {
                    // en çok 4 sn beklenir; engelleyici çağrı arka planda sürer, yerel konumla devam edilir
                    val pull = async { runCatching { core.pullBook(id) } }
                    val pulled = withTimeoutOrNull(4000) { pull.await() }
                    pulled?.exceptionOrNull()?.let { handleError(it) }
                }
                // başka cihazda daha yeni, farklı bir konum varsa sayfa yüklenmeden önce sorulur
                val sug = runCatching { core.positionSuggestion(id) }.getOrNull()
                if (sug != null) {
                    pendingDetail = detail
                    _state.update { it.copy(suggestion = sug, pageCount = count, loading = true) }
                    return@launch
                }
                finishLoad(id, detail)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failLoad(e)
            }
        }
    }

    /** "Oraya git" / "Burada kal": yanıt çekirdeğe işlenir, konum yeniden okunur ve yükleme tamamlanır. */
    private fun resolveSuggestion(accept: Boolean) {
        val detail = pendingDetail ?: return
        val id = bookId
        pendingDetail = null
        _state.update { it.copy(suggestion = null) }
        loadJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                if (accept) core.acceptPosition(id) else core.declinePosition(id)
                finishLoad(id, detail)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failLoad(e)
            }
        }
    }

    private fun finishLoad(id: Long, detail: BookDetail) {
        val count = detail.book.pageCount
        val page = core.getProgress(id).locator.toIntOrNull()?.takeIf { it in 0 until count } ?: 0
        val mode = rc.prefs.comicModeFor(id)
        val bookmarks = core.listBookmarks(id)
        savedPage = page
        tracker.start(id, (page + 1).toDouble() / count)
        _state.update {
            it.copy(
                loading = false, error = null, title = detail.book.title,
                seriesId = detail.book.seriesId, pageCount = count, page = page, mode = mode,
                bookmarks = bookmarks, nextBookId = detail.nextBookId,
                nextBookTitle = detail.nextBookTitle,
            )
        }
    }

    private fun failLoad(e: Exception) {
        handleError(e)
        _state.update { it.copy(loading = false, error = e.message ?: rc.getString(R.string.reader_error)) }
    }

    private fun pageChanged(p: Int) {
        val s = _state.value
        if (s.pageCount <= 0) return
        val page = p.coerceIn(0, s.pageCount - 1)
        val percent = ((page + 1).toDouble() / s.pageCount).coerceIn(0.0, 1.0)
        _state.update { it.copy(page = page) }
        tracker.addPage(page.toString())
        tracker.setPercent(percent)
        if (page != savedPage) {
            savedPage = page
            saveChannel.trySend(page)
        }
    }

    private fun toggleBookmark() {
        val id = bookId
        val page = _state.value.page
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                val existing = core.listBookmarks(id).firstOrNull { it.locator == page.toString() }
                if (existing != null) core.deleteBookmark(existing.localId)
                else core.addBookmark(id, page.toString(), "Sayfa ${page + 1}")
            }.onFailure { handleError(it) }
            reloadBookmarks()
            rc.sync.onLocalChange(2000)
        }
    }

    private fun reloadBookmarks() {
        runCatching { core.listBookmarks(bookId) }
            .onSuccess { list -> _state.update { it.copy(bookmarks = list) } }
    }

    private fun handleError(e: Throwable) {
        if (e is CoreException.Unauthorized) rc.prefs.update { it.copy(needsRelogin = true) }
    }

    override fun onCleared() {
        tracker.stop()
        // kanalda kalan son sayfa kapsam iptal edilmeden yazılır
        val page = saveChannel.tryReceive().getOrNull() ?: return
        val id = bookId
        val count = _state.value.pageCount.coerceAtLeast(1)
        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch {
            runCatching {
                core.saveProgress(id, page.toString(), (page + 1).toDouble() / count, LocalDate.now().toString())
            }
            rc.sync.onLocalChange(0)
        }
    }
}

/** Ses tuşu / dokunma ile geçerli moddaki kaydırıcıyı bir adım (+1 ileri, -1 geri) oynatır. */
private class StepHolder {
    var step: ((Int) -> Unit)? = null
}

@Composable
fun ComicReaderScreen(
    bookId: Long,
    onBack: (seriesId: Long) -> Unit,
    onNextBook: (Long) -> Unit,
    onSessionExpired: () -> Unit,
    vm: ComicReaderViewModel = viewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(bookId) { vm.onEvent(ComicEvent.Load(bookId)) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        vm.tracker.foreground = true
        vm.tracker.touch()
    }
    LifecycleEventEffect(Lifecycle.Event.ON_PAUSE) { vm.tracker.foreground = false }
    DisposableEffect(Unit) { onDispose { vm.tracker.stop() } }

    val stepper = remember { StepHolder() }
    // ileri/geri yönü RTL'de de mantıksaldır (ileri = sonraki sayfa)
    ReaderWindowEffects(state.prefs, onVolume = { forward ->
        stepper.step?.invoke(if (forward) 1 else -1)
        true
    })

    val app = LocalContext.current.applicationContext as RcApp
    Box(Modifier.fillMaxSize().background(comicBgColor(state.prefs.comicBg))) {
        val error = state.error
        when {
            state.loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
            error != null -> Column(
                Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                EmptyState(Icons.Outlined.ErrorOutline, error)
                Button(onClick = { vm.onEvent(ComicEvent.Retry) }) { Text(stringResource(R.string.action_retry)) }
            }
            else -> ComicBody(bookId, state, vm, stepper, onBack, onNextBook, onSessionExpired)
        }
        ReaderHint(state.prefs) { app.prefs.update { it.copy(readerHintSeen = true) } }
    }

    // C5: başka cihazda daha yeni, farklı bir konum var
    state.suggestion?.let { sug ->
        val page = sug.locator.toIntOrNull()
        val where = if (page != null) {
            stringResource(R.string.reader_remote_page, page + 1, state.pageCount)
        } else {
            "%${(sug.percent * 100).roundToInt()}"
        }
        RemotePositionDialog(
            where = where,
            onGo = { vm.onEvent(ComicEvent.AcceptSuggestion) },
            onStay = { vm.onEvent(ComicEvent.DeclineSuggestion) },
        )
    }
}

@Composable
private fun ComicBody(
    bookId: Long,
    state: ComicState,
    vm: ComicReaderViewModel,
    stepper: StepHolder,
    onBack: (Long) -> Unit,
    onNextBook: (Long) -> Unit,
    onSessionExpired: () -> Unit,
) {
    // yalnız yerelde olmayan bir sayfa 401 ile getirilemezse oturum düşmüş sayılır
    val onExpired: () -> Unit = {
        vm.markSessionExpired()
        onSessionExpired()
    }
    Box(Modifier.fillMaxSize()) {
        if (state.mode == "webtoon") WebtoonPages(bookId, state, vm, stepper, onExpired)
        else PagerPages(bookId, state, vm, stepper, onExpired)

        AnimatedVisibility(
            visible = state.barsVisible,
            modifier = Modifier.align(Alignment.TopCenter),
            enter = fadeIn(),
            exit = fadeOut(),
        ) { ComicTopBar(state, vm, onBack) }

        AnimatedVisibility(
            visible = state.barsVisible,
            modifier = Modifier.align(Alignment.BottomCenter),
            enter = fadeIn(),
            exit = fadeOut(),
        ) { ComicBottomBar(state, vm) }

        val nextId = state.nextBookId
        if (nextId != null && state.page == state.pageCount - 1) {
            Card(
                Modifier
                    .align(Alignment.BottomCenter)
                    .navigationBarsPadding()
                    .padding(start = Dimens.gutter, end = Dimens.gutter, bottom = if (state.barsVisible) 144.dp else Dimens.s5),
            ) {
                Row(
                    Modifier.padding(Dimens.s3),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
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
    }

    if (state.showBookmarks) {
        BookmarksSheet(
            items = state.bookmarks,
            onOpen = { vm.onEvent(ComicEvent.OpenBookmark(it)) },
            onDelete = { vm.onEvent(ComicEvent.DeleteBookmark(it)) },
            onDismiss = { vm.onEvent(ComicEvent.HideBookmarks) },
        )
    }
    if (state.settingsOpen) ComicSettingsSheet(state, vm)
}

@Composable
private fun ComicSettingsSheet(state: ComicState, vm: ComicReaderViewModel) {
    val prefs = state.prefs
    fun patch(f: (UiPrefs) -> UiPrefs) = vm.onEvent(ComicEvent.PatchPrefs(f))
    ReaderSettingsSheet(
        title = stringResource(R.string.comic_settings_title),
        onDismiss = { vm.onEvent(ComicEvent.CloseSettings) },
    ) {
        ReaderSection(stringResource(R.string.comic_direction), first = true) {
            ReaderChoiceRow(
                options = listOf(
                    "ltr" to stringResource(R.string.comic_dir_ltr),
                    "rtl" to stringResource(R.string.comic_dir_rtl),
                    "webtoon" to stringResource(R.string.comic_dir_webtoon),
                ),
                selected = state.mode,
                onSelect = { vm.onEvent(ComicEvent.SetMode(it)) },
            )
        }
        ReaderSection(stringResource(R.string.reader_background)) {
            ReaderChoiceRow(
                options = listOf(
                    "black" to stringResource(R.string.reader_theme_black),
                    "dark" to stringResource(R.string.reader_theme_dark),
                    "sepia" to stringResource(R.string.reader_theme_sepia),
                    "light" to stringResource(R.string.reader_theme_light),
                ),
                selected = prefs.comicBg,
                onSelect = { k -> patch { it.copy(comicBg = k) } },
            )
        }
        ReaderSection(stringResource(R.string.reader_section_behavior)) {
            ReaderSwitchRow(stringResource(R.string.settings_tap_zones), prefs.tapZones) { v ->
                patch { it.copy(tapZones = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_volume_keys), prefs.volumeKeys) { v ->
                patch { it.copy(volumeKeys = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_keep_awake), prefs.keepAwake) { v ->
                patch { it.copy(keepAwake = v) }
            }
            ReaderSwitchRow(stringResource(R.string.settings_immersive), prefs.immersive) { v ->
                patch { it.copy(immersive = v) }
            }
        }
    }
}

@Composable
private fun PagerPages(
    bookId: Long,
    state: ComicState,
    vm: ComicReaderViewModel,
    stepper: StepHolder,
    onExpired: () -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val count = state.pageCount
    val rtl = state.mode == "rtl"
    val tapZones = state.prefs.tapZones
    val pagerState = rememberPagerState(initialPage = state.page.coerceIn(0, count - 1)) { count }
    val loader = SingletonImageLoader.get(ctx)

    LaunchedEffect(pagerState) {
        snapshotFlow { pagerState.settledPage }.distinctUntilChanged().collect { p ->
            vm.onEvent(ComicEvent.PageChanged(p))
            // önbellek ısıtma: mantıksal ileri yönde sonraki iki sayfa
            for (n in (p + 1)..(p + 2)) {
                if (n < count) {
                    loader.enqueue(ImageRequest.Builder(ctx).data(RcImage.Page(bookId, n)).size(2000, 2000).build())
                }
            }
        }
    }
    LaunchedEffect(pagerState) {
        vm.effects.collect { if (it is ComicEffect.ScrollTo) pagerState.scrollToPage(it.page.coerceIn(0, count - 1)) }
    }
    DisposableEffect(pagerState) {
        stepper.step = { dir ->
            scope.launch { pagerState.animateScrollToPage((pagerState.currentPage + dir).coerceIn(0, count - 1)) }
        }
        onDispose { stepper.step = null }
    }

    HorizontalPager(
        state = pagerState,
        modifier = Modifier.fillMaxSize(),
        beyondViewportPageCount = 1,
        reverseLayout = rtl,
        key = { it },
    ) { page ->
        ZoomableBox(
            onTap = { offset, size ->
                val x = if (size.width > 0) offset.x / size.width else 0.5f
                // LTR: sol = geri, sağ = ileri; RTL'de tersi
                val dir = when {
                    !tapZones -> 0
                    x < 1f / 3f -> if (rtl) 1 else -1
                    x > 2f / 3f -> if (rtl) -1 else 1
                    else -> 0
                }
                if (dir == 0) {
                    vm.onEvent(ComicEvent.ToggleBars)
                } else {
                    scope.launch {
                        pagerState.animateScrollToPage((pagerState.currentPage + dir).coerceIn(0, count - 1))
                    }
                }
            },
        ) {
            val request = remember(bookId, page) {
                ImageRequest.Builder(ctx).data(RcImage.Page(bookId, page)).size(2000, 2000).build()
            }
            AsyncImage(
                model = request,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxSize(),
                onError = { if (it.result.throwable is CoreException.Unauthorized) onExpired() },
            )
        }
    }
}

@Composable
private fun WebtoonPages(
    bookId: Long,
    state: ComicState,
    vm: ComicReaderViewModel,
    stepper: StepHolder,
    onExpired: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val count = state.pageCount
    val listState = rememberLazyListState(initialFirstVisibleItemIndex = state.page.coerceIn(0, count - 1))

    LaunchedEffect(listState) {
        snapshotFlow { listState.firstVisibleItemIndex }.distinctUntilChanged().collect {
            vm.onEvent(ComicEvent.PageChanged(it))
        }
    }
    LaunchedEffect(listState) {
        vm.effects.collect { if (it is ComicEffect.ScrollTo) listState.scrollToItem(it.page.coerceIn(0, count - 1)) }
    }
    DisposableEffect(listState) {
        stepper.step = { dir ->
            scope.launch { listState.animateScrollBy(dir * listState.layoutInfo.viewportSize.height * 0.9f) }
        }
        onDispose { stepper.step = null }
    }

    // webtoon'da yakınlaştırma yok; dokunma yalnız çubukları açar/kapatır
    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .pointerInput(Unit) { detectTapGestures(onTap = { vm.onEvent(ComicEvent.ToggleBars) }) },
    ) {
        items(count) { page ->
            AsyncImage(
                model = RcImage.Page(bookId, page),
                contentDescription = null,
                contentScale = ContentScale.FillWidth,
                modifier = Modifier.fillMaxWidth().heightIn(min = 320.dp),
                onError = { if (it.result.throwable is CoreException.Unauthorized) onExpired() },
            )
        }
    }
}

@Composable
private fun ComicTopBar(state: ComicState, vm: ComicReaderViewModel, onBack: (Long) -> Unit) {
    val bookmarked = state.bookmarks.any { it.locator == state.page.toString() }
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
            IconButton(onClick = { vm.onEvent(ComicEvent.ToggleBookmark) }) {
                Icon(
                    if (bookmarked) Icons.Filled.Bookmark else Icons.Filled.BookmarkBorder,
                    contentDescription = stringResource(
                        if (bookmarked) R.string.reader_bookmark_remove else R.string.reader_bookmark_add,
                    ),
                )
            }
            IconButton(onClick = { vm.onEvent(ComicEvent.ShowBookmarks) }) {
                Icon(Icons.Outlined.Bookmarks, contentDescription = stringResource(R.string.reader_bookmarks))
            }
        }
    }
}

@Composable
private fun ComicBottomBar(state: ComicState, vm: ComicReaderViewModel) {
    val count = state.pageCount
    var drag by remember { mutableStateOf<Float?>(null) }
    val shown = drag ?: state.page.toFloat()
    Surface(color = MaterialTheme.colorScheme.surface.copy(alpha = 0.94f), modifier = Modifier.fillMaxWidth()) {
        Column(
            Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal)).padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
        ) {
            Text(
                stringResource(R.string.reader_page_of, shown.roundToInt() + 1, count),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
            if (count > 1) {
                // RTL'de kaydırıcı da sağdan sola ilerler
                val dir = if (state.mode == "rtl") LayoutDirection.Rtl else LocalLayoutDirection.current
                CompositionLocalProvider(LocalLayoutDirection provides dir) {
                    Slider(
                        value = shown,
                        onValueChange = { drag = it },
                        valueRange = 0f..(count - 1).toFloat(),
                        steps = 0, // her sayfa için tik noktası izi doldurur; değer bırakınca yuvarlanır
                        onValueChangeFinished = {
                            drag?.let { vm.onEvent(ComicEvent.SeekTo(it.roundToInt())) }
                            drag = null
                        },
                    )
                }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                TextButton(onClick = { vm.onEvent(ComicEvent.OpenSettings) }) {
                    Icon(Icons.Outlined.Settings, contentDescription = null)
                    Spacer(Modifier.width(4.dp))
                    Text(stringResource(R.string.reader_settings_label))
                }
            }
        }
    }
}
