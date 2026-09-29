package app.tab.feature.clock

import android.app.Activity
import android.content.pm.ActivityInfo
import android.os.SystemClock
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.shape.GenericShape
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import java.time.LocalTime
import java.util.Locale

/**
 * Jam meja: murni di client, tanpa jaringan sama sekali.
 *
 * Saat halaman ini aktif: layar dijaga menyala, orientasi dikunci landscape, dan kecerahan
 * diredupkan otomatis bila tidak disentuh. Semuanya dikembalikan saat halaman ditinggalkan.
 */
@Composable
fun ClockScreen(active: Boolean, modifier: Modifier = Modifier, dimAfterMs: Long = 30_000) {
    val context = LocalContext.current
    val view = LocalView.current
    val activity = context as? Activity
    val dim = remember { DimPolicy(dimAfterMs).also { it.touched(SystemClock.uptimeMillis()) } }
    var dimmed by remember { mutableStateOf(false) }

    DisposableEffect(active) {
        val window = activity?.window
        if (active) {
            view.keepScreenOn = true
            activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
        onDispose {
            view.keepScreenOn = false
            activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            window?.attributes = window?.attributes?.apply { screenBrightness = -1f } // ikut sistem
        }
    }

    // Peredupan: cek tiap detik; kecerahan jendela ikut turun.
    LaunchedEffect(active) {
        while (active) {
            val d = dim.isDim(SystemClock.uptimeMillis())
            if (d != dimmed) {
                dimmed = d
                activity?.window?.let { w ->
                    w.attributes = w.attributes.apply { screenBrightness = if (d) 0.03f else -1f }
                }
            }
            kotlinx.coroutines.delay(1000)
        }
    }

    var time by remember { mutableStateOf(LocalTime.now()) }
    LaunchedEffect(active) {
        while (active) {
            time = LocalTime.now()
            // Bangun tepat di pergantian menit, bukan polling cepat.
            kotlinx.coroutines.delay(millisToNextMinute(time).coerceIn(50, 60_000))
        }
    }

    Box(
        modifier
            .fillMaxSize()
            .background(Color.Black)
            .pointerInput(Unit) {
                detectTapGestures(onPress = {
                    dim.touched(SystemClock.uptimeMillis())
                    if (dimmed) {
                        dimmed = false
                        activity?.window?.let { w -> w.attributes = w.attributes.apply { screenBrightness = -1f } }
                    }
                })
            },
        contentAlignment = Alignment.Center,
    ) {
        val digits = clockDigits(time)
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(20.dp), modifier = Modifier.alpha(if (dimmed) 0.35f else 1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                FlipCard(digits[0]); FlipCard(digits[1])
                Text(":", color = Color(0xFF6E6E6E), fontSize = 72.sp, fontWeight = FontWeight.Bold)
                FlipCard(digits[2]); FlipCard(digits[3])
            }
            Text(
                dateLine(LocalDate.now(), Locale.getDefault()),
                color = Color(0xFF9A9A9A),
                style = MaterialTheme.typography.titleLarge,
            )
        }
    }
}

private fun halfShape(top: Boolean) = GenericShape { size, _ ->
    addRect(
        if (top) Rect(0f, 0f, size.width, size.height / 2)
        else Rect(0f, size.height / 2, size.width, size.height),
    )
}

/** Satu kartu digit dengan animasi balik (flip) saat nilainya berubah. */
@Composable
private fun FlipCard(value: Char) {
    var old by remember { mutableStateOf(value) }
    var current by remember { mutableStateOf(value) }
    val progress = remember { Animatable(1f) }

    LaunchedEffect(value) {
        if (value != current) {
            old = current
            current = value
            progress.snapTo(0f)
            progress.animateTo(1f, tween(560))
        }
    }

    val p = progress.value
    val density = LocalDensity.current.density
    Box(Modifier.width(120.dp).height(170.dp)) {
        // Lapisan statis: paruh atas = angka baru, paruh bawah = angka lama sampai flip selesai.
        Half(current, top = true)
        Half(if (p < 1f) old else current, top = false)
        // Paruh atas angka lama jatuh ke bawah, lalu paruh bawah angka baru terbuka.
        if (p < 0.5f) {
            Half(old, top = true, rotation = -180f * p, cameraDistance = 14f * density)
        } else if (p < 1f) {
            Half(current, top = false, rotation = 180f * (1f - p), cameraDistance = 14f * density)
        }
    }
}

@Composable
private fun Half(char: Char, top: Boolean, rotation: Float = 0f, cameraDistance: Float = 8f) {
    Box(
        Modifier
            .fillMaxSize()
            .graphicsLayer {
                rotationX = rotation
                transformOrigin = TransformOrigin(0.5f, if (top) 1f else 0f)
                this.cameraDistance = cameraDistance
            }
            .clip(halfShape(top))
            .background(if (top) Color(0xFF1E1E1E) else Color(0xFF181818))
            .padding(bottom = 0.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(char.toString(), color = Color(0xFFF2F2F2), fontSize = 130.sp, fontWeight = FontWeight.Bold)
    }
}

