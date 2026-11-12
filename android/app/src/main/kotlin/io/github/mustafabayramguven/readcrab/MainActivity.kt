package io.github.mustafabayramguven.readcrab

import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mustafabayramguven.readcrab.ui.nav.AppNav
import io.github.mustafabayramguven.readcrab.ui.theme.ReadCrabTheme

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
            ReadCrabTheme(prefs) { AppNav(app) }
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
