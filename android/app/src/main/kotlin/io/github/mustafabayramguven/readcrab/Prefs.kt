package io.github.mustafabayramguven.readcrab

import android.content.Context
import io.github.mustafabayramguven.readcrab.ui.theme.ThemeMode
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

data class UiPrefs(
    val themeMode: ThemeMode = ThemeMode.SYSTEM,
    val amoled: Boolean = false,
    val dynamicColor: Boolean = false,
    val wifiOnly: Boolean = true,
    val immersive: Boolean = true,
    val keepAwake: Boolean = true,
    val tapZones: Boolean = true,
    val volumeKeys: Boolean = false,
    val readerTheme: String = "auto",     // auto | light | dark | sepia | black
    val textFont: String = "literata",    // literata | merriweather | source-serif | eb-garamond | inter | atkinson | lexend
    val textSize: Int = 18,               // 14..28, adım 2
    val lineHeight: Float = 1.7f,         // 1.2..2.2, adım 0.1
    val margin: Int = 0,                  // 0..20, adım 1 (genişliğin yüzdesi)
    val align: String = "left",           // left | justify
    val textMode: String = "scroll",      // scroll | page
    val comicMode: String = "ltr",        // ltr | rtl | webtoon (varsayılan; kitap başına ezilebilir)
    val needsRelogin: Boolean = false,
)

// Yazı tipi adları marka adıdır; strings.xml'e girmez (kimlik to etiket)
val TEXT_FONTS: List<Pair<String, String>> = listOf(
    "literata" to "Literata",
    "merriweather" to "Merriweather",
    "source-serif" to "Source Serif 4",
    "eb-garamond" to "EB Garamond",
    "inter" to "Inter",
    "atkinson" to "Atkinson Hyperlegible",
    "lexend" to "Lexend",
)

private val READER_THEMES = setOf("auto", "light", "dark", "sepia", "black")
private val TEXT_FONT_KEYS = TEXT_FONTS.map { it.first }.toSet()
private val ALIGNS = setOf("left", "justify")
private val TEXT_MODES = setOf("scroll", "page")
private val COMIC_MODES = setOf("ltr", "rtl", "webtoon")

// Anahtar adları alan adlarıdır; sınırlar web calc.js normalizePrefs ile aynıdır.
private fun normalize(p: UiPrefs): UiPrefs = p.copy(
    textSize = p.textSize.coerceIn(14, 28),
    lineHeight = p.lineHeight.coerceIn(1.2f, 2.2f),
    margin = p.margin.coerceIn(0, 20),
    readerTheme = p.readerTheme.takeIf { it in READER_THEMES } ?: "auto",
    textFont = p.textFont.takeIf { it in TEXT_FONT_KEYS } ?: "literata",
    align = p.align.takeIf { it in ALIGNS } ?: "left",
    textMode = p.textMode.takeIf { it in TEXT_MODES } ?: "scroll",
    comicMode = p.comicMode.takeIf { it in COMIC_MODES } ?: "ltr",
)

class Prefs(ctx: Context) {
    private val sp = ctx.getSharedPreferences("rc_prefs", Context.MODE_PRIVATE)
    private val _state = MutableStateFlow(load())
    val state: StateFlow<UiPrefs> = _state.asStateFlow()

    @Synchronized
    fun update(f: (UiPrefs) -> UiPrefs) {
        val p = normalize(f(_state.value))
        sp.edit()
            .putString("themeMode", p.themeMode.name)
            .putBoolean("amoled", p.amoled)
            .putBoolean("dynamicColor", p.dynamicColor)
            .putBoolean("wifiOnly", p.wifiOnly)
            .putBoolean("immersive", p.immersive)
            .putBoolean("keepAwake", p.keepAwake)
            .putBoolean("tapZones", p.tapZones)
            .putBoolean("volumeKeys", p.volumeKeys)
            .putString("readerTheme", p.readerTheme)
            .putString("textFont", p.textFont)
            .putInt("textSize", p.textSize)
            .putFloat("lineHeight", p.lineHeight)
            .putInt("margin", p.margin)
            .putString("align", p.align)
            .putString("textMode", p.textMode)
            .putString("comicMode", p.comicMode)
            .putBoolean("needsRelogin", p.needsRelogin)
            .apply()
        _state.value = p
    }

    fun comicModeFor(bookId: Long): String =
        sp.getString("cmode.$bookId", null)?.takeIf { it in COMIC_MODES } ?: _state.value.comicMode

    fun setComicModeFor(bookId: Long, mode: String) {
        sp.edit().putString("cmode.$bookId", mode).apply()
    }

    private fun load(): UiPrefs {
        val d = UiPrefs()
        return normalize(
            UiPrefs(
                themeMode = ThemeMode.entries.firstOrNull { it.name == sp.getString("themeMode", null) } ?: d.themeMode,
                amoled = sp.getBoolean("amoled", d.amoled),
                dynamicColor = sp.getBoolean("dynamicColor", d.dynamicColor),
                wifiOnly = sp.getBoolean("wifiOnly", d.wifiOnly),
                immersive = sp.getBoolean("immersive", d.immersive),
                keepAwake = sp.getBoolean("keepAwake", d.keepAwake),
                tapZones = sp.getBoolean("tapZones", d.tapZones),
                volumeKeys = sp.getBoolean("volumeKeys", d.volumeKeys),
                readerTheme = sp.getString("readerTheme", d.readerTheme) ?: d.readerTheme,
                textFont = sp.getString("textFont", d.textFont) ?: d.textFont,
                textSize = sp.getInt("textSize", d.textSize),
                lineHeight = sp.getFloat("lineHeight", d.lineHeight),
                margin = sp.getInt("margin", d.margin),
                align = sp.getString("align", d.align) ?: d.align,
                textMode = sp.getString("textMode", d.textMode) ?: d.textMode,
                comicMode = sp.getString("comicMode", d.comicMode) ?: d.comicMode,
                needsRelogin = sp.getBoolean("needsRelogin", d.needsRelogin),
            ),
        )
    }
}
