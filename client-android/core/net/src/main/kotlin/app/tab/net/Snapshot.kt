package app.tab.net

import app.tab.model.Message
import app.tab.model.Mode

/** Hasil satu aksi deck, dengan nomor urut agar dua feedback identik tetap terbaca berbeda. */
data class DeckResult(val aid: String, val ok: Boolean, val msg: String?, val seq: Long)

/** Keadaan terkini yang dilaporkan host untuk sesi ini. Dibuang saat koneksi baru berdiri. */
data class Snapshot(
    val mode: Mode = Mode.Idle,
    val modeError: String? = null,
    val inputSettings: Message.InputSettings? = null,
    val deckProfiles: Message.DeckProfiles? = null,
    val deckProfile: Message.DeckProfile? = null,
    val deckResult: DeckResult? = null,
    val metrics: Message.Metrics? = null,
    val nowPlaying: Message.NowPlaying? = null,
    /** `elapsedRealtime` saat [nowPlaying] diterima; dasar interpolasi posisi di client. */
    val nowPlayingAtMs: Long = 0,
    val artwork: Map<String, ByteArray> = emptyMap(),
    val lyrics: Message.LyricsDoc? = null,
    val hostClipboard: String? = null,
    val lastError: Message.Error? = null,
)

/** Merakit `ArtworkChunk` menjadi gambar utuh; chunk yang datang tidak berurutan tetap benar. */
internal class ArtworkAssembler {
    private class Partial(val n: Int) {
        val parts = arrayOfNulls<ByteArray>(n)
        var got = 0
    }

    private val partial = HashMap<String, Partial>()

    /** Kembalikan gambar lengkap bila chunk ini yang terakhir. */
    fun add(c: Message.ArtworkChunk): ByteArray? {
        if (c.n <= 0 || c.n > MAX_CHUNKS || c.i !in 0 until c.n) return null
        val p = partial.getOrPut(c.id) { Partial(c.n) }
        if (p.n != c.n) {
            partial.remove(c.id)
            return null
        }
        if (p.parts[c.i] == null) {
            p.parts[c.i] = c.b
            p.got++
        }
        if (p.got < p.n) return null
        partial.remove(c.id)
        val total = p.parts.sumOf { it!!.size }
        val out = ByteArray(total)
        var off = 0
        for (part in p.parts) {
            part!!.copyInto(out, off)
            off += part.size
        }
        return out
    }

    fun clear() = partial.clear()

    companion object {
        const val MAX_CHUNKS = 1024
    }
}

internal fun Snapshot.reduce(msg: Message, nowMs: Long, art: ArtworkAssembler, deckSeq: Long): Snapshot = when (msg) {
    is Message.ModeState -> copy(mode = if (msg.ok) msg.m else mode, modeError = if (msg.ok) null else msg.msg)
    is Message.InputSettings -> copy(inputSettings = msg)
    is Message.DeckProfiles -> copy(deckProfiles = msg)
    is Message.DeckProfile -> copy(deckProfile = msg)
    is Message.DeckFeedback -> copy(deckResult = DeckResult(msg.aid, msg.ok, msg.msg, deckSeq))
    is Message.Metrics -> copy(metrics = msg)
    is Message.NowPlaying -> copy(nowPlaying = msg, nowPlayingAtMs = nowMs)
    is Message.ArtworkChunk -> {
        val whole = art.add(msg)
        if (whole == null) this else {
            // Simpan paling banyak 8 gambar; yang tertua dibuang.
            val next = LinkedHashMap(artwork)
            next[msg.id] = whole
            while (next.size > 8) next.remove(next.keys.first())
            copy(artwork = next)
        }
    }
    is Message.LyricsDoc -> copy(lyrics = msg)
    is Message.ClipboardUpdate -> copy(hostClipboard = msg.s)
    is Message.Error -> copy(lastError = msg)
    else -> this
}
