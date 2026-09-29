package app.tab

import app.tab.data.HostCodec
import app.tab.data.TokenCipher
import app.tab.model.Id16
import app.tab.net.HostRecord
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class HostCodecTest {
    /** Enkripsi palsu yang dapat dikenali: memastikan token memang lewat cipher. */
    private object FakeCipher : TokenCipher {
        override fun encrypt(plain: ByteArray) = "ENC:" + plain.joinToString(",")
        override fun decrypt(encoded: String): ByteArray {
            require(encoded.startsWith("ENC:")) { "bukan hasil encrypt" }
            return encoded.removePrefix("ENC:").split(",").map { it.toInt().toByte() }.toByteArray()
        }
    }

    private object BrokenCipher : TokenCipher {
        override fun encrypt(plain: ByteArray) = error("tidak dipakai")
        override fun decrypt(encoded: String): ByteArray = error("kunci Keystore hilang")
    }

    private fun rec(token: ByteArray?) = HostRecord(
        Id16.fromHex("0123456789abcdef0123456789abcdef")!!, "MacBook", ByteArray(32) { it.toByte() }, token, "192.168.1.5", 4180,
    )

    @Test
    fun roundTripKeepsEverything() {
        val token = ByteArray(32) { (it * 3).toByte() }
        val json = HostCodec.encode(listOf(rec(token)), FakeCipher)
        val back = HostCodec.decode(json, FakeCipher).single()
        assertEquals("MacBook", back.name)
        assertEquals("192.168.1.5", back.address)
        assertEquals(4180, back.port)
        assertContentEquals(token, back.token)
        assertContentEquals(ByteArray(32) { it.toByte() }, back.publicKey)
    }

    @Test
    fun tokenNeverAppearsInPlainTextInStoredJson() {
        val token = ByteArray(32) { 0x7f }
        val json = HostCodec.encode(listOf(rec(token)), FakeCipher)
        assertTrue(!json.contains("7f7f7f7f"), "token tidak boleh tersimpan polos")
    }

    @Test
    fun unpairedHostHasNullToken() {
        val back = HostCodec.decode(HostCodec.encode(listOf(rec(null)), FakeCipher), FakeCipher).single()
        assertNull(back.token)
    }

    @Test
    fun lostKeystoreKeyDegradesToUnpairedInsteadOfCrashing() {
        val json = HostCodec.encode(listOf(rec(ByteArray(32))), FakeCipher)
        val back = HostCodec.decode(json, BrokenCipher).single()
        assertNull(back.token)
        assertEquals("MacBook", back.name)
    }
}
