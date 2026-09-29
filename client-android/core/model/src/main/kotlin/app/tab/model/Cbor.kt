package app.tab.model

import java.io.ByteArrayOutputStream

/**
 * Codec CBOR minimal (RFC 8949) yang cukup untuk protokol Tab.
 *
 * Ditulis sendiri, bukan lewat kotlinx-serialization, karena kontraknya menuntut byte yang
 * identik dengan `ciborium` di sisi Rust: urutan key mengikuti urutan field, bilangan bulat
 * selalu dalam bentuk terpendek, dan float dituliskan dalam lebar **terpendek yang tidak
 * kehilangan nilai** (f16 → f32 → f64). kotlinx-serialization selalu menulis f32, sehingga
 * `1.0` akan berbeda satu byte-header dari fixture dan uji kontrak tidak bisa byte-identik.
 */
sealed interface CborValue {
    data class Uint(val v: Long) : CborValue
    data class NInt(val v: Long) : CborValue
    class Bytes(val v: ByteArray) : CborValue {
        override fun equals(other: Any?) = other is Bytes && v.contentEquals(other.v)
        override fun hashCode() = v.contentHashCode()
    }

    data class Text(val v: String) : CborValue
    data class Arr(val v: List<CborValue>) : CborValue
    data class Map(val v: List<Pair<CborValue, CborValue>>) : CborValue
    data class Bool(val v: Boolean) : CborValue
    data object Null : CborValue
    data class Flt(val v: Double) : CborValue
}

class CborException(message: String) : Exception(message)

object Cbor {
    private const val MAX_DEPTH = 16

    fun encode(value: CborValue): ByteArray {
        val out = ByteArrayOutputStream(128)
        write(out, value)
        return out.toByteArray()
    }

    fun decode(bytes: ByteArray): CborValue {
        val r = Reader(bytes)
        val v = r.read(0)
        if (r.pos != bytes.size) throw CborException("sisa ${bytes.size - r.pos} byte setelah nilai")
        return v
    }

    // ---------------------------------------------------------------- encode

    private fun head(out: ByteArrayOutputStream, major: Int, arg: Long) {
        val m = major shl 5
        when {
            arg < 24 -> out.write(m or arg.toInt())
            arg <= 0xFF -> { out.write(m or 24); out.write(arg.toInt()) }
            arg <= 0xFFFF -> { out.write(m or 25); out.write((arg shr 8).toInt()); out.write(arg.toInt()) }
            arg <= 0xFFFF_FFFFL -> {
                out.write(m or 26)
                for (s in intArrayOf(24, 16, 8, 0)) out.write((arg shr s).toInt())
            }
            else -> {
                out.write(m or 27)
                for (s in intArrayOf(56, 48, 40, 32, 24, 16, 8, 0)) out.write((arg shr s).toInt())
            }
        }
    }

    private fun write(out: ByteArrayOutputStream, v: CborValue) {
        when (v) {
            is CborValue.Uint -> head(out, 0, v.v)
            // -1 - n  → argumen = -1 - v
            is CborValue.NInt -> head(out, 1, -1 - v.v)
            is CborValue.Bytes -> { head(out, 2, v.v.size.toLong()); out.write(v.v) }
            is CborValue.Text -> {
                val b = v.v.toByteArray(Charsets.UTF_8)
                head(out, 3, b.size.toLong()); out.write(b)
            }
            is CborValue.Arr -> { head(out, 4, v.v.size.toLong()); v.v.forEach { write(out, it) } }
            is CborValue.Map -> {
                head(out, 5, v.v.size.toLong())
                v.v.forEach { (k, x) -> write(out, k); write(out, x) }
            }
            is CborValue.Bool -> out.write(if (v.v) 0xF5 else 0xF4)
            CborValue.Null -> out.write(0xF6)
            is CborValue.Flt -> writeFloat(out, v.v)
        }
    }

    private fun writeFloat(out: ByteArrayOutputStream, d: Double) {
        val half = doubleToHalfExact(d)
        if (half != null) {
            out.write(0xF9); out.write(half shr 8); out.write(half and 0xFF)
            return
        }
        val f = d.toFloat()
        if (f.toDouble() == d) {
            val bits = f.toRawBits()
            out.write(0xFA)
            for (s in intArrayOf(24, 16, 8, 0)) out.write(bits shr s and 0xFF)
        } else {
            val bits = d.toRawBits()
            out.write(0xFB)
            for (s in intArrayOf(56, 48, 40, 32, 24, 16, 8, 0)) out.write((bits shr s).toInt() and 0xFF)
        }
    }

