package io.github.mustafabayramguven.readcrab

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

// VALIDATED aranmaz: LAN sunucuları için internet doğrulaması yapılmaz.
class Connectivity(ctx: Context) {
    private val _online = MutableStateFlow(false)
    val online: StateFlow<Boolean> = _online.asStateFlow()

    init {
        val cm = ctx.getSystemService(ConnectivityManager::class.java)
        _online.value = cm.activeNetwork != null
        cm.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                _online.value = true
            }

            override fun onLost(network: Network) {
                _online.value = false
            }

            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                _online.value = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            }
        })
    }
}
