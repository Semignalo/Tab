package app.tab.feature.monitor

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.tab.model.Capabilities
import app.tab.model.Message
import app.tab.ui.kit.tabCard
import java.util.Locale

/**
 * Gauge live. Aturan protokol: apa yang tidak tersedia **tidak ditampilkan** — bukan nol.
 * Suhu dan GPU hanya muncul bila host melaporkan kapabilitasnya *dan* datanya memang ada.
 */
@Composable
fun MonitorScreen(metrics: Message.Metrics?, caps: Capabilities, modifier: Modifier = Modifier) {
    if (metrics == null) {
        Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("Menunggu data dari host…", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        return
    }
    Column(
        modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            GaugeCard("CPU", metrics.cpua, "${fmt1(metrics.cpua)}%", Modifier.weight(1f))
            val memPct = percent(metrics.mu, metrics.mt)
            GaugeCard("Memori", memPct, "${fmtBytes(metrics.mu)} / ${fmtBytes(metrics.mt)}", Modifier.weight(1f))
        }

        if (caps.temps && metrics.tcpu != null) {
            Card("Suhu CPU") { Text("${fmt1(metrics.tcpu!!)} °C", style = MaterialTheme.typography.headlineSmall) }
        }
        val gpu = metrics.gpu
        if (caps.gpu && gpu != null) {
            Card("GPU") {
                gpu.u?.let { Bar("Beban", it, "${fmt1(it)}%") }
                gpu.t?.let { Text("Suhu ${fmt1(it)} °C") }
                if (gpu.mu != null && gpu.mt != null) Bar("VRAM", percent(gpu.mu!!, gpu.mt!!), "${fmtBytes(gpu.mu!!)} / ${fmtBytes(gpu.mt!!)}")
            }
        }

        Card("Core CPU") {
            Row(Modifier.fillMaxWidth().height(64.dp), horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.Bottom) {
                for (c in metrics.cpu) {
                    Box(Modifier.weight(1f).fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                        Box(
                            Modifier.fillMaxWidth().fillMaxHeight((c / 100f).coerceIn(0.02f, 1f))
                                .clip(RoundedCornerShape(3.dp)).background(gaugeColor(c)),
                        )
                    }
                }
            }
        }

        if (metrics.st > 0) {
            Card("Swap") { Bar("", percent(metrics.su, metrics.st), "${fmtBytes(metrics.su)} / ${fmtBytes(metrics.st)}") }
        }

        Card("Jaringan") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text("↓ ${fmtRate(metrics.rx)}", style = MaterialTheme.typography.titleMedium)
                Text("↑ ${fmtRate(metrics.tx)}", style = MaterialTheme.typography.titleMedium)
            }
        }

        if (metrics.disks.isNotEmpty()) {
            Card("Disk") {
                for (d in metrics.disks) Bar(d.n, percent(d.u, d.t), "${fmtBytes(d.u)} / ${fmtBytes(d.t)}")
            }
        }

        Card("Daya") {
            val p = metrics.pwr
            Text(
                buildString {
                    append(if (p.ac) "Tersambung listrik" else "Baterai")
                    if (p.pct != null) append(" · ${p.pct}%")
                },
                style = MaterialTheme.typography.titleMedium,
            )
        }
    }
}

@Composable
private fun Card(title: String, content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().tabCard(radius = 28.dp).padding(18.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        content()
    }
}

@Composable
private fun GaugeCard(title: String, pct: Float, caption: String, modifier: Modifier) {
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val color = gaugeColor(pct)
    Column(
        modifier.tabCard(radius = 28.dp).padding(18.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Box(Modifier.size(110.dp), contentAlignment = Alignment.Center) {
            Canvas(Modifier.fillMaxSize()) {
                val stroke = Stroke(width = 14.dp.toPx(), cap = StrokeCap.Round)
                val inset = stroke.width / 2
                val arc = Size(size.width - stroke.width, size.height - stroke.width)
                drawArc(track, 135f, 270f, false, Offset(inset, inset), arc, style = stroke)
                drawArc(color, 135f, 270f * (pct / 100f).coerceIn(0f, 1f), false, Offset(inset, inset), arc, style = stroke)
            }
            Text("${pct.toInt()}%", fontSize = 24.sp, style = MaterialTheme.typography.titleLarge)
        }
        Text(caption, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Bar(label: String, pct: Float, caption: String) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(label, style = MaterialTheme.typography.bodyMedium)
            Text(caption, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Box(Modifier.fillMaxWidth().height(8.dp).clip(RoundedCornerShape(4.dp)).background(MaterialTheme.colorScheme.surfaceContainerHighest)) {
            Box(
                Modifier.fillMaxHeight().fillMaxWidth((pct / 100f).coerceIn(0f, 1f))
                    .clip(RoundedCornerShape(4.dp)).background(gaugeColor(pct)),
            )
        }
    }
}

@Composable
private fun gaugeColor(pct: Float): Color = when {
    pct >= 90f -> MaterialTheme.colorScheme.error
    pct >= 70f -> MaterialTheme.colorScheme.tertiary
    else -> MaterialTheme.colorScheme.primary
}

// --------------------------------------------------------------- format murni

fun percent(used: Long, total: Long): Float = if (total <= 0) 0f else (used.toDouble() / total * 100).toFloat().coerceIn(0f, 100f)

private fun fmt1(v: Float) = String.format(Locale.US, "%.1f", v)

fun fmtBytes(b: Long): String {
    val units = arrayOf("B", "KB", "MB", "GB", "TB")
    var v = b.toDouble()
    var i = 0
    while (v >= 1024 && i < units.lastIndex) {
        v /= 1024
        i++
    }
    return if (i == 0) "${b} B" else String.format(Locale.US, "%.1f %s", v, units[i])
}

fun fmtRate(bytesPerSec: Long): String = fmtBytes(bytesPerSec) + "/s"
