package app.tab.model

/**
 * Katalog pesan sesi. Wire format dan urutan field mengikuti `protocol/PROTOCOL.md` serta
 * `crates/tab-protocol/src/message.rs`; kebenarannya diuji terhadap berkas .cbor di protocol/fixtures.
 */
sealed interface Message {
    val tag: String
    fun toCbor(): CborValue

    // ============================== control ==============================

    data class Hello(
        val pv: Int, val dev: Id16, val name: String, val plat: String,
        val platv: String, val app: String, val scr: Screen,
    ) : Message {
        override val tag get() = "Hello"
        override fun toCbor() = tagged(tag) {
            uint("pv", pv); id("dev", dev); text("name", name); text("plat", plat)
            text("platv", platv); text("app", app); put("scr", scr.toCbor())
        }
    }

    data class Welcome(
        val pv: Int, val hid: Id16, val name: String, val os: String, val osv: String,
        val app: String, val sid: Id16, val caps: Capabilities,
    ) : Message {
        override val tag get() = "Welcome"
        override fun toCbor() = tagged(tag) {
            uint("pv", pv); id("hid", hid); text("name", name); text("os", os); text("osv", osv)
            text("app", app); id("sid", sid); put("caps", caps.toCbor())
        }
    }

    data class PairRequired(val ttlMs: Long) : Message {
        override val tag get() = "PairRequired"
        override fun toCbor() = tagged(tag) { uint("ttl_ms", ttlMs) }
    }

    data class PairRequest(val pin: String) : Message {
        override val tag get() = "PairRequest"
        override fun toCbor() = tagged(tag) { text("pin", pin) }
    }

    class PairOk(val tok: ByteArray) : Message {
        override val tag get() = "PairOk"
        override fun toCbor() = tagged(tag) { bytes("tok", tok) }
        override fun equals(other: Any?) = other is PairOk && tok.contentEquals(other.tok)
        override fun hashCode() = tok.contentHashCode()
        // Token tidak pernah ikut ke log.
        override fun toString() = "PairOk(tok=***)"
    }

    data class Ping(val n: Long, val tc: Long) : Message {
        override val tag get() = "Ping"
        override fun toCbor() = tagged(tag) { uint("n", n); uint("tc", tc) }
    }

    data class Pong(val n: Long, val tc: Long, val th: Long) : Message {
        override val tag get() = "Pong"
        override fun toCbor() = tagged(tag) { uint("n", n); uint("tc", tc); uint("th", th) }
    }

    data class SetMode(val m: Mode) : Message {
        override val tag get() = "SetMode"
        override fun toCbor() = tagged(tag) { text("m", m.wire) }
    }

    data class ModeState(val m: Mode, val ok: Boolean, val msg: String? = null) : Message {
        override val tag get() = "ModeState"
        override fun toCbor() = tagged(tag) { text("m", m.wire); bool("ok", ok); optText("msg", msg) }
    }

    data class Bye(val r: String) : Message {
        override val tag get() = "Bye"
        override fun toCbor() = tagged(tag) { text("r", r) }
    }

    data class Error(val c: ErrorCode, val msg: String, val attemptsLeft: Int? = null) : Message {
        override val tag get() = "Error"
        override fun toCbor() = tagged(tag) {
            text("c", c.name); text("msg", msg); optUint("attempts_left", attemptsLeft?.toLong())
        }
    }

    // ========================== trackpad & input ==========================

    data class InputBatch(val s: Long, val ev: List<InputEvent>) : Message {
        override val tag get() = "InputBatch"
        override fun toCbor() = tagged(tag) {
            uint("s", s); put("ev", CborValue.Arr(ev.map { it.toCbor() }))
        }
    }

    data class InputSettings(val sens: Float, val natural: Boolean) : Message {
        override val tag get() = "InputSettings"
        override fun toCbor() = tagged(tag) { f32("sens", sens); bool("natural", natural) }
    }

    data class SetInputSettings(val sens: Float? = null, val natural: Boolean? = null) : Message {
        override val tag get() = "SetInputSettings"
        override fun toCbor() = tagged(tag) { optF32("sens", sens); optBool("natural", natural) }
    }

