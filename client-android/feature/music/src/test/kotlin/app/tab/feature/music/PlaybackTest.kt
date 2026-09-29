package app.tab.feature.music

import app.tab.model.LyricLine
import app.tab.model.Message
import kotlin.test.Test
import kotlin.test.assertEquals

class PlaybackTest {
    private fun np(pos: Long, dur: Long = 200_000, play: Boolean = true) =
        Message.NowPlaying("t", "a", "b", dur, pos, play, null, false)

    @Test
    fun positionAdvancesWhilePlaying() {
        assertEquals(10_000, positionMs(np(10_000), receivedAtMs = 1_000, nowMs = 1_000))
        assertEquals(10_750, positionMs(np(10_000), receivedAtMs = 1_000, nowMs = 1_750))
    }

    @Test
    fun positionHoldsWhilePaused() {
        assertEquals(10_000, positionMs(np(10_000, play = false), 1_000, 9_000))
    }

    @Test
    fun positionNeverExceedsDurationOrGoesBackwards() {
        assertEquals(200_000, positionMs(np(199_500), 0, 5_000))
        assertEquals(10_000, positionMs(np(10_000), receivedAtMs = 5_000, nowMs = 4_000), "jam mundur tidak memundurkan posisi")
        assertEquals(50_000, positionMs(np(50_000, dur = 0), 0, 0), "durasi tidak diketahui: tanpa batas atas")
    }

    private val lines = listOf(LyricLine(1_000, "a"), LyricLine(5_000, "b"), LyricLine(9_000, "c"))

    @Test
    fun currentLineFollowsPosition() {
        assertEquals(-1, currentLineIndex(lines, 0))
        assertEquals(-1, currentLineIndex(lines, 999))
        assertEquals(0, currentLineIndex(lines, 1_000))
        assertEquals(0, currentLineIndex(lines, 4_999))
        assertEquals(1, currentLineIndex(lines, 5_000))
        assertEquals(2, currentLineIndex(lines, 999_999))
    }

    @Test
    fun currentLineHandlesEmptyLyrics() {
        assertEquals(-1, currentLineIndex(emptyList(), 5_000))
    }

    @Test
    fun timeFormatting() {
        assertEquals("0:00", formatTime(0))
        assertEquals("0:07", formatTime(7_400))
        assertEquals("3:05", formatTime(185_000))
        assertEquals("1:02:03", formatTime(3_723_000))
        assertEquals("0:00", formatTime(-5))
    }
}
