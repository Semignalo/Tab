package app.tab.net

import app.tab.model.Codec
import app.tab.model.Decoded
import app.tab.model.Id16
import app.tab.model.Message
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.trySendBlocking
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import java.io.DataInputStream
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket

/** Bukti identitas untuk sebuah koneksi. */
sealed interface Credential {
    /** Perangkat baru: `Noise_NK`, lalu PIN. */
    data object Pair : Credential

    /** Perangkat yang sudah dipasangkan: `Noise_NKpsk2` dengan [token]. */
    class Resume(val token: ByteArray) : Credential
}

/** Gagal sebelum sesi terenkripsi berdiri. */
class HandshakeException(message: String, val cause2: Throwable? = null) : IOException(message, cause2)

/**
 * Satu koneksi TCP terenkripsi: preamble → handshake Noise → frame terenkripsi.
 *
 * Pembaca dan penulis masing-masing satu coroutine, sehingga nonce Noise per arah tidak pernah
 * diakses bersamaan. Pesan tak dikenal (versi protokol lebih baru) dilewati diam-diam.
 */
class Connection private constructor(
    private val socket: Socket,
    private val session: Noise.Session,
    private val scope: CoroutineScope,
) {
    private val out = java.io.DataOutputStream(socket.getOutputStream().buffered(4096))
    private val incoming = Channel<Message>(capacity = 256)
    private val outgoing = Channel<Message>(capacity = 1024)

    @Volatile
    var closeCause: Throwable? = null
        private set

    val isClosed: Boolean get() = !scope.isActive

    init {
        scope.launch(Dispatchers.IO) { readLoop() }
        scope.launch(Dispatchers.IO) { writeLoop() }
    }

    /** `false` bila koneksi sudah tertutup atau antrean penuh (host tidak membaca). */
    fun send(message: Message): Boolean = outgoing.trySend(message).isSuccess

    /** Pesan berikutnya; melempar bila koneksi tertutup. */
    suspend fun receive(): Message = incoming.receive()

    suspend fun receive(timeoutMs: Long): Message = withTimeout(timeoutMs) { incoming.receive() }

    fun close() {
        socket.closeQuietly()
        scope.cancel()
        incoming.close()
        outgoing.close()
    }

    private fun fail(t: Throwable) {
        if (closeCause == null) closeCause = t
        close()
    }

    private fun writeOne(msg: Message) {
        Framing.write(out, session.encrypt(Codec.encode(msg)))
    }

    private fun readLoop() {
        try {
            val input = DataInputStream(socket.getInputStream().buffered(8192))
            while (scope.isActive) {
                val frame = Framing.read(input)
                val plain = session.decrypt(frame)
                when (val d = Codec.decode(plain)) {
                    is Decoded.Known -> incoming.trySendBlocking(d.message).getOrThrow()
                    is Decoded.Unknown -> Unit
                }
            }
        } catch (t: Throwable) {
            fail(t)
        }
    }

    private suspend fun writeLoop() {
        try {
            while (true) {
                val first = outgoing.receiveCatching().getOrNull() ?: break
                writeOne(first)
                // Tulis semua yang sudah menunggu, lalu flush sekali: satu paket per batch, tanpa
                // menahan pesan tunggal (latensi input).
                while (true) {
                    val next = outgoing.tryReceive().getOrNull() ?: break
                    writeOne(next)
                }
                out.flush()
            }
        } catch (t: Throwable) {
            fail(t)
        }
    }

    companion object {
        private const val CONNECT_TIMEOUT_MS = 5_000
        private const val HANDSHAKE_TIMEOUT_MS = 5_000

        /**
         * Buka koneksi dan selesaikan handshake. [hostPublic] adalah kunci yang sudah dipin.
         * @throws HandshakeException bila token/kunci tidak cocok atau host menutup koneksi
         * @throws IOException bila host tidak terjangkau
         */
        suspend fun open(
            address: InetSocketAddress,
            hostPublic: ByteArray,
            device: Id16,
            credential: Credential,
        ): Connection = withContext(Dispatchers.IO) {
            val socket = Socket()
            try {
                socket.tcpNoDelay = true
                socket.keepAlive = true
                socket.connect(address, CONNECT_TIMEOUT_MS)
                socket.soTimeout = HANDSHAKE_TIMEOUT_MS

                val handshake = when (credential) {
                    is Credential.Pair -> Noise.Handshake.pairing(hostPublic)
                    is Credential.Resume -> Noise.Handshake.resume(hostPublic, credential.token)
                }
                val intent = if (credential is Credential.Pair) INTENT_PAIR else INTENT_RESUME

                val outStream = DataOutputStreamOf(socket)
                Framing.write(outStream, byteArrayOf(intent.toByte()) + device.bytes)
                Framing.write(outStream, handshake.writeMessage1())
                outStream.flush()

                val session = try {
                    handshake.readMessage2(Framing.read(DataInputStream(socket.getInputStream())))
                } catch (e: NoiseException) {
                    throw HandshakeException("handshake ditolak: ${e.message}", e)
                } catch (e: IOException) {
                    // Host menutup tanpa menjawab: perangkat tidak dikenal / token dicabut.
                    throw HandshakeException("host menutup koneksi saat handshake", e)
                }
                socket.soTimeout = 0
                Connection(socket, session, CoroutineScope(SupervisorJob() + Dispatchers.IO))
            } catch (t: Throwable) {
                socket.closeQuietly()
                throw t
            }
        }

        private const val INTENT_PAIR = 0x01
        private const val INTENT_RESUME = 0x02

        private fun DataOutputStreamOf(s: Socket) = java.io.DataOutputStream(s.getOutputStream().buffered(1024))
    }
}

internal fun Socket.closeQuietly() {
    try {
        close()
    } catch (_: IOException) {
    }
}

/** Framing TCP: `u32` big-endian panjang + body, maksimum 65_535 byte. */
internal object Framing {
    fun read(input: DataInputStream): ByteArray {
        val len = input.readInt()
        if (len <= 0 || len > Noise.MAX_MESSAGE) throw IOException("panjang frame tidak sah: $len")
        val buf = ByteArray(len)
        input.readFully(buf)
        return buf
    }

    fun write(out: java.io.DataOutputStream, body: ByteArray) {
        require(body.isNotEmpty() && body.size <= Noise.MAX_MESSAGE) { "ukuran frame tidak sah" }
        out.writeInt(body.size)
        out.write(body)
    }
}
