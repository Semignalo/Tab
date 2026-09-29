package app.tab.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.text.KeyboardOptions
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.tab.TabApplication
import app.tab.net.ClientState
import app.tab.net.DiscoveredHost
import app.tab.net.HostRecord
import kotlinx.coroutines.launch

@Composable
fun HostListScreen(
    app: TabApplication,
    state: ClientState,
    notice: String?,
    onDismissNotice: () -> Unit,
    onPair: (DiscoveredHost) -> Unit,
) {
    val client = app.client
    val discovered by app.discovery.hosts.collectAsStateWithLifecycle()
    val scanning by app.discovery.scanning.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var manualDialog by remember { mutableStateOf(false) }
    var manualMessage by remember { mutableStateOf<String?>(null) }
    var stored by remember { mutableStateOf(client.knownHosts()) }

    // Pencarian jalan hanya selama layar ini tampil.
    LaunchedEffect(Unit) { app.discovery.run() }
    LaunchedEffect(state, discovered) { stored = client.knownHosts() }

    LazyColumn(
        Modifier.fillMaxSize().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            Column(Modifier.padding(top = 24.dp, bottom = 4.dp)) {
                Text("Tab", style = MaterialTheme.typography.displaySmall)
                Text(
                    "Pilih komputer yang ingin dikendalikan.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        val banner = when (state) {
            is ClientState.NeedsPairing -> "Pairing dengan ${state.host.name} tidak berlaku lagi (${state.reason}). Pasangkan ulang."
            is ClientState.KeyChanged -> "Kunci keamanan ${state.host.name} berubah. Bisa jadi komputer diinstal ulang — atau ada yang menyamar. Hapus pairing lama lalu pasangkan ulang hanya bila Anda yakin."
            else -> notice
        }
        if (banner != null) {
            item {
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(MaterialTheme.colorScheme.errorContainer).padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(banner, Modifier.weight(1f), color = MaterialTheme.colorScheme.onErrorContainer, style = MaterialTheme.typography.bodyMedium)
                    if (notice != null) TextButton(onClick = onDismissNotice) { Text("OK") }
                }
            }
        }

        item { SectionTitle(if (scanning && discovered.isEmpty()) "Mencari di jaringan…" else "Di jaringan ini") }

        if (discovered.isEmpty()) {
            item {
                Text(
                    "Belum ada yang terlihat. Pastikan Tab berjalan di komputer dan keduanya di Wi-Fi yang sama. " +
                        "Di Windows, izinkan \"Private network\" saat dialog Firewall muncul.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        items(discovered, key = { it.hid.toHex() }) { host ->
            val record = stored.firstOrNull { it.hid == host.hid }
            HostRow(
                title = host.name,
                subtitle = "${host.response.os} ${host.response.osv} · ${host.address.hostAddress}",
                badge = when {
                    record?.token != null -> "Dipasangkan"
                    else -> "Baru"
                },
                onClick = {
                    if (record?.token != null) client.connect(host.hid) else onPair(host)
                },
            )
        }

        val hidden = stored.filter { s -> discovered.none { it.hid == s.hid } }
        if (hidden.isNotEmpty()) {
            item { SectionTitle("Sudah dipasangkan (tidak terlihat sekarang)") }
            items(hidden, key = { "s-" + it.hid.toHex() }) { rec ->
                StoredRow(rec, onConnect = { if (rec.token != null) client.connect(rec.hid) }, onForget = { client.forget(rec.hid) })
            }
        }

        item {
            OutlinedButton(onClick = { manualDialog = true }, modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 24.dp)) {
                Text("Tambah lewat alamat IP")
            }
        }
    }

    if (manualDialog) {
        ManualDialog(
            message = manualMessage,
            onDismiss = { manualDialog = false; manualMessage = null },
            onProbe = { ip ->
                scope.launch {
                    manualMessage = "Mencari…"
                    val found = app.discovery.probe(ip)
                    if (found == null) {
                        manualMessage = "Tidak ada host Tab di alamat itu."
                    } else {
                        manualDialog = false
                        manualMessage = null
                        if (client.knownHosts().any { it.hid == found.hid && it.token != null }) client.connect(found.hid) else onPair(found)
                    }
                }
            },
        )
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 10.dp))
}

@Composable
private fun HostRow(title: String, subtitle: String, badge: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainer)
            .clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(badge, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
    }
}

@Composable
private fun StoredRow(rec: HostRecord, onConnect: () -> Unit, onForget: () -> Unit) {
    var confirm by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainerLow).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).clickable(onClick = onConnect)) {
            Text(rec.name, style = MaterialTheme.typography.titleMedium)
            Text(
                if (rec.token != null) "Terakhir di ${rec.address}" else "Perlu dipasangkan ulang",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        TextButton(onClick = { confirm = true }) { Text("Lupakan") }
    }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("Lupakan ${rec.name}?") },
            text = { Text("Pairing di HP ini dihapus. Untuk mencabut akses sepenuhnya, cabut juga perangkat ini di daftar perangkat pada Tab di komputer.") },
            confirmButton = { TextButton(onClick = { confirm = false; onForget() }) { Text("Lupakan") } },
            dismissButton = { TextButton(onClick = { confirm = false }) { Text("Batal") } },
        )
    }
}

@Composable
private fun ManualDialog(message: String?, onDismiss: () -> Unit, onProbe: (String) -> Unit) {
    var ip by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Alamat komputer") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Untuk jaringan yang memblokir pencarian otomatis. Masukkan IP komputer, mis. 192.168.1.20.", style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(
                    value = ip,
                    onValueChange = { ip = it },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                    label = { Text("Alamat IP") },
                )
                if (message != null) Text(message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary)
            }
        },
        confirmButton = { Button(onClick = { onProbe(ip) }, enabled = ip.isNotBlank()) { Text("Cari") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Tutup") } },
    )
}
