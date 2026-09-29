package app.tab.feature.deck

import androidx.compose.ui.graphics.Color
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class DeckLogicTest {
    @Test
    fun cellIndexIsRowMajor() {
        assertEquals(0, cellIndex(0, 0, 3))
        assertEquals(2, cellIndex(0, 2, 3))
        assertEquals(3, cellIndex(1, 0, 3))
        assertEquals(5, cellIndex(1, 2, 3))
    }

    @Test
    fun parsesSixAndEightDigitColors() {
        assertEquals(Color(0xFFFF8800), parseColor("#ff8800"))
        assertEquals(Color(0x80112233), parseColor("80112233"))
    }

    @Test
    fun rejectsBadColors() {
        assertNull(parseColor(null))
        assertNull(parseColor("#12345"))
        assertNull(parseColor("merah"))
        assertNull(parseColor(""))
    }

    @Test
    fun textIsReadableOnLightAndDarkBackgrounds() {
        assertEquals(Color.White, readableOn(Color(0xFF101010)))
        assertEquals(Color(0xFF111111), readableOn(Color(0xFFF5F5F5)))
    }

    @Test
    fun iconFallsBackToLabelInitial() {
        assertEquals("⏯", iconSymbol("play_pause", "Putar"))
        assertEquals("K", iconSymbol(null, "kerja"))
        assertEquals("•", iconSymbol("tidak-dikenal", ""))
    }
}
