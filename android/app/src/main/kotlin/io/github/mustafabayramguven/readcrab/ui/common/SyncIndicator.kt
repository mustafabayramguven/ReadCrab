package io.github.mustafabayramguven.readcrab.ui.common

import android.text.format.DateUtils
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudDone
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.CloudUpload
import androidx.compose.material.icons.outlined.SyncProblem
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.sync.SyncKind

@Composable
fun SyncIndicatorButton(modifier: Modifier = Modifier) {
    val app = LocalContext.current.applicationContext as RcApp
    val ind by app.sync.indicator.collectAsStateWithLifecycle()
    var dialog by remember { mutableStateOf(false) }

    val stateText = when (ind.kind) {
        SyncKind.SYNCING -> stringResource(R.string.sync_state_syncing)
        SyncKind.SYNCED -> stringResource(R.string.sync_state_synced)
        SyncKind.PENDING -> stringResource(R.string.sync_state_pending, ind.pending)
        SyncKind.OFFLINE -> stringResource(R.string.sync_state_offline)
        SyncKind.ERROR -> stringResource(R.string.sync_state_error)
    }
    val tint = if (ind.kind == SyncKind.ERROR) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant
    val lastText = ind.lastOkAt?.let {
        stringResource(R.string.sync_dialog_last, DateUtils.getRelativeTimeSpanString(it * 1000).toString())
    } ?: stringResource(R.string.sync_dialog_never)

    IconButton(onClick = { dialog = true }, modifier = modifier) {
        BadgedBox(
            badge = {
                if (ind.kind == SyncKind.PENDING && ind.pending > 0) {
                    Badge { Text(ind.pending.toString()) }
                }
            },
        ) {
            when (ind.kind) {
                SyncKind.SYNCING -> CircularProgressIndicator(
                    Modifier.size(20.dp).semantics { contentDescription = stateText },
                    strokeWidth = 2.dp,
                )
                SyncKind.SYNCED -> Icon(Icons.Outlined.CloudDone, contentDescription = stateText, tint = tint)
                SyncKind.PENDING -> Icon(Icons.Outlined.CloudUpload, contentDescription = stateText, tint = tint)
                SyncKind.OFFLINE -> Icon(Icons.Outlined.CloudOff, contentDescription = stateText, tint = tint)
                SyncKind.ERROR -> Icon(Icons.Outlined.SyncProblem, contentDescription = stateText, tint = tint)
            }
        }
    }

    if (dialog) {
        AlertDialog(
            onDismissRequest = { dialog = false },
            title = { Text(stateText) },
            text = {
                Column {
                    Text(lastText)
                    if (ind.pending > 0) Text(stringResource(R.string.sync_state_pending, ind.pending))
                    ind.error?.let { Text(stringResource(R.string.sync_dialog_error, it)) }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    dialog = false
                    app.sync.syncNow()
                }) { Text(stringResource(R.string.settings_sync_now)) }
            },
            dismissButton = {
                TextButton(onClick = { dialog = false }) { Text(stringResource(R.string.action_close)) }
            },
        )
    }
}
