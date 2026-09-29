package app.tab.feature.monitor

import kotlin.test.Test
import kotlin.test.assertEquals

class FormatTest {
    @Test
    fun bytesUseBinaryUnits() {
        assertEquals("0 B", fmtBytes(0))
        assertEquals("1023 B", fmtBytes(1023))
        assertEquals("1.0 KB", fmtBytes(1024))
        assertEquals("1.5 MB", fmtBytes(1_572_864))
        assertEquals("16.0 GB", fmtBytes(16L shl 30))
    }

    @Test
    fun ratesAppendPerSecond() {
        assertEquals("2.0 KB/s", fmtRate(2048))
    }

    @Test
    fun percentIsClampedAndSafeOnZeroTotal() {
        assertEquals(0f, percent(5, 0))
        assertEquals(50f, percent(1, 2))
        assertEquals(100f, percent(300, 200))
    }
}
