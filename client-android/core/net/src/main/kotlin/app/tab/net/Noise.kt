package app.tab.net

import org.bouncycastle.crypto.digests.Blake2sDigest
import org.bouncycastle.crypto.macs.HMac
import org.bouncycastle.crypto.modes.ChaCha20Poly1305
import org.bouncycastle.crypto.params.KeyParameter
import org.bouncycastle.crypto.params.ParametersWithIV
import org.bouncycastle.math.ec.rfc7748.X25519
import java.security.SecureRandom

/**
 * Noise Protocol Framework untuk Tab, sisi initiator saja.
 *
 * Dua pola (PROTOCOL.md §4):
 *  - `Noise_NK_25519_ChaChaPoly_BLAKE2s`     — perangkat baru; PIN membuktikan kepemilikan.
 *  - `Noise_NKpsk2_25519_ChaChaPoly_BLAKE2s` — resume; token menjadi PSK, tidak pernah dikirim.
 *
 * Primitif kripto dari Bouncy Castle; hanya "lem" protokol yang ditulis di sini, dan
 * kebenarannya diuji langsung terhadap host Rust (snow) di `LiveHostTest`.
 */
object Noise {
    const val NAME_PAIR = "Noise_NK_25519_ChaChaPoly_BLAKE2s"
    const val NAME_RESUME = "Noise_NKpsk2_25519_ChaChaPoly_BLAKE2s"
    const val TOKEN_LEN = 32
    const val DH_LEN = 32
    const val TAG_LEN = 16
    const val MAX_MESSAGE = 65_535
    const val HASH_LEN = 32

    fun fingerprint(publicKey: ByteArray): ByteArray = blake2s(publicKey)

    fun blake2s(vararg parts: ByteArray): ByteArray {
        val d = Blake2sDigest(256)
        for (p in parts) d.update(p, 0, p.size)
        return ByteArray(HASH_LEN).also { d.doFinal(it, 0) }
    }

    private fun hmac(key: ByteArray, vararg parts: ByteArray): ByteArray {
        val mac = HMac(Blake2sDigest(256))
        mac.init(KeyParameter(key))
        for (p in parts) mac.update(p, 0, p.size)
        return ByteArray(HASH_LEN).also { mac.doFinal(it, 0) }
    }

    /** HKDF Noise: `outputs` ∈ {2, 3} kunci 32 byte. */
    internal fun hkdf(chainingKey: ByteArray, ikm: ByteArray, outputs: Int): List<ByteArray> {
        val temp = hmac(chainingKey, ikm)
        val o1 = hmac(temp, byteArrayOf(0x01))
        val o2 = hmac(temp, o1, byteArrayOf(0x02))
        if (outputs == 2) return listOf(o1, o2)
        val o3 = hmac(temp, o2, byteArrayOf(0x03))
        return listOf(o1, o2, o3)
    }

    internal fun dh(privateKey: ByteArray, publicKey: ByteArray): ByteArray {
        val out = ByteArray(DH_LEN)
        X25519.scalarMult(privateKey, 0, publicKey, 0, out, 0)
        // Hasil semua-nol berarti titik publik berorde kecil: tolak.
        if (out.all { it == 0.toByte() }) throw NoiseException("kunci publik tidak sah")
        return out
    }

    internal class CipherState {
        var key: ByteArray? = null
        var nonce: Long = 0

        fun init(k: ByteArray) {
            key = k
            nonce = 0
        }

        fun hasKey() = key != null

        private fun nonceBytes(): ByteArray {
            val n = ByteArray(12)
            var v = nonce
            for (i in 0 until 8) {
                n[4 + i] = (v and 0xFF).toByte()
                v = v ushr 8
            }
            return n
        }

        fun encrypt(ad: ByteArray, plain: ByteArray): ByteArray {
            val k = key ?: return plain
            val aead = ChaCha20Poly1305()
            aead.init(true, ParametersWithIV(KeyParameter(k), nonceBytes()))
            aead.processAADBytes(ad, 0, ad.size)
            val out = ByteArray(plain.size + TAG_LEN)
            var n = aead.processBytes(plain, 0, plain.size, out, 0)
            n += aead.doFinal(out, n)
            check(n == out.size)
            nonce++
            return out
        }

