package app.tab.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.tab.TabApplication
import app.tab.net.ClientState
import app.tab.net.PairStart
import kotlinx.coroutines.launch

/**
 * Akar navigasi. Layar yang tampil sepenuhnya diturunkan dari [ClientState], jadi tidak ada
 * keadaan navigasi terpisah yang bisa tidak sinkron dengan sambungan sebenarnya.
 */
@Composable
fun TabRoot(app: TabApplication) {
    val client = app.client
    val state by client.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    // Menyambung ulang setelah putus sesaat: tetap tampilkan layar terakhir (dengan selubung),
    // jangan lempar user ke daftar host.
    var lastConnected by remember { mutableStateOf<ClientState.Connected?>(null) }
    LaunchedEffect(state) {
        when (val s = state) {
            is ClientState.Connected -> lastConnected = s
            is ClientState.Idle, is ClientState.NeedsPairing, is ClientState.KeyChanged -> lastConnected = null
            else -> Unit
        }
    }

    var pairError by remember { mutableStateOf<String?>(null) }

    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding()) {
        when (val s = state) {
            is ClientState.Connected -> MainScreen(app, s, reconnecting = false)
            is ClientState.Connecting -> {
                val last = lastConnected
                if (last != null) MainScreen(app, last, reconnecting = true, reconnectAttempt = s.attempt, onCancelReconnect = { client.disconnect() })
                else ConnectingScreen(s, onCancel = { client.disconnect() })
            }
            is ClientState.AwaitingPin -> PairScreen(
                hostName = s.hostName,
                ttlMs = s.ttlMs,
                onSubmit = { client.submitPin(it) },
                onCancel = { client.cancelPairing() },
            )
            else -> HostListScreen(
                app = app,
                state = s,
                notice = pairError,
                onDismissNotice = { pairError = null },
                onPair = { host ->
                    scope.launch {
                        pairError = null
                        when (val r = client.startPairing(host)) {
                            is PairStart.NeedsPin -> Unit // AwaitingPin mengambil alih layar
                            is PairStart.Refused -> pairError = pairingMessage(r)
                        }
                    }
                },
            )
        }
    }
}

private fun pairingMessage(r: PairStart.Refused): String = when (r.code) {
    app.tab.model.ErrorCode.PairingBusy -> "Di komputer, klik \"Pair new device\" dulu lalu coba lagi."
    app.tab.model.ErrorCode.ProtocolUnsupported -> "Versi Tab di komputer tidak cocok dengan aplikasi ini."
    else -> r.message
}

@Composable
private fun ConnectingScreen(s: ClientState.Connecting, onCancel: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        androidx.compose.material3.CircularProgressIndicator()
        Text(
            "Menyambung ke ${s.host.name}…",
            Modifier.padding(top = 20.dp),
            style = MaterialTheme.typography.titleMedium,
        )
        if (s.attempt > 0) {
            Text(
                "Percobaan ke-${s.attempt + 1}${s.lastError?.let { " · $it" } ?: ""}",
                Modifier.padding(top = 6.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "Pastikan HP dan komputer di Wi-Fi yang sama dan Tab berjalan di komputer.",
                Modifier.padding(top = 10.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        TextButton(onClick = onCancel, modifier = Modifier.padding(top = 16.dp)) { Text("Batal") }
    }
}
