package io.github.mustafabayramguven.readcrab.ui.reader

import android.app.Activity
import android.os.SystemClock
import android.view.WindowManager
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.UiPrefs
import io.github.mustafabayramguven.readcrab.VolumeKeys
import io.github.mustafabayramguven.readcrab.core.BookmarkItem
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Okuma süresi sayacı (web `tracker.js` ile aynı kural): ön plandaysa ve son girdi 60 sn içindeyse
 * her saniye 1 sn etkin süre sayılır. 15 sn'de bir çekirdeğe yazılır, 30 sn'de bir itme istenir.
 */
class ReadingTracker(private val app: RcApp, private val scope: CoroutineScope) {
    /** Ekran `LifecycleEventEffect` ile ON_RESUME/ON_PAUSE'ta günceller. */
    @Volatile var foreground: Boolean = true

    private val stopped = AtomicBoolean(true)
    private val pages = ConcurrentHashMap.newKeySet<String>()
    @Volatile private var lastInput = SystemClock.elapsedRealtime()
    @Volatile private var endPercent = 0.0
    @Volatile private var active = 0L
    @Volatile private var clientId: String? = null
    private var ticker: Job? = null

    fun start(bookId: Long, startPercent: Double) {
        if (!stopped.getAndSet(false)) return
        val sp = startPercent.coerceIn(0.0, 1.0)
        endPercent = sp
        active = 0L
        pages.clear()
        lastInput = SystemClock.elapsedRealtime()
        ticker = scope.launch(Dispatchers.IO) {
            val id = runCatching { app.core.pingStart(bookId, sp, LocalDate.now().toString()) }.getOrNull()
                ?: return@launch
            clientId = id
            var tick = 0
            while (isActive) {
                delay(1000)
                tick++
                if (foreground && SystemClock.elapsedRealtime() - lastInput <= 60_000) active++
                if (tick % 15 == 0) flush(id)
                if (tick % 30 == 0) app.sync.onLocalChange(0)
            }
        }
    }

    fun touch() {
        lastInput = SystemClock.elapsedRealtime()
    }

    fun addPage(key: String) {
        pages.add(key)
        touch()
    }

    fun setPercent(p: Double) {
        endPercent = p.coerceIn(0.0, 1.0)
    }

    private fun flush(id: String) {
        runCatching { app.core.pingUpdate(id, active, pages.size.toLong(), endPercent) }
    }

    /** Son güncelleme + kapatma + eşitleme isteği. Birden çok kez çağrılabilir. */
    fun stop() {
        if (stopped.getAndSet(true)) return
        ticker?.cancel()
        val id = clientId
        // ViewModel kapanırken kendi kapsamı iptal olmuş olabilir: ayrı kapsam
        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch {
            if (id != null) {
                flush(id)
                runCatching { app.core.pingClose(id) }
            }
            app.sync.onLocalChange(0)
        }
        clientId = null
    }
}

/** Tam ekran, ekran açık tutma ve ses tuşu yönlendirmesi; ayrılınca hepsi geri alınır. */
@Composable
fun ReaderWindowEffects(prefs: UiPrefs, onVolume: ((forward: Boolean) -> Boolean)?) {
    val view = LocalView.current
    val window = (LocalContext.current as Activity).window

    DisposableEffect(prefs.immersive) {
        val controller = WindowCompat.getInsetsController(window, view)
        if (prefs.immersive) {
            controller.hide(WindowInsetsCompat.Type.systemBars())
            controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        onDispose { controller.show(WindowInsetsCompat.Type.systemBars()) }
    }

    DisposableEffect(prefs.keepAwake) {
        if (prefs.keepAwake) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        onDispose { window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON) }
    }

    val latest by rememberUpdatedState(onVolume)
    DisposableEffect(prefs.volumeKeys, onVolume != null) {
        if (prefs.volumeKeys && onVolume != null) {
            VolumeKeys.handler = { forward -> latest?.invoke(forward) ?: false }
        }
        onDispose { VolumeKeys.handler = null }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BookmarksSheet(
    items: List<BookmarkItem>,
    onOpen: (BookmarkItem) -> Unit,
    onDelete: (Long) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        if (items.isEmpty()) {
            Text(
                stringResource(R.string.reader_bookmarks_empty),
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(Dimens.gutter),
            )
        } else {
            LazyColumn {
                items(items, key = { it.localId }) { b ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { onOpen(b) }
                            .padding(start = Dimens.gutter, end = Dimens.s2, top = Dimens.s1, bottom = Dimens.s1),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            b.label,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                        if (b.pending) {
                            Icon(
                                Icons.Outlined.CloudOff,
                                contentDescription = null,
                                modifier = Modifier.padding(end = Dimens.s2).size(16.dp),
                            )
                        }
                        IconButton(onClick = { onDelete(b.localId) }) {
                            Icon(Icons.Outlined.Delete, contentDescription = stringResource(R.string.action_delete))
                        }
                    }
                }
            }
        }
    }
}

