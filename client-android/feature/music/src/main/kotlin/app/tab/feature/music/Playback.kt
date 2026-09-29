package app.tab.feature.music

import app.tab.model.LyricLine
import app.tab.model.Message
import java.util.Locale

/**
 * Posisi lagu saat ini, diinterpolasi di client.
 *
 * Host hanya mengirim `pos` paling sering 1 Hz; timeline dan baris lirik yang halus datang dari
 * sini, tanpa menambah trafik. Bila sedang jeda, posisi tidak bergerak.
 */
fun positionMs(np: Message.NowPlaying, receivedAtMs: Long, nowMs: Long): Long {
    val advanced = if (np.play) np.pos + (nowMs - receivedAtMs).coerceAtLeast(0) else np.pos
    return if (np.dur > 0) advanced.coerceAtMost(np.dur) else advanced
}

/** Indeks baris yang sedang dinyanyikan (baris terakhir dengan `t <= posMs`), atau -1 sebelum baris pertama. */
fun currentLineIndex(lines: List<LyricLine>, posMs: Long): Int {
    var lo = 0
    var hi = lines.size - 1
    var ans = -1
    while (lo <= hi) {
        val mid = (lo + hi) ushr 1
        if (lines[mid].t <= posMs) {
            ans = mid
            lo = mid + 1
        } else {
            hi = mid - 1
        }
    }
    return ans
}

/** `m:ss`, atau `h:mm:ss` untuk ≥ 1 jam. */
fun formatTime(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    val h = total / 3600
    val m = (total % 3600) / 60
    val s = total % 60
    return if (h > 0) String.format(Locale.US, "%d:%02d:%02d", h, m, s) else String.format(Locale.US, "%d:%02d", m, s)
}
