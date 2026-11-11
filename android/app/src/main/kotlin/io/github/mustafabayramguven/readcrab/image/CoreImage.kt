package io.github.mustafabayramguven.readcrab.image

import coil3.ImageLoader
import coil3.decode.DataSource
import coil3.decode.ImageSource
import coil3.fetch.FetchResult
import coil3.fetch.Fetcher
import coil3.fetch.SourceFetchResult
import coil3.key.Keyer
import coil3.request.Options
import io.github.mustafabayramguven.readcrab.core.Core
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okio.FileSystem
import okio.Path.Companion.toOkioPath
import java.io.File

// Coil 3 özel modeli: kapak ve çizgi roman sayfası çekirdekten dosya yolu olarak çözülür.
sealed interface RcImage {
    data class Cover(val bookId: Long, val version: Long) : RcImage
    data class Page(val bookId: Long, val page: Int) : RcImage
}

class RcKeyer : Keyer<RcImage> {
    override fun key(data: RcImage, options: Options): String = when (data) {
        is RcImage.Cover -> "rc-cover-${data.bookId}-${data.version}"
        is RcImage.Page -> "rc-page-${data.bookId}-${data.page}"
    }
}

class RcFetcher(private val data: RcImage, private val core: Core) : Fetcher {
    override suspend fun fetch(): FetchResult {
        val path = withContext(Dispatchers.IO) {
            when (data) {
                is RcImage.Cover -> core.coverPath(data.bookId, data.version)
                is RcImage.Page -> core.pagePath(data.bookId, data.page)
            }
        }
        return SourceFetchResult(
            source = ImageSource(file = File(path).toOkioPath(), fileSystem = FileSystem.SYSTEM),
            mimeType = null,
            dataSource = DataSource.DISK,
        )
    }

    class Factory(private val core: Core) : Fetcher.Factory<RcImage> {
        override fun create(data: RcImage, options: Options, imageLoader: ImageLoader): Fetcher = RcFetcher(data, core)
    }
}
