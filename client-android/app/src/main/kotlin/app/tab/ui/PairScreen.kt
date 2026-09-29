package app.tab.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.tab.net.PairResult
import app.tab.ui.kit.headlineWithAccent
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Layar PIN. PIN 6 digit tampil di layar komputer; dikirim otomatis begitu digit keenam
 * diketik. Berlaku 120 detik dan maksimum 5 percobaan salah — sisanya ditampilkan apa adanya.
 */
@Composable
fun PairScreen(
    hostName: String,
    ttlMs: Long,
    onSubmit: suspend (String) -> PairResult,
    onCancel: () -> Unit,
) {
    var pin by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var secondsLeft by remember { mutableIntStateOf((ttlMs / 1000).toInt()) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }

    LaunchedEffect(Unit) { focus.requestFocus() }
    LaunchedEffect(Unit) {
        while (secondsLeft > 0) {
            delay(1000)
            secondsLeft--
        }
        error = "PIN kedaluwarsa. Buat PIN baru di komputer, lalu mulai lagi."
    }

    fun submit(value: String) {
        busy = true
        scope.launch {
            when (val r = onSubmit(value)) {
                PairResult.Paired -> Unit // ClientState.Connected mengambil alih layar
                is PairResult.WrongPin -> {
                    error = if (r.attemptsLeft > 0) "PIN salah. Sisa ${r.attemptsLeft} percobaan." else "PIN salah 5 kali. Buat PIN baru di komputer."
                    pin = ""
                    busy = false
                    if (r.attemptsLeft == 0) onCancel()
                }
                is PairResult.Refused -> {
                    error = r.message
                    busy = false
                    onCancel()
                }
            }
        }
    }

    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            headlineWithAccent("Pasangkan dengan ", hostName),
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onBackground,
            textAlign = TextAlign.Center,
        )
        Text(
            "Ketik PIN 6 digit yang tampil di layar komputer.",
            Modifier.padding(top = 8.dp, bottom = 24.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        OutlinedTextField(
            value = pin,
            onValueChange = { v ->
                val digits = v.filter { it.isDigit() }.take(6)
                pin = digits
                error = null
                if (digits.length == 6 && !busy && secondsLeft > 0) submit(digits)
            },
            enabled = !busy && secondsLeft > 0,
            singleLine = true,
            textStyle = MaterialTheme.typography.headlineMedium.copy(letterSpacing = 10.sp, textAlign = TextAlign.Center),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
            modifier = Modifier.fillMaxWidth(0.7f).focusRequester(focus),
            isError = error != null,
        )
        Text(
            error ?: if (secondsLeft > 0) "Berlaku ${secondsLeft} detik lagi" else "",
            Modifier.padding(top = 12.dp),
            style = MaterialTheme.typography.bodySmall,
            color = if (error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        TextButton(onClick = onCancel, modifier = Modifier.padding(top = 12.dp)) { Text("Batal") }
    }
}