    data class ClipboardPush(val s: String) : Message {
        override val tag get() = "ClipboardPush"
        override fun toCbor() = tagged(tag) { text("s", s) }
        // Isi clipboard tidak pernah ikut ke log.
        override fun toString() = "ClipboardPush(${s.length} karakter)"
    }

    data class ClipboardUpdate(val s: String) : Message {
        override val tag get() = "ClipboardUpdate"
        override fun toCbor() = tagged(tag) { text("s", s) }
        override fun toString() = "ClipboardUpdate(${s.length} karakter)"
    }

    // ================================ deck ================================

    data class DeckProfiles(val list: List<ProfileRef>, val active: String) : Message {
        override val tag get() = "DeckProfiles"
        override fun toCbor() = tagged(tag) {
            put("list", CborValue.Arr(list.map { it.toCbor() })); text("active", active)
        }
    }

    data class DeckProfile(
        val id: String, val name: String, val cols: Int, val rows: Int, val btns: List<DeckButton>,
    ) : Message {
        override val tag get() = "DeckProfile"
        override fun toCbor() = tagged(tag) {
            text("id", id); text("name", name); uint("cols", cols); uint("rows", rows)
            put("btns", CborValue.Arr(btns.map { it.toCbor() }))
        }
    }

    data class SelectProfile(val id: String) : Message {
        override val tag get() = "SelectProfile"
        override fun toCbor() = tagged(tag) { text("id", id) }
    }

    data class DeckPress(val aid: String, val i: Int) : Message {
        override val tag get() = "DeckPress"
        override fun toCbor() = tagged(tag) { text("aid", aid); uint("i", i) }
    }

    data class DeckRelease(val aid: String, val i: Int) : Message {
        override val tag get() = "DeckRelease"
        override fun toCbor() = tagged(tag) { text("aid", aid); uint("i", i) }
    }

    data class DeckFeedback(val aid: String, val ok: Boolean, val msg: String? = null) : Message {
        override val tag get() = "DeckFeedback"
        override fun toCbor() = tagged(tag) { text("aid", aid); bool("ok", ok); optText("msg", msg) }
    }

    // ============================== telemetry ==============================

    data class SubscribeTelemetry(val kinds: List<TelemetryKind>, val iv: Long) : Message {
        override val tag get() = "SubscribeTelemetry"
        override fun toCbor() = tagged(tag) {
            put("kinds", CborValue.Arr(kinds.map { CborValue.Text(it.wire) })); uint("iv", iv)
        }
    }

    data class Metrics(
        val cpu: List<Float>, val cpua: Float, val mu: Long, val mt: Long, val su: Long,
        val st: Long, val disks: List<Disk>, val rx: Long, val tx: Long, val pwr: Power,
        val tcpu: Float? = null, val gpu: Gpu? = null,
    ) : Message {
        override val tag get() = "Metrics"
        override fun toCbor() = tagged(tag) {
            put("cpu", CborValue.Arr(cpu.map { CborValue.Flt(it.toDouble()) }))
            f32("cpua", cpua); uint("mu", mu); uint("mt", mt); uint("su", su); uint("st", st)
            put("disks", CborValue.Arr(disks.map { it.toCbor() }))
            uint("rx", rx); uint("tx", tx); put("pwr", pwr.toCbor())
            optF32("tcpu", tcpu)
            if (gpu != null) put("gpu", gpu.toCbor())
        }
    }

    // ================================ music ================================

    data class NowPlaying(
        val title: String, val artist: String, val album: String, val dur: Long, val pos: Long,
        val play: Boolean, val art: String? = null, val lyr: Boolean,
    ) : Message {
        override val tag get() = "NowPlaying"
        override fun toCbor() = tagged(tag) {
            text("title", title); text("artist", artist); text("album", album)
            uint("dur", dur); uint("pos", pos); bool("play", play); optText("art", art)
            bool("lyr", lyr)
        }
    }

    data class MediaCommand(val c: MediaCmd) : Message {
        override val tag get() = "MediaCommand"
        override fun toCbor() = tagged(tag) { text("c", c.wire) }
    }

