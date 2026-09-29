package app.tab.net

import app.tab.model.ErrorCode
import app.tab.model.InputEvent
import app.tab.model.Message
import app.tab.model.Mode
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Before
import java.io.File
import java.net.InetSocketAddress
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** Host Rust sungguhan (`tab-testhost`) sebagai proses anak; membaca stdout-nya sebagai log. */
class HostProcess(exe: File) {
    private val proc = ProcessBuilder(exe.absolutePath).redirectErrorStream(true).start()
    val lines = CopyOnWriteArrayList<String>()
    val sessionPort: Int
    val discoveryPort: Int
    val publicKey: ByteArray
    val hid: String

    init {
        Thread {
            proc.inputStream.bufferedReader().forEachLine { lines += it }
        }.apply { isDaemon = true }.start()
        val ready = waitLine(10_000) { it.startsWith("READY ") }
        val kv = ready.removePrefix("READY ").split(" ").associate { it.substringBefore("=") to it.substringAfter("=") }
        sessionPort = kv.getValue("session").toInt()
        discoveryPort = kv.getValue("discovery").toInt()
        publicKey = ByteArray(32) { kv.getValue("pk").substring(it * 2, it * 2 + 2).toInt(16).toByte() }
        hid = kv.getValue("hid")
    }

    fun command(line: String) {
        proc.outputStream.write((line + "\n").toByteArray())
        proc.outputStream.flush()
    }

    fun waitLine(timeoutMs: Long = 5_000, from: Int = 0, pred: (String) -> Boolean): String {
        val end = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < end) {
            lines.drop(from).firstOrNull(pred)?.let { return it }
            Thread.sleep(10)
        }
        error("baris yang ditunggu tidak muncul. Log host:\n" + lines.joinToString("\n"))
    }

    fun newPin(): String {
        val from = lines.size
        command("pair")
        return waitLine(from = from) { it.startsWith("PIN ") }.removePrefix("PIN ")
    }

    fun stop() {
        runCatching { command("quit") }
        proc.destroy()
    }
}

/**
 * Uji lintas-bahasa: klien Kotlin ↔ host Rust sungguhan. Inilah bukti bahwa implementasi
 * Noise, framing, dan CBOR di sini benar-benar cocok dengan sisi Rust (snow + ciborium).
 * Dilewati bila `tab-testhost` belum dibangun.
 */
class LiveHostTest {
    private lateinit var host: HostProcess
    private lateinit var scope: CoroutineScope
    private val profile = DeviceProfile("Pixel Test", "15", "0.1.0", 1080, 2400, 2.6f)

    @Before
    fun setUp() {
        val exe = File(System.getProperty("tab.testhost") ?: "")
        assumeTrue("tab-testhost belum dibangun: cargo build -p tab-host --bins", exe.isFile)
        host = HostProcess(exe)
        scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    }

    @After
    fun tearDown() {
        if (::scope.isInitialized) scope.cancel()
        if (::host.isInitialized) host.stop()
    }

    private fun targets() = { listOf(InetSocketAddress("127.0.0.1", host.discoveryPort)) }

    private fun newClient(storage: HostStorage = MemoryHostStorage()) =
        TabClient(scope, storage, profile, discoveryTargets = targets()) to storage

    private suspend fun discover(storage: HostStorage): DiscoveredHost =
        DiscoveryClient.discover(storage.deviceId(), 800, targets()()).single()

    private suspend fun pairFully(client: TabClient, storage: HostStorage): DiscoveredHost {
        val found = discover(storage)
        val pin = host.newPin()
        assertIs<PairStart.NeedsPin>(client.startPairing(found))
        assertEquals(PairResult.Paired, client.submitPin(pin))
        withTimeout(5_000) { client.state.first { it is ClientState.Connected } }
        return found
    }

    @Test
    fun discoveryFindsHostAndVerifiesFingerprint() = runBlocking<Unit> {
        val storage = MemoryHostStorage()
        val found = discover(storage)
        assertEquals("TestHost", found.name)
        assertEquals(host.sessionPort, found.port)
        assertContentEquals32(host.publicKey, found.publicKey)
        assertEquals(false, found.known)
    }

    private fun assertContentEquals32(a: ByteArray, b: ByteArray) =
        assertTrue(a.contentEquals(b), "kunci publik host berbeda")

    @Test
    fun pairingWithPinConnectsAndPersistsToken() = runBlocking<Unit> {
        val (client, storage) = newClient()
        val found = pairFully(client, storage)

        val st = assertIs<ClientState.Connected>(client.state.value)
        assertTrue(Mode.Trackpad in st.welcome.caps.modes)
        assertEquals(false, st.welcome.caps.obs)
        val rec = assertNotNull(storage.host(found.hid))
        assertNotNull(rec.token, "token harus tersimpan")
        assertEquals(32, rec.token!!.size)
    }

    @Test
    fun wrongPinReportsAttemptsLeftThenCorrectPinStillWorks() = runBlocking<Unit> {
        val (client, storage) = newClient()
        val found = discover(storage)
        val pin = host.newPin()
        val wrong = if (pin == "000000") "111111" else "000000"
        assertIs<PairStart.NeedsPin>(client.startPairing(found))
        assertEquals(PairResult.WrongPin(4), client.submitPin(wrong))
        assertEquals(PairResult.WrongPin(3), client.submitPin(wrong))
        assertEquals(PairResult.Paired, client.submitPin(pin))
    }

    @Test
    fun pairingWithoutOpenWindowIsRefusedAsBusy() = runBlocking<Unit> {
        val (client, storage) = newClient()
        val found = discover(storage)
        val r = client.startPairing(found)
        val refused = assertIs<PairStart.Refused>(r)
        assertEquals(ErrorCode.PairingBusy, refused.code)
    }