        fun decrypt(ad: ByteArray, cipher: ByteArray): ByteArray {
            val k = key ?: return cipher
            if (cipher.size < TAG_LEN) throw NoiseException("frame terlalu pendek")
            val aead = ChaCha20Poly1305()
            aead.init(false, ParametersWithIV(KeyParameter(k), nonceBytes()))
            aead.processAADBytes(ad, 0, ad.size)
            val out = ByteArray(cipher.size - TAG_LEN)
            try {
                var n = aead.processBytes(cipher, 0, cipher.size, out, 0)
                n += aead.doFinal(out, n)
                check(n == out.size)
            } catch (e: org.bouncycastle.crypto.InvalidCipherTextException) {
                throw NoiseException("dekripsi gagal")
            }
            nonce++
            return out
        }
    }

    /** Handshake initiator yang sedang berjalan. */
    class Handshake private constructor(
        private val remoteStatic: ByteArray,
        private val psk: ByteArray?,
        private val random: SecureRandom,
    ) {
        private val cs = CipherState()
        private var h: ByteArray
        private var ck: ByteArray
        private var ephemeralPrivate = ByteArray(32)
        private var wrote = false

        init {
            require(remoteStatic.size == DH_LEN) { "kunci publik host harus 32 byte" }
            val name = (if (psk == null) NAME_PAIR else NAME_RESUME).toByteArray(Charsets.US_ASCII)
            h = if (name.size <= HASH_LEN) name.copyOf(HASH_LEN) else blake2s(name)
            ck = h
            mixHash(ByteArray(0)) // prolog kosong
            mixHash(remoteStatic) // pre-message: <- s
        }

        private fun mixHash(data: ByteArray) {
            h = blake2s(h, data)
        }

        private fun mixKey(ikm: ByteArray) {
            val (nck, k) = hkdf(ck, ikm, 2)
            ck = nck
            cs.init(k)
        }

        private fun mixKeyAndHash(ikm: ByteArray) {
            val (nck, tempH, k) = hkdf(ck, ikm, 3)
            ck = nck
            mixHash(tempH)
            cs.init(k)
        }

        /** Pesan pertama: `e, es` dengan payload kosong. */
        fun writeMessage1(): ByteArray {
            check(!wrote) { "pesan 1 sudah ditulis" }
            wrote = true
            random.nextBytes(ephemeralPrivate)
            val ePub = ByteArray(DH_LEN)
            X25519.scalarMultBase(ephemeralPrivate, 0, ePub, 0)
            mixHash(ePub)
            if (psk != null) mixKey(ePub)
            mixKey(dh(ephemeralPrivate, remoteStatic))
            val payload = cs.encrypt(h, ByteArray(0))
            mixHash(payload)
            return ePub + payload
        }

        /** Pesan kedua: `e, ee[, psk]`. Kegagalan di sini berarti token/kunci tidak cocok. */
        fun readMessage2(msg: ByteArray): Session {
            check(wrote) { "pesan 1 belum ditulis" }
            if (msg.size < DH_LEN + TAG_LEN || msg.size > MAX_MESSAGE) throw NoiseException("handshake rusak")
            val re = msg.copyOfRange(0, DH_LEN)
            mixHash(re)
            if (psk != null) mixKey(re)
            mixKey(dh(ephemeralPrivate, re))
            if (psk != null) mixKeyAndHash(psk)
            val payload = msg.copyOfRange(DH_LEN, msg.size)
            cs.decrypt(h, payload)
            mixHash(payload)

            val (k1, k2) = hkdf(ck, ByteArray(0), 2)
            ephemeralPrivate.fill(0)
            val send = CipherState().apply { init(k1) }
            val recv = CipherState().apply { init(k2) }
            return Session(send, recv)
        }

        companion object {
            fun pairing(hostPublic: ByteArray, random: SecureRandom = SecureRandom()) =
                Handshake(hostPublic, null, random)

            fun resume(hostPublic: ByteArray, token: ByteArray, random: SecureRandom = SecureRandom()): Handshake {
                if (token.size != TOKEN_LEN) throw NoiseException("token harus $TOKEN_LEN byte")
                return Handshake(hostPublic, token, random)
            }
        }
    }

    /** Sesi terenkripsi setelah handshake. Tidak thread-safe per arah: satu penulis, satu pembaca. */
    class Session internal constructor(
        private val send: CipherState,
        private val recv: CipherState,
    ) {
        private val empty = ByteArray(0)

        @Synchronized
        fun encrypt(plain: ByteArray): ByteArray {
            if (plain.size > MAX_MESSAGE - TAG_LEN) throw NoiseException("payload terlalu besar")
            return send.encrypt(empty, plain)
        }

        @Synchronized
        fun decrypt(frame: ByteArray): ByteArray = recv.decrypt(empty, frame)
    }
}

class NoiseException(message: String) : Exception(message)
