package app.tab.feature.trackpad

import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import app.tab.model.InputEvent
import app.tab.model.ModifierBits
import app.tab.ui.kit.PillSurface
import app.tab.ui.kit.tabCard

private const val SENTINEL = " "

/**
 * Layar trackpad: permukaan multi-touch + keyboard + tombol modifier + clipboard.
 *
 * Seluruh klasifikasi gesture ada di [GestureEngine]; layar ini hanya menerjemahkan sentuhan
 * mentah ke [Pointer] (dalam dp) dan meneruskan event yang dihasilkan.
 */
@Composable
fun TrackpadScreen(
    supportedGestures: Set<String>,
    sensitivity: Float,
    naturalScroll: Boolean,
    hostClipboard: String?,
    showGuide: Boolean,
    clipboardEnabled: Boolean,
    onDismissGuide: () -> Unit,
    onEvent: (InputEvent) -> Unit,
    onSensitivityChange: (Float) -> Unit,
    onNaturalChange: (Boolean) -> Unit,
    onPushClipboard: () -> Unit,
    onCopyHostClipboard: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val emit by rememberUpdatedState(onEvent)
    val engine = remember(supportedGestures) { GestureEngine({ emit(it) }, supportedGestures) }
    DisposableEffect(engine) { onDispose { engine.reset() } }

    // Inersia scroll digerakkan per frame, hanya selama aktif.
    LaunchedEffect(engine) {
        while (true) {
            androidx.compose.runtime.withFrameNanos {
                if (engine.momentumActive) engine.tick(SystemClock.uptimeMillis())
            }
        }
    }

    var keyboardOn by remember { mutableStateOf(false) }
    var mods by remember { mutableIntStateOf(0) }
    var showSettings by remember { mutableStateOf(false) }

    Column(modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            FilterChip(selected = keyboardOn, onClick = { keyboardOn = !keyboardOn }, label = { Text("Keyboard") })
            if (clipboardEnabled) {
                FilterChip(selected = false, onClick = onPushClipboard, label = { Text("HP → host") })
                FilterChip(
                    selected = false,
                    enabled = hostClipboard != null,
                    onClick = onCopyHostClipboard,
                    label = { Text("Host → HP") },
                )
            }
            Box(Modifier.weight(1f))
            FilterChip(selected = showSettings, onClick = { showSettings = !showSettings }, label = { Text("Setelan") })
        }

        if (showSettings) {
            SettingsPanel(sensitivity, naturalScroll, onSensitivityChange, onNaturalChange)
        }

        TouchSurface(engine, Modifier.weight(1f).fillMaxWidth())

        if (keyboardOn) {
            KeyRow(mods = mods, onMods = { mods = it; onEvent(InputEvent.Modifiers(it)) }, onEvent = onEvent)
            HiddenKeyboardInput(onEvent)
        }
    }

    if (showGuide) GestureGuide(onDismissGuide)
}

