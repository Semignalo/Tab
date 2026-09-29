package app.tab.net

import app.tab.model.Discovery
import app.tab.model.Id16
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException

/** Host yang menjawab discovery, dengan kunci publik yang sudah lolos pemeriksaan fingerprint. */
class DiscoveredHost(
    val response: Discovery.Response,
    val address: InetAddress,
) {
    val hid: Id16 get() = response.hid
    val name: String get() = response.name
    val port: Int get() = response.port
    val publicKey: ByteArray get() = response.pk
    val known: Boolean get() = response.known
    val socketAddress: InetSocketAddress get() = InetSocketAddress(address, response.port)
}

object DiscoveryClient {
    private const val SENDS = 3
    private const val SEND_GAP_MS = 300L

    /**
     * Alamat broadcast setiap antarmuka IPv4 aktif — bukan hanya 255.255.255.255, yang sering
     * diblokir Android — ditambah broadcast global sebagai cadangan.
     */
    fun broadcastTargets(port: Int = Discovery.PORT): List<InetSocketAddress> {
        val out = LinkedHashSet<InetSocketAddress>()
        try {
            for (nif in NetworkInterface.getNetworkInterfaces().toList()) {
                if (!nif.isUp || nif.isLoopback) continue
                for (ia in nif.interfaceAddresses) {
                    val b = ia.broadcast ?: continue
                    if (b is Inet4Address) out += InetSocketAddress(b, port)
                }
            }
        } catch (_: Exception) {
            // antarmuka tidak bisa dienumerasi: pakai cadangan saja
        }
        out += InetSocketAddress(InetAddress.getByName("255.255.255.255"), port)
        return out.toList()
    }

    /**
     * Kirim permintaan [SENDS] kali ke [targets] lalu kumpulkan balasan sampai [waitMs].
     * Balasan tanpa fingerprint yang cocok dengan kunci publiknya dibuang.
     */
    suspend fun discover(
        device: Id16,
        waitMs: Long = 1500,
        targets: List<InetSocketAddress> = broadcastTargets(),
    ): List<DiscoveredHost> = withContext(Dispatchers.IO) {
        val found = LinkedHashMap<Id16, DiscoveredHost>()
        val request = Discovery.encodeRequest(device)
        DatagramSocket().use { sock ->
            sock.broadcast = true
            val buf = ByteArray(2048)
            val start = System.nanoTime()
            var sent = 0
            var nextSend = 0L
            while (true) {
                val elapsedMs = (System.nanoTime() - start) / 1_000_000
                if (elapsedMs >= waitMs) break
                if (sent < SENDS && elapsedMs >= nextSend) {
                    for (t in targets) {
                        try {
                            sock.send(DatagramPacket(request, request.size, t))
                        } catch (_: Exception) {
                            // satu antarmuka gagal tidak boleh menggagalkan yang lain
                        }
                    }
                    sent++
                    nextSend += SEND_GAP_MS
                }
                val untilNext = if (sent < SENDS) nextSend - elapsedMs else waitMs - elapsedMs
                sock.soTimeout = untilNext.coerceIn(10, waitMs - elapsedMs).toInt()
                try {
                    val p = DatagramPacket(buf, buf.size)
                    sock.receive(p)
                    val res = Discovery.decodeResponse(p.data, p.length) ?: continue
                    if (res.pk.size != Noise.DH_LEN) continue
                    if (!Noise.fingerprint(res.pk).contentEquals(res.fp)) continue
                    found.putIfAbsent(res.hid, DiscoveredHost(res, p.address))
                } catch (_: SocketTimeoutException) {
                    // lanjut: kirim ulang atau selesai
                }
            }
        }
        found.values.toList()
    }
}
