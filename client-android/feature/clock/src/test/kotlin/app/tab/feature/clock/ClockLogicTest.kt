package app.tab.feature.clock

import java.time.LocalDate
import java.time.LocalTime
import java.util.Locale
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ClockLogicTest {
    @Test
    fun digitsAreZeroPadded24h() {
        assertEquals(listOf('0', '9', '0', '5'), clockDigits(LocalTime.of(9, 5)))
        assertEquals(listOf('2', '3', '5', '9'), clockDigits(LocalTime.of(23, 59)))
        assertEquals(listOf('0', '0', '0', '0'), clockDigits(LocalTime.MIDNIGHT))
    }

    @Test
    fun dateLineUsesGivenLocale() {
        val d = LocalDate.of(2026, 9, 29)
        assertEquals("Tuesday, 29 September 2026", dateLine(d, Locale.US))
        assertTrue(dateLine(d, Locale.forLanguageTag("id-ID")).contains("Selasa"))
    }

    @Test
    fun nextMinuteDelayIsWithinOneMinute() {
        assertEquals(60_000, millisToNextMinute(LocalTime.of(10, 0, 0)))
        assertEquals(1_000, millisToNextMinute(LocalTime.of(10, 0, 59)))
        assertEquals(500, millisToNextMinute(LocalTime.of(10, 0, 59, 500_000_000)))
    }

    @Test
    fun dimsOnlyAfterInactivity() {
        val p = DimPolicy(30_000)
        p.touched(1_000)
        assertFalse(p.isDim(20_000))
        assertTrue(p.isDim(31_000))
        p.touched(31_000)
        assertFalse(p.isDim(40_000))
    }
}
