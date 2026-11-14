package io.github.mustafabayramguven.readcrab.ui.nav

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.ui.library.LibraryScreen
import io.github.mustafabayramguven.readcrab.ui.login.LoginScreen
import io.github.mustafabayramguven.readcrab.ui.reader.comic.ComicReaderScreen
import io.github.mustafabayramguven.readcrab.ui.reader.text.TextReaderScreen
import io.github.mustafabayramguven.readcrab.ui.series.SeriesScreen
import io.github.mustafabayramguven.readcrab.ui.settings.SettingsScreen
import io.github.mustafabayramguven.readcrab.ui.stats.StatsScreen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@Composable
fun AppNav(app: RcApp) {
    val nav = rememberNavController()
    val prefs by app.prefs.state.collectAsStateWithLifecycle()
    val start = remember { if (app.core.session() == null) "giris" else "kutuphane" }
    val toLogin: () -> Unit = { nav.navigate("giris") { popUpTo(0) } }
    // oturum düştüyse girişe yalnız çevrimiçiyken ve okuyucuda değilken dönülür (çevrimdışı okuma sürer)
    val sessionExpired: () -> Unit = {
        val route = nav.currentDestination?.route
        if (app.connectivity.online.value && route != null && route != "giris" && !route.startsWith("oku/")) toLogin()
    }
    val online by app.connectivity.online.collectAsStateWithLifecycle()
    val entry by nav.currentBackStackEntryAsState()
    LaunchedEffect(prefs.needsRelogin, online, entry?.destination?.route) {
        if (prefs.needsRelogin) sessionExpired()
    }
    // okuyucudan geri: yığın boşsa kütüphaneye; seriesId kullanılmaz
    val readerBack: (Long) -> Unit = { _ ->
        if (!nav.popBackStack()) nav.navigate("kutuphane") { popUpTo(0) }
    }
    // sonraki cilt: mevcut okuyucunun yerine geçer, geri tuşu seriye döner
    val nextBook: (Long) -> Unit = { id ->
        nav.navigate("oku/$id") { popUpTo("oku/{bookId}") { inclusive = true } }
    }
    NavHost(navController = nav, startDestination = start) {
        composable("giris") {
            LoginScreen(
                onLoggedIn = {
                    nav.navigate("kutuphane") { popUpTo("giris") { inclusive = true } }
                },
            )
        }
        composable("kutuphane") {
            LibraryScreen(
                onOpenSeries = { id -> nav.navigate("seri/$id") },
                onOpenBook = { id -> nav.navigate("oku/$id") },
                onOpenSettings = { nav.navigate("ayarlar") },
                onOpenStats = { nav.navigate("istatistik") },
                onSessionExpired = sessionExpired,
            )
        }
        composable(
            route = "seri/{seriesId}",
            arguments = listOf(navArgument("seriesId") { type = NavType.LongType }),
        ) { entry ->
            SeriesScreen(
                seriesId = entry.arguments!!.getLong("seriesId"),
                onBack = { nav.popBackStack() },
                onOpenBook = { id -> nav.navigate("oku/$id") },
                onSessionExpired = sessionExpired,
            )
        }
        composable(
            route = "oku/{bookId}",
            arguments = listOf(navArgument("bookId") { type = NavType.LongType }),
        ) { entry ->
            ReaderRoute(
                app = app,
                bookId = entry.arguments!!.getLong("bookId"),
                onBack = readerBack,
                onNextBook = nextBook,
                onSessionExpired = toLogin,
            )
        }
        composable("istatistik") {
            StatsScreen(
                onBack = { nav.popBackStack() },
                onSessionExpired = sessionExpired,
            )
        }
        composable("ayarlar") {
            SettingsScreen(
                onBack = { nav.popBackStack() },
                onLoggedOut = toLogin,
                onSessionExpired = sessionExpired,
            )
        }
    }
}

@Composable
private fun ReaderRoute(
    app: RcApp, bookId: Long,
    onBack: (Long) -> Unit, onNextBook: (Long) -> Unit, onSessionExpired: () -> Unit,
) {
    // 0 = yükleniyor, 1 = çizgi roman, 2 = metin, 3 = hata
    val kind by produceState(0, bookId) {
        value = withContext(Dispatchers.IO) {
            runCatching { if (app.core.getBook(bookId).book.isComic) 1 else 2 }.getOrDefault(3)
        }
    }
    when (kind) {
        0 -> Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface), Alignment.Center) { CircularProgressIndicator() }
        1 -> ComicReaderScreen(bookId = bookId, onBack = onBack, onNextBook = onNextBook, onSessionExpired = onSessionExpired)
        2 -> TextReaderScreen(bookId = bookId, onBack = onBack, onNextBook = onNextBook, onSessionExpired = onSessionExpired)
        else -> LaunchedEffect(Unit) { onBack(0L) }
    }
}
