package app.tab.model

/** Hasil membaca satu pesan. */
sealed interface Decoded {
    data class Known(val message: Message) : Decoded

    /** Tipe dari versi protokol lebih baru: **diabaikan**, bukan error. */
    data class Unknown(val tag: String) : Decoded
}

object Codec {
    fun encode(message: Message): ByteArray = Cbor.encode(message.toCbor())

    /**
     * @throws CborException bila bukan CBOR sah
     * @throws ProtocolException bila tag dikenal tetapi isinya tidak sesuai kontrak
     */
    fun decode(bytes: ByteArray): Decoded {
        val root = Cbor.decode(bytes)
        val f = Fields.asFields(root, "pesan")
        val tag = f.str("t")
        return parse(tag, f)?.let { Decoded.Known(it) } ?: Decoded.Unknown(tag)
    }

    private fun parse(tag: String, f: Fields): Message? = when (tag) {
        "Hello" -> Message.Hello(
            f.int("pv"), f.id("dev"), f.str("name"), f.str("plat"), f.str("platv"), f.str("app"),
            f.fields("scr").let { Screen(it.long("w"), it.long("h"), it.f32("dpi")) },
        )
        "Welcome" -> Message.Welcome(
            f.int("pv"), f.id("hid"), f.str("name"), f.str("os"), f.str("osv"), f.str("app"),
            f.id("sid"), capabilities(f.fields("caps")),
        )
        "PairRequired" -> Message.PairRequired(f.long("ttl_ms"))
        "PairRequest" -> Message.PairRequest(f.str("pin"))
        "PairOk" -> Message.PairOk(f.bytes("tok"))
        "Ping" -> Message.Ping(f.long("n"), f.long("tc"))
        "Pong" -> Message.Pong(f.long("n"), f.long("tc"), f.long("th"))
        "SetMode" -> Message.SetMode(Mode.parse(f.str("m")))
        "ModeState" -> Message.ModeState(Mode.parse(f.str("m")), f.bool("ok"), f.optStr("msg"))
        "Bye" -> Message.Bye(f.str("r"))
        "Error" -> Message.Error(ErrorCode.parse(f.str("c")), f.str("msg"), f.optLong("attempts_left")?.toInt())
        "InputBatch" -> Message.InputBatch(f.long("s"), f.list("ev").map { InputEvent.fromCbor(it) })
        "InputSettings" -> Message.InputSettings(f.f32("sens"), f.bool("natural"))
        "SetInputSettings" -> Message.SetInputSettings(f.optF32("sens"), f.optBool("natural"))
        "ClipboardPush" -> Message.ClipboardPush(f.str("s"))
        "ClipboardUpdate" -> Message.ClipboardUpdate(f.str("s"))
        "DeckProfiles" -> Message.DeckProfiles(
            f.listOf("list") { ProfileRef(it.str("id"), it.str("name")) }, f.str("active"),
        )
        "DeckProfile" -> Message.DeckProfile(
            f.str("id"), f.str("name"), f.int("cols"), f.int("rows"),
            f.listOf("btns") {
                DeckButton(it.int("i"), it.str("lbl"), it.optStr("ic"), it.optStr("col"), it.str("aid"))
            },
        )
        "SelectProfile" -> Message.SelectProfile(f.str("id"))
        "DeckPress" -> Message.DeckPress(f.str("aid"), f.int("i"))
        "DeckRelease" -> Message.DeckRelease(f.str("aid"), f.int("i"))
        "DeckFeedback" -> Message.DeckFeedback(f.str("aid"), f.bool("ok"), f.optStr("msg"))
        "SubscribeTelemetry" -> Message.SubscribeTelemetry(
            f.strings("kinds").map { TelemetryKind.parse(it) }, f.long("iv"),
        )
        "Metrics" -> Message.Metrics(
            cpu = f.floats("cpu"), cpua = f.f32("cpua"), mu = f.long("mu"), mt = f.long("mt"),
            su = f.long("su"), st = f.long("st"),
            disks = f.listOf("disks") { Disk(it.str("n"), it.long("u"), it.long("t")) },
            rx = f.long("rx"), tx = f.long("tx"),
            pwr = f.fields("pwr").let { Power(it.bool("ac"), it.optLong("pct")?.toInt()) },
            tcpu = f.optF32("tcpu"),
            gpu = if (f.optValue("gpu") != null) f.fields("gpu").let {
                Gpu(it.optF32("u"), it.optF32("t"), it.optLong("mu"), it.optLong("mt"))
            } else null,
        )
        "NowPlaying" -> Message.NowPlaying(
            f.str("title"), f.str("artist"), f.str("album"), f.long("dur"), f.long("pos"),
            f.bool("play"), f.optStr("art"), f.bool("lyr"),
        )
        "MediaCommand" -> Message.MediaCommand(MediaCmd.parse(f.str("c")))
        "MediaSeek" -> Message.MediaSeek(f.long("ms"))
        "MediaVolume" -> Message.MediaVolume(f.f32("v"))
        "GetArtwork" -> Message.GetArtwork(f.str("id"))
        "ArtworkChunk" -> Message.ArtworkChunk(f.str("id"), f.int("i"), f.int("n"), f.bytes("b"))
        "GetLyrics" -> Message.GetLyrics(f.str("id"))
        "LyricsDoc" -> Message.LyricsDoc(
            f.str("id"), f.listOf("lines") { LyricLine(it.long("t"), it.str("s")) },
        )
        else -> null
    }

    private fun capabilities(c: Fields) = Capabilities(
        modes = c.strings("modes").map { Mode.parse(it) },
        clipboard = c.bool("clipboard"),
        gestures = c.strings("gestures"),
        nowPlaying = c.bool("now_playing"),
        mediaSeek = c.bool("media_seek"),
        mediaVolume = c.bool("media_volume"),
        obs = c.bool("obs"),
        temps = c.bool("temps"),
        gpu = c.bool("gpu"),
        secondScreen = c.bool("second_screen"),
        gamepad = c.bool("gamepad"),
        maxFrame = c.long("max_frame"),
    )
}