    /** Bit f16 bila `d` bisa diwakili tepat, selain itu `null`. */
    internal fun doubleToHalfExact(d: Double): Int? {
        if (d.isNaN()) return 0x7E00
        if (d.isInfinite()) return if (d > 0) 0x7C00 else 0xFC00
        if (d == 0.0) return if (1.0 / d < 0) 0x8000 else 0
        val bits = d.toRawBits()
        val sign = (bits ushr 63).toInt() shl 15
        val exp = ((bits ushr 52) and 0x7FF).toInt() - 1023
        val mant = bits and 0xF_FFFF_FFFF_FFFFL
        if (exp > 15) return null
        if (exp >= -14) {
            // normal f16: 10 bit mantissa, sisanya harus nol
            if (mant and ((1L shl 42) - 1) != 0L) return null
            return sign or ((exp + 15) shl 10) or (mant shr 42).toInt()
        }
        // subnormal f16: nilai = m * 2^-24
        val scaled = Math.abs(d) * (1L shl 24).toDouble()
        val m = scaled.toLong()
        if (m.toDouble() != scaled || m <= 0 || m >= 1024) return null
        return sign or m.toInt()
    }

    internal fun halfToDouble(h: Int): Double {
        val sign = if (h and 0x8000 != 0) -1.0 else 1.0
        val exp = (h shr 10) and 0x1F
        val mant = h and 0x3FF
        return when (exp) {
            0 -> sign * mant * Math.pow(2.0, -24.0)
            31 -> if (mant == 0) sign * Double.POSITIVE_INFINITY else Double.NaN
            else -> sign * (1.0 + mant / 1024.0) * Math.pow(2.0, (exp - 15).toDouble())
        }
    }

    // ---------------------------------------------------------------- decode

    private class Reader(val b: ByteArray) {
        var pos = 0

        private fun need(n: Long) {
            if (n < 0 || pos + n > b.size) throw CborException("data terpotong")
        }

        private fun u8(): Int { need(1); return b[pos++].toInt() and 0xFF }

        private fun uint(bytes: Int): Long {
            need(bytes.toLong())
            var v = 0L
            repeat(bytes) { v = (v shl 8) or (b[pos++].toLong() and 0xFF) }
            return v
        }

        private fun argument(info: Int): Long = when {
            info < 24 -> info.toLong()
            info == 24 -> uint(1)
            info == 25 -> uint(2)
            info == 26 -> uint(4)
            info == 27 -> uint(8).also { if (it < 0) throw CborException("bilangan terlalu besar") }
            else -> throw CborException("panjang tak-terbatas tidak didukung")
        }

        fun read(depth: Int): CborValue {
            if (depth > MAX_DEPTH) throw CborException("terlalu dalam")
            val ib = u8()
            val major = ib shr 5
            val info = ib and 0x1F
            return when (major) {
                0 -> CborValue.Uint(argument(info))
                1 -> CborValue.NInt(-1 - argument(info))
                2 -> {
                    val n = argument(info); need(n)
                    CborValue.Bytes(b.copyOfRange(pos, pos + n.toInt()).also { pos += n.toInt() })
                }
                3 -> {
                    val n = argument(info); need(n)
                    CborValue.Text(String(b, pos, n.toInt(), Charsets.UTF_8).also { pos += n.toInt() })
                }
                4 -> {
                    val n = argument(info)
                    // Panjang yang dideklarasikan tidak boleh melebihi sisa byte (1 byte per elemen minimum).
                    if (n > b.size - pos) throw CborException("array melebihi data")
                    CborValue.Arr(List(n.toInt()) { read(depth + 1) })
                }
                5 -> {
                    val n = argument(info)
                    if (n > (b.size - pos) / 2) throw CborException("map melebihi data")
                    CborValue.Map(List(n.toInt()) { read(depth + 1) to read(depth + 1) })
                }
                7 -> when (info) {
                    20 -> CborValue.Bool(false)
                    21 -> CborValue.Bool(true)
                    22, 23 -> CborValue.Null
                    25 -> CborValue.Flt(halfToDouble(uint(2).toInt()))
                    26 -> CborValue.Flt(java.lang.Float.intBitsToFloat(uint(4).toInt()).toDouble())
                    27 -> CborValue.Flt(java.lang.Double.longBitsToDouble(uint(8)))
                    else -> throw CborException("nilai sederhana $info tidak didukung")
                }
                else -> throw CborException("tag CBOR tidak didukung")
            }
        }
    }
}
