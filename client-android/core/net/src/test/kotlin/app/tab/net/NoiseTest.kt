package app.tab.net

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals

class NoiseTest {
    private fun hex(s: String) = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }

    @Test
    fun blake2sMatchesRfc7693Vector() {
        // RFC 7693 Appendix B: BLAKE2s-256("abc")
        assertEquals(
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982",
            Noise.blake2s("abc".toByteArray()).toHex(),
        )
    }

    @Test
    fun x25519MatchesRfc7748Vector() {
        // RFC 7748 §5.2, vektor uji pertama.
        val scalar = hex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4")
        val point = hex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")
        assertEquals(
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
            Noise.dh(scalar, point).toHex(),
        )
    }

    @Test
    fun hkdfOutputsAreDistinctAndDeterministic() {
        val ck = ByteArray(32) { 1 }
        val a = Noise.hkdf(ck, ByteArray(32) { 2 }, 3)
        val b = Noise.hkdf(ck, ByteArray(32) { 2 }, 3)
        assertEquals(3, a.size)
        for (i in 0..2) assertContentEquals(a[i], b[i])
        assertNotEquals(a[0].toHex(), a[1].toHex())
        assertNotEquals(a[1].toHex(), a[2].toHex())
        // Dua keluaran pertama sama untuk output=2 dan output=3.
        val two = Noise.hkdf(ck, ByteArray(32) { 2 }, 2)
        assertContentEquals(a[0], two[0])
        assertContentEquals(a[1], two[1])
    }

    @Test
    fun cipherStateRoundTripsAndAdvancesNonce() {
        val send = Noise.CipherState().apply { init(ByteArray(32) { 9 }) }
        val recv = Noise.CipherState().apply { init(ByteArray(32) { 9 }) }
        val ad = byteArrayOf(1, 2, 3)
        val c1 = send.encrypt(ad, "halo".toByteArray())
        val c2 = send.encrypt(ad, "halo".toByteArray())
        assertNotEquals(c1.toHex(), c2.toHex(), "nonce harus maju, ciphertext berbeda")
        assertContentEquals("halo".toByteArray(), recv.decrypt(ad, c1))
        assertContentEquals("halo".toByteArray(), recv.decrypt(ad, c2))
    }

    @Test
    fun tamperedOrReplayedFramesAreRejected() {
        val send = Noise.CipherState().apply { init(ByteArray(32) { 9 }) }
        val recv = Noise.CipherState().apply { init(ByteArray(32) { 9 }) }
        val ad = ByteArray(0)
        val c = send.encrypt(ad, "data".toByteArray())
        val bad = c.copyOf().also { it[it.size - 1] = (it[it.size - 1].toInt() xor 0xff).toByte() }
        assertFailsWith<NoiseException> { recv.decrypt(ad, bad) }
        // Nonce tidak maju pada kegagalan: frame asli tetap bisa dibaca sekali...
        assertContentEquals("data".toByteArray(), recv.decrypt(ad, c))
        // ...tetapi tidak bisa diputar ulang.
        assertFailsWith<NoiseException> { recv.decrypt(ad, c) }
    }

    @Test
    fun resumeRequires32ByteToken() {
        assertFailsWith<NoiseException> { Noise.Handshake.resume(ByteArray(32), ByteArray(16)) }
    }

    @Test
    fun firstHandshakeMessageHasExpectedShape() {
        val hs = Noise.Handshake.pairing(ByteArray(32) { 5 })
        val m1 = hs.writeMessage1()
        assertEquals(32 + 16, m1.size, "e (32) + tag payload kosong (16)")
    }

    @Test
    fun backoffFollowsSpecSchedule() {
        val fixed = object : kotlin.random.Random() {
            override fun nextBits(bitCount: Int) = 0 // nextDouble() == 0.0 → faktor jitter 0.8
        }
        val expectedBase = longArrayOf(250, 500, 1000, 2000, 4000, 5000, 5000)
        for ((i, base) in expectedBase.withIndex()) {
            val d = Backoff.delayMs(i, fixed)
            assertEquals((base * 0.8).toLong(), d, "attempt $i")
        }
        // Jitter selalu ±20%
        repeat(200) {
            val d = Backoff.delayMs(2)
            assert(d in 800..1200) { "jitter di luar ±20%: $d" }
        }
    }
}
