package io.github.mustafabayramguven.readcrab.ui.library

import android.app.Application
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DownloadDone
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.LocalFireDepartment
import androidx.compose.material.icons.outlined.MenuBook
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
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
import io.github.mustafabayramguven.readcrab.core.ContinueItem
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.SeriesItem
import io.github.mustafabayramguven.readcrab.sync.SyncKind
import io.github.mustafabayramguven.readcrab.ui.common.CoverImage
import io.github.mustafabayramguven.readcrab.ui.common.EmptyState
import io.github.mustafabayramguven.readcrab.ui.common.OfflineBanner
import io.github.mustafabayramguven.readcrab.ui.common.SyncIndicatorButton
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class LibraryState(
    val query: String = "", val onlyDownloaded: Boolean = false,
    val series: List<SeriesItem> = emptyList(), val continueItems: List<ContinueItem> = emptyList(),
    val refreshing: Boolean = false, val online: Boolean = true, val message: String? = null,
    val loaded: Boolean = false,
)

sealed interface LibraryEvent {
    data class Query(val q: String) : LibraryEvent
    data class OnlyDownloaded(val on: Boolean) : LibraryEvent
    data object Refresh : LibraryEvent
    data object MessageShown : LibraryEvent
}

class LibraryViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = app as RcApp

    private val _state = MutableStateFlow(LibraryState())
    val state: StateFlow<LibraryState> = _state.asStateFlow()

    init {
        viewModelScope.launch { rc.connectivity.online.collect { on -> _state.update { it.copy(online = on) } } }
        viewModelScope.launch { rc.downloads.infos.collect { reload() } }
        viewModelScope.launch { rc.sync.completed.collect { reload() } }
        viewModelScope.launch {
            rc.sync.indicator.collect { ind ->
                val err = ind.error
                val msg = if (ind.kind == SyncKind.ERROR && err != null) {
                    getApplication<Application>().getString(R.string.library_refresh_failed, err)
                } else {
                    null
                }
                _state.update { it.copy(refreshing = ind.kind == SyncKind.SYNCING, message = msg ?: it.message) }
            }
        }
    }

    fun onEvent(e: LibraryEvent) {
        when (e) {
            is LibraryEvent.Query -> { _state.update { it.copy(query = e.q) }; reload() }
            is LibraryEvent.OnlyDownloaded -> { _state.update { it.copy(onlyDownloaded = e.on) }; reload() }
            LibraryEvent.Refresh -> rc.sync.syncNow()
            LibraryEvent.MessageShown -> _state.update { it.copy(message = null) }
        }
    }

    private fun reload() {
        viewModelScope.launch(Dispatchers.IO) {
            val s = _state.value
            try {
                val series = rc.core.listSeries(s.query, s.onlyDownloaded)
                val cont = rc.core.continueReading()
                _state.update { it.copy(series = series, continueItems = cont, loaded = true) }
            } catch (e: CoreException) {
                _state.update { it.copy(loaded = true, message = e.message) }
            }
        }
    }

}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LibraryScreen(
    onOpenSeries: (Long) -> Unit,
    onOpenBook: (Long) -> Unit,
    onOpenSettings: () -> Unit,
    onOpenStats: () -> Unit,
    onSessionExpired: () -> Unit,
    vm: LibraryViewModel = viewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()
    val app = LocalContext.current.applicationContext as RcApp
    val prefs by app.prefs.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    var searching by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(prefs.needsRelogin) { if (prefs.needsRelogin) onSessionExpired() }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            vm.onEvent(LibraryEvent.MessageShown)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    if (searching) {
                        val focus = remember { FocusRequester() }
                        LaunchedEffect(Unit) { focus.requestFocus() }
                        OutlinedTextField(
                            value = state.query,
                            onValueChange = { vm.onEvent(LibraryEvent.Query(it)) },
                            placeholder = { Text(stringResource(R.string.library_search_hint)) },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth().focusRequester(focus),
                        )
                    } else {
                        Text(stringResource(R.string.library_title))
                    }
                },
                actions = {
                    // aramada yalnız kapat düğmesi kalır; dört ikon arama alanını sıkıştırıyordu
                    if (!searching) {
                        IconButton(onClick = onOpenStats) {
                            Icon(Icons.Outlined.LocalFireDepartment, contentDescription = stringResource(R.string.stats_open))
                        }
                        SyncIndicatorButton()
                    }
                    IconButton(onClick = {
                        if (searching) vm.onEvent(LibraryEvent.Query(""))
                        searching = !searching
                    }) {
                        if (searching) {
                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.action_close))
                        } else {
                            Icon(Icons.Filled.Search, contentDescription = stringResource(R.string.action_search))
                        }
                    }
                    if (!searching) {
                        IconButton(onClick = onOpenSettings) {
                            Icon(Icons.Outlined.Settings, contentDescription = stringResource(R.string.action_settings))
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { pad ->
        Column(Modifier.fillMaxSize().padding(pad)) {
            if (!state.online) OfflineBanner(stringResource(R.string.library_offline))
            FilterChip(
                selected = state.onlyDownloaded,
                onClick = { vm.onEvent(LibraryEvent.OnlyDownloaded(!state.onlyDownloaded)) },
                label = { Text(stringResource(R.string.library_only_downloaded)) },
                modifier = Modifier.padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
            )
            PullToRefreshBox(
                isRefreshing = state.refreshing,
                onRefresh = { vm.onEvent(LibraryEvent.Refresh) },
                modifier = Modifier.weight(1f),
            ) {
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(Dimens.gridMin),
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(Dimens.gutter),
                    horizontalArrangement = Arrangement.spacedBy(Dimens.gridGap),
                    verticalArrangement = Arrangement.spacedBy(Dimens.gridGap),
                ) {
                    if (state.query.isEmpty() && state.continueItems.isNotEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }) {
                            ContinueRow(state.continueItems, onOpenBook)
                        }
                    }
                    items(state.series, key = { it.id }) { s ->
                        SeriesCard(s, onClick = { onOpenSeries(s.id) })
                    }
                    if (state.loaded && state.series.isEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }) {
                            EmptyState(
                                Icons.Outlined.MenuBook,
                                stringResource(
                                    when {
                                        state.query.isNotEmpty() -> R.string.library_empty_search
                                        state.onlyDownloaded -> R.string.library_empty_downloaded
                                        else -> R.string.library_empty
                                    },
                                ),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ContinueRow(items: List<ContinueItem>, onOpenBook: (Long) -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        Text(
            stringResource(R.string.library_continue),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(bottom = Dimens.s2),
        )
        LazyRow(horizontalArrangement = Arrangement.spacedBy(Dimens.gridGap)) {
            items(items, key = { it.bookId }) { c ->
                Column(Modifier.width(Dimens.continueCardWidth).clickable { onOpenBook(c.bookId) }) {
                    CoverImage(c.bookId, c.coverV, Modifier.fillMaxWidth())
                    LinearProgressIndicator(
                        progress = { c.percent.toFloat() },
                        modifier = Modifier.fillMaxWidth().padding(top = Dimens.s1),
                    )
                    Text(
                        c.title,
                        style = MaterialTheme.typography.labelLarge,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun SeriesCard(s: SeriesItem, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Box {
            CoverImage(s.coverBookId, s.coverV, Modifier.fillMaxWidth())
            if (s.bookCount > 0 && s.downloadedCount == s.bookCount) {
                // zeminsiz ikon kapak üstünde kayboluyordu
                Icon(
                    Icons.Filled.DownloadDone,
                    contentDescription = stringResource(R.string.download_done),
                    tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(Dimens.s1)
                        .background(MaterialTheme.colorScheme.primaryContainer, CircleShape)
                        .padding(2.dp)
                        .size(18.dp),
                )
            }
        }
        Text(
            s.name,
            style = MaterialTheme.typography.labelLarge,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = Dimens.s1),
        )
        Text(
            stringResource(R.string.series_books_count, s.bookCount),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (s.percent > 0) {
            LinearProgressIndicator(
                progress = { s.percent.toFloat() },
                modifier = Modifier.fillMaxWidth().height(Dimens.progressTrack),
            )
        }
    }
}
