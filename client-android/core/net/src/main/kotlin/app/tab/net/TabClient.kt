package app.tab.net

import app.tab.model.ErrorCode
import app.tab.model.Id16
import app.tab.model.InputEvent
import app.tab.model.Message
import app.tab.model.Mode
import app.tab.model.Screen
import app.tab.model.TelemetryKind
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ClosedReceiveChannelException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.IOException
import java.net.InetSocketAddress
import kotlin.random.Random

data class DeviceProfile(
    val name: String,
    val platformVersion: String,
    val appVersion: String,
    val screenWidth: Int,
    val screenHeight: Int,
    val dpi: Float,
    val platform: String = "android",
)

sealed interface ClientState {
    data object Idle : ClientState

    /** [attempt] = 0 untuk sambungan pertama; naik pada tiap percobaan ulang. */
    data class Connecting(val host: HostRecord, val attempt: Int, val lastError: String?) : ClientState

    data class AwaitingPin(val hostName: String, val ttlMs: Long) : ClientState
    data class Connected(val host: HostRecord, val welcome: Message.Welcome) : ClientState

    /** Token dicabut/tidak cocok: perangkat harus dipasangkan ulang. Tidak ada retry otomatis. */
    data class NeedsPairing(val host: HostRecord, val reason: String) : ClientState

    /** Kunci host berubah tanpa pairing ulang — kemungkinan host palsu; koneksi ditolak. */
    data class KeyChanged(val host: HostRecord) : ClientState
}

sealed interface PairStart {
    data class NeedsPin(val ttlMs: Long) : PairStart
    data class Refused(val code: ErrorCode?, val message: String) : PairStart
}

sealed interface PairResult {
    data object Paired : PairResult
    data class WrongPin(val attemptsLeft: Int) : PairResult
    data class Refused(val code: ErrorCode?, val message: String) : PairResult
}

/** Jadwal backoff §9: 250 ms → 500 ms → 1 s → 2 s → 4 s, batas 5 s, jitter ±20%. */
object Backoff {
    fun delayMs(attempt: Int, random: Random = Random.Default): Long {
        val base = (250L shl attempt.coerceIn(0, 5)).coerceAtMost(5_000L)
        val jitter = 0.8 + random.nextDouble() * 0.4
        return (base * jitter).toLong()
    }
}

/**
 * Klien Tab lengkap: pairing, sambungan yang dijaga terus (reconnect + backoff), heartbeat,
 * pemulihan mode/langganan setelah reconnect, dan pipa input berbatch.
 *
 * Dari sisi user, Wi-Fi yang putus sesaat tidak boleh menyebabkan keluar dari mode maupun
 * meminta pairing ulang — karena itu mode terakhir dan langganan telemetri diingat di sini,
 * bukan di layar.
 */
