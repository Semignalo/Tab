package app.tab.feature.trackpad

import app.tab.model.Button
import app.tab.model.InputEvent
import app.tab.model.ScrollPhase
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.hypot

/** Satu jari di layar. Koordinat dalam satuan logis (dp), bukan piksel mentah. */
data class Pointer(val id: Long, val x: Float, val y: Float)

data class GestureConfig(
    /** Gerak kumulatif di bawah ini masih dianggap "diam" (tap), bukan geser. */
    val tapSlop: Float = 10f,
    val tapTimeoutMs: Long = 220,
    /** Jendela setelah tap agar sentuhan berikutnya menjadi drag / klik ganda. */
    val doubleTapWindowMs: Long = 300,
    val swipeThreshold: Float = 56f,
    /** Kecepatan (dp/ms) minimum saat jari diangkat agar inersia scroll dimulai. */
    val inertiaMinVelocity: Float = 0.25f,
    /** Konstanta waktu peluruhan inersia (ms). */
    val inertiaTauMs: Float = 320f,
    val velocityWindowMs: Long = 90,
)

/**
 * Klasifikasi gesture — **logika murni**, tanpa Android dan tanpa jaringan.
 *
 * Klasifikasi dilakukan di client karena di sinilah data sentuhan mentah berada; host hanya
 * menerima niat yang sudah jelas (klik, scroll, drag), sehingga scroll dua jari tidak pernah
 * salah terbaca sebagai klik.
 *
 * Aturan:
 *  - 1 jari: geser = pointer (ditahan sampai melewati slop supaya tap tidak menggeser kursor);
 *    tap = klik kiri; tap lalu langsung sentuh dan geser = drag.
 *  - 2 jari: geser = scroll (+ inersia); tap = klik kanan.
 *  - 3 jari: tap = klik tengah.
 *  - 4 jari: geser kiri/kanan/atas = gesture Spaces/Mission Control (bila didukung host).
 *  - Setelah gesture multi-jari, jari yang tersisa tidak menggerakkan kursor sampai semuanya
 *    diangkat — kalau tidak, kursor "melompat" di akhir scroll.
 */