    @Test
    fun secondLaunchResumesWithStoredTokenAndNoPin() = runBlocking<Unit> {
        val (first, storage) = newClient()
        val found = pairFully(first, storage)
        first.disconnect()

        // "Aplikasi dibuka lagi": klien baru, penyimpanan yang sama.
        val (second, _) = newClient(storage)
        second.connect(found.hid)
        withTimeout(8_000) { second.state.first { it is ClientState.Connected } }
        // Tidak ada PIN baru diminta.
        assertEquals(1, host.lines.count { it.startsWith("PIN ") }, "hanya satu PIN (pairing pertama)")
        // Heartbeat berjalan: RTT terukur.
        withTimeout(6_000) { second.rttMs.first { it != null } }
    }

    @Test
    fun inputEventsReachTheHostScaledAndInOrder() = runBlocking<Unit> {
        val (client, storage) = newClient()
        pairFully(client, storage)
        client.setMode(Mode.Trackpad)
        host.command("settings 2.0 true")
        withTimeout(5_000) { client.snapshot.first { it.inputSettings?.sens == 2.0f } }

        client.input(InputEvent.PointerMove(3f, -1.5f))
        client.input(InputEvent.PointerButton(app.tab.model.Button.L, true))
        client.input(InputEvent.PointerButton(app.tab.model.Button.L, false))
        client.input(InputEvent.Key("a", true))
        client.input(InputEvent.Key("a", false))
        client.input(InputEvent.Text("héllo ✓"))

        host.waitLine { it.contains("Move(6.0, -3.0)") }
        host.waitLine { it.contains("Button(L, true)") }
        host.waitLine { it.contains("Button(L, false)") }
        host.waitLine { it.contains("Key(\"a\", true)") }
        host.waitLine { it.contains("Text(\"héllo ✓\")") }
    }

    @Test
    fun monitorModeStreamsMetricsAndLeavingStopsThem() = runBlocking<Unit> {
        val (client, storage) = newClient()
        pairFully(client, storage)
        client.setMode(Mode.Monitor)
        client.subscribeMetrics(250)
        val m = withTimeout(5_000) { client.snapshot.first { it.metrics != null } }.metrics!!
        assertTrue(m.cpu.isNotEmpty())
        assertEquals(null, m.tcpu, "sensor yang tidak ada dihilangkan, bukan nol")

        client.setMode(Mode.Clock)
        withTimeout(5_000) { client.snapshot.first { it.mode == Mode.Clock } }
        delay(400)
        val before = client.snapshot.value.metrics
        delay(900)
        assertTrue(client.snapshot.value.metrics === before, "telemetri harus berhenti di luar mode Monitor")
    }

    @Test
    fun deckProfileArrivesAndPressGivesFeedback() = runBlocking<Unit> {
        val (client, storage) = newClient()
        pairFully(client, storage)
        client.setMode(Mode.Deck)
        val profile = withTimeout(5_000) { client.snapshot.first { it.deckProfile != null } }.deckProfile!!
        assertEquals(6, profile.btns.size)

        val play = profile.btns.first { it.aid == "play" }
        client.send(Message.DeckPress("play", play.i))
        val fb = withTimeout(5_000) { client.snapshot.first { it.deckResult != null } }.deckResult!!
        assertEquals("play", fb.aid)
        assertTrue(fb.ok)
        host.waitLine { it.startsWith("RAN ") && it.contains("media_play") }
    }

    @Test
    fun artworkIsReassembledFromChunks() = runBlocking<Unit> {
        val (client, storage) = newClient()
        pairFully(client, storage)
        client.send(Message.GetArtwork("fake-art"))
        val snap = withTimeout(5_000) { client.snapshot.first { it.artwork.containsKey("fake-art") } }
        assertEquals(20_000, snap.artwork.getValue("fake-art").size)
    }

    @Test
    fun droppedConnectionReconnectsAndRestoresModeAndSubscription() = runBlocking<Unit> {
        val (client, storage) = newClient()
        pairFully(client, storage)
        client.setMode(Mode.Monitor)
        client.subscribeMetrics(250)
        withTimeout(5_000) { client.snapshot.first { it.metrics != null } }

        client.dropConnectionForTest()
        withTimeout(8_000) { client.state.first { it is ClientState.Connecting } }
        withTimeout(10_000) { client.state.first { it is ClientState.Connected } }

        // Mode dan langganan pulih sendiri; metrik kembali mengalir tanpa campur tangan user.
        withTimeout(8_000) { client.snapshot.first { it.mode == Mode.Monitor && it.metrics != null } }
    }

    @Test
    fun revokingTheDeviceOnTheHostForcesRepairing() = runBlocking<Unit> {
        val (client, storage) = newClient()
        val found = pairFully(client, storage)
        host.command("revoke ${storage.deviceId().toHex()}")

        val st = withTimeout(8_000) { client.state.first { it is ClientState.NeedsPairing } }
        assertIs<ClientState.NeedsPairing>(st)
        assertEquals(null, storage.host(found.hid)!!.token, "token lokal dihapus")
    }

    @Test
    fun revokedTokenIsDetectedOnNextLaunch() = runBlocking<Unit> {
        val (first, storage) = newClient()
        val found = pairFully(first, storage)
        first.disconnect()
        host.command("revoke ${storage.deviceId().toHex()}")
        delay(300)

        val (second, _) = newClient(storage)
        second.connect(found.hid)
        withTimeout(10_000) { second.state.first { it is ClientState.NeedsPairing } }
    }
}