    data class MediaSeek(val ms: Long) : Message {
        override val tag get() = "MediaSeek"
        override fun toCbor() = tagged(tag) { uint("ms", ms) }
    }

    data class MediaVolume(val v: Float) : Message {
        override val tag get() = "MediaVolume"
        override fun toCbor() = tagged(tag) { f32("v", v) }
    }

    data class GetArtwork(val id: String) : Message {
        override val tag get() = "GetArtwork"
        override fun toCbor() = tagged(tag) { text("id", id) }
    }

    class ArtworkChunk(val id: String, val i: Int, val n: Int, val b: ByteArray) : Message {
        override val tag get() = "ArtworkChunk"
        override fun toCbor() = tagged(tag) { text("id", id); uint("i", i); uint("n", n); bytes("b", b) }
        override fun equals(other: Any?) =
            other is ArtworkChunk && id == other.id && i == other.i && n == other.n && b.contentEquals(other.b)
        override fun hashCode() = 31 * (31 * id.hashCode() + i) + b.contentHashCode()
        override fun toString() = "ArtworkChunk($id, $i/$n, ${b.size} B)"
    }

    data class GetLyrics(val id: String) : Message {
        override val tag get() = "GetLyrics"
        override fun toCbor() = tagged(tag) { text("id", id) }
    }

    data class LyricsDoc(val id: String, val lines: List<LyricLine>) : Message {
        override val tag get() = "LyricsDoc"
        override fun toCbor() = tagged(tag) {
            text("id", id); put("lines", CborValue.Arr(lines.map { it.toCbor() }))
        }
    }
}

// ============================ tipe pendukung ============================

data class Screen(val w: Long, val h: Long, val dpi: Float) {
    fun toCbor() = map { uint("w", w); uint("h", h); f32("dpi", dpi) }
}

data class Capabilities(
    val modes: List<Mode>, val clipboard: Boolean, val gestures: List<String>,
    val nowPlaying: Boolean, val mediaSeek: Boolean, val mediaVolume: Boolean, val obs: Boolean,
    val temps: Boolean, val gpu: Boolean, val secondScreen: Boolean, val gamepad: Boolean,
    val maxFrame: Long,
) {
    fun toCbor() = map {
        put("modes", CborValue.Arr(modes.map { CborValue.Text(it.wire) }))
        bool("clipboard", clipboard)
        put("gestures", CborValue.Arr(gestures.map { CborValue.Text(it) }))
        bool("now_playing", nowPlaying); bool("media_seek", mediaSeek)
        bool("media_volume", mediaVolume); bool("obs", obs); bool("temps", temps)
        bool("gpu", gpu); bool("second_screen", secondScreen); bool("gamepad", gamepad)
        uint("max_frame", maxFrame)
    }
}

enum class Mode(val wire: String) {
    Idle("idle"), Trackpad("trackpad"), Deck("deck"), Monitor("monitor"), Music("music"),
    Clock("clock");

    companion object {
        fun parse(s: String): Mode = entries.firstOrNull { it.wire == s }
            ?: throw ProtocolException("mode tidak dikenal: $s")
    }
}

enum class ErrorCode {
    ProtocolUnsupported, PairRequired, PinInvalid, PinExpired, PairingBusy, TokenRevoked,
    RateLimited, Unsupported, Internal;

    companion object {
        fun parse(s: String): ErrorCode = entries.firstOrNull { it.name == s }
            ?: throw ProtocolException("kode error tidak dikenal: $s")
    }
}

enum class TelemetryKind(val wire: String) {
    Metrics("metrics"), NowPlaying("nowplaying");

    companion object {
        fun parse(s: String): TelemetryKind = entries.firstOrNull { it.wire == s }
            ?: throw ProtocolException("jenis telemetri tidak dikenal: $s")
    }
}

enum class MediaCmd(val wire: String) {
    Play("play"), Pause("pause"), Toggle("toggle"), Next("next"), Prev("prev");

    companion object {
        fun parse(s: String): MediaCmd = entries.firstOrNull { it.wire == s }
            ?: throw ProtocolException("perintah media tidak dikenal: $s")
    }
}

enum class Button(val wire: String) {
    L("l"), R("r"), M("m");

