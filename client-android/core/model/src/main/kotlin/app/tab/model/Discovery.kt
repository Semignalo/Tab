package app.tab.model

/** Datagram discovery UDP 4179 (PROTOCOL.md §2). Tidak terenkripsi. */
object Discovery {
    const val PORT = 4179
    const val PROTOCOL_VERSION = 1
    const val MAX_DATAGRAM = 1200
    private val MAGIC = byteArrayOf('T'.code.toByte(), 'A'.code.toByte(), 'B'.code.toByte(), 'D'.code.toByte())
    private const val WIRE_VERSION = 1
    private const val KIND_REQUEST = 0x01
    private const val KIND_RESPONSE = 0x02
    private const val HEADER = 6

    data class Response(
        val hid: Id16, val name: String, val os: String, val osv: String, val app: String,
        val pmin: Int, val pmax: Int, val port: Int, val fp: ByteArray, val pk: ByteArray,
        val known: Boolean,
    ) {
        override fun equals(other: Any?) = other is Response && hid == other.hid && name == other.name &&
            port == other.port && fp.contentEquals(other.fp) && pk.contentEquals(other.pk) && known == other.known
        override fun hashCode() = hid.hashCode()
    }

    fun encodeRequest(device: Id16, pv: Int = PROTOCOL_VERSION): ByteArray =
        wrap(KIND_REQUEST, map { id("dev", device); uint("pv", pv) })

    fun encodeResponse(r: Response): ByteArray = wrap(KIND_RESPONSE, map {
        id("hid", r.hid); text("name", r.name); text("os", r.os); text("osv", r.osv)
        text("app", r.app); uint("pmin", r.pmin); uint("pmax", r.pmax); uint("port", r.port)
        bytes("fp", r.fp); bytes("pk", r.pk); bool("known", r.known)
    })

    private fun wrap(kind: Int, body: CborValue): ByteArray {
        val payload = Cbor.encode(body)
        val out = ByteArray(HEADER + payload.size)
        MAGIC.copyInto(out)
        out[4] = WIRE_VERSION.toByte()
        out[5] = kind.toByte()
        payload.copyInto(out, HEADER)
        require(out.size <= MAX_DATAGRAM) { "datagram melebihi $MAX_DATAGRAM byte" }
        return out
    }

    /**
     * Baca balasan host. `null` untuk apa pun yang bukan balasan sah — datagram asing
     * dibuang tanpa reaksi.
     */
    fun decodeResponse(d: ByteArray, len: Int = d.size): Response? {
        if (len <= HEADER || len > MAX_DATAGRAM) return null
        for (i in 0 until 4) if (d[i] != MAGIC[i]) return null
        if (d[4].toInt() != WIRE_VERSION || d[5].toInt() != KIND_RESPONSE) return null
        return try {
            val f = Fields.asFields(Cbor.decode(d.copyOfRange(HEADER, len)), "DiscoverResponse")
            Response(
                f.id("hid"), f.str("name"), f.str("os"), f.str("osv"), f.str("app"),
                f.int("pmin"), f.int("pmax"), f.int("port"), f.bytes("fp"), f.bytes("pk"),
                f.bool("known"),
            )
        } catch (_: CborException) {
            null
        } catch (_: ProtocolException) {
            null
        }
    }
}
