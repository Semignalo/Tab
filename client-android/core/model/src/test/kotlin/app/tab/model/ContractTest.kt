package app.tab.model

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.test.assertFailsWith

/**
 * Uji kontrak terhadap berkas .cbor di protocol/fixtures, yang dihasilkan sisi Rust. Setiap fixture
 * harus (1) terbaca menjadi tipe yang benar dan (2) terserialisasi ulang menjadi byte yang
 * **identik** — inilah yang mencegah kedua implementasi protokol berjalan sendiri-sendiri.
 */
class ContractTest {
    private val dir = File(System.getProperty("tab.fixtures") ?: "../../protocol/fixtures")

    /** nama file → tag pesan yang diharapkan */
    private val expected = mapOf(
        "hello" to "Hello", "welcome" to "Welcome", "pair_required" to "PairRequired",
        "pair_request" to "PairRequest", "pair_ok" to "PairOk", "ping" to "Ping", "pong" to "Pong",
        "set_mode" to "SetMode", "mode_state" to "ModeState", "error_pin_invalid" to "Error",
        "input_batch" to "InputBatch", "input_settings" to "InputSettings",
        "deck_profile" to "DeckProfile", "deck_press" to "DeckPress", "metrics" to "Metrics",
        "now_playing" to "NowPlaying", "media_command" to "MediaCommand",
    )

    @Test
    fun fixtureDirectoryIsFound() {
        assertTrue(dir.isDirectory, "fixture tidak ditemukan di ${dir.absolutePath}")
        assertTrue(dir.listFiles { f -> f.extension == "cbor" }!!.size >= 17)
    }

    @Test
    fun everyFixtureDecodesToTheRightTypeAndReEncodesByteForByte() {
        val files = dir.listFiles { f -> f.extension == "cbor" }!!.sortedBy { it.name }
        for (file in files) {
            val bytes = file.readBytes()
            val decoded = Codec.decode(bytes)
            val known = assertIs<Decoded.Known>(decoded, "${file.name} terbaca sebagai Unknown")
            val name = file.nameWithoutExtension
            expected[name]?.let { assertEquals(it, known.message.tag, "tag ${file.name}") }
            assertEquals(
                bytes.toHex(), Codec.encode(known.message).toHex(),
                "${file.name}: hasil encode ulang tidak identik dengan fixture Rust",
            )
        }
    }

    @Test
    fun everyExpectedFixtureExists() {
        for (name in expected.keys) assertTrue(File(dir, "$name.cbor").exists(), "$name.cbor hilang")
    }

    @Test
    fun helloFixtureHasExpectedContent() {
        val m = Codec.decode(File(dir, "hello.cbor").readBytes()) as Decoded.Known
        val h = assertIs<Message.Hello>(m.message)
        assertEquals(1, h.pv)
        assertEquals("Pixel 8", h.name)
        assertEquals("android", h.plat)
        assertEquals(2.625f, h.scr.dpi)
        assertEquals(Id16.fromHex("0123456789abcdef0123456789abcdef"), h.dev)
    }

    @Test
    fun capabilitiesFixtureIsHonest() {
        val w = (Codec.decode(File(dir, "welcome.cbor").readBytes()) as Decoded.Known).message
        val caps = assertIs<Message.Welcome>(w).caps
        assertTrue(Mode.Trackpad in caps.modes)
        assertEquals(false, caps.secondScreen)
        assertEquals(false, caps.gamepad)
    }

    @Test
    fun metricsFixtureOmitsAbsentSensorsAndDecodesTheRest() {
        val m = (Codec.decode(File(dir, "metrics.cbor").readBytes()) as Decoded.Known).message
        val metrics = assertIs<Message.Metrics>(m)
        assertEquals(4, metrics.cpu.size)
        assertEquals(true, metrics.pwr.ac)
    }

    @Test
    fun unknownMessageTypeIsIgnoredNotAnError() {
        val bytes = Cbor.encode(tagged("SecondScreenFrame") { uint("w", 1920) })
        assertEquals(Decoded.Unknown("SecondScreenFrame"), Codec.decode(bytes))
    }

    @Test
    fun knownTagWithBrokenFieldsIsAnError() {
        val bytes = Cbor.encode(tagged("Hello") { })
        assertFailsWith<ProtocolException> { Codec.decode(bytes) }
    }

    @Test
    fun garbageIsRejectedWithoutCrashing() {
        assertFailsWith<CborException> { Codec.decode(byteArrayOf(0x5f)) }
        assertFailsWith<CborException> { Codec.decode(byteArrayOf(0x82.toByte(), 0x01)) }
        // array yang mengaku 2^32 elemen tidak boleh mengalokasikan apa pun
        assertFailsWith<CborException> {
            Codec.decode(byteArrayOf(0x9a.toByte(), 0xff.toByte(), 0xff.toByte(), 0xff.toByte(), 0xff.toByte()))
        }
        assertFailsWith<CborException> { Codec.decode(byteArrayOf(0x01, 0x02)) }
    }

    @Test
    fun secretsNeverAppearInToString() {
        val tok = Message.PairOk(ByteArray(32) { 0x7a })
        assertTrue(!tok.toString().contains("7a") && !tok.toString().contains("122"))
        assertTrue(!Message.ClipboardPush("kata sandi rahasia").toString().contains("rahasia"))
    }

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }
}
