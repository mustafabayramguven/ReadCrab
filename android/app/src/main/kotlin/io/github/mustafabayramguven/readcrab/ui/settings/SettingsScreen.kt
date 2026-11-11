package io.github.mustafabayramguven.readcrab.ui.settings

import android.app.Application
import android.os.Build
import android.text.format.DateUtils
import android.text.format.Formatter
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.TEXT_FONTS
import io.github.mustafabayramguven.readcrab.UiPrefs
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.SessionInfo
import io.github.mustafabayramguven.readcrab.core.StorageUsage
import io.github.mustafabayramguven.readcrab.core.SyncStatus
import io.github.mustafabayramguven.readcrab.ui.common.ConfirmDialog
import io.github.mustafabayramguven.readcrab.ui.common.SettingsSwitchRow
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.ui.theme.ThemeMode
import io.github.mustafabayramguven.readcrab.work.Scheduler
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

data class SettingsState(
    val prefs: UiPrefs, val session: SessionInfo?, val sync: SyncStatus?, val storage: StorageUsage?,
    val syncing: Boolean = false, val message: String? = null, val confirmLogout: Boolean = false,
)

sealed interface SettingsEvent {
    data class Patch(val f: (UiPrefs) -> UiPrefs) : SettingsEvent
    data object SyncNow : SettingsEvent
    data object ClearCache : SettingsEvent
    data object AskLogout : SettingsEvent
    data object CancelLogout : SettingsEvent
    data object ConfirmLogout : SettingsEvent
    data object MessageShown : SettingsEvent
}

sealed interface SettingsEffect { data object LoggedOut : SettingsEffect }

class SettingsViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = app as RcApp

    private val _state = MutableStateFlow(SettingsState(rc.prefs.state.value, null, null, null))
    val state: StateFlow<SettingsState> = _state.asStateFlow()

    private val _effects = MutableSharedFlow<SettingsEffect>(extraBufferCapacity = 1)
    val effects: SharedFlow<SettingsEffect> = _effects.asSharedFlow()

    init {
        viewModelScope.launch { rc.prefs.state.collect { p -> _state.update { it.copy(prefs = p) } } }
        refresh()
    }

    private fun refresh() {
        viewModelScope.launch(Dispatchers.IO) {
            val session = rc.core.session()
            val sync = rc.core.syncStatus()
            val storage = try { rc.core.storageUsage() } catch (e: CoreException) { null }
            _state.update { it.copy(session = session, sync = sync, storage = storage) }
        }
    }

    fun onEvent(e: SettingsEvent) {
        when (e) {
            is SettingsEvent.Patch -> rc.prefs.update(e.f)
            SettingsEvent.SyncNow -> syncNow()
            SettingsEvent.ClearCache -> clearCache()
            SettingsEvent.AskLogout -> _state.update { it.copy(confirmLogout = true) }
            SettingsEvent.CancelLogout -> _state.update { it.copy(confirmLogout = false) }
            SettingsEvent.ConfirmLogout -> logout()
            SettingsEvent.MessageShown -> _state.update { it.copy(message = null) }
        }
    }

    private fun syncNow() {
        _state.update { it.copy(syncing = true) }
        viewModelScope.launch(Dispatchers.IO) {
            val msg = try {
                rc.core.syncNow()
                getApplication<Application>().getString(R.string.sync_done)
            } catch (e: CoreException.Unauthorized) {
                rc.prefs.update { it.copy(needsRelogin = true) } // ekran onSessionExpired'ı prefs'ten çağırır
                null
            } catch (e: CoreException) {
                getApplication<Application>().getString(R.string.library_refresh_failed, e.message ?: "")
            }
            _state.update { it.copy(syncing = false, message = msg) }
            refresh()
        }
    }

    private fun clearCache() {
        viewModelScope.launch(Dispatchers.IO) {
            val msg = try {
                rc.core.clearCache()
                getApplication<Application>().getString(R.string.settings_cache_cleared)
            } catch (e: CoreException) {
                e.message
            }
            _state.update { it.copy(message = msg) }
            refresh()
        }
    }

    private fun logout() {
        _state.update { it.copy(confirmLogout = false) }
        viewModelScope.launch(Dispatchers.IO) {
            rc.core.logout()
            Scheduler.cancelAll(getApplication<Application>())
            rc.downloads.refresh()
            _effects.emit(SettingsEffect.LoggedOut)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onBack: () -> Unit, onLoggedOut: () -> Unit, onSessionExpired: () -> Unit, vm: SettingsViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    val ctx = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    val p = state.prefs

    LaunchedEffect(Unit) {
        vm.effects.collect { e -> when (e) { SettingsEffect.LoggedOut -> onLoggedOut() } }
    }
    LaunchedEffect(p.needsRelogin) { if (p.needsRelogin) onSessionExpired() }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            vm.onEvent(SettingsEvent.MessageShown)
        }
    }

    fun patch(f: (UiPrefs) -> UiPrefs) = vm.onEvent(SettingsEvent.Patch(f))

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { pad ->
        Column(Modifier.fillMaxWidth().padding(pad).verticalScroll(rememberScrollState())) {
            // Görünüm
            Section(R.string.settings_section_appearance) {
                Text(stringResource(R.string.settings_theme), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                val modes = ThemeMode.entries
                val labels = listOf(R.string.theme_system, R.string.theme_light, R.string.theme_dark)
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = Dimens.gutter)) {
                    modes.forEachIndexed { i, m ->
                        SegmentedButton(
                            selected = p.themeMode == m,
                            onClick = { patch { it.copy(themeMode = m) } },
                            shape = SegmentedButtonDefaults.itemShape(index = i, count = modes.size),
                        ) { Text(stringResource(labels[i])) }
                    }
                }
                SettingsSwitchRow(stringResource(R.string.settings_amoled), stringResource(R.string.settings_amoled_desc), p.amoled) { v -> patch { it.copy(amoled = v) } }
                if (Build.VERSION.SDK_INT >= 31) {
                    SettingsSwitchRow(stringResource(R.string.settings_dynamic), stringResource(R.string.settings_dynamic_desc), p.dynamicColor) { v -> patch { it.copy(dynamicColor = v) } }
                }
            }
            HorizontalDivider()

            // Okuyucu
            Section(R.string.settings_section_reader) {
                Text(stringResource(R.string.reader_theme), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                ChipRow(
                    listOf(
                        "auto" to stringResource(R.string.reader_theme_auto),
                        "light" to stringResource(R.string.reader_theme_light),
                        "dark" to stringResource(R.string.reader_theme_dark),
                        "sepia" to stringResource(R.string.reader_theme_sepia),
                        "black" to stringResource(R.string.reader_theme_black),
                    ),
                    p.readerTheme,
                ) { v -> patch { it.copy(readerTheme = v) } }

                Text(stringResource(R.string.reader_font), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                ChipRow(TEXT_FONTS, p.textFont) { v -> patch { it.copy(textFont = v) } }

                Text(
                    "${stringResource(R.string.reader_font_size)}: ${p.textSize}",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )
                Slider(
                    value = p.textSize.toFloat(),
                    onValueChange = { v -> patch { it.copy(textSize = v.roundToInt()) } },
                    valueRange = 14f..28f,
                    steps = 6,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )

                Text(
                    "${stringResource(R.string.reader_line_height)}: ${"%.1f".format(p.lineHeight)}",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )
                Slider(
                    value = p.lineHeight,
                    onValueChange = { v -> patch { it.copy(lineHeight = (v * 10).roundToInt() / 10f) } },
                    valueRange = 1.2f..2.2f,
                    steps = 9,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )

                Text(
                    "${stringResource(R.string.reader_margin)}: ${p.margin}",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )
                Slider(
                    value = p.margin.toFloat(),
                    onValueChange = { v -> patch { it.copy(margin = v.roundToInt()) } },
                    valueRange = 0f..20f,
                    steps = 19,
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                )

                Text(stringResource(R.string.reader_align), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                val aligns = listOf("left" to R.string.reader_align_left, "justify" to R.string.reader_align_justify)
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = Dimens.gutter)) {
                    aligns.forEachIndexed { i, (key, label) ->
                        SegmentedButton(
                            selected = p.align == key,
                            onClick = { patch { it.copy(align = key) } },
                            shape = SegmentedButtonDefaults.itemShape(index = i, count = aligns.size),
                        ) { Text(stringResource(label)) }
                    }
                }

                Text(stringResource(R.string.reader_mode), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                val modesReader = listOf("page" to R.string.reader_mode_page, "scroll" to R.string.reader_mode_scroll)
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = Dimens.gutter)) {
                    modesReader.forEachIndexed { i, (key, label) ->
                        SegmentedButton(
                            selected = p.textMode == key,
                            onClick = { patch { it.copy(textMode = key) } },
                            shape = SegmentedButtonDefaults.itemShape(index = i, count = modesReader.size),
                        ) { Text(stringResource(label)) }
                    }
                }

                Text(stringResource(R.string.comic_direction), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = Dimens.gutter))
                ChipRow(
                    listOf(
                        "ltr" to stringResource(R.string.comic_dir_ltr),
                        "rtl" to stringResource(R.string.comic_dir_rtl),
                        "webtoon" to stringResource(R.string.comic_dir_webtoon),
                    ),
                    p.comicMode,
                ) { v -> patch { it.copy(comicMode = v) } }

                SettingsSwitchRow(stringResource(R.string.settings_immersive), null, p.immersive) { v -> patch { it.copy(immersive = v) } }
                SettingsSwitchRow(stringResource(R.string.settings_keep_awake), null, p.keepAwake) { v -> patch { it.copy(keepAwake = v) } }
                SettingsSwitchRow(stringResource(R.string.settings_tap_zones), stringResource(R.string.settings_tap_zones_desc), p.tapZones) { v -> patch { it.copy(tapZones = v) } }
                SettingsSwitchRow(stringResource(R.string.settings_volume_keys), null, p.volumeKeys) { v -> patch { it.copy(volumeKeys = v) } }
            }
            HorizontalDivider()

            // İndirmeler
            Section(R.string.settings_section_downloads) {
                SettingsSwitchRow(stringResource(R.string.settings_wifi_only), null, p.wifiOnly) { v -> patch { it.copy(wifiOnly = v) } }
                state.storage?.let { st ->
                    val pad0 = Modifier.padding(horizontal = Dimens.gutter)
                    Text(stringResource(R.string.settings_storage), style = MaterialTheme.typography.bodyLarge, modifier = pad0)
                    Text(stringResource(R.string.storage_downloads, Formatter.formatShortFileSize(ctx, st.downloadsBytes)), modifier = pad0)
                    Text(stringResource(R.string.storage_cache, Formatter.formatShortFileSize(ctx, st.cacheBytes)), modifier = pad0)
                    Text(stringResource(R.string.storage_covers, Formatter.formatShortFileSize(ctx, st.coversBytes)), modifier = pad0)
                }
                Button(
                    onClick = { vm.onEvent(SettingsEvent.ClearCache) },
                    modifier = Modifier.padding(horizontal = Dimens.gutter),
                ) { Text(stringResource(R.string.settings_clear_cache)) }
            }
            HorizontalDivider()

            // Eşitleme
            Section(R.string.settings_section_sync) {
                val sync = state.sync
                val pad0 = Modifier.padding(horizontal = Dimens.gutter)
                val lastOk = sync?.lastOkAt
                Text(
                    if (lastOk != null) stringResource(R.string.sync_last_ok, DateUtils.getRelativeTimeSpanString(lastOk * 1000).toString())
                    else stringResource(R.string.sync_never),
                    modifier = pad0,
                )
                Text(stringResource(R.string.sync_pending, sync?.pendingOps ?: 0), modifier = pad0)
                sync?.lastError?.let { Text(stringResource(R.string.sync_error, it), color = MaterialTheme.colorScheme.error, modifier = pad0) }
                if (state.syncing) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = Dimens.gutter))
                Button(
                    onClick = { vm.onEvent(SettingsEvent.SyncNow) },
                    enabled = !state.syncing,
                    modifier = pad0,
                ) { Text(stringResource(R.string.settings_sync_now)) }
            }
            HorizontalDivider()

            // Hesap
            Section(R.string.settings_section_account) {
                val pad0 = Modifier.padding(horizontal = Dimens.gutter)
                Text(stringResource(R.string.settings_server, state.session?.serverUrl ?: ""), modifier = pad0)
                Text(stringResource(R.string.settings_user, state.session?.username ?: ""), modifier = pad0)
                Spacer(Modifier.height(Dimens.s2))
                Button(onClick = { vm.onEvent(SettingsEvent.AskLogout) }, modifier = pad0) {
                    Text(stringResource(R.string.settings_logout))
                }
            }
        }
    }

    if (state.confirmLogout) {
        val pending = state.sync?.pendingOps ?: 0
        val body = stringResource(R.string.logout_confirm_body) +
            if (pending > 0) "\n" + stringResource(R.string.logout_confirm_pending, pending) else ""
        ConfirmDialog(
            title = stringResource(R.string.logout_confirm_title),
            body = body,
            confirmLabel = stringResource(R.string.settings_logout),
            onConfirm = { vm.onEvent(SettingsEvent.ConfirmLogout) },
            onDismiss = { vm.onEvent(SettingsEvent.CancelLogout) },
        )
    }
}

@Composable
private fun Section(title: Int, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(vertical = Dimens.s2)) {
        Text(
            stringResource(title),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(horizontal = Dimens.gutter, vertical = Dimens.s2),
        )
        content()
    }
}

@Composable
private fun ChipRow(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = Dimens.gutter),
        horizontalArrangement = Arrangement.spacedBy(Dimens.s2),
    ) {
        options.forEach { (key, label) ->
            FilterChip(selected = key == selected, onClick = { onSelect(key) }, label = { Text(label) })
        }
    }
}
