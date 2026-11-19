package io.github.mustafabayramguven.readcrab.ui.login

import android.app.Application
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import io.github.mustafabayramguven.readcrab.R
import io.github.mustafabayramguven.readcrab.RcApp
import io.github.mustafabayramguven.readcrab.core.CoreException
import io.github.mustafabayramguven.readcrab.ui.theme.Dimens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class LoginState(
    val server: String = "", val username: String = "", val password: String = "",
    val loading: Boolean = false, val error: String? = null, val relogin: Boolean = false, val showPassword: Boolean = false,
)

sealed interface LoginEvent {
    data class Server(val v: String) : LoginEvent
    data class Username(val v: String) : LoginEvent
    data class Password(val v: String) : LoginEvent
    data object TogglePassword : LoginEvent
    data object Submit : LoginEvent
}

sealed interface LoginEffect { data object LoggedIn : LoginEffect }

class LoginViewModel(app: Application) : AndroidViewModel(app) {
    private val rc = app as RcApp

    private val _state = MutableStateFlow(LoginState())
    val state: StateFlow<LoginState> = _state.asStateFlow()

    private val _effects = MutableSharedFlow<LoginEffect>(extraBufferCapacity = 1)
    val effects: SharedFlow<LoginEffect> = _effects.asSharedFlow()

    init {
        val s = rc.core.session()
        _state.update {
            it.copy(
                server = s?.serverUrl ?: "",
                username = s?.username ?: "",
                relogin = rc.prefs.state.value.needsRelogin,
            )
        }
    }

    fun onEvent(e: LoginEvent) {
        when (e) {
            is LoginEvent.Server -> _state.update { it.copy(server = e.v) }
            is LoginEvent.Username -> _state.update { it.copy(username = e.v) }
            is LoginEvent.Password -> _state.update { it.copy(password = e.v) }
            LoginEvent.TogglePassword -> _state.update { it.copy(showPassword = !it.showPassword) }
            LoginEvent.Submit -> submit()
        }
    }

    private fun submit() {
        val s = _state.value
        viewModelScope.launch(Dispatchers.IO) {
            _state.update { it.copy(loading = true, error = null) }
            try {
                rc.core.login(s.server, s.username, s.password)
                rc.prefs.update { it.copy(needsRelogin = false) }
                rc.sync.syncNow()
                _effects.emit(LoginEffect.LoggedIn)
            } catch (e: CoreException) {
                val msg = when (e) {
                    is CoreException.Unauthorized -> getApplication<Application>().getString(R.string.login_error_credentials)
                    is CoreException.Network -> getApplication<Application>().getString(R.string.login_error_network)
                    is CoreException.Tls -> getApplication<Application>().getString(R.string.login_error_tls)
                    else -> e.message ?: ""
                }
                _state.update { it.copy(loading = false, error = msg) }
            }
        }
    }
}

@Composable
fun LoginScreen(onLoggedIn: () -> Unit, vm: LoginViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()

    LaunchedEffect(Unit) {
        vm.effects.collect { e -> when (e) { LoginEffect.LoggedIn -> onLoggedIn() } }
    }

    Column(
        Modifier
            .fillMaxSize()
            .safeDrawingPadding() // kenar-kenara: durum çubuğu, çentik, klavye
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Dimens.gutter, vertical = Dimens.s5),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        // Foreground vektörü saydam zeminde beyaz glif; açık temada görünmesi için mor zemin şart
        Image(
            painter = painterResource(R.drawable.ic_launcher_foreground),
            contentDescription = null,
            modifier = Modifier
                .size(72.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(Color(0xFF6750A4)),
        )
        Spacer(Modifier.height(Dimens.s4))
        Text(stringResource(R.string.login_title), style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(Dimens.s4))

        if (state.relogin) {
            Text(
                stringResource(R.string.login_relogin),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(Dimens.s3))
        }

        OutlinedTextField(
            value = state.server,
            onValueChange = { vm.onEvent(LoginEvent.Server(it)) },
            label = { Text(stringResource(R.string.login_server)) },
            placeholder = { Text(stringResource(R.string.login_server_hint)) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(Dimens.s3))
        OutlinedTextField(
            value = state.username,
            onValueChange = { vm.onEvent(LoginEvent.Username(it)) },
            label = { Text(stringResource(R.string.login_username)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(Dimens.s3))
        OutlinedTextField(
            value = state.password,
            onValueChange = { vm.onEvent(LoginEvent.Password(it)) },
            label = { Text(stringResource(R.string.login_password)) },
            singleLine = true,
            visualTransformation = if (state.showPassword) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            trailingIcon = {
                IconButton(onClick = { vm.onEvent(LoginEvent.TogglePassword) }) {
                    Icon(
                        if (state.showPassword) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                        contentDescription = stringResource(R.string.login_show_password),
                    )
                }
            },
            modifier = Modifier.fillMaxWidth(),
        )

        state.error?.let {
            Spacer(Modifier.height(Dimens.s3))
            Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        }

        Spacer(Modifier.height(Dimens.s4))
        Button(
            onClick = { vm.onEvent(LoginEvent.Submit) },
            enabled = !state.loading,
            modifier = Modifier.fillMaxWidth(),
        ) {
            if (state.loading) {
                CircularProgressIndicator(Modifier.size(20.dp))
            } else {
                Text(stringResource(R.string.login_button))
            }
        }
    }
}
