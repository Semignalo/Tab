package app.tab.feature.clock

import java.time.LocalDate
import java.time.LocalTime
import java.time.format.TextStyle
import java.util.Locale

/** Empat digit HH:MM (24 jam). */
fun clockDigits(time: LocalTime): List<Char> {
    val h = time.hour
    val m = time.minute
    return listOf('0' + h / 10, '0' + h % 10, '0' + m / 10, '0' + m % 10)
}

/** Mis. "Selasa, 29 September 2026". Bahasa mengikuti [locale]. */
fun dateLine(date: LocalDate, locale: Locale): String {
    val day = date.dayOfWeek.getDisplayName(TextStyle.FULL, locale)
    val month = date.month.getDisplayName(TextStyle.FULL, locale)
    return "$day, ${date.dayOfMonth} $month ${date.year}"
}

/** Milidetik sampai menit berikutnya, dipakai agar pembaruan jatuh tepat di pergantian menit. */
fun millisToNextMinute(time: LocalTime): Long =
    (60 - time.second) * 1000L - time.nano / 1_000_000

/** Peredupan otomatis: layar diredupkan bila tidak disentuh selama [afterMs]. */
class DimPolicy(private val afterMs: Long = 30_000) {
    private var lastTouch = 0L

    fun touched(nowMs: Long) {
        lastTouch = nowMs
    }

    fun isDim(nowMs: Long): Boolean = nowMs - lastTouch >= afterMs
}
