package app.tab.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.tab.TabApplication
import app.tab.feature.clock.ClockScreen
import app.tab.feature.deck.DeckFeedbackUi
import app.tab.feature.deck.DeckScreen
import app.tab.feature.monitor.MonitorScreen
import app.tab.feature.music.MusicScreen
import app.tab.feature.trackpad.TrackpadScreen
import app.tab.model.Message
import app.tab.model.Mode
import app.tab.net.ClientState
import app.tab.ui.kit.SegmentedTabs
import kotlinx.coroutines.launch

private val PAGE_ORDER = listOf(Mode.Trackpad, Mode.Deck, Mode.Monitor, Mode.Music, Mode.Clock)

private fun label(m: Mode) = when (m) {
    Mode.Trackpad -> "Trackpad"
    Mode.Deck -> "Deck"
    Mode.Monitor -> "Monitor"
    Mode.Music -> "Musik"
    Mode.Clock -> "Jam"
    Mode.Idle -> ""
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun MainScreen(
    app: TabApplication,
    state: ClientState.Connected,
    reconnecting: Boolean,
    reconnectAttempt: Int = 0,
    onCancelReconnect: () -> Unit = {},
) {
    val client = app.client
    val caps = state.welcome.caps
    val modes = remember(caps) { PAGE_ORDER.filter { it in caps.modes } }
    val snap by client.snapshot.collectAsStateWithLifecycle()
    val rtt by client.rttMs.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    if (modes.isEmpty()) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Host tidak menawarkan mode apa pun.") }
        return
    }

    val pager = rememberPagerState(initialPage = modes.indexOf(snap.mode).coerceAtLeast(0)) { modes.size }

    // Berpindah halaman = berpindah mode; host hanya mengirim telemetri untuk mode aktif.
    LaunchedEffect(pager.settledPage, modes) {
        client.setMode(modes[pager.settledPage])
        if (modes[pager.settledPage] == Mode.Monitor) client.subscribeMetrics(1000) else client.unsubscribeMetrics()
    }

    Column(Modifier.fillMaxSize()) {
        TopBar(
            hostName = state.host.name,
            rttMs = rtt,
            modes = modes,
            selected = pager.currentPage,
            onSelect = { scope.launch { pager.animateScrollToPage(it) } },
            onDisconnect = { client.disconnect() },
        )
        snap.modeError?.let {
            Text(it, Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth(), beyondViewportPageCount = 0) { page ->
            val mode = modes[page]
            when (mode) {
                Mode.Trackpad -> TrackpadPage(app, snap)
                Mode.Deck -> DeckScreen(
                    profiles = snap.deckProfiles,
                    profile = snap.deckProfile,
                    feedback = snap.deckResult?.let { DeckFeedbackUi(it.aid, it.ok, it.msg, it.seq) },
                    onSelectProfile = { client.send(Message.SelectProfile(it)) },
                    onPress = { aid, cell -> client.send(Message.DeckPress(aid, cell)) },
                )
                Mode.Monitor -> MonitorScreen(snap.metrics, caps)
                Mode.Music -> MusicScreen(
                    nowPlaying = snap.nowPlaying,
                    receivedAtMs = snap.nowPlayingAtMs,
                    artwork = snap.nowPlaying?.art?.let { snap.artwork[it] },
                    lyrics = snap.lyrics,
                    caps = caps,
                    clockMs = { System.nanoTime() / 1_000_000 },
                    onCommand = { client.send(Message.MediaCommand(it)) },
                    onSeek = { client.send(Message.MediaSeek(it)) },
                    onVolume = { client.send(Message.MediaVolume(it)) },
                    onRequestArtwork = { client.send(Message.GetArtwork(it)) },
                    onRequestLyrics = { client.send(Message.GetLyrics(it)) },
                )
                Mode.Clock -> ClockScreen(active = pager.currentPage == page)
                Mode.Idle -> Unit
            }
        }
    }

    if (reconnecting) ReconnectOverlay(reconnectAttempt, onCancelReconnect)
}

@Composable
private fun TrackpadPage(app: TabApplication, snap: app.tab.net.Snapshot) {
    val client = app.client
    val context = LocalContext.current
    var guideSeen by remember { mutableStateOf(app.storage.guideSeen()) }
    val caps = (client.state.value as? ClientState.Connected)?.welcome?.caps
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    // Gutter kiri/kanan sengaja tidak menangkap sentuhan agar pager tetap bisa digeser dari tepi.
    TrackpadScreen(
        modifier = Modifier.padding(horizontal = 20.dp),
        supportedGestures = caps?.gestures?.toSet() ?: emptySet(),
        sensitivity = snap.inputSettings?.sens ?: 1f,
        naturalScroll = snap.inputSettings?.natural ?: true,
        hostClipboard = snap.hostClipboard,
        showGuide = !guideSeen,
        clipboardEnabled = caps?.clipboard == true,
        onDismissGuide = { app.storage.setGuideSeen(); guideSeen = true },
        onEvent = client::input,
        onSensitivityChange = { client.send(Message.SetInputSettings(sens = it)) },
        onNaturalChange = { client.send(Message.SetInputSettings(natural = it)) },
        onPushClipboard = {
            val text = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
            if (!text.isNullOrEmpty()) client.send(Message.ClipboardPush(text))
        },
        onCopyHostClipboard = {
            snap.hostClipboard?.let { clipboard.setPrimaryClip(ClipData.newPlainText("Tab", it)) }
        },
    )
}

@Composable
private fun TopBar(
    hostName: String,
    rttMs: Float?,
    modes: List<Mode>,
    selected: Int,
    onSelect: (Int) -> Unit,
    onDisconnect: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val dot = when {
                rttMs == null -> Color(0xFF9E9E9E)
                rttMs < 30 -> Color(0xFF4CAF50)
                rttMs < 100 -> Color(0xFFFFC107)
                else -> Color(0xFFF44336)
            }
            Box(Modifier.size(10.dp).clip(CircleShape).background(dot))
            Text(
                " $hostName" + (rttMs?.let { "  ·  ${"%.0f".format(it)} ms" } ?: ""),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.weight(1f),
            )
            Box {
                TextButton(onClick = { menu = true }) { Text("⋮") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text("Putuskan") }, onClick = { menu = false; onDisconnect() })
                }
            }
        }
        SegmentedTabs(modes.map { label(it) }, selected, onSelect)
    }
}

@Composable
private fun ReconnectOverlay(attempt: Int, onCancel: () -> Unit) {
    Box(Modifier.fillMaxSize().background(Color(0xB0000000)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
            androidx.compose.material3.CircularProgressIndicator()
            Text("Menyambung ulang…", color = Color.White, style = MaterialTheme.typography.titleMedium)
            if (attempt > 1) Text("Percobaan ke-${attempt + 1}", color = Color(0xFFCCCCCC), style = MaterialTheme.typography.bodySmall)
            TextButton(onClick = onCancel) { Text("Batal") }
        }
    }
}
