package io.github.mustafabayramguven.readcrab.ui.stats

import androidx.compose.ui.graphics.Color
import java.time.LocalDate
import kotlin.math.ceil

data class GridCell(val date: String, val col: Int, val row: Int)
data class YearGrid(val cols: Int, val cells: List<GridCell>, val months: List<Pair<Int, Int>>) // (ay 0..11, ilk sütun)
data class DayMark(val date: String, val seconds: Long, val pages: Long, val qualified: Boolean, val frozen: Boolean, val inCurrent: Boolean)
data class Link(val a: Int, val b: Int, val wrap: Boolean, val current: Boolean)

/** Hafta sütunları, satır 0 = Pazartesi (web `yearGrid` ile aynı). */
fun yearGrid(year: Int): YearGrid {
    val jan1 = LocalDate.of(year, 1, 1)
    val offset = jan1.dayOfWeek.value - 1
    val n = jan1.lengthOfYear()
    val cells = ArrayList<GridCell>(n)
    val months = ArrayList<Pair<Int, Int>>()
    var seen = -1
    var maxCol = 0
    for (i in 0 until n) {
        val d = jan1.plusDays(i.toLong())
        val col = (i + offset) / 7
        cells += GridCell(d.toString(), col, (i + offset) % 7)
        if (d.monthValue - 1 != seen) { seen = d.monthValue - 1; months += seen to col }
        maxCol = col
    }
    return YearGrid(maxCol + 1, cells, months)
}

fun heatThresholds(values: List<Long>): List<Long> {
    val s = values.filter { it > 0 }.sorted()
    if (s.isEmpty()) return listOf(0L, 0L, 0L)
    return listOf(0.25, 0.5, 0.75).map { s[ceil(it * s.size).toInt() - 1] }
}

fun heatLevel(v: Long, th: List<Long>): Int = when {
    v <= 0 -> 0
    v <= th[0] -> 1
    v <= th[1] -> 2
    v <= th[2] -> 3
    else -> 4
}

fun chainLinks(cells: List<GridCell>, byDate: Map<String, DayMark>): List<Link> {
    fun linked(d: DayMark?) = d != null && (d.qualified || d.frozen)
    val out = ArrayList<Link>()
    for (i in 0 until cells.size - 1) {
        val da = byDate[cells[i].date]
        val db = byDate[cells[i + 1].date]
        if (linked(da) && linked(db)) {
            out += Link(i, i + 1, cells[i + 1].col != cells[i].col, da!!.inCurrent && db!!.inCurrent)
        }
    }
    return out
}

/** Yoğunluk skalası: [seviye0..seviye4]. base = surfaceContainerHigh (AMOLED'de Color.Black), top = primary. */
fun heatColors(base: Color, top: Color, amoled: Boolean): List<Color> {
    val t = if (amoled) listOf(0.35f, 0.55f, 0.78f, 1f) else listOf(0.25f, 0.45f, 0.70f, 1f)
    fun mix(f: Float) = Color(
        base.red + (top.red - base.red) * f,
        base.green + (top.green - base.green) * f,
        base.blue + (top.blue - base.blue) * f,
    )
    return listOf(base) + t.map { mix(it) }
}
