package app.tab.feature.music

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.tab.model.Capabilities
import app.tab.model.MediaCmd
import app.tab.model.Message

@Composable
fun MusicScreen(
    nowPlaying: Message.NowPlaying?,
    receivedAtMs: Long,
    artwork: ByteArray?,
    lyrics: Message.LyricsDoc?,
    caps: Capabilities,
    clockMs: () -> Long,
    onCommand: (MediaCmd) -> Unit,
    onSeek: (Long) -> Unit,
    onVolume: (Float) -> Unit,
    onRequestArtwork: (String) -> Unit,
    onRequestLyrics: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (nowPlaying == null) {
        Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("Tidak ada yang diputar", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        return
    }

    val np = nowPlaying
    // Artwork dikirim sekali per lagu lalu di-cache; minta hanya bila belum ada.
    LaunchedEffect(np.art, artwork == null) {
        if (np.art != null && artwork == null) onRequestArtwork(np.art!!)
    }
    LaunchedEffect(np.title, np.artist, np.lyr) {
        if (np.lyr) onRequestLyrics(np.art ?: "current")
    }

    // Interpolasi lokal ±10 Hz: cukup halus untuk timeline dan baris lirik, hemat baterai.
    var now by remember { mutableStateOf(clockMs()) }
    LaunchedEffect(np.play) {
        while (np.play) {
            now = clockMs()
            kotlinx.coroutines.delay(100)
        }
        now = clockMs()
    }
    val pos = positionMs(np, receivedAtMs, now)

    Column(modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
            Cover(artwork, Modifier.weight(0.4f))
            Column(Modifier.weight(0.6f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(np.title, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, maxLines = 2)
                Text(np.artist, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                if (np.album.isNotBlank()) {
                    Text(np.album, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
        }

        Timeline(pos, np.dur, enabled = caps.mediaSeek, onSeek = onSeek)

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { onCommand(MediaCmd.Prev) }) { Text("⏮", fontSize = 26.sp) }
            FilledIconButton(onClick = { onCommand(MediaCmd.Toggle) }, modifier = Modifier.padding(4.dp)) {
                Text(if (np.play) "⏸" else "▶", fontSize = 26.sp)
            }
            IconButton(onClick = { onCommand(MediaCmd.Next) }) { Text("⏭", fontSize = 26.sp) }
        }

        if (caps.mediaVolume) {
            var vol by remember { mutableStateOf(0.5f) }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("🔈")
                Slider(vol, { vol = it }, onValueChangeFinished = { onVolume(vol) }, modifier = Modifier.weight(1f))
                Text("🔊")
            }
        }

        if (lyrics != null && lyrics.lines.isNotEmpty()) {
            Lyrics(lyrics, pos, Modifier.weight(1f).fillMaxWidth())
        }
    }
}

@Composable
private fun Cover(bytes: ByteArray?, modifier: Modifier) {
    val bitmap = remember(bytes) { bytes?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }?.asImageBitmap() }
    Box(
        modifier.aspectRatio(1f).clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh),
        contentAlignment = Alignment.Center,
    ) {
        if (bitmap != null) {
            Image(bitmap, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        } else {
            Text("♪", fontSize = 40.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun Timeline(pos: Long, dur: Long, enabled: Boolean, onSeek: (Long) -> Unit) {
    var drag by remember { mutableStateOf<Float?>(null) }
    val fraction = if (dur > 0) (pos.toFloat() / dur).coerceIn(0f, 1f) else 0f
    Column {
        Slider(
            value = drag ?: fraction,
            onValueChange = { if (enabled) drag = it },
            onValueChangeFinished = {
                drag?.let { onSeek((it * dur).toLong()) }
                drag = null
            },
            enabled = enabled && dur > 0,
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(formatTime(drag?.let { (it * dur).toLong() } ?: pos), style = MaterialTheme.typography.labelMedium)
            Text(if (dur > 0) formatTime(dur) else "--:--", style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Composable
private fun Lyrics(doc: Message.LyricsDoc, pos: Long, modifier: Modifier) {
    val current = currentLineIndex(doc.lines, pos)
    val state = rememberLazyListState()
    // Baris aktif dijaga di sekitar tengah daftar.
    LaunchedEffect(current) {
        if (current >= 0) state.animateScrollToItem((current - 2).coerceAtLeast(0))
    }
    LazyColumn(modifier, state = state, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        itemsIndexed(doc.lines) { i, line ->
            val active = i == current
            Text(
                line.s.ifBlank { "♪" },
                Modifier.fillMaxWidth(),
                textAlign = TextAlign.Center,
                fontSize = if (active) 20.sp else 16.sp,
                fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
            )
        }
    }
}