// Ölçüler: yatay dolgu Dimens.gutter (16.dp), bölüm başlığı üst boşluğu Dimens.s4 (16.dp), satır en az 48.dp,
// slider satırı: etiket+değer üstte, altında Slider, dikey aralık Dimens.s2 (8.dp), alt dolgu Dimens.s5 (24.dp)
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderSettingsSheet(title: String, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
    ) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = Dimens.gutter).padding(bottom = Dimens.s5),
        ) {
            Text(title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(bottom = Dimens.s2))
            content()
        }
    }
}

@Composable
fun ColumnScope.ReaderSection(title: String, first: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    if (!first) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant, modifier = Modifier.padding(top = Dimens.s4))
    Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary,
         modifier = Modifier.padding(top = Dimens.s4, bottom = Dimens.s2))
    content()
}

@Composable
fun ReaderSliderRow(label: String, valueText: String, value: Float, range: ClosedFloatingPointRange<Float>,
                    steps: Int, onChange: (Float) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        Text(valueText, style = MaterialTheme.typography.labelLarge)
    }
    Slider(value = value, onValueChange = onChange, valueRange = range, steps = steps)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderChoiceRow(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit) {
    // en çok 3 seçenek: segmented; fazlası: yatay kaydırmalı FilterChip
    if (options.size <= 3) {
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
            options.forEachIndexed { i, (key, label) ->
                SegmentedButton(selected = selected == key, onClick = { onSelect(key) },
                    shape = SegmentedButtonDefaults.itemShape(i, options.size)) { Text(label) }
            }
        }
    } else {
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(Dimens.s2)) {
            for ((key, label) in options) FilterChip(selected = selected == key, onClick = { onSelect(key) }, label = { Text(label) })
        }
    }
}

@Composable
fun ReaderSwitchRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

/** İlk açılış ipucu: bir kez, 5 sn. `Box` içinde çağrılır. */
@Composable
fun BoxScope.ReaderHint(prefs: UiPrefs, onSeen: () -> Unit) {
    if (prefs.readerHintSeen) return
    LaunchedEffect(Unit) { delay(5000); onSeen() }
    Surface(
        shape = RoundedCornerShape(50), color = MaterialTheme.colorScheme.inverseSurface,
        modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 96.dp),
    ) {
        Text(stringResource(R.string.reader_hint_menu), color = MaterialTheme.colorScheme.inverseOnSurface,
             style = MaterialTheme.typography.labelLarge,
             modifier = Modifier.padding(horizontal = Dimens.s4, vertical = Dimens.s3))
    }
}

/** C5: başka cihaz konumu sorusu. `where` çağıran tarafından biçimlenir. */
@Composable
fun RemotePositionDialog(where: String, onGo: () -> Unit, onStay: () -> Unit) {
    AlertDialog(
        onDismissRequest = onStay,
        title = { Text(stringResource(R.string.reader_remote_title)) },
        text = { Text(stringResource(R.string.reader_remote_body, where)) },
        confirmButton = { TextButton(onClick = onGo) { Text(stringResource(R.string.reader_remote_go)) } },
        dismissButton = { TextButton(onClick = onStay) { Text(stringResource(R.string.reader_remote_stay)) } },
    )
}
