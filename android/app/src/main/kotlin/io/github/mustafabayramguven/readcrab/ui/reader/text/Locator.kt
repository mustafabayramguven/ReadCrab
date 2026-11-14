package io.github.mustafabayramguven.readcrab.ui.reader.text

import java.util.Locale

data class TextLocator(val ch: Int, val p: Double, val b: Int?)

object Locator {
    private val RE = Regex("""^\{\s*"ch"\s*:\s*(\d+)\s*,\s*"p"\s*:\s*(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\s*(?:,\s*"b"\s*:\s*(\d+)\s*)?\}$""")

    /** Web (`{"ch","p"}`) ve mobil (`{"ch","p","b"}`) biçimlerini okur; geçersizse null. `p` [0,1]'e kırpılır. */
    fun parse(s: String): TextLocator? {
        val m = RE.matchEntire(s.trim()) ?: return null
        val ch = m.groupValues[1].toIntOrNull() ?: return null
        val p = m.groupValues[2].toDoubleOrNull()?.takeIf { it.isFinite() } ?: return null
        val b = m.groupValues[3].takeIf { it.isNotEmpty() }?.toIntOrNull()
        return TextLocator(ch, p.coerceIn(0.0, 1.0), b)
    }

    /** Web ile uyumlu; `b` null/negatifse yazılmaz. */
    fun build(ch: Int, p: Double, b: Int?): String =
        "{\"ch\":$ch,\"p\":${"%.6f".format(Locale.ROOT, p.coerceIn(0.0, 1.0))}" +
            (if (b != null && b >= 0) ",\"b\":$b" else "") + "}"
}
