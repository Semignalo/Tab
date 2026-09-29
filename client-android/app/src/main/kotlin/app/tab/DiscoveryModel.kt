package app.tab

import app.tab.model.Discovery
import app.tab.model.Id16
import app.tab.net.DiscoveredHost
import app.tab.net.DiscoveryClient
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import java.net.InetAddress
import java.net.InetSocketAddress
import kotlin.coroutines.coroutineContext

/** Daftar host yang terlihat di jaringan. Berjalan hanya selama layar daftar tampil. */
class DiscoveryModel(private val device: Id16) {
    private val _hosts = MutableStateFlow<List<DiscoveredHost>>(emptyList())
    val hosts: StateFlow<List<DiscoveredHost>> = _hosts.asStateFlow()

    private val _scanning = MutableStateFlow(false)
    val scanning: StateFlow<Boolean> = _scanning.asStateFlow()

    /** Panggil dari coroutine yang dibatalkan saat layar pergi. */
    suspend fun run() {
        while (coroutineContext.isActive) {
            _scanning.value = true
            try {
                _hosts.value = DiscoveryClient.discover(device, 1500)
            } catch (_: Exception) {
                // jaringan belum siap; coba lagi
            } finally {
                _scanning.value = false
            }
            delay(2500)
        }
    }

    /** Untuk jaringan yang memblokir broadcast: tanya satu alamat IP langsung (unicast). */
    suspend fun probe(ip: String): DiscoveredHost? = try {
        val addr = InetAddress.getByName(ip.trim())
        DiscoveryClient.discover(device, 1500, listOf(InetSocketAddress(addr, Discovery.PORT))).firstOrNull()
    } catch (_: Exception) {
        null
    }
}
