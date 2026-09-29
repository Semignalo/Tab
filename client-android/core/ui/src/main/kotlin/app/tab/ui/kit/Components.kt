package app.tab.ui.kit

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Kartu putih berradius besar: garis tipis + bayangan lembut di tema terang, garis tepi saja
 * di tema gelap (bayangan tidak terlihat di latar gelap).
 */
@Composable
fun Modifier.tabCard(radius: Dp = 28.dp, elevation: Dp = 18.dp): Modifier {
    val shape = RoundedCornerShape(radius)
    val dark = isDarkTheme()
    return this
        .then(
            if (dark) Modifier else Modifier.shadow(
                elevation, shape,
                ambientColor = Color(0x0F000000), spotColor = Color(0x1A000000),
            ),
        )
        .clip(shape)
        .background(MaterialTheme.colorScheme.surfaceContainer)
        .border(BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant), shape)
}

enum class PillStyle { Dark, Accent, Quiet }

/** Tombol pil: hitam (utama), biru (aksen), atau senyap (abu). */
@Composable
fun PillButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    style: PillStyle = PillStyle.Dark,
    enabled: Boolean = true,
) {
    val dark = isDarkTheme()
    val (bg, fg) = when (style) {
        // Di tema gelap tombol "hitam" dibalik menjadi putih supaya tetap menonjol.
        PillStyle.Dark -> if (dark) Color(0xFFF5F5F7) to Color(0xFF1D1D1F) else TabColors.Ink to Color.White
        PillStyle.Accent -> MaterialTheme.colorScheme.primary to MaterialTheme.colorScheme.onPrimary
        PillStyle.Quiet -> MaterialTheme.colorScheme.surfaceContainerHigh to MaterialTheme.colorScheme.onSurface
    }
    Box(
        modifier
            .alpha(if (enabled) 1f else 0.4f)
            .clip(CircleShape)
            .background(bg)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 22.dp, vertical = 11.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = fg, style = MaterialTheme.typography.labelLarge)
    }
}

/** Chip kecil berbentuk pil untuk toggle/aksi ringan (isi bebas). */
@Composable
fun PillSurface(
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    content: @Composable () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val bg = if (selected) cs.secondary else cs.surfaceContainerHigh
    val fg = if (selected) cs.onSecondary else cs.onSurface
    Box(
        modifier
            .alpha(if (enabled) 1f else 0.4f)
            .clip(CircleShape)
            .background(bg)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        contentAlignment = Alignment.Center,
    ) {
        CompositionLocalProvider(LocalContentColor provides fg) {
            ProvideTextStyle(MaterialTheme.typography.labelLarge, content)
        }
    }
}

@Composable
fun PillChip(
    text: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) = PillSurface(selected, onClick, modifier, enabled) { Text(text) }

/** Tab bergaya segmen: lintasan abu, segmen aktif putih menonjol. */
@Composable
fun SegmentedTabs(
    items: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    Row(
        modifier
            .clip(CircleShape)
            .background(cs.surfaceContainerHigh)
            .padding(4.dp)
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        items.forEachIndexed { i, label ->
            val active = i == selected
            val pill: Shape = CircleShape
            Box(
                Modifier
                    .then(
                        if (active && !isDarkTheme()) Modifier.shadow(
                            4.dp, pill, ambientColor = Color(0x14000000), spotColor = Color(0x22000000),
                        ) else Modifier,
                    )
                    .clip(pill)
                    .background(if (active) cs.surfaceContainerLowest.let { if (isDarkTheme()) cs.surfaceContainerHighest else it } else Color.Transparent)
                    .clickable { onSelect(i) }
                    .padding(horizontal = 18.dp, vertical = 8.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    label,
                    color = if (active) cs.onSurface else cs.onSurfaceVariant,
                    style = MaterialTheme.typography.labelLarge,
                )
            }
        }
    }
}

/** Kata kunci berwarna gradasi biru di dalam judul; sisanya warna teks biasa. */
@Composable
fun headlineWithAccent(before: String, accent: String, after: String = ""): androidx.compose.ui.text.AnnotatedString {
    val brush = Brush.linearGradient(listOf(TabColors.GradientStart, TabColors.GradientEnd))
    return buildAnnotatedString {
        append(before)
        withStyle(SpanStyle(brush = brush)) { append(accent) }
        append(after)
    }
}

/** Kotak ikon bersudut membulat (squircle) untuk simbol di kartu. */
@Composable
fun IconSquircle(
    symbol: String,
    tint: Color,
    modifier: Modifier = Modifier,
    size: Dp = 56.dp,
    textStyle: TextStyle = MaterialTheme.typography.headlineSmall,
) {
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(size * 0.3f))
            .background(tint.copy(alpha = 0.14f)),
        contentAlignment = Alignment.Center,
    ) {
        Text(symbol, style = textStyle, color = tint)
    }
}
