package io.github.mustafabayramguven.readcrab.ui.series

import android.Manifest
import android.app.Application
import android.os.Build
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.BookItem
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.DownloadInfo
import io.github.mustafabayramguven.readcrab.core.DownloadState
import io.github.mustafabayramguven.readcrab.core.ReadStatus
import io.github.mustafabayramguven.readcrab.core.SeriesDetail
import io.github.mustafabayramguven.readcrab.ui.common.CoverImage
import io.github.mustafabayramguven.readcrab.ui.common.formatPercent
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.work.Scheduler
import java.time.LocalDate
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SeriesState(
    val loaded: Boolean = false, val detail: SeriesDetail? = null, val online: Boolean = true,
    val message: String? = null, val busy: Boolean = false,
)

sealed interface SeriesEvent {
    data class Download(val bookIds: List<Long>) : SeriesEvent
    data class RemoveDownload(val bookIds: List<Long>) : SeriesEvent
    data class MarkStatus(val bookIds: List<Long>, val status: ReadStatus) : SeriesEvent
    data object MessageShown : SeriesEvent
}

class SeriesViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = app as RcApp

    private val _state = MutableStateFlow(SeriesState())
    val state: StateFlow<SeriesState> = _state.asStateFlow()

    private var seriesId = 0L

    init {
        viewModelScope.launch { rc.connectivity.online.collect { on -> _state.update { it.copy(online = on) } } }
    }

    fun load(id: Long) {
        seriesId = id
        reload()
    }

    fun onEvent(e: SeriesEvent) {
        when (e) {
            is SeriesEvent.Download -> action { rc.downloads.enqueue(e.bookIds) }
            is SeriesEvent.RemoveDownload -> action { rc.downloads.remove(e.bookIds) }
            is SeriesEvent.MarkStatus -> action {
                for (id in e.bookIds) rc.core.setStatus(id, e.status, LocalDate.now().toString())
                Scheduler.kickSync(getApplication<Application>())
            }
            SeriesEvent.MessageShown -> _state.update { it.copy(message = null) }
        }
    }

    private fun reload() {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val d = rc.core.getSeries(seriesId)
                _state.update { it.copy(loaded = true, detail = d) }
            } catch (e: CoreException) {
                fail(e)
            }
        }
    }

    private fun action(block: () -> Unit) {
        _state.update { it.copy(busy = true) }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                block()
            } catch (e: CoreException) {
                fail(e)
                return@launch
            }
            _state.update { it.copy(busy = false) }
            reload()
        }
    }

    private fun fail(e: CoreException) {
        if (e is CoreException.Unauthorized) rc.prefs.update { it.copy(needsRelogin = true) }
        _state.update { it.copy(busy = false, message = e.message) }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SeriesScreen(
    seriesId: Long,
    onBack: () -> Unit,
    onOpenBook: (Long) -> Unit,
    onSessionExpired: () -> Unit,
    vm: SeriesViewModel = viewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()
    val ctx = LocalContext.current
    val app = ctx.applicationContext as RcApp
    val prefs by app.prefs.state.collectAsStateWithLifecycle()
    val infos by app.downloads.infos.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    var menuOpen by remember { mutableStateOf(false) }
    var notifAsked by rememberSaveable { mutableStateOf(false) }
    val notifLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }

    LaunchedEffect(seriesId) { vm.load(seriesId) }
    LaunchedEffect(prefs.needsRelogin) { if (prefs.needsRelogin) onSessionExpired() }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            vm.onEvent(SeriesEvent.MessageShown)
        }
    }

    val detail = state.detail
    val books = detail?.books ?: emptyList()

    // Satırı olmayan kitap indirilmemiştir (infos yalnız satırı olanları içerir)
    fun dl(id: Long): DownloadInfo =
        infos[id] ?: DownloadInfo(bookId = id, state = DownloadState.NONE, partsDone = 0, partsTotal = 0, bytes = 0L, error = null)

    fun enqueue(ids: List<Long>) {
        if (Build.VERSION.SDK_INT >= 33 && !notifAsked) {
            notifAsked = true
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        vm.onEvent(SeriesEvent.Download(ids))
    }

    val target = books.firstOrNull { it.status != ReadStatus.FINISHED } ?: books.firstOrNull()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(detail?.name ?: "") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
                actions = {
                    Box {
                        IconButton(onClick = { menuOpen = true }) {
                            Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.action_more))
                        }
                        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.series_download_all)) },
                                onClick = {
                                    menuOpen = false
                                    enqueue(books.filter { it.error == null && dl(it.id).state != DownloadState.DONE }.map { it.id })
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.series_delete_all)) },
                                onClick = {
                                    menuOpen = false
                                    vm.onEvent(SeriesEvent.RemoveDownload(books.map { it.id }))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.series_mark_all_read)) },
                                onClick = {
                                    menuOpen = false
                                    vm.onEvent(SeriesEvent.MarkStatus(books.map { it.id }, ReadStatus.FINISHED))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.series_mark_all_unread)) },
                                onClick = {
                                    menuOpen = false
                                    vm.onEvent(SeriesEvent.MarkStatus(books.map { it.id }, ReadStatus.UNREAD))
                                },
                            )
                        }
                    }
                },
            )
        },
        floatingActionButton = {
            if (target != null) {
                ExtendedFloatingActionButton(
                    onClick = { onOpenBook(target.id) },
                    icon = { Icon(Icons.Filled.PlayArrow, contentDescription = null) },
                    text = {
                        Text(
                            stringResource(if (target.percent > 0) R.string.series_continue else R.string.series_read),
                        )
                    },
                )
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad)) {
            items(books, key = { it.id }) { b ->
                BookRow(
                    b = b,
                    dl = dl(b.id),
                    online = state.online,
                    onTap = {
                        val err = b.error
                        when {
                            err != null -> scope.launch { snackbar.showSnackbar(ctx.getString(R.string.book_unavailable, err)) }
                            !state.online && dl(b.id).state != DownloadState.DONE ->
                                scope.launch { snackbar.showSnackbar(ctx.getString(R.string.book_needs_network)) }
                            else -> onOpenBook(b.id)
                        }
                    },
                    onDownload = { enqueue(listOf(b.id)) },
                    onRemove = { vm.onEvent(SeriesEvent.RemoveDownload(listOf(b.id))) },
                    onMark = { st -> vm.onEvent(SeriesEvent.MarkStatus(listOf(b.id), st)) },
                )
            }
        }
    }
}

