package io.github.mustafabayramguven.readcrab.ui.stats

import android.app.Application
import android.text.format.DateUtils
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.AcUnit
import androidx.compose.material.icons.outlined.LocalFireDepartment
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.core.StatsDay
import io.github.mustafabayramguven.readcrab.core.StatsSnapshot
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import io.github.mustafabayramguven.readcrab.ui.theme.isEffectiveDark
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max

data class StatsState(
    val year: Int, val snapshot: StatsSnapshot? = null, val loading: Boolean = true,
    val error: String? = null, val selected: String? = null, val online: Boolean = true,
)

sealed interface StatsEvent {
    data class Year(val y: Int) : StatsEvent
    data class Select(val date: String?) : StatsEvent   // aynı güne tekrar dokunmak seçimi kaldırır
    data object Refresh : StatsEvent
}

class StatsViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = app as RcApp

    private val _state = MutableStateFlow(StatsState(year = LocalDate.now().year))
    val state: StateFlow<StatsState> = _state.asStateFlow()

    @Volatile private var seq = 0

    init {
        viewModelScope.launch { rc.connectivity.online.collect { on -> _state.update { it.copy(online = on) } } }
        load(_state.value.year)
    }

    fun onEvent(e: StatsEvent) {
        when (e) {
            is StatsEvent.Year -> {
                _state.update { it.copy(year = e.y, selected = null, snapshot = null, loading = true, error = null) }
                load(e.y)
            }
            is StatsEvent.Select -> _state.update { it.copy(selected = if (it.selected == e.date) null else e.date) }
            StatsEvent.Refresh -> load(_state.value.year)
        }
    }

    // Önce önbellek (varsa hemen gösterilir), sonra çevrimiçiyse ağdan.
    private fun load(y: Int) {
        val mine = ++seq
        viewModelScope.launch(Dispatchers.IO) {
            val cached = try { rc.core.statsCached(y) } catch (e: CoreException) { null }
            if (mine != seq) return@launch
            if (cached != null) _state.update { it.copy(snapshot = cached, loading = false, error = null) }
            if (!rc.connectivity.online.value) {
                if (cached == null) {
                    _state.update {
                        it.copy(loading = false, error = getApplication<Application>().getString(R.string.library_offline))
                    }
                }
                return@launch
            }
            try {
                val snap = rc.core.statsLoad(y, LocalDate.now().toString())
                if (mine == seq) _state.update { it.copy(snapshot = snap, loading = false, error = null) }
            } catch (e: CoreException.Unauthorized) {
                rc.prefs.update { it.copy(needsRelogin = true) }
            } catch (e: CoreException) {
                if (mine == seq && cached == null) _state.update { it.copy(loading = false, error = e.message) }
            }
        }
    }
}

