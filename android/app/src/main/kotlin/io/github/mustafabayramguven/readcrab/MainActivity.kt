package io.github.mustafabayramguven.readcrab

import android.graphics.Color
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mustafabayramguven.readcrab.ui.nav.AppNav
import io.github.mustafabayramguven.readcrab.ui.theme.ReadCrabTheme
import io.github.mustafabayramguven.readcrab.ui.theme.isEffectiveDark

// Okuyucular açıkken ses tuşlarını kendi sayfa çevirmesine yönlendirir.
object VolumeKeys {
    @Volatile var handler: ((forward: Boolean) -> Boolean)? = null
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val app = application as RcApp
            val prefs by app.prefs.state.collectAsStateWithLifecycle()
            // durum/gezinme çubuğu ikonları sistem değil uygulama temasını izler
            val dark = isEffectiveDark(prefs, isSystemInDarkTheme())
            DisposableEffect(dark) {
                enableEdgeToEdge(
                    statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark },
                    // androidx varsayılan perdeleri; API 26-28 düğmeli gezinmede ikon okunurluğu için
                    navigationBarStyle = SystemBarStyle.auto(0xE6FFFFFF.toInt(), 0x801B1B1B.toInt()) { dark },
                )
                onDispose {}
            }
            // kökte Surface yoksa LocalContentColor Siyah kalır (okuyucu çubukları, giriş ekranı)
            ReadCrabTheme(prefs) { Surface(Modifier.fillMaxSize()) { AppNav(app) } }
        }
    }

    override fun onStart() {
        super.onStart()
        (application as RcApp).sync.onForeground()
    }

    override fun onStop() {
        (application as RcApp).sync.foreground = false
        super.onStop()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP) {
            val h = VolumeKeys.handler
            if (h != null && h(keyCode == KeyEvent.KEYCODE_VOLUME_DOWN)) return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if ((keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP) && VolumeKeys.handler != null) return true
        return super.onKeyUp(keyCode, event)
    }
}