class GestureEngine(
    private val emit: (InputEvent) -> Unit,
    private val supportedGestures: Set<String>,
    private val cfg: GestureConfig = GestureConfig(),
) {
    private var prevCount = 0
    private var prevX = 0f
    private var prevY = 0f

    private var sessionStart = 0L
    private var maxPointers = 0
    private var travel = 0f
    private var pendingX = 0f
    private var pendingY = 0f
    private var moveActive = false
    private var suppressSingle = false

    private var lastTapEnd = Long.MIN_VALUE / 2
    private var lastTapWasSingle = false
    private var dragCandidate = false
    private var dragging = false

    private var scrolling = false
    private var scrollPendingX = 0f
    private var scrollPendingY = 0f
    private val samples = ArrayDeque<Triple<Long, Float, Float>>()

    private var swipeX = 0f
    private var swipeY = 0f
    private var swipeFired = false

    private var momentumVx = 0f
    private var momentumVy = 0f
    private var momentumLast = 0L
    var momentumActive = false
        private set

    fun onPointers(timeMs: Long, pointers: List<Pointer>) {
        val n = pointers.size
        if (n > 0 && momentumActive) stopMomentum()

        if (n == 0) {
            if (prevCount > 0) allUp(timeMs)
            prevCount = 0
            return
        }

        val cx = pointers.sumOf { it.x.toDouble() }.toFloat() / n
        val cy = pointers.sumOf { it.y.toDouble() }.toFloat() / n

        if (prevCount == 0) {
            startSession(timeMs, n)
        } else if (n != prevCount) {
            countChanged(timeMs, prevCount, n)
        }
        maxPointers = maxOf(maxPointers, n)

        // Saat jumlah jari berubah, titik tengah melompat; jangan hitung itu sebagai gerakan.
        val fresh = prevCount == 0 || n != prevCount
        val dx = if (fresh) 0f else cx - prevX
        val dy = if (fresh) 0f else cy - prevY
        prevX = cx
        prevY = cy
        travel += hypot(dx, dy)

        when {
            n == 1 -> single(dx, dy)
            n == 2 -> scroll(timeMs, dx, dy)
            n >= 4 -> swipe(dx, dy)
        }
        prevCount = n
    }

    /** Panggil ~60 Hz; menggerakkan inersia scroll. */
    fun tick(timeMs: Long) {
        if (!momentumActive) return
        val dt = (timeMs - momentumLast).coerceAtLeast(0)
        if (dt == 0L) return
        momentumLast = timeMs
        val decay = exp(-dt / cfg.inertiaTauMs)
        momentumVx *= decay
        momentumVy *= decay
        if (hypot(momentumVx, momentumVy) < 0.02f) {
            emit(InputEvent.Scroll(0f, 0f, ScrollPhase.End, true))
            momentumActive = false
            return
        }
        emit(InputEvent.Scroll(momentumVx * dt, momentumVy * dt, ScrollPhase.Update, true))
    }

    /** Batalkan apa pun yang sedang berlangsung (mis. layar ditinggalkan). */
    fun reset() {
        if (dragging) emit(InputEvent.PointerButton(Button.L, false))
        if (scrolling) emit(InputEvent.Scroll(0f, 0f, ScrollPhase.End, false))
        if (momentumActive) stopMomentum()
        prevCount = 0
        dragging = false
        scrolling = false
    }

    // ---------------------------------------------------------------- internal

    private fun startSession(t: Long, n: Int) {
        sessionStart = t
        maxPointers = 0
        travel = 0f
        pendingX = 0f
        pendingY = 0f
        moveActive = false
        suppressSingle = false
        dragging = false
        scrolling = false
        scrollPendingX = 0f
        scrollPendingY = 0f
        samples.clear()
        swipeX = 0f
        swipeY = 0f
        swipeFired = false
        dragCandidate = n == 1 && lastTapWasSingle && t - lastTapEnd <= cfg.doubleTapWindowMs
    }

    private fun countChanged(t: Long, from: Int, to: Int) {
        if (from == 2 && scrolling) endScroll(t)
        if (to >= 2 || from >= 2) {
            // Gesture multi-jari sudah dipakai: jari tunggal yang tersisa diam sampai semua diangkat.
            suppressSingle = true
            pendingX = 0f
            pendingY = 0f
        }
        if (to == 2) {
            scrollPendingX = 0f
            scrollPendingY = 0f
            samples.clear()
        }
    }

    private fun single(dx: Float, dy: Float) {
        if (suppressSingle) return
        if (!moveActive) {
            pendingX += dx
            pendingY += dy
            if (hypot(pendingX, pendingY) < cfg.tapSlop) return
            moveActive = true
            if (dragCandidate) {
                emit(InputEvent.PointerButton(Button.L, true))
                dragging = true
            }
            emit(InputEvent.PointerMove(pendingX, pendingY))
            pendingX = 0f
            pendingY = 0f
        } else if (dx != 0f || dy != 0f) {
            emit(InputEvent.PointerMove(dx, dy))
        }
    }

    private fun scroll(t: Long, dx: Float, dy: Float) {
        if (!scrolling) {
            scrollPendingX += dx
            scrollPendingY += dy
            if (hypot(scrollPendingX, scrollPendingY) < SCROLL_SLOP) return
            scrolling = true
            emit(InputEvent.Scroll(0f, 0f, ScrollPhase.Begin, false))
            emit(InputEvent.Scroll(scrollPendingX, scrollPendingY, ScrollPhase.Update, false))
            record(t, scrollPendingX, scrollPendingY)
            scrollPendingX = 0f
            scrollPendingY = 0f
        } else if (dx != 0f || dy != 0f) {
            emit(InputEvent.Scroll(dx, dy, ScrollPhase.Update, false))
            record(t, dx, dy)
        }
    }

    private fun record(t: Long, dx: Float, dy: Float) {
        samples.addLast(Triple(t, dx, dy))
        while (samples.isNotEmpty() && t - samples.first().first > cfg.velocityWindowMs) samples.removeFirst()
    }

    private fun endScroll(t: Long) {
        scrolling = false
        emit(InputEvent.Scroll(0f, 0f, ScrollPhase.End, false))
        // Kecepatan dari sampel terakhir; jari yang berhenti dulu sebelum diangkat = tanpa inersia.
        if (samples.size >= 2 && t - samples.last().first <= cfg.velocityWindowMs) {
            val span = (samples.last().first - samples.first().first).coerceAtLeast(16)
            val vx = samples.sumOf { it.second.toDouble() }.toFloat() / span
            val vy = samples.sumOf { it.third.toDouble() }.toFloat() / span
            if (hypot(vx, vy) >= cfg.inertiaMinVelocity) {
                momentumVx = vx
                momentumVy = vy
                momentumLast = t
                momentumActive = true
            }
        }
        samples.clear()
    }

    private fun stopMomentum() {
        momentumActive = false
        emit(InputEvent.Scroll(0f, 0f, ScrollPhase.End, true))
    }

    private fun swipe(dx: Float, dy: Float) {
        if (swipeFired) return
        swipeX += dx
        swipeY += dy
        val gesture = when {
            abs(swipeX) >= cfg.swipeThreshold && abs(swipeX) > abs(swipeY) ->
                // Geser ke kiri = pindah ke Space di kanan (arah konten, seperti trackpad Mac).
                if (swipeX < 0) "spaces_right" else "spaces_left"
            swipeY <= -cfg.swipeThreshold && abs(swipeY) > abs(swipeX) -> "mission_control"
            else -> return
        }
        swipeFired = true
        if (gesture in supportedGestures) emit(InputEvent.Gesture(gesture, 4))
    }

    private fun allUp(t: Long) {
        if (scrolling) endScroll(t)
        if (dragging) {
            emit(InputEvent.PointerButton(Button.L, false))
            dragging = false
            lastTapWasSingle = false
            return
        }
        val isTap = travel <= cfg.tapSlop && t - sessionStart <= cfg.tapTimeoutMs && !swipeFired
        val button = when (maxPointers) {
            1 -> Button.L
            2 -> Button.R
            3 -> Button.M
            else -> null
        }
        if (isTap && button != null) {
            emit(InputEvent.PointerButton(button, true))
            emit(InputEvent.PointerButton(button, false))
            lastTapEnd = t
            lastTapWasSingle = maxPointers == 1
        } else {
            lastTapWasSingle = false
        }
    }

    companion object {
        private const val SCROLL_SLOP = 4f
    }
}