// Heatmap ölçüleri (kesin): hücre 14, boşluk 3, sol 32 (gün etiketi), üst 18 (ay etiketi)
private val CELL = 14.dp
private val GAP = 3.dp
private val LEFT = 32.dp
private val TOP = 18.dp
private val STEP = CELL + GAP
private val MONTH_LABELS = listOf("Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara")
private val WEEKDAY_LABELS = mapOf(0 to "Pzt", 2 to "Çar", 4 to "Cum")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StatsScreen(onBack: () -> Unit, onSessionExpired: () -> Unit, vm: StatsViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    val app = LocalContext.current.applicationContext as RcApp
    val prefs by app.prefs.state.collectAsStateWithLifecycle()
    LaunchedEffect(prefs.needsRelogin) { if (prefs.needsRelogin) onSessionExpired() }

    val snap = state.snapshot
    val darkNow = isEffectiveDark(prefs, isSystemInDarkTheme())
    val amoledOn = darkNow && prefs.amoled

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.stats_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
                actions = {
                    IconButton(onClick = { vm.onEvent(StatsEvent.Refresh) }, enabled = state.online) {
                        Icon(Icons.Outlined.Refresh, contentDescription = stringResource(R.string.action_retry))
                    }
                },
            )
        },
    ) { pad ->
        LazyColumn(
            Modifier.fillMaxSize().padding(pad),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = Dimens.gutter, vertical = Dimens.s3),
            verticalArrangement = Arrangement.spacedBy(Dimens.s3),
        ) {
            if (snap != null) {
                item { StreakCard(snap, amoledOn) }
                item { SummaryRow(snap) }
            }
            item {
                val years = ((snap?.years ?: emptyList()) + state.year).distinct().sortedDescending()
                Column {
                    Text(stringResource(R.string.stats_year), style = MaterialTheme.typography.labelLarge)
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(Dimens.s2)) {
                        items(years) { y ->
                            FilterChip(
                                selected = y == state.year,
                                onClick = { vm.onEvent(StatsEvent.Year(y)) },
                                label = { Text(y.toString()) },
                            )
                        }
                    }
                }
            }
            if (state.loading) {
                item { CircularProgressIndicator() }
            }
            state.error?.let { err ->
                item { Text(err, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error) }
            }
            if (snap != null) {
                item {
                    HeatmapCard(
                        snap = snap,
                        selected = state.selected,
                        amoled = amoledOn,
                        onSelect = { vm.onEvent(StatsEvent.Select(it)) },
                    )
                }
                state.selected?.let { sel ->
                    item { DayCard(sel, snap.days.firstOrNull { it.date == sel }) }
                }
                item {
                    Column {
                        if (snap.fromCache) {
                            Text(stringResource(R.string.stats_cached), style = MaterialTheme.typography.bodySmall)
                        }
                        Text(
                            stringResource(
                                R.string.stats_updated,
                                DateUtils.getRelativeTimeSpanString(
                                    snap.fetchedAt * 1000,
                                    System.currentTimeMillis(),
                                    DateUtils.MINUTE_IN_MILLIS,
                                ).toString(),
                            ),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun durationText(sec: Long): String {
    val min = sec / 60
    return if (min < 60) {
        stringResource(R.string.stats_duration_min, min)
    } else {
        stringResource(R.string.stats_duration_hm, min / 60, min % 60)
    }
}

@Composable
private fun StreakCard(snap: StatsSnapshot, amoled: Boolean) {
    val cs = MaterialTheme.colorScheme
    val n = max(1, ceil((snap.streakMinSeconds - snap.todaySeconds) / 60.0).toInt())
    val risk = snap.streakAtRisk
    val content = if (risk) cs.onErrorContainer else cs.onSurface
    val body: @Composable ColumnScope.() -> Unit = {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.LocalFireDepartment, contentDescription = null, modifier = Modifier.size(32.dp))
            Spacer(Modifier.width(Dimens.s2))
            Text(stringResource(R.string.stats_days, snap.streakCurrent), style = MaterialTheme.typography.headlineMedium)
        }
        Text(
            stringResource(R.string.stats_streak_longest) + ": " + stringResource(R.string.stats_days, snap.streakLongest),
            style = MaterialTheme.typography.labelLarge,
        )
        Text(
            when {
                risk -> stringResource(R.string.stats_risk, n)
                snap.streakCurrent > 0 -> stringResource(R.string.stats_ok)
                else -> stringResource(R.string.stats_none, n)
            },
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.AcUnit, contentDescription = null, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(Dimens.s1))
            Text(
                stringResource(if (snap.freezeAvailable) R.string.stats_freeze_ready else R.string.stats_freeze_used),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
    if (amoled) {
        OutlinedCard(
            modifier = Modifier.fillMaxWidth(),
            border = BorderStroke(1.dp, cs.outlineVariant),
            colors = CardDefaults.outlinedCardColors(
                containerColor = if (risk) cs.errorContainer else Color.Black,
                contentColor = content,
            ),
        ) { StreakColumn(body) }
    } else {
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = if (risk) cs.errorContainer else cs.surfaceContainerLow,
                contentColor = content,
            ),
        ) { StreakColumn(body) }
    }
}

@Composable
private fun StreakColumn(body: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().padding(Dimens.s4), verticalArrangement = Arrangement.spacedBy(Dimens.s1), content = body)
}

@Composable
private fun SummaryRow(snap: StatsSnapshot) {
    Row(horizontalArrangement = Arrangement.spacedBy(Dimens.s2)) {
        listOf(
            durationText(snap.totalSeconds) to R.string.stats_total_time,
            snap.totalPages.toString() to R.string.stats_total_pages,
            snap.finishedCount.toString() to R.string.stats_finished,
        ).forEach { (value, label) ->
            Card(Modifier.weight(1f)) {
                Column(Modifier.padding(Dimens.s3)) {
                    Text(value, style = MaterialTheme.typography.titleMedium)
                    Text(stringResource(label), style = MaterialTheme.typography.labelSmall)
                }
            }
        }
    }
}

// Düz dikdörtgen: hücre, desen ve kenarlık çizimi (ısı haritası ve gösterge aynı çizimi kullanır)
private fun DrawScope.drawCell(
    x: Float, y: Float, s: Float, fill: Color, alpha: Float = 1f,
    hatch: Color? = null, stroke: Color? = null, strokeWidth: Float = 1f,
) {
    val rr = CornerRadius(3.dp.toPx())
    val rect = Rect(Offset(x, y), Size(s, s))
    drawRoundRect(color = fill, topLeft = Offset(x, y), size = Size(s, s), cornerRadius = rr, alpha = alpha)
    if (hatch != null) {
        clipPath(Path().apply { addRoundRect(RoundRect(rect, rr)) }) {
            val spacing = 5.dp.toPx()
            var k = -s
            while (k < 2 * s) {
                drawLine(
                    color = hatch,
                    start = Offset(x + k, y + s),
                    end = Offset(x + k + s, y),
                    strokeWidth = 1.5.dp.toPx(),
                )
                k += spacing
            }
        }
    }
    if (stroke != null) {
        drawRoundRect(
            color = stroke,
            topLeft = Offset(x, y),
            size = Size(s, s),
            cornerRadius = rr,
            style = Stroke(strokeWidth),
        )
    }
}

@Composable
private fun HeatmapCard(
    snap: StatsSnapshot,
    selected: String?,
    amoled: Boolean,
    onSelect: (String?) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    val textMeasurer = rememberTextMeasurer()
    val grid = remember(snap.year) { yearGrid(snap.year) }
    val base = if (amoled) Color.Black else cs.surfaceContainerHigh
    val levels = remember(base, cs.primary, amoled) { heatColors(base, cs.primary, amoled) }
    val marks = remember(snap) {
        snap.days.associate { it.date to DayMark(it.date, it.seconds, it.pages, it.qualified, it.frozen, it.inCurrent) }
    }
    val values = remember(snap) { snap.days.map { it.seconds } }
    val th = remember(values) { heatThresholds(values) }
    val links = remember(grid, marks) { chainLinks(grid.cells, marks) }
    val byPos = remember(grid) { grid.cells.associateBy { it.col to it.row } }
    val readDays = snap.days.count { it.seconds > 0 }
    val dayStat = snap.days.associateBy { it.date }

    val widthDp = LEFT + CELL * grid.cols + GAP * (grid.cols - 1)
    val heightDp = TOP + CELL * 7 + GAP * 7

    val scroll = rememberScrollState()
    var viewport by remember { mutableIntStateOf(0) }
    val today = LocalDate.now()
    // açılış: geçerli yılda bugünün sütunu sağda; geçmiş yılda sona
    LaunchedEffect(snap.year, viewport) {
        if (viewport == 0) return@LaunchedEffect
        snapshotFlow { scroll.maxValue }.first { it > 0 }
        val target = if (snap.year == today.year) {
            val todayCol = grid.cells.firstOrNull { it.date == today.toString() }?.col ?: 0
            with(density) {
                (LEFT.toPx() + todayCol * STEP.toPx() + CELL.toPx() / 2f - viewport + 3 * STEP.toPx()).toInt()
            }
        } else {
            scroll.maxValue
        }
        scroll.scrollTo(target.coerceIn(0, scroll.maxValue))
    }

    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(Dimens.s3)) {
            Box(
                Modifier
                    .horizontalScroll(scroll)
                    .onSizeChanged { viewport = it.width },
            ) {
                Canvas(
                    Modifier
                        .size(widthDp, heightDp)
                        .pointerInput(byPos) {
                            detectTapGestures { p ->
                                val stepPx = with(density) { STEP.toPx() }
                                val leftPx = with(density) { LEFT.toPx() }
                                val topPx = with(density) { TOP.toPx() }
                                val cellPx = with(density) { CELL.toPx() }
                                val col = floor((p.x - leftPx) / stepPx).toInt()
                                val row = floor((p.y - topPx) / stepPx).toInt()
                                val inCell = (p.x - leftPx) - col * stepPx <= cellPx && (p.y - topPx) - row * stepPx <= cellPx
                                val hit = byPos[col to row]
                                if (inCell && hit != null) onSelect(hit.date)
                            }
                        }
                        .semantics { contentDescription = "Okuma takvimi ${snap.year}, $readDays gün" },
                ) {
                    val cell = CELL.toPx()
                    val gap = GAP.toPx()
                    val left = LEFT.toPx()
                    val top = TOP.toPx()
                    val step = cell + gap
                    val labelStyle = TextStyle(fontSize = 11.sp, color = cs.onSurfaceVariant)

                    // (1) ay ve hafta günü etiketleri
                    grid.months.forEach { (m, col) ->
                        drawText(textMeasurer, MONTH_LABELS[m], topLeft = Offset(left + col * step, 0f), style = labelStyle)
                    }
                    WEEKDAY_LABELS.forEach { (r, label) ->
                        drawText(textMeasurer, label, topLeft = Offset(0f, top + r * step), style = labelStyle)
                    }

                    // (2) zincir bağlantıları (hücrelerin altında)
                    links.forEach { l ->
                        val ca = grid.cells[l.a]
                        val cb = grid.cells[l.b]
                        val color = if (l.current) cs.primary else cs.onSurfaceVariant.copy(alpha = 0.35f)
                        val barW = 4.dp.toPx()
                        val cx = (cell - barW) / 2f
                        if (!l.wrap) {
                            drawRect(color, Offset(left + ca.col * step + cx, top + ca.row * step + cell), Size(barW, gap))
                        } else {
                            drawRect(color, Offset(left + ca.col * step + cx, top + 6 * step + cell), Size(barW, gap))
                            drawRect(color, Offset(left + cb.col * step + cx, top - gap), Size(barW, gap))
                        }
                    }

                    // (3) hücreler
                    grid.cells.forEach { gc ->
                        val d = marks[gc.date]
                        val sec = d?.seconds ?: 0L
                        val q = d?.qualified == true
                        val fz = d?.frozen == true
                        val cur = d?.inCurrent == true
                        val weak = d != null && !q && !fz && sec > 0
                        val x = left + gc.col * step
                        val y = top + gc.row * step
                        val level = heatLevel(sec, th)
                        when {
                            fz -> drawCell(
                                x, y, cell, base, hatch = cs.tertiary,
                                stroke = cs.tertiary, strokeWidth = 1.dp.toPx(),
                            )
                            else -> drawCell(
                                x, y, cell, levels[level], alpha = if (weak) 0.5f else 1f,
                                stroke = if (level == 0 && amoled) cs.outlineVariant else null,
                                strokeWidth = 1.dp.toPx(),
                            )
                        }
                        if (cur) {
                            drawRoundRect(
                                color = cs.primary,
                                topLeft = Offset(x, y),
                                size = Size(cell, cell),
                                cornerRadius = CornerRadius(3.dp.toPx()),
                                style = Stroke(1.5.dp.toPx()),
                            )
                        }
                        if (selected == gc.date) {
                            drawRoundRect(
                                color = cs.onSurface,
                                topLeft = Offset(x, y),
                                size = Size(cell, cell),
                                cornerRadius = CornerRadius(3.dp.toPx()),
                                style = Stroke(2.dp.toPx()),
                            )
                        }
                    }
                }
            }
            Spacer(Modifier.height(Dimens.s2))
            Legend(levels, amoled, cs.primary, cs.tertiary, base, cs.outlineVariant)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Legend(
    levels: List<Color>,
    amoled: Boolean,
    primary: Color,
    tertiary: Color,
    base: Color,
    outlineVariant: Color,
) {
    val labelStyle = MaterialTheme.typography.labelSmall
    val swatch = Modifier.size(12.dp)
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(stringResource(R.string.stats_legend_less), style = labelStyle)
        levels.forEachIndexed { i, c ->
            Canvas(swatch) {
                drawCell(0f, 0f, size.width, c,
                    stroke = if (i == 0 && amoled) outlineVariant else null)
            }
        }
        Text(stringResource(R.string.stats_legend_more), style = labelStyle)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Canvas(swatch) { drawCell(0f, 0f, size.width, levels[3], stroke = primary, strokeWidth = 1.5.dp.toPx()) }
            Text(stringResource(R.string.stats_legend_qualified), style = labelStyle)
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Canvas(swatch) { drawCell(0f, 0f, size.width, base, hatch = tertiary, stroke = tertiary) }
            Text(stringResource(R.string.stats_legend_frozen), style = labelStyle)
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Canvas(swatch) { drawCell(0f, 0f, size.width, levels[1], alpha = 0.5f) }
            Text(stringResource(R.string.stats_legend_weak), style = labelStyle)
        }
    }
}

@Composable
private fun DayCard(date: String, day: StatsDay?) {
    val fmt = DateTimeFormatter.ofPattern("d MMMM yyyy", Locale("tr"))
    val sec = day?.seconds ?: 0L
    val status = when {
        day?.qualified == true -> stringResource(R.string.stats_cell_qualified)
        day?.frozen == true -> stringResource(R.string.stats_cell_frozen)
        sec > 0 -> stringResource(R.string.stats_cell_weak)
        else -> stringResource(R.string.stats_day_none)
    }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(Dimens.s4), verticalArrangement = Arrangement.spacedBy(Dimens.s1)) {
            Text(LocalDate.parse(date).format(fmt), style = MaterialTheme.typography.titleMedium)
            if (sec > 0) {
                Text(durationText(sec), style = MaterialTheme.typography.bodyMedium)
                Text(stringResource(R.string.stats_day_pages, day?.pages ?: 0L), style = MaterialTheme.typography.bodyMedium)
            }
            Text(status, style = MaterialTheme.typography.bodyMedium)
            if (day?.inCurrent == true) {
                Text(stringResource(R.string.stats_cell_current), style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}
