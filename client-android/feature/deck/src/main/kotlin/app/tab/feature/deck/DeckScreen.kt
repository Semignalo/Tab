package app.tab.feature.deck

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.tab.model.Message
import app.tab.ui.kit.IconSquircle
import app.tab.ui.kit.PillChip
import app.tab.ui.kit.tabCard

/** Hasil aksi terakhir, `seq` naik setiap kali agar hasil identik tetap memicu ulang tampilan. */
data class DeckFeedbackUi(val aid: String, val ok: Boolean, val message: String?, val seq: Long)

@Composable
fun DeckScreen(
    profiles: Message.DeckProfiles?,
    profile: Message.DeckProfile?,
    feedback: DeckFeedbackUi?,
    onSelectProfile: (String) -> Unit,
    onPress: (aid: String, cell: Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptic = LocalHapticFeedback.current
    var banner by remember { mutableStateOf<DeckFeedbackUi?>(null) }

    // Kegagalan aksi harus terlihat, bukan diam: banner muncul beberapa detik lalu hilang.
    LaunchedEffect(feedback?.seq) {
        if (feedback != null) {
            banner = feedback
            if (!feedback.ok) haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            kotlinx.coroutines.delay(if (feedback.ok) 900 else 3500)
            banner = null
        }
    }

    Column(modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        if (profiles != null && profiles.list.size > 1) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (p in profiles.list) {
                    PillChip(p.name, selected = p.id == profiles.active, onClick = { onSelectProfile(p.id) })
                }
            }
        }

        val b = banner
        if (b != null) {
            val color = if (b.ok) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.errorContainer
            Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(color).padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    if (b.ok) "✔ ${b.aid}" else "✘ ${b.message ?: "aksi gagal"}",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }

        if (profile == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Memuat deck…", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            DeckGrid(profile, onPress = { aid, cell ->
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onPress(aid, cell)
            })
        }
    }
}

@Composable
private fun DeckGrid(profile: Message.DeckProfile, onPress: (String, Int) -> Unit) {
    val byCell = profile.btns.associateBy { it.i }
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        repeat(profile.rows) { r ->
            Row(Modifier.weight(1f).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                repeat(profile.cols) { c ->
                    val cell = cellIndex(r, c, profile.cols)
                    val btn = byCell[cell]
                    Box(Modifier.weight(1f).fillMaxSize()) {
                        if (btn != null) DeckTile(btn, onPress)
                    }
                }
            }
        }
    }
}

@Composable
private fun DeckTile(btn: app.tab.model.DeckButton, onPress: (String, Int) -> Unit) {
    val tint = parseColor(btn.col) ?: MaterialTheme.colorScheme.primary
    Box(
        Modifier
            .fillMaxSize()
            .tabCard(radius = 28.dp)
            .clickable { onPress(btn.aid, btn.i) }
            .padding(12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            IconSquircle(iconSymbol(btn.ic, btn.lbl), tint, size = 64.dp)
            Text(
                btn.lbl,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.titleSmall,
                maxLines = 2,
            )
        }
    }
}

// --------------------------------------------------------------- logika murni

fun cellIndex(row: Int, col: Int, cols: Int): Int = row * cols + col

/** `#RRGGBB` / `#AARRGGBB` → warna; `null` bila tidak sah. */
fun parseColor(hex: String?): Color? {
    val s = hex?.trim()?.removePrefix("#") ?: return null
    val v = s.toLongOrNull(16) ?: return null
    return when (s.length) {
        6 -> Color(0xFF000000 or v)
        8 -> Color(v)
        else -> null
    }
}

/** Warna teks yang terbaca di atas [bg] (kontras sederhana berdasarkan luminans). */
fun readableOn(bg: Color): Color {
    val l = 0.299f * bg.red + 0.587f * bg.green + 0.114f * bg.blue
    return if (l > 0.6f) Color(0xFF111111) else Color.White
}

/** Nama ikon dari host → simbol. Tanpa ikon: huruf pertama label, supaya tombol tidak kosong. */
fun iconSymbol(name: String?, label: String): String = when (name) {
    "skip_previous" -> "⏮"
    "play_pause", "play" -> "⏯"
    "skip_next" -> "⏭"
    "volume_up" -> "🔊"
    "volume_down" -> "🔉"
    "volume_off", "mute" -> "🔇"
    "copy" -> "⧉"
    "cut" -> "✂"
    "undo" -> "↶"
    "redo" -> "↷"
    "select_all" -> "▦"
    "lock" -> "🔒"
    "paste" -> "📋"
    "screenshot" -> "📷"
    "mic" -> "🎙"
    "record" -> "⏺"
    "stop" -> "⏹"
    "browser", "web" -> "🌐"
    "folder" -> "📁"
    "app" -> "▣"
    "scene", "obs" -> "🎬"
    else -> label.trim().firstOrNull()?.uppercase() ?: "•"
}
