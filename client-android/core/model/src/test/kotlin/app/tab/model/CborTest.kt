package app.tab.model

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals

class CborTest {
    private fun enc(v: CborValue) = Cbor.encode(v)
    private fun hex(vararg b: Int) = ByteArray(b.size) { b[it].toByte() }

    @Test
    fun integersUseShortestForm() {
        assertContentEquals(hex(0x00), enc(CborValue.Uint(0)))
        assertContentEquals(hex(0x17), enc(CborValue.Uint(23)))
        assertContentEquals(hex(0x18, 0x18), enc(CborValue.Uint(24)))
        assertContentEquals(hex(0x19, 0x01, 0x00), enc(CborValue.Uint(256)))
        assertContentEquals(hex(0x1a, 0x00, 0x01, 0xe2, 0x40), enc(CborValue.Uint(123456)))
        assertContentEquals(hex(0x1b, 0, 0, 0, 2, 0x18, 0x71, 0x1a, 0), enc(CborValue.Uint(0x2_1871_1a00L)))
        assertContentEquals(hex(0x20), enc(CborValue.NInt(-1)))
        assertContentEquals(hex(0x38, 0x63), enc(CborValue.NInt(-100)))
    }

    @Test
    fun floatsUseShortestLosslessWidthLikeCiborium() {
        // Sama seperti ciborium: nilai yang tepat di f16 memakai f16; sisanya f32, lalu f64.
        assertContentEquals(hex(0xf9, 0x42, 0x00), enc(CborValue.Flt(3.0)))
        assertContentEquals(hex(0xf9, 0x41, 0x40), enc(CborValue.Flt(2.625)))
        assertContentEquals(hex(0xf9, 0x3c, 0x00), enc(CborValue.Flt(1.0)))
        assertContentEquals(hex(0xf9, 0x00, 0x00), enc(CborValue.Flt(0.0)))
        assertContentEquals(hex(0xf9, 0xc1, 0x00), enc(CborValue.Flt(-2.5)))
        // 0.1f tidak muat f16 → f32
        assertContentEquals(hex(0xfa, 0x3d, 0xcc, 0xcc, 0xcd), enc(CborValue.Flt(0.1f.toDouble())))
        // 0.1 (double) tidak muat f32 → f64
        assertEquals(0xfb, enc(CborValue.Flt(0.1))[0].toInt() and 0xff)
        // 65504 = f16 maksimum; 65520 melebihi
        assertContentEquals(hex(0xf9, 0x7b, 0xff), enc(CborValue.Flt(65504.0)))
        assertEquals(0xfa, enc(CborValue.Flt(65520.0))[0].toInt() and 0xff)
        // subnormal terkecil f16
        assertContentEquals(hex(0xf9, 0x00, 0x01), enc(CborValue.Flt(Math.pow(2.0, -24.0))))
    }

    @Test
    fun floatsRoundTripThroughEveryWidth() {
        val samples = doubleArrayOf(0.0, 1.0, -1.0, 0.5, 2.625, 1080.0, 0.1, 3.14159, 1e10, -65504.0, 6.1e-5, 1e-7)
        for (d in samples) {
            val back = Cbor.decode(enc(CborValue.Flt(d))) as CborValue.Flt
            assertEquals(d, back.v, "roundtrip $d")
        }
    }

    @Test
    fun halfDecodingCoversNormalSubnormalAndSpecials() {
        assertEquals(1.0, Cbor.halfToDouble(0x3c00))
        assertEquals(-2.0, Cbor.halfToDouble(0xc000))
        assertEquals(65504.0, Cbor.halfToDouble(0x7bff))
        assertEquals(Math.pow(2.0, -24.0), Cbor.halfToDouble(0x0001))
        assertEquals(Double.POSITIVE_INFINITY, Cbor.halfToDouble(0x7c00))
        assertEquals(true, Cbor.halfToDouble(0x7e00).isNaN())
    }

    @Test
    fun stringsBytesAndContainersRoundTrip() {
        val v = CborValue.Map(
            listOf(
                CborValue.Text("t") to CborValue.Text("Halo dunia ✓"),
                CborValue.Text("b") to CborValue.Bytes(ByteArray(300) { it.toByte() }),
                CborValue.Text("a") to CborValue.Arr(listOf(CborValue.Bool(true), CborValue.Null, CborValue.NInt(-5))),
            ),
        )
        assertEquals(v, Cbor.decode(enc(v)))
    }
}
