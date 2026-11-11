package io.github.mustafabayramguven.readcrab

import androidx.compose.ui.graphics.Color
import io.github.mustafabayramguven.readcrab.ui.theme.DarkScheme
import io.github.mustafabayramguven.readcrab.ui.theme.amoled
import io.github.mustafabayramguven.readcrab.ui.theme.readerColors
import org.junit.Assert.assertEquals
import org.junit.Test

class ThemeTest {
    @Test
    fun amoledBlacksOutAllSurfaces() {
        val s = amoled(DarkScheme)
        listOf(
            s.background, s.surface, s.surfaceDim, s.surfaceBright, s.surfaceVariant,
            s.surfaceContainerLowest, s.surfaceContainerLow, s.surfaceContainer,
            s.surfaceContainerHigh, s.surfaceContainerHighest,
        ).forEach { assertEquals(Color.Black, it) }
    }

    @Test
    fun amoledKeepsPrimary() {
        assertEquals(DarkScheme.primary, amoled(DarkScheme).primary)
    }

    @Test
    fun darkSurfaceIsBlueprintValue() {
        assertEquals(Color(0xFF141218), DarkScheme.surface)
    }

    @Test
    fun sepiaReaderBackground() {
        assertEquals("#F4ECD8", readerColors("sepia").bg)
    }
}
