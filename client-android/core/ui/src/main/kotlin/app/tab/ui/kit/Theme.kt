package app.tab.ui.kit

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.shape.RoundedCornerShape

/**
 * Bahasa visual Tab: minimalis, terang, banyak ruang kosong; kartu putih berradius besar di
 * atas latar abu sangat muda, garis tipis, bayangan lembut, tombol pil hitam, dan satu warna
 * aksen biru (dengan gradasi hanya untuk kata kunci pada judul).
 *
 * Warna dinamis Material You sengaja **tidak** dipakai: tampilan harus sama di semua perangkat.
 */
object TabColors {
    val Accent = Color(0xFF0071E3)
    val AccentDark = Color(0xFF2997FF)
    val GradientStart = Color(0xFF4C81DE)
    val GradientEnd = Color(0xFF274BCE)
    val Danger = Color(0xFFE40014)

    val Ink = Color(0xFF1D1D1F)
    val InkSecondary = Color(0xFF515154)
    val InkTertiary = Color(0xFF86868B)
}

private val LightScheme = lightColorScheme(
    primary = TabColors.Accent,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFEAF2FD),
    onPrimaryContainer = Color(0xFF0B3D7A),
    secondary = TabColors.Ink,
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFF5F5F7),
    onSecondaryContainer = TabColors.Ink,
    tertiary = Color(0xFFB25E00),
    background = Color(0xFFFBFBFD),
    onBackground = TabColors.Ink,
    surface = Color(0xFFFBFBFD),
    onSurface = TabColors.Ink,
    onSurfaceVariant = TabColors.InkSecondary,
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color.White,
    surfaceContainer = Color.White,
    surfaceContainerHigh = Color(0xFFF5F5F7),
    surfaceContainerHighest = Color(0xFFE8E8ED),
    outline = TabColors.InkTertiary,
    outlineVariant = Color(0x0F000000),
    error = TabColors.Danger,
    onError = Color.White,
    errorContainer = Color(0xFFFDECEE),
    onErrorContainer = Color(0xFF8A000C),
)

private val DarkScheme = darkColorScheme(
    primary = TabColors.AccentDark,
    onPrimary = Color(0xFF00203F),
    primaryContainer = Color(0xFF12324F),
    onPrimaryContainer = Color(0xFFCFE6FF),
    secondary = Color(0xFFF5F5F7),
    onSecondary = Color(0xFF1D1D1F),
    secondaryContainer = Color(0xFF2C2C2E),
    onSecondaryContainer = Color(0xFFF5F5F7),
    tertiary = Color(0xFFFFB454),
    background = Color(0xFF0A0A0C),
    onBackground = Color(0xFFF5F5F7),
    surface = Color(0xFF0A0A0C),
    onSurface = Color(0xFFF5F5F7),
    onSurfaceVariant = Color(0xFFA1A1A6),
    surfaceContainerLowest = Color(0xFF111113),
    surfaceContainerLow = Color(0xFF1C1C1E),
    surfaceContainer = Color(0xFF1C1C1E),
    surfaceContainerHigh = Color(0xFF2C2C2E),
    surfaceContainerHighest = Color(0xFF3A3A3C),
    outline = Color(0xFF8E8E93),
    outlineVariant = Color(0x1FFFFFFF),
    error = Color(0xFFFF5A64),
    onError = Color(0xFF3B0004),
    errorContainer = Color(0xFF4A0F14),
    onErrorContainer = Color(0xFFFFD9DC),
)

private fun tight(size: Int, line: Int, weight: FontWeight, tracking: Float) =
    TextStyle(fontSize = size.sp, lineHeight = line.sp, fontWeight = weight, letterSpacing = tracking.sp)

/** Judul besar dan rapat (letter-spacing negatif) seperti gaya hero; isi tetap ringan. */
private val TabTypography = Typography(
    displayLarge = tight(56, 58, FontWeight.Bold, -1.6f),
    displayMedium = tight(44, 48, FontWeight.Bold, -1.2f),
    displaySmall = tight(36, 40, FontWeight.Bold, -0.9f),
    headlineLarge = tight(32, 36, FontWeight.Bold, -0.7f),
    headlineMedium = tight(28, 32, FontWeight.SemiBold, -0.5f),
    headlineSmall = tight(24, 28, FontWeight.SemiBold, -0.4f),
    titleLarge = tight(20, 26, FontWeight.SemiBold, -0.3f),
    titleMedium = tight(16, 22, FontWeight.SemiBold, -0.2f),
    titleSmall = tight(14, 20, FontWeight.SemiBold, -0.1f),
    bodyLarge = tight(16, 24, FontWeight.Normal, 0f),
    bodyMedium = tight(14, 21, FontWeight.Normal, 0f),
    bodySmall = tight(12, 18, FontWeight.Normal, 0f),
    labelLarge = tight(14, 20, FontWeight.Medium, 0f),
    labelMedium = tight(12, 16, FontWeight.Medium, 0.1f),
    labelSmall = tight(11, 14, FontWeight.Medium, 0.2f),
)

private val TabShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(18.dp),
    large = RoundedCornerShape(28.dp),
    extraLarge = RoundedCornerShape(33.dp),
)

@Composable
fun TabTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (dark) DarkScheme else LightScheme,
        typography = TabTypography,
        shapes = TabShapes,
        content = content,
    )
}

/** Apakah tema aktif gelap; dipakai untuk memilih bayangan (terang) vs garis tepi (gelap). */
@Composable
fun isDarkTheme(): Boolean = MaterialTheme.colorScheme.background.luminance() < 0.5f
