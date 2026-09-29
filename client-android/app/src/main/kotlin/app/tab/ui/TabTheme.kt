package app.tab.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val Dark = darkColorScheme(
    primary = Color(0xFF7DD3FC),
    secondary = Color(0xFFA5B4FC),
    background = Color(0xFF0B0F14),
    surface = Color(0xFF0B0F14),
)

private val Light = lightColorScheme(
    primary = Color(0xFF0369A1),
    secondary = Color(0xFF4F46E5),
)

/** Warna dinamis Material You bila tersedia (Android 12+), selain itu palet Tab. */
@Composable
fun TabTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val ctx = LocalContext.current
            if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        }
        dark -> Dark
        else -> Light
    }
    MaterialTheme(colorScheme = scheme, content = content)
}
