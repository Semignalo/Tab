package app.tab.feature.trackpad

import app.tab.model.Button
import app.tab.model.InputEvent
import app.tab.model.ScrollPhase
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class GestureEngineTest {
    private val out = ArrayList<InputEvent>()
    private fun engine(gestures: Set<String> = emptySet()) = GestureEngine({ out += it }, gestures)

    private fun p(id: Long, x: Float, y: Float) = Pointer(id, x, y)

    private fun GestureEngine.down(t: Long, vararg pts: Pointer) = onPointers(t, pts.toList())
    private fun GestureEngine.up(t: Long) = onPointers(t, emptyList())

    private fun clicks() = out.filterIsInstance<InputEvent.PointerButton>()

    @Test
    fun singleFingerTapIsLeftClick() {
        val e = engine()
        e.down(0, p(1, 100f, 100f))
        e.up(80)
        assertEquals(
            listOf<InputEvent>(InputEvent.PointerButton(Button.L, true), InputEvent.PointerButton(Button.L, false)),
            out,
        )
    }

    @Test
    fun tinyJitterDoesNotMoveThePointerNorCancelTheTap() {
        val e = engine()
        e.down(0, p(1, 100f, 100f))
        e.down(16, p(1, 102f, 101f))
        e.down(32, p(1, 101f, 103f))
        e.up(90)
        assertTrue(out.none { it is InputEvent.PointerMove }, "gerak di bawah slop ditahan")
        assertEquals(2, clicks().size)
    }

    @Test
    fun slowTouchIsNotATap() {
        val e = engine()
        e.down(0, p(1, 0f, 0f))
        e.up(500)
        assertTrue(out.isEmpty())
    }

    @Test
    fun dragPastSlopFlushesHeldBackMovementThenFollowsFinger() {
        val e = engine()
        e.down(0, p(1, 0f, 0f))
        e.down(16, p(1, 6f, 0f)) // ditahan
        assertTrue(out.isEmpty())
        e.down(32, p(1, 12f, 0f)) // melewati slop 10 → lepaskan total 12
        assertEquals(InputEvent.PointerMove(12f, 0f), out.single())
        e.down(48, p(1, 15f, 4f))
        assertEquals(InputEvent.PointerMove(3f, 4f), out.last())
        e.up(100)
        assertTrue(clicks().isEmpty(), "geser bukan klik")
    }

    @Test
    fun twoFingerTapIsRightClick() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 50f, 0f))
        e.up(100)
        assertEquals(listOf(Button.R, Button.R), clicks().map { it.b })
        assertEquals(listOf(true, false), clicks().map { it.d })
    }

    @Test
    fun threeFingerTapIsMiddleClick() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 20f, 0f), p(3, 40f, 0f))
        e.up(100)
        assertEquals(Button.M, clicks().first().b)
    }

    @Test
    fun twoFingerMoveScrollsWithBeginUpdateEndAndNeverClicks() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 40f, 0f))
        e.down(16, p(1, 0f, 10f), p(2, 40f, 10f))
        e.down(32, p(1, 0f, 20f), p(2, 40f, 20f))
        // berhenti dulu (tanpa inersia) lalu angkat
        e.up(400)

        val scrolls = out.filterIsInstance<InputEvent.Scroll>()
        assertEquals(ScrollPhase.Begin, scrolls.first().ph)
        assertEquals(ScrollPhase.End, scrolls.last().ph)
        assertEquals(false, scrolls.last().mom)
        assertEquals(20f, scrolls.filter { it.ph == ScrollPhase.Update }.sumOf { it.dy.toDouble() }.toFloat())
        assertTrue(clicks().isEmpty(), "scroll dua jari tidak boleh terbaca sebagai klik")
        assertTrue(!e.momentumActive)
    }

    @Test
    fun fastFlickStartsMomentumThatDecaysToAnEnd() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 40f, 0f))
        e.down(16, p(1, 0f, 12f), p(2, 40f, 12f))
        e.down(32, p(1, 0f, 24f), p(2, 40f, 24f))
        e.down(48, p(1, 0f, 36f), p(2, 40f, 36f))
        e.up(52)
        assertTrue(e.momentumActive, "kecepatan tinggi → inersia")

        var t = 52L
        val before = out.size
        while (e.momentumActive && t < 10_000) {
            t += 16
            e.tick(t)
        }
        assertTrue(!e.momentumActive, "inersia harus berakhir")
        val mom = out.drop(before).filterIsInstance<InputEvent.Scroll>()
        assertTrue(mom.all { it.mom }, "event inersia bertanda mom=true")
        assertEquals(ScrollPhase.End, mom.last().ph)
        val dys = mom.filter { it.ph == ScrollPhase.Update }.map { it.dy }
        assertTrue(dys.zipWithNext().all { (a, b) -> b <= a + 0.001f }, "laju harus menurun")
    }

    @Test
    fun touchingScreenStopsMomentum() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 40f, 0f))
        e.down(16, p(1, 0f, 15f), p(2, 40f, 15f))
        e.down(32, p(1, 0f, 30f), p(2, 40f, 30f))
        e.up(36)
        assertTrue(e.momentumActive)
        e.down(60, p(1, 5f, 5f))
        assertTrue(!e.momentumActive)
        val last = out.last { it is InputEvent.Scroll } as InputEvent.Scroll
        assertEquals(ScrollPhase.End, last.ph)
        assertTrue(last.mom)
    }

    @Test
    fun remainingFingerAfterScrollDoesNotJumpThePointer() {
        val e = engine()
        e.down(0, p(1, 0f, 0f), p(2, 40f, 0f))
        e.down(16, p(1, 0f, 30f), p(2, 40f, 30f))
        e.down(32, p(1, 0f, 30f)) // satu jari diangkat
        e.down(48, p(1, 60f, 90f)) // jari tersisa bergerak jauh
        e.up(60)
        assertTrue(out.none { it is InputEvent.PointerMove })
    }

    @Test
    fun tapThenQuickTouchAndDragPressesAndHoldsLeftButton() {
        val e = engine()
        e.down(0, p(1, 0f, 0f))
        e.up(60) // klik pertama
        out.clear()
        e.down(150, p(1, 0f, 0f)) // dalam jendela 300 ms
        e.down(170, p(1, 20f, 0f))
        e.down(190, p(1, 40f, 5f))
        e.up(240)

        assertEquals(InputEvent.PointerButton(Button.L, true), out.first())
        assertTrue(out.filterIsInstance<InputEvent.PointerMove>().isNotEmpty())
        assertEquals(InputEvent.PointerButton(Button.L, false), out.last())
        assertEquals(2, clicks().size, "tepat satu down + satu up")
    }

    @Test
    fun tapThenTapIsDoubleClick() {
        val e = engine()
        e.down(0, p(1, 0f, 0f)); e.up(50)
        e.down(150, p(1, 0f, 0f)); e.up(200)
        assertEquals(4, clicks().size)
    }

    @Test
    fun lateSecondTouchIsNotADrag() {
        val e = engine()
        e.down(0, p(1, 0f, 0f)); e.up(50)
        out.clear()
        e.down(600, p(1, 0f, 0f))
        e.down(620, p(1, 30f, 0f))
        e.up(700)
        assertTrue(clicks().isEmpty())
    }

    @Test
    fun fourFingerSwipeEmitsSpacesGestureOnlyWhenHostSupportsIt() {
        val supported = engine(setOf("spaces_left", "spaces_right", "mission_control"))
        supported.down(0, p(1, 200f, 0f), p(2, 220f, 0f), p(3, 240f, 0f), p(4, 260f, 0f))
        supported.down(30, p(1, 100f, 0f), p(2, 120f, 0f), p(3, 140f, 0f), p(4, 160f, 0f))
        supported.up(80)
        assertEquals(InputEvent.Gesture("spaces_right", 4), out.single())

        out.clear()
        val unsupported = engine(emptySet())
        unsupported.down(0, p(1, 200f, 0f), p(2, 220f, 0f), p(3, 240f, 0f), p(4, 260f, 0f))
        unsupported.down(30, p(1, 100f, 0f), p(2, 120f, 0f), p(3, 140f, 0f), p(4, 160f, 0f))
        unsupported.up(80)
        assertTrue(out.isEmpty(), "kapabilitas false → tidak ada yang dikirim")
    }

    @Test
    fun fourFingerSwipeUpIsMissionControlAndFiresOnce() {
        val e = engine(setOf("mission_control"))
        e.down(0, p(1, 0f, 200f), p(2, 20f, 200f), p(3, 40f, 200f), p(4, 60f, 200f))
        e.down(30, p(1, 0f, 100f), p(2, 20f, 100f), p(3, 40f, 100f), p(4, 60f, 100f))
        e.down(60, p(1, 0f, 0f), p(2, 20f, 0f), p(3, 40f, 0f), p(4, 60f, 0f))
        e.up(100)
        assertEquals(listOf<InputEvent>(InputEvent.Gesture("mission_control", 4)), out)
    }

    @Test
    fun resetReleasesHeldDragAndEndsScroll() {
        val e = engine()
        e.down(0, p(1, 0f, 0f)); e.up(50)
        e.down(120, p(1, 0f, 0f))
        e.down(140, p(1, 30f, 0f))
        out.clear()
        e.reset()
        assertEquals(InputEvent.PointerButton(Button.L, false), out.single())
    }
}