    companion object {
        fun parse(s: String): Button = entries.firstOrNull { it.wire == s }
            ?: throw ProtocolException("tombol mouse tidak dikenal: $s")
    }
}

enum class ScrollPhase(val wire: String) {
    Begin("b"), Update("u"), End("e");

    companion object {
        fun parse(s: String): ScrollPhase = entries.firstOrNull { it.wire == s }
            ?: throw ProtocolException("fase scroll tidak dikenal: $s")
    }
}

object ModifierBits {
    const val SHIFT = 1
    const val CTRL = 2
    const val ALT = 4
    const val META = 8
}

sealed interface InputEvent {
    fun toCbor(): CborValue

    data class PointerMove(val dx: Float, val dy: Float) : InputEvent {
        override fun toCbor() = tagged("PointerMove") { f32("dx", dx); f32("dy", dy) }
    }

    data class PointerAbs(val x: Float, val y: Float) : InputEvent {
        override fun toCbor() = tagged("PointerAbs") { f32("x", x); f32("y", y) }
    }

    data class PointerButton(val b: Button, val d: Boolean) : InputEvent {
        override fun toCbor() = tagged("PointerButton") { text("b", b.wire); bool("d", d) }
    }

    data class Scroll(val dx: Float, val dy: Float, val ph: ScrollPhase, val mom: Boolean) : InputEvent {
        override fun toCbor() = tagged("Scroll") {
            f32("dx", dx); f32("dy", dy); text("ph", ph.wire); bool("mom", mom)
        }
    }

    data class Gesture(val g: String, val f: Int) : InputEvent {
        override fun toCbor() = tagged("Gesture") { text("g", g); uint("f", f) }
    }

    data class Key(val k: String, val d: Boolean) : InputEvent {
        override fun toCbor() = tagged("Key") { text("k", k); bool("d", d) }
    }

    data class Text(val s: String) : InputEvent {
        override fun toCbor() = tagged("Text") { text("s", s) }
        override fun toString() = "Text(${s.length} karakter)"
    }

    data class Modifiers(val m: Int) : InputEvent {
        override fun toCbor() = tagged("Modifiers") { uint("m", m) }
    }

    companion object {
        fun fromCbor(v: CborValue): InputEvent {
            val f = Fields.asFields(v, "InputEvent")
            return when (val t = f.str("t")) {
                "PointerMove" -> PointerMove(f.f32("dx"), f.f32("dy"))
                "PointerAbs" -> PointerAbs(f.f32("x"), f.f32("y"))
                "PointerButton" -> PointerButton(Button.parse(f.str("b")), f.bool("d"))
                "Scroll" -> Scroll(f.f32("dx"), f.f32("dy"), ScrollPhase.parse(f.str("ph")), f.bool("mom"))
                "Gesture" -> Gesture(f.str("g"), f.int("f"))
                "Key" -> Key(f.str("k"), f.bool("d"))
                "Text" -> Text(f.str("s"))
                "Modifiers" -> Modifiers(f.int("m"))
                else -> throw ProtocolException("InputEvent tidak dikenal: $t")
            }
        }
    }
}

data class ProfileRef(val id: String, val name: String) {
    fun toCbor() = map { text("id", id); text("name", name) }
}

data class DeckButton(
    val i: Int, val lbl: String, val ic: String? = null, val col: String? = null, val aid: String,
) {
    fun toCbor() = map {
        uint("i", i); text("lbl", lbl); optText("ic", ic); optText("col", col); text("aid", aid)
    }
}

data class Disk(val n: String, val u: Long, val t: Long) {
    fun toCbor() = map { text("n", n); uint("u", u); uint("t", t) }
}

data class Power(val ac: Boolean, val pct: Int? = null) {
    fun toCbor() = map { bool("ac", ac); optUint("pct", pct?.toLong()) }
}

data class Gpu(val u: Float? = null, val t: Float? = null, val mu: Long? = null, val mt: Long? = null) {
    fun toCbor() = map { optF32("u", u); optF32("t", t); optUint("mu", mu); optUint("mt", mt) }
}

data class LyricLine(val t: Long, val s: String) {
    fun toCbor() = map { uint("t", t); text("s", s) }
}
