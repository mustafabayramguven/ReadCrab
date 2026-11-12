package io.github.mustafabayramguven.readcrab.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.mustafabayramguven.readcrab.UiPrefs

val LightScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF6750A4), onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFEADDFF), onPrimaryContainer = Color(0xFF21005D),
    secondary = Color(0xFF625B71), onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFE8DEF8), onSecondaryContainer = Color(0xFF1D192B),
    tertiary = Color(0xFF7D5260), onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFFFD8E4), onTertiaryContainer = Color(0xFF31111D),
    error = Color(0xFFB3261E), onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFF9DEDC), onErrorContainer = Color(0xFF410E0B),
    background = Color(0xFFFEF7FF), onBackground = Color(0xFF1D1B20),
    surface = Color(0xFFFEF7FF), onSurface = Color(0xFF1D1B20),
    surfaceVariant = Color(0xFFE7E0EC), onSurfaceVariant = Color(0xFF49454F),
    outline = Color(0xFF79747E), outlineVariant = Color(0xFFCAC4D0),
    scrim = Color(0xFF000000),
    inverseSurface = Color(0xFF322F35), inverseOnSurface = Color(0xFFF5EFF7), inversePrimary = Color(0xFFD0BCFF),
    surfaceDim = Color(0xFFDED8E1), surfaceBright = Color(0xFFFEF7FF),
    surfaceContainerLowest = Color(0xFFFFFFFF), surfaceContainerLow = Color(0xFFF7F2FA),
    surfaceContainer = Color(0xFFF3EDF7), surfaceContainerHigh = Color(0xFFECE6F0),
    surfaceContainerHighest = Color(0xFFE6E0E9),
)

val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFD0BCFF), onPrimary = Color(0xFF381E72),
    primaryContainer = Color(0xFF4F378B), onPrimaryContainer = Color(0xFFEADDFF),
    secondary = Color(0xFFCCC2DC), onSecondary = Color(0xFF332D41),
    secondaryContainer = Color(0xFF4A4458), onSecondaryContainer = Color(0xFFE8DEF8),
    tertiary = Color(0xFFEFB8C8), onTertiary = Color(0xFF492532),
    tertiaryContainer = Color(0xFF633B48), onTertiaryContainer = Color(0xFFFFD8E4),
    error = Color(0xFFF2B8B5), onError = Color(0xFF601410),
    errorContainer = Color(0xFF8C1D18), onErrorContainer = Color(0xFFF9DEDC),
    background = Color(0xFF141218), onBackground = Color(0xFFE6E0E9),
    surface = Color(0xFF141218), onSurface = Color(0xFFE6E0E9),
    surfaceVariant = Color(0xFF49454F), onSurfaceVariant = Color(0xFFCAC4D0),
    outline = Color(0xFF938F99), outlineVariant = Color(0xFF49454F),
    scrim = Color(0xFF000000),
    inverseSurface = Color(0xFFE6E0E9), inverseOnSurface = Color(0xFF322F35), inversePrimary = Color(0xFF6750A4),
    surfaceDim = Color(0xFF141218), surfaceBright = Color(0xFF3B383E),
    surfaceContainerLowest = Color(0xFF0F0D13), surfaceContainerLow = Color(0xFF1D1B20),
    surfaceContainer = Color(0xFF211F26), surfaceContainerHigh = Color(0xFF2B2930),
    surfaceContainerHighest = Color(0xFF36343B),
)

// Saf siyah (AMOLED): koyu şemanın üstüne, yalnız koyu etkinken uygulanır.
fun amoled(s: ColorScheme): ColorScheme = s.copy(
    background = Color.Black, surface = Color.Black, surfaceDim = Color.Black,
    surfaceBright = Color.Black, surfaceVariant = Color.Black,
    surfaceContainerLowest = Color.Black, surfaceContainerLow = Color.Black,
    surfaceContainer = Color.Black, surfaceContainerHigh = Color.Black,
    surfaceContainerHighest = Color.Black, surfaceTint = Color.Black,
    outlineVariant = Color(0xFF3A363E), // siyah yüzeyde kart/ayırıcı çizgisi görünsün
)

enum class ThemeMode { SYSTEM, LIGHT, DARK }

@Composable
fun ReadCrabTheme(prefs: UiPrefs, content: @Composable () -> Unit) {
    val dark = when (prefs.themeMode) {
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
    }
    val ctx = LocalContext.current
    val dynamic = prefs.dynamicColor && Build.VERSION.SDK_INT >= 31
    val base = when {
        dynamic && dark -> dynamicDarkColorScheme(ctx)
        dynamic -> dynamicLightColorScheme(ctx)
        dark -> DarkScheme
        else -> LightScheme
    }
    val scheme = if (dark && prefs.amoled) amoled(base) else base
    MaterialTheme(colorScheme = scheme, typography = RcTypography, shapes = RcShapes, content = content)
}

// ReaderTheme "auto" çözümü için
fun isEffectiveDark(prefs: UiPrefs, systemDark: Boolean): Boolean = when (prefs.themeMode) {
    ThemeMode.SYSTEM -> systemDark
    ThemeMode.LIGHT -> false
    ThemeMode.DARK -> true
}

private val baseType = Typography()

val RcTypography: Typography = baseType.copy(
    titleLarge = baseType.titleLarge.copy(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.Medium),
    titleMedium = baseType.titleMedium.copy(fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.Medium, letterSpacing = 0.15.sp),
    labelLarge = baseType.labelLarge.copy(fontSize = 14.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    labelSmall = baseType.labelSmall.copy(fontSize = 11.sp, lineHeight = 16.sp),
    bodyMedium = baseType.bodyMedium.copy(fontSize = 14.sp, lineHeight = 20.sp),
)

val RcShapes = Shapes(
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(12.dp),
    large = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

object Dimens {
    val s1 = 4.dp
    val s2 = 8.dp
    val s3 = 12.dp
    val s4 = 16.dp
    val s5 = 24.dp
    val s6 = 32.dp
    val gutter = 16.dp // ekran kenar boşluğu
    val coverRadius = 8.dp
    val coverAspect = 2f / 3f
    val gridMin = 112.dp
    val gridGap = 12.dp
    val barHeight = 56.dp
    val progressTrack = 3.dp
    val bookRowCover = 56.dp // genişlik; yükseklik 84.dp
    val continueCardWidth = 104.dp
}

data class ReaderPalette(val bg: String, val fg: String, val link: String)

// Okuyucu renkleri; "auto" çağıran tarafından light|dark|black'e çözülür
fun readerColors(theme: String): ReaderPalette = when (theme) {
    "light" -> ReaderPalette("#FEF7FF", "#1D1B20", "#6750A4")
    "dark" -> ReaderPalette("#141218", "#E6E0E9", "#D0BCFF")
    "sepia" -> ReaderPalette("#F4ECD8", "#433422", "#7A4A12")
    "black" -> ReaderPalette("#000000", "#E6E0E9", "#D0BCFF")
    else -> throw IllegalArgumentException("Bilinmeyen okuyucu teması: $theme")
}

// Çizgi roman zemini (Prefs.comicBg); bilinmeyen anahtar siyah
fun comicBgColor(key: String): Color = when (key) {
    "dark" -> Color(0xFF141218)
    "sepia" -> Color(0xFFF4ECD8)
    "light" -> Color(0xFFFEF7FF)
    else -> Color.Black
}
