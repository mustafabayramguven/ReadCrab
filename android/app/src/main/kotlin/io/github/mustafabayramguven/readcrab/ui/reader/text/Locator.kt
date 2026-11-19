package io.github.mustafabayramguven.readcrab.ui.reader.text

import java.util.Locale

/**
 * Metin kitabı konumu. `c`: satır başının bölümdeki anlamlı karakter sırası (`web/anchor.js`);
 * web, Android ve terminal okuyucu aynı satırı bulur. `b`: eski Android blok sırası, yalnız okunur.
 */
data class TextLocator(val ch: Int, val p: Double, val b: Int? = null, val c: Int? = null)

object Locator {
    private val PAIR = Regex("""\s*"([a-z]+)"\s*:\s*(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\s*""")

    /** `{"ch","p"}` ve isteğe bağlı `b`, `c` alanlarını sırasından bağımsız okur; geçersizse null. `p` [0,1]'e kırpılır. */
    fun parse(s: String): TextLocator? {
        val t = s.trim()
        if (t.length < 2 || t.first() != '{' || t.last() != '}') return null
        val fields = HashMap<String, String>()
        for (part in t.substring(1, t.length - 1).split(',')) {
            val m = PAIR.matchEntire(part) ?: return null
            fields[m.groupValues[1]] = m.groupValues[2]
        }
        val ch = fields["ch"]?.toIntOrNull()?.takeIf { it >= 0 } ?: return null
        val p = fields["p"]?.toDoubleOrNull()?.takeIf { it.isFinite() } ?: return null
        val b = fields["b"]?.toIntOrNull()?.takeIf { it >= 0 }
        val c = fields["c"]?.toIntOrNull()?.takeIf { it >= 0 }
        return TextLocator(ch, p.coerceIn(0.0, 1.0), b, c)
    }

    /** Web ve terminal okuyucuyla aynı biçim; `c` null/negatifse yazılmaz. */
    fun build(ch: Int, p: Double, c: Int?): String =
        "{\"ch\":$ch,\"p\":${"%.6f".format(Locale.ROOT, p.coerceIn(0.0, 1.0))}" +
            (if (c != null && c >= 0) ",\"c\":$c" else "") + "}"
}
