package io.github.mustafabayramguven.readcrab.ui.reader.text

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LocatorTest {
    @Test
    fun parsesWebFormat() {
        assertEquals(TextLocator(3, 0.42), Locator.parse("{\"ch\":3,\"p\":0.42}"))
    }

    @Test
    fun parsesLineAnchorFromAnyClient() {
        // web JSON.stringify ve terminal okuyucu
        assertEquals(TextLocator(3, 0.42, c = 1234), Locator.parse("{\"ch\":3,\"p\":0.42,\"c\":1234}"))
        assertEquals(TextLocator(3, 0.42, c = 7), Locator.parse("{ \"c\": 7, \"ch\": 3, \"p\": 0.42 }"))
    }

    @Test
    fun readsLegacyBlockAnchor() {
        assertEquals(TextLocator(2, 0.123456, b = 17), Locator.parse("{\"ch\":2,\"p\":0.123456,\"b\":17}"))
    }

    @Test
    fun roundTripsLineAnchor() {
        assertEquals(TextLocator(2, 0.123456, c = 4567), Locator.parse(Locator.build(2, 0.123456, 4567)))
    }

    @Test
    fun buildOmitsNullAndNegativeAnchor() {
        assertEquals("{\"ch\":2,\"p\":0.500000}", Locator.build(2, 0.5, null))
        assertEquals("{\"ch\":2,\"p\":0.500000}", Locator.build(2, 0.5, -1))
        assertEquals("{\"ch\":2,\"p\":0.500000,\"c\":9}", Locator.build(2, 0.5, 9))
    }

    @Test
    fun rejectsInvalidInput() {
        listOf("", "5", "{}", "{\"ch\":-1,\"p\":0.1}", "{\"p\":0.1}", "{\"ch\":1,\"p\":\"x\"}", "{\"ch\":1,\"p\":0.1,}")
            .forEach { assertNull(it, Locator.parse(it)) }
    }

    @Test
    fun ignoresNegativeAnchor() {
        assertNull(Locator.parse("{\"ch\":1,\"p\":0.1,\"c\":-3}")!!.c)
    }

    @Test
    fun clampsProgressToUnitRange() {
        assertEquals(1.0, Locator.parse("{\"ch\":1,\"p\":1.7}")!!.p, 0.0)
    }

    @Test
    fun builtLocatorStaysWithinLimit() {
        assertTrue(Locator.build(9999, 1.0, Int.MAX_VALUE).length <= 200)
    }
}
