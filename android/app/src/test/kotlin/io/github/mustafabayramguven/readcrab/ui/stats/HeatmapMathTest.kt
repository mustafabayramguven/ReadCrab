package io.github.mustafabayramguven.readcrab.ui.stats

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

class HeatmapMathTest {
    @Test
    fun yearGrid2026Basics() {
        val g = yearGrid(2026)
        assertEquals(GridCell("2026-01-01", 0, 3), g.cells[0])
        assertEquals(365, g.cells.size)
        assertEquals("2026-12-31", g.cells.last().date)
        assertTrue(g.cols in 53..54)
        assertEquals(0 to 0, g.months[0])
        assertEquals(12, g.months.size)
    }

    @Test
    fun yearGridRowsAndLeapYear() {
        assertEquals(6, yearGrid(2023).cells[0].row)
        assertEquals(366, yearGrid(2028).cells.size)
    }

    @Test
    fun heatThresholdsAndLevels() {
        assertEquals(listOf(3L, 5L, 8L), heatThresholds(listOf(10L, 9L, 8L, 7L, 6L, 5L, 4L, 3L, 2L, 1L, 0L)))
        assertEquals(listOf(0L, 0L, 0L), heatThresholds(emptyList()))
        assertEquals(
            listOf(0, 0, 1, 1, 2, 2, 3, 3, 4),
            listOf(0L, -2L, 1L, 3L, 4L, 5L, 6L, 8L, 9L).map { heatLevel(it, listOf(3L, 5L, 8L)) },
        )
    }

    @Test
    fun chainLinksMatchWeb() {
        val cells = yearGrid(2026).cells
        val idx = { d: String -> cells.indexOfFirst { it.date == d } }
        val byDate = mutableMapOf(
            "2026-11-03" to DayMark("2026-11-03", 300L, 0L, qualified = true, frozen = false, inCurrent = true),
            "2026-11-04" to DayMark("2026-11-04", 0L, 0L, qualified = false, frozen = true, inCurrent = true),
            "2026-11-05" to DayMark("2026-11-05", 400L, 0L, qualified = true, frozen = false, inCurrent = true),
            "2026-11-06" to DayMark("2026-11-06", 100L, 0L, qualified = false, frozen = false, inCurrent = false),
            "2026-11-07" to DayMark("2026-11-07", 350L, 0L, qualified = true, frozen = false, inCurrent = false),
            "2026-11-08" to DayMark("2026-11-08", 310L, 0L, qualified = true, frozen = false, inCurrent = false),
        )
        val links = chainLinks(cells, byDate)
        assertEquals(
            listOf(idx("2026-11-03") to idx("2026-11-04"), idx("2026-11-04") to idx("2026-11-05"), idx("2026-11-07") to idx("2026-11-08")),
            links.map { it.a to it.b },
        )
        assertEquals(listOf(true, true, false), links.map { it.current })
        assertFalse(links[2].wrap) // 11-07 Cumartesi ve 11-08 Pazar aynı sütunda

        byDate["2026-11-09"] = DayMark("2026-11-09", 300L, 0L, qualified = true, frozen = false, inCurrent = false)
        val wrapLink = chainLinks(cells, byDate).first { it.a == idx("2026-11-08") }
        assertTrue(wrapLink.wrap) // Pazar -> sonraki Pazartesi: yeni sütun
    }

    private fun assertColor(expected: Color, actual: Color) {
        val pairs = listOf(
            expected.red to actual.red,
            expected.green to actual.green,
            expected.blue to actual.blue,
        )
        pairs.forEach { (e, a) -> assertTrue("kanal farkı > 1/255", abs(e * 255f - a * 255f) <= 1f) }
    }

    @Test
    fun heatColorsLightScheme() {
        val colors = heatColors(Color(0xFFECE6F0), Color(0xFF6750A4), amoled = false)
        listOf(0xFFECE6F0, 0xFFCBC1DD, 0xFFB0A3CE, 0xFF8F7DBB, 0xFF6750A4).forEachIndexed { i, hex ->
            assertColor(Color(hex), colors[i])
        }
    }

    @Test
    fun heatColorsDarkScheme() {
        val colors = heatColors(Color(0xFF2B2930), Color(0xFFD0BCFF), amoled = false)
        listOf(0xFF2B2930, 0xFF544E64, 0xFF756B8D, 0xFF9F90C1, 0xFFD0BCFF).forEachIndexed { i, hex ->
            assertColor(Color(hex), colors[i])
        }
    }

    @Test
    fun heatColorsAmoledScheme() {
        val colors = heatColors(Color.Black, Color(0xFFD0BCFF), amoled = true)
        listOf(0xFF000000, 0xFF494259, 0xFF72678C, 0xFFA293C7, 0xFFD0BCFF).forEachIndexed { i, hex ->
            assertColor(Color(hex), colors[i])
        }
    }
}
