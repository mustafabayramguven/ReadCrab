package io.github.mustafabayramguven.readcrab.ui.reader.text

import android.annotation.SuppressLint
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebViewAssetLoader
import kotlinx.coroutines.flow.Flow
import org.json.JSONObject
import java.io.File

sealed interface WebCommand {
    data class SetPrefs(val json: String) : WebCommand
    data class Load(val path: String, val ch: Int, val ratio: Double) : WebCommand // path: "/book/{id}/c{n}.html"
    data class Page(val dir: Int) : WebCommand // +1 ileri, -1 geri
}

/**
 * Tek kabuk sayfası (`reader.html`) + yerel dosyadan bölüm gövdesi. Yazı tipleri, `frame.css` ve
 * bölüm HTML'i `WebViewAssetLoader` ile sunucudaki web okuyucuyla aynı kaynaklardan okunur.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun ReaderWebView(
    commands: Flow<WebCommand>,
    bgColor: Int,
    onMessage: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    val loader = remember {
        WebViewAssetLoader.Builder()
            .setDomain("appassets.androidplatform.net")
            .addPathHandler("/static/", WebViewAssetLoader.AssetsPathHandler(ctx))
            .addPathHandler("/book/", WebViewAssetLoader.InternalStoragePathHandler(ctx, File(ctx.filesDir, "rc/books")))
            .build()
    }
    var web by remember { mutableStateOf<WebView?>(null) }
    AndroidView(
        modifier = modifier,
        factory = { c ->
            WebView(c).apply {
                settings.javaScriptEnabled = true
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                settings.domStorageEnabled = false
                settings.setSupportZoom(false)
                settings.textZoom = 100
                isVerticalScrollBarEnabled = false
                isHorizontalScrollBarEnabled = false
                overScrollMode = View.OVER_SCROLL_NEVER
                webViewClient = object : WebViewClient() {
                    override fun shouldInterceptRequest(v: WebView, r: WebResourceRequest) = loader.shouldInterceptRequest(r.url)
                    override fun shouldOverrideUrlLoading(v: WebView, r: WebResourceRequest) = true // dış gezinme yok
                }
                addJavascriptInterface(object { @JavascriptInterface fun post(json: String) = onMessage(json) }, "Android")
                loadUrl("https://appassets.androidplatform.net/static/reader.html")
                web = this
            }
        },
        update = { it.setBackgroundColor(bgColor) },
    )
    LaunchedEffect(web) {
        val w = web ?: return@LaunchedEffect
        commands.collect { c ->
            val js = when (c) {
                is WebCommand.SetPrefs -> "RC.setPrefs(${c.json})"
                is WebCommand.Load -> "RC.load(${JSONObject.quote(c.path)}, ${c.ch}, ${c.ratio})"
                is WebCommand.Page -> "RC.page(${c.dir})"
            }
            w.evaluateJavascript(js, null) // ana iş parçacığı: collect Main'de çalışır
        }
    }
    DisposableEffect(Unit) { onDispose { web?.destroy() } }
}