@Composable
private fun BookRow(
    b: BookItem,
    dl: DownloadInfo,
    online: Boolean,
    onTap: () -> Unit,
    onDownload: () -> Unit,
    onRemove: () -> Unit,
    onMark: (ReadStatus) -> Unit,
) {
    val ctx = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    val dim = b.error != null || (!online && dl.state != DownloadState.DONE)

    Box(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .fillMaxWidth()
                .alpha(if (dim) 0.5f else 1f)
                .combinedClickable(onClick = onTap, onLongClick = { menu = true })
                .padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CoverImage(b.id, b.coverV, Modifier.width(Dimens.bookRowCover))
            Spacer(Modifier.width(Dimens.s3))
            Column(Modifier.weight(1f)) {
                Text(b.title, style = MaterialTheme.typography.bodyLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                val kind = if (b.isComic) {
                    stringResource(R.string.book_meta_comic, b.pageCount)
                } else {
                    stringResource(R.string.book_meta_text, b.pageCount)
                }
                Text(
                    "$kind · ${b.format.uppercase()} · ${Formatter.formatShortFileSize(ctx, b.size)}",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val statusText = when (b.status) {
                    ReadStatus.UNREAD -> stringResource(R.string.book_status_unread)
                    ReadStatus.READING -> stringResource(R.string.book_status_reading, formatPercent(b.percent))
                    ReadStatus.FINISHED -> stringResource(R.string.book_status_finished)
                    ReadStatus.DROPPED -> stringResource(R.string.book_status_dropped)
                }
                Text(statusText, style = MaterialTheme.typography.labelMedium)
                LinearProgressIndicator(
                    progress = { b.percent.toFloat() },
                    modifier = Modifier.fillMaxWidth().padding(top = Dimens.s1),
                )
            }
            when (dl.state) {
                DownloadState.NONE -> IconButton(onClick = onDownload) {
                    Icon(Icons.Outlined.Download, contentDescription = stringResource(R.string.book_download))
                }
                DownloadState.QUEUED, DownloadState.RUNNING -> Column(
                    Modifier.padding(start = Dimens.s2).combinedClickable(onClick = onRemove),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CircularProgressIndicator(
                        progress = { if (dl.partsTotal > 0) dl.partsDone.toFloat() / dl.partsTotal else 0f },
                        modifier = Modifier.size(24.dp),
                    )
                    Text(
                        stringResource(R.string.download_running, dl.partsDone, dl.partsTotal),
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
                DownloadState.DONE -> IconButton(onClick = { menu = true }) {
                    Icon(Icons.Filled.CheckCircle, contentDescription = stringResource(R.string.download_done))
                }
                DownloadState.FAILED -> IconButton(onClick = onDownload) {
                    Icon(Icons.Filled.ErrorOutline, contentDescription = stringResource(R.string.download_failed))
                }
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.book_mark_read)) },
                onClick = { menu = false; onMark(ReadStatus.FINISHED) },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.book_mark_unread)) },
                onClick = { menu = false; onMark(ReadStatus.UNREAD) },
            )
            if (dl.state == DownloadState.NONE) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.book_download)) },
                    onClick = { menu = false; onDownload() },
                )
            } else {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.book_delete_download)) },
                    onClick = { menu = false; onRemove() },
                )
            }
        }
    }
}