class TabClient(
    private val scope: CoroutineScope,
    private val storage: HostStorage,
    private val device: DeviceProfile,
    private val discoveryTargets: () -> List<InetSocketAddress> = { DiscoveryClient.broadcastTargets() },
    private val nowMs: () -> Long = { System.nanoTime() / 1_000_000 },
    private val random: Random = Random.Default,
) {
    private val _state = MutableStateFlow<ClientState>(ClientState.Idle)
    val state: StateFlow<ClientState> = _state.asStateFlow()

    private val _snapshot = MutableStateFlow(Snapshot())
    val snapshot: StateFlow<Snapshot> = _snapshot.asStateFlow()

    private val _rttMs = MutableStateFlow<Float?>(null)

    /** RTT terakhir dalam milidetik (dari Ping/Pong tiap 2 s). */
    val rttMs: StateFlow<Float?> = _rttMs.asStateFlow()

    private val deviceId: Id16 = storage.deviceId()
    private val assembler = ArtworkAssembler()
    private var deckSeq = 0L

    @Volatile
    private var live: Connection? = null
    private var supervisor: Job? = null

    // Yang harus dipulihkan setelah reconnect.
    @Volatile
    private var desiredMode: Mode? = null
    @Volatile
    private var metricsIntervalMs: Long? = null

    // Pairing yang sedang menunggu PIN.
    private var pendingConn: Connection? = null
    private var pendingHost: DiscoveredHost? = null

    private val inputChannel = Channel<InputEvent>(Channel.UNLIMITED)

    init {
        scope.launch { pumpInput() }
    }

    fun knownHosts(): List<HostRecord> = storage.hosts()

    // ------------------------------------------------------------------ pairing

    /** Buka sesi pairing ke [host]; hasilnya menentukan apakah UI meminta PIN. */
    suspend fun startPairing(host: DiscoveredHost): PairStart {
        cancelPairing()
        disconnect()
        val conn = try {
            Connection.open(host.socketAddress, host.publicKey, deviceId, Credential.Pair)
        } catch (e: IOException) {
            return PairStart.Refused(null, "tidak bisa tersambung: ${e.message}")
        }
        conn.send(hello())
        return when (val m = receiveOrNull(conn)) {
            is Message.PairRequired -> {
                pendingConn = conn
                pendingHost = host
                _state.value = ClientState.AwaitingPin(host.name, m.ttlMs)
                PairStart.NeedsPin(m.ttlMs)
            }
            is Message.Error -> {
                conn.close()
                PairStart.Refused(m.c, m.msg)
            }
            else -> {
                conn.close()
                PairStart.Refused(null, "host tidak menjawab")
            }
        }
    }

    suspend fun submitPin(pin: String): PairResult {
        val conn = pendingConn ?: return PairResult.Refused(null, "tidak ada sesi pairing")
        val host = pendingHost ?: return PairResult.Refused(null, "tidak ada sesi pairing")
        conn.send(Message.PairRequest(pin))
        when (val m = receiveOrNull(conn)) {
            is Message.PairOk -> {
                val welcome = receiveOrNull(conn) as? Message.Welcome
                if (welcome == null) {
                    cancelPairing()
                    return PairResult.Refused(null, "host tidak mengirim Welcome")
                }
                val record = HostRecord(
                    hid = host.hid, name = host.name, publicKey = host.publicKey, token = m.tok,
                    address = host.address.hostAddress ?: "", port = host.port,
                )
                storage.save(record)
                pendingConn = null
                pendingHost = null
                // Koneksi pairing langsung menjadi sesi hidup; tidak perlu reconnect.
                startSupervisor(record.hid, conn to welcome)
                return PairResult.Paired
            }
            is Message.Error -> {
                if (m.c == ErrorCode.PinInvalid && (m.attemptsLeft ?: 0) > 0) {
                    return PairResult.WrongPin(m.attemptsLeft ?: 0)
                }
                cancelPairing()
                return if (m.c == ErrorCode.PinInvalid) PairResult.WrongPin(0)
                else PairResult.Refused(m.c, m.msg)
            }
            else -> {
                cancelPairing()
                return PairResult.Refused(null, "koneksi terputus")
            }
        }
    }

    fun cancelPairing() {
        pendingConn?.close()
        pendingConn = null
        pendingHost = null
        if (_state.value is ClientState.AwaitingPin) _state.value = ClientState.Idle
    }

    // ------------------------------------------------------------------ sambungan

    /** Mulai (atau mulai ulang) sambungan yang dijaga ke host yang sudah dipasangkan. */
    fun connect(hid: Id16) = startSupervisor(hid, null)

    fun disconnect() {
        supervisor?.cancel()
        supervisor = null
        live?.close()
        live = null
        _rttMs.value = null
        if (_state.value !is ClientState.NeedsPairing && _state.value !is ClientState.KeyChanged) {
            _state.value = ClientState.Idle
        }
    }

    /** Lupakan host secara lokal (token di sisi host tetap ada sampai dicabut di sana). */
    fun forget(hid: Id16) {
        disconnect()
        storage.remove(hid)
        _state.value = ClientState.Idle
    }

    /** Untuk test: putuskan koneksi seolah Wi-Fi mati, tanpa memberi tahu supervisor. */
    internal fun dropConnectionForTest() {
        live?.close()
    }

    private fun startSupervisor(hid: Id16, initial: Pair<Connection, Message.Welcome>?) {
        supervisor?.cancel()
        supervisor = scope.launch { supervise(hid, initial) }
    }

    private suspend fun supervise(hid: Id16, initial: Pair<Connection, Message.Welcome>?) {
        var attempt = 0
        var lastError: String? = null
        var conn = initial?.first
        var welcome = initial?.second
        var record = storage.host(hid) ?: return

        while (scope.isActive) {
            if (conn == null || welcome == null) {
                record = storage.host(hid) ?: return
                _state.value = ClientState.Connecting(record, attempt, lastError)
                when (val r = tryConnect(record)) {
                    is ConnectResult.Ok -> {
                        conn = r.conn
                        welcome = r.welcome
                        record = r.record
                    }
                    is ConnectResult.Revoked -> {
                        val cleared = record.copy(token = null)
                        storage.save(cleared)
                        _state.value = ClientState.NeedsPairing(cleared, r.reason)
                        return
                    }
                    is ConnectResult.KeyChanged -> {
                        _state.value = ClientState.KeyChanged(record)
                        return
                    }
                    is ConnectResult.Retry -> {
                        lastError = r.reason
                        delay(Backoff.delayMs(attempt, random))
                        attempt++
                        continue
                    }
                }
            }

            val end = runSession(conn, welcome, record)
            conn.close()
            live = null
            _rttMs.value = null
            conn = null
            welcome = null
            if (end == SessionEnd.Revoked) {
                val cleared = record.copy(token = null)
                storage.save(cleared)
                _state.value = ClientState.NeedsPairing(cleared, "perangkat dicabut di host")
                return
            }
            // Gangguan sesaat: coba lagi segera dengan jadwal backoff dari awal.
            attempt = 0
            lastError = "koneksi terputus"
            delay(Backoff.delayMs(0, random))
        }
    }

    private sealed interface ConnectResult {
        class Ok(val conn: Connection, val welcome: Message.Welcome, val record: HostRecord) : ConnectResult
        class Revoked(val reason: String) : ConnectResult
        class Retry(val reason: String) : ConnectResult
        data object KeyChanged : ConnectResult
    }

    private sealed interface Attempt {
        class Ok(val conn: Connection, val welcome: Message.Welcome) : Attempt
        class Unreachable(val why: String) : Attempt
        class HandshakeFailed(val tokenMismatch: Boolean) : Attempt
        class Refused(val revoked: Boolean, val why: String) : Attempt
    }

    /** Urutan §9: alamat cache → discovery → sambung. */
    private suspend fun tryConnect(rec: HostRecord): ConnectResult {
        val token = rec.token ?: return ConnectResult.Revoked("belum dipasangkan")

        var current = rec
        val direct = attempt(current, token, InetSocketAddress(current.address, current.port))
        when (direct) {
            is Attempt.Ok -> return ConnectResult.Ok(direct.conn, direct.welcome, current)
            is Attempt.Refused -> return if (direct.revoked) ConnectResult.Revoked(direct.why) else ConnectResult.Retry(direct.why)
            // Handshake gagal di pesan kedua: PSK tidak cocok, artinya token sudah tidak berlaku.
            is Attempt.HandshakeFailed -> if (direct.tokenMismatch) return ConnectResult.Revoked("token tidak cocok dengan host")
            is Attempt.Unreachable -> Unit
        }

        // Alamat lama tidak menjawab (atau host menutup tanpa penjelasan): cari host lewat discovery.
        val found = DiscoveryClient.discover(deviceId, 1200, discoveryTargets()).firstOrNull { it.hid == rec.hid }
            ?: return ConnectResult.Retry(if (direct is Attempt.Unreachable) direct.why else "host tidak ditemukan")
        if (!found.publicKey.contentEquals(rec.publicKey)) return ConnectResult.KeyChanged
        if (!found.known) return ConnectResult.Revoked("host tidak mengenal perangkat ini")

        current = rec.copy(address = found.address.hostAddress ?: rec.address, port = found.port)
        storage.save(current)
        return when (val a = attempt(current, token, found.socketAddress)) {
            is Attempt.Ok -> ConnectResult.Ok(a.conn, a.welcome, current)
            is Attempt.Refused -> if (a.revoked) ConnectResult.Revoked(a.why) else ConnectResult.Retry(a.why)
            is Attempt.HandshakeFailed -> if (a.tokenMismatch) ConnectResult.Revoked("token tidak cocok dengan host") else ConnectResult.Retry("handshake gagal")
            is Attempt.Unreachable -> ConnectResult.Retry(a.why)
        }
    }

    private suspend fun attempt(rec: HostRecord, token: ByteArray, addr: InetSocketAddress): Attempt {
        val conn = try {
            Connection.open(addr, rec.publicKey, deviceId, Credential.Resume(token))
        } catch (e: HandshakeException) {
            return Attempt.HandshakeFailed(tokenMismatch = e.cause2 is NoiseException)
        } catch (e: IOException) {
            return Attempt.Unreachable(e.message ?: e.javaClass.simpleName)
        }
        conn.send(hello())
        return when (val first = receiveOrNull(conn)) {
            is Message.Welcome -> Attempt.Ok(conn, first)
            is Message.Error -> {
                conn.close()
                Attempt.Refused(first.c == ErrorCode.TokenRevoked, first.msg)
            }
            // Ditutup sebelum Welcome: host tidak mengenal perangkat ini.
            else -> {
                conn.close()
                Attempt.HandshakeFailed(tokenMismatch = false)
            }
        }
    }

    /** Baca satu pesan dengan batas waktu; `null` bila koneksi tutup atau timeout. */
    private suspend fun receiveOrNull(conn: Connection): Message? = try {
        conn.receive(5_000)
    } catch (_: TimeoutCancellationException) {
        null
    } catch (_: ClosedReceiveChannelException) {
        null
    }

    private enum class SessionEnd { Dropped, Revoked }

    private suspend fun runSession(conn: Connection, welcome: Message.Welcome, rec: HostRecord): SessionEnd {
        live = conn
        assembler.clear()
        _snapshot.value = Snapshot(mode = desiredMode ?: Mode.Idle)
        _state.value = ClientState.Connected(rec, welcome)

        // Pulihkan mode dan langganan: gangguan Wi-Fi tidak boleh terasa oleh user.
        desiredMode?.let { conn.send(Message.SetMode(it)) }
        metricsIntervalMs?.let { conn.send(Message.SubscribeTelemetry(listOf(TelemetryKind.Metrics), it)) }

        var lastRx = nowMs()
        val heartbeat = scope.launch {
            var n = 0L
            while (isActive) {
                n++
                conn.send(Message.Ping(n, nowMs() * 1000))
                delay(PING_INTERVAL_MS)
                // Tanpa satu pun frame dari host selama 6 s → anggap putus.
                if (nowMs() - lastRx > SESSION_TIMEOUT_MS) {
                    conn.close()
                    return@launch
                }
            }
        }

        try {
            while (true) {
                val msg = conn.receive()
                lastRx = nowMs()
                when (msg) {
                    is Message.Pong -> _rttMs.value = (nowMs() * 1000 - msg.tc) / 1000f
                    is Message.Bye -> return SessionEnd.Dropped
                    is Message.Error -> {
                        if (msg.c == ErrorCode.TokenRevoked) return SessionEnd.Revoked
                        _snapshot.update { it.reduce(msg, nowMs(), assembler, deckSeq) }
                    }
                    else -> {
                        if (msg is Message.DeckFeedback) deckSeq++
                        _snapshot.update { it.reduce(msg, nowMs(), assembler, deckSeq) }
                    }
                }
            }
        } catch (_: ClosedReceiveChannelException) {
            return SessionEnd.Dropped
        } catch (e: CancellationException) {
            throw e
        } finally {
            heartbeat.cancel()
        }
    }

    private fun hello() = Message.Hello(
        pv = 1, dev = deviceId, name = device.name, plat = device.platform,
        platv = device.platformVersion, app = device.appVersion,
        scr = Screen(device.screenWidth.toLong(), device.screenHeight.toLong(), device.dpi),
    )

    // ------------------------------------------------------------------ perintah

    fun send(message: Message): Boolean = live?.send(message) ?: false

    /** Pindah mode; mode diingat untuk dipulihkan setelah reconnect. */
    fun setMode(mode: Mode) {
        desiredMode = mode
        send(Message.SetMode(mode))
    }

    fun subscribeMetrics(intervalMs: Long) {
        val iv = intervalMs.coerceAtLeast(250)
        metricsIntervalMs = iv
        send(Message.SubscribeTelemetry(listOf(TelemetryKind.Metrics), iv))
    }

    fun unsubscribeMetrics() {
        metricsIntervalMs = null
        send(Message.SubscribeTelemetry(emptyList(), 1000))
    }

    /** Antrekan event input; dikirim berbatch (maks. 64 per batch, ≥ 8 ms antar batch). */
    fun input(event: InputEvent) {
        inputChannel.trySend(event)
    }

    private suspend fun pumpInput() {
        var seq = 0L
        for (first in inputChannel) {
            val batch = ArrayList<InputEvent>(8)
            batch += first
            while (batch.size < MAX_BATCH) {
                batch += inputChannel.tryReceive().getOrNull() ?: break
            }
            // Tidak tersambung: buang, jangan menumpuk lalu membanjiri host saat pulih.
            live?.send(Message.InputBatch(++seq, batch))
            delay(BATCH_INTERVAL_MS)
        }
    }

    companion object {
        const val PING_INTERVAL_MS = 2_000L
        const val SESSION_TIMEOUT_MS = 6_000L
        const val MAX_BATCH = 64
        const val BATCH_INTERVAL_MS = 8L
    }
}