@Composable
private fun TouchSurface(engine: GestureEngine, modifier: Modifier) {
    val density = LocalDensity.current.density
    Box(
        modifier
            .tabCard(radius = 33.dp)
            .pointerInput(engine, density) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent(PointerEventPass.Main)
                        val now = event.changes.maxOfOrNull { it.uptimeMillis } ?: SystemClock.uptimeMillis()
                        val down = event.changes.filter { it.pressed }.map {
                            Pointer(it.id.value, it.position.x / density, it.position.y / density)
                        }
                        engine.onPointers(now, down)
                        event.changes.forEach { it.consume() }
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            "1 jari: kursor · tap: klik\n2 jari: scroll · tap: klik kanan\n3 jari: tap klik tengah",
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
private fun SettingsPanel(
    sensitivity: Float,
    natural: Boolean,
    onSensitivity: (Float) -> Unit,
    onNatural: (Boolean) -> Unit,
) {
    var draft by remember(sensitivity) { mutableFloatStateOf(sensitivity) }
    Column(
        Modifier.fillMaxWidth().tabCard(radius = 24.dp, elevation = 10.dp).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text("Kecepatan kursor  ×${"%.1f".format(draft)}", style = MaterialTheme.typography.labelLarge)
        Slider(
            value = draft,
            onValueChange = { draft = it },
            onValueChangeFinished = { onSensitivity(draft) },
            valueRange = 0.3f..3f,
        )
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Text("Scroll natural", Modifier.weight(1f), style = MaterialTheme.typography.labelLarge)
            Switch(checked = natural, onCheckedChange = onNatural)
        }
        Text(
            "Setelan ini disimpan di host, jadi sama di semua perangkat.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun KeyRow(mods: Int, onMods: (Int) -> Unit, onEvent: (InputEvent) -> Unit) {
    fun toggle(bit: Int) = onMods(mods xor bit)
    fun tap(key: String) {
        onEvent(InputEvent.Key(key, true))
        onEvent(InputEvent.Key(key, false))
    }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilterChip(mods and ModifierBits.CTRL != 0, { toggle(ModifierBits.CTRL) }, { Text("Ctrl") })
            FilterChip(mods and ModifierBits.ALT != 0, { toggle(ModifierBits.ALT) }, { Text("Alt") })
            FilterChip(mods and ModifierBits.SHIFT != 0, { toggle(ModifierBits.SHIFT) }, { Text("Shift") })
            FilterChip(mods and ModifierBits.META != 0, { toggle(ModifierBits.META) }, { Text("Cmd/Win") })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for ((label, key) in listOf("Esc" to "esc", "Tab" to "tab", "←" to "left", "↑" to "up", "↓" to "down", "→" to "right", "Del" to "delete")) {
                FilterChip(false, { tap(key) }, { Text(label) })
            }
        }
    }
}

/**
 * Menangkap teks IME lewat kolom tak terlihat. Teks dikirim utuh sebagai `Text` (bukan per
 * tombol), dan penghapusan terdeteksi dari kolom yang menjadi kosong — karena itu kolom selalu
 * diisi satu spasi sebagai penanda.
 */
@Composable
private fun HiddenKeyboardInput(onEvent: (InputEvent) -> Unit) {
    var value by remember { mutableStateOf(TextFieldValue(SENTINEL, TextRange(1))) }
    val focus = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        focus.requestFocus()
        keyboard?.show()
    }
    BasicTextField(
        value = value,
        onValueChange = { new ->
            val text = new.text
            when {
                text.length > 1 -> onEvent(InputEvent.Text(text.removePrefix(SENTINEL).ifEmpty { text.drop(1) }))
                text.isEmpty() -> {
                    onEvent(InputEvent.Key("backspace", true))
                    onEvent(InputEvent.Key("backspace", false))
                }
            }
            value = TextFieldValue(SENTINEL, TextRange(1))
        },
        modifier = Modifier.size(1.dp).focusRequester(focus),
        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, keyboardType = KeyboardType.Text, imeAction = ImeAction.Send),
        keyboardActions = KeyboardActions(onSend = {
            onEvent(InputEvent.Key("enter", true))
            onEvent(InputEvent.Key("enter", false))
        }),
    )
}

@Composable
private fun GestureGuide(onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onClick = onDismiss) { Text("Mengerti") } },
        title = { Text("Gesture trackpad") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("• 1 jari geser — gerakkan kursor")
                Text("• 1 jari tap — klik kiri")
                Text("• Tap lalu langsung tahan & geser — seret (drag)")
                Text("• 2 jari geser — scroll, lepas cepat untuk inersia")
                Text("• 2 jari tap — klik kanan")
                Text("• 3 jari tap — klik tengah")
                Text("• 4 jari geser kiri/kanan — pindah Space (macOS)")
                Text("• 4 jari geser atas — Mission Control (macOS)")
            }
        },
    )
}

/** Nama lama dipertahankan agar pemanggilnya tidak berubah; tampilannya kini pil Tab. */
@Composable
private fun FilterChip(
    selected: Boolean,
    onClick: () -> Unit,
    label: @Composable () -> Unit,
    enabled: Boolean = true,
) = PillSurface(selected, onClick, enabled = enabled, content = label)
