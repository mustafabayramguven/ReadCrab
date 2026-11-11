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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.outlined.Bookmarks
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.Color
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
import io.github.mustafabayramguven.readcrab.core.BookmarkItem
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.image.RcImage
import io.github.mustafabayramguven.readcrab.ui.common.EmptyState
import io.github.mustafabayramguven.readcrab.ui.reader.BookmarksSheet
import io.github.mustafabayramguven.readcrab.ui.reader.ReaderWindowEffects
import io.github.mustafabayramguven.readcrab.ui.reader.ReadingTracker
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.work.Scheduler
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
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
    val barsVisible: Boolean = true, val showBookmarks: Boolean = false,
    val bookmarks: List<BookmarkItem> = emptyList(),
    val nextBookId: Long? = null, val nextBookTitle: String? = null, val prefs: UiPrefs = UiPrefs(),
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

    init {
        viewModelScope.launch { rc.prefs.state.collect { p -> _state.update { it.copy(prefs = p) } } }
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
                Scheduler.kickSync(rc)
            }
            ComicEvent.Retry -> load(bookId)
        }
    }

    private fun load(id: Long) {
        bookId = id
        loadedFor = id
        loadJob?.cancel()
        _state.update { it.copy(loading = true, error = null) }
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
                val page = core.getProgress(id).locator.toIntOrNull()?.takeIf { it in 0 until count } ?: 0
                val mode = rc.prefs.comicModeFor(id)
                val bookmarks = core.listBookmarks(id)
                tracker.start(id, (page + 1).toDouble() / count)
                _state.update {
                    it.copy(
                        loading = false, error = null, title = detail.book.title,
                        seriesId = detail.book.seriesId, pageCount = count, page = page, mode = mode,
                        bookmarks = bookmarks, nextBookId = detail.nextBookId,
                        nextBookTitle = detail.nextBookTitle,
                    )
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                handleError(e)
                _state.update { it.copy(loading = false, error = e.message ?: rc.getString(R.string.reader_error)) }
            }
        }
    }

    private fun pageChanged(p: Int) {
        val s = _state.value
        if (s.pageCount <= 0) return
        val page = p.coerceIn(0, s.pageCount - 1)
        val percent = ((page + 1).toDouble() / s.pageCount).coerceIn(0.0, 1.0)
        _state.update { it.copy(page = page) }
        tracker.addPage(page.toString())
        tracker.setPercent(percent)
        val id = bookId
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { core.saveProgress(id, page.toString(), percent, LocalDate.now().toString()) }
                .onFailure { handleError(it) }
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
            Scheduler.kickSync(rc)
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
    LaunchedEffect(state.prefs.needsRelogin) { if (state.prefs.needsRelogin) onSessionExpired() }
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

    Box(Modifier.fillMaxSize().background(Color.Black)) {
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
            else -> ComicBody(bookId, state, vm, stepper, onBack, onNextBook)
        }
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
) {
    Box(Modifier.fillMaxSize()) {
        if (state.mode == "webtoon") WebtoonPages(bookId, state, vm, stepper)
        else PagerPages(bookId, state, vm, stepper)

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
                    .padding(start = Dimens.gutter, end = Dimens.gutter, bottom = if (state.barsVisible) 96.dp else Dimens.s5),
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
}

@Composable
private fun PagerPages(bookId: Long, state: ComicState, vm: ComicReaderViewModel, stepper: StepHolder) {
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
            )
        }
    }
}

@Composable
private fun WebtoonPages(bookId: Long, state: ComicState, vm: ComicReaderViewModel, stepper: StepHolder) {
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
            )
        }
    }
}

@Composable
private fun ComicTopBar(state: ComicState, vm: ComicReaderViewModel, onBack: (Long) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val bookmarked = state.bookmarks.any { it.locator == state.page.toString() }
    Surface(color = MaterialTheme.colorScheme.surface.copy(alpha = 0.94f), modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.statusBarsPadding().height(Dimens.barHeight),
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
            Box {
                IconButton(onClick = { menu = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.action_more))
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    val modes = listOf(
                        "ltr" to R.string.comic_dir_ltr,
                        "rtl" to R.string.comic_dir_rtl,
                        "webtoon" to R.string.comic_dir_webtoon,
                    )
                    for ((key, label) in modes) {
                        DropdownMenuItem(
                            text = { Text(stringResource(label)) },
                            onClick = {
                                menu = false
                                vm.onEvent(ComicEvent.SetMode(key))
                            },
                            leadingIcon = {
                                if (state.mode == key) Icon(Icons.Filled.Check, contentDescription = null)
                            },
                        )
                    }
                }
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
            Modifier.navigationBarsPadding().padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
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
                        steps = (count - 2).coerceAtLeast(0),
                        onValueChangeFinished = {
                            drag?.let { vm.onEvent(ComicEvent.SeekTo(it.roundToInt())) }
                            drag = null
                        },
                    )
                }
            }
        }
    }
}
