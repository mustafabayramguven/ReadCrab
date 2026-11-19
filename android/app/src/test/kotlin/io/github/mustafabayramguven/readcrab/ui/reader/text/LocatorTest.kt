package io.github.mustafabayramguven.readcrab.ui.reader.text

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LocatorTest {
    @Test
    fun parsesWebFormat() {
        assertEquals(TextLocator(3, 0.42, null), Locator.parse("{\"ch\":3,\"p\":0.42}"))
    }

    @Test
    fun roundTripsMobileFormat() {
        assertEquals(TextLocator(2, 0.123456, 17), Locator.parse(Locator.build(2, 0.123456, 17)))
    }

    @Test
    fun buildOmitsNullAndNegativeBlock() {
        assertEquals("{\"ch\":2,\"p\":0.500000}", Locator.build(2, 0.5, null))
        assertEquals("{\"ch\":2,\"p\":0.500000}", Locator.build(2, 0.5, -1))
    }

    @Test
    fun rejectsInvalidInput() {
        listOf("", "5", "{\"ch\":-1,\"p\":0.1}", "{\"p\":0.1}").forEach { assertNull(it, Locator.parse(it)) }
    }

    @Test
    fun clampsProgressToUnitRange() {
        assertEquals(1.0, Locator.parse("{\"ch\":1,\"p\":1.7}")!!.p, 0.0)
    }

    @Test
    fun builtLocatorStaysWithinLimit() {
        assertTrue(Locator.build(9999, 1.0, 99999).length <= 200)
    }
}
