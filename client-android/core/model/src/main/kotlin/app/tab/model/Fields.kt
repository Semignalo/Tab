package app.tab.model

/** Kesalahan struktur pesan: tag dikenal tetapi isinya tidak sesuai kontrak. */
class ProtocolException(message: String) : Exception(message)

/** Pembangun map CBOR berurutan; urutan pemanggilan = urutan key di wire. */
class MapBuilder {
    private val entries = ArrayList<Pair<CborValue, CborValue>>()

    fun put(key: String, value: CborValue) {
        entries += CborValue.Text(key) to value
    }

    fun text(key: String, v: String) = put(key, CborValue.Text(v))
    fun uint(key: String, v: Long) {
        require(v >= 0) { "$key negatif" }
        put(key, CborValue.Uint(v))
    }
    fun uint(key: String, v: Int) = uint(key, v.toLong())
    fun bool(key: String, v: Boolean) = put(key, CborValue.Bool(v))
    fun f32(key: String, v: Float) = put(key, CborValue.Flt(v.toDouble()))
    fun bytes(key: String, v: ByteArray) = put(key, CborValue.Bytes(v))
    fun id(key: String, v: Id16) = bytes(key, v.bytes)

    fun optText(key: String, v: String?) { if (v != null) text(key, v) }
    fun optUint(key: String, v: Long?) { if (v != null) uint(key, v) }
    fun optF32(key: String, v: Float?) { if (v != null) f32(key, v) }
    fun optBool(key: String, v: Boolean?) { if (v != null) bool(key, v) }

    fun build() = CborValue.Map(entries)
}

fun map(block: MapBuilder.() -> Unit): CborValue.Map = MapBuilder().apply(block).build()

fun tagged(tag: String, block: MapBuilder.() -> Unit): CborValue.Map =
    MapBuilder().apply { text("t", tag); block() }.build()

/** Akses bertipe ke sebuah map CBOR dengan key teks. */
class Fields(private val m: CborValue.Map) {
    private val index: Map<String, CborValue> = buildMap {
        for ((k, v) in m.v) if (k is CborValue.Text) put(k.v, v)
    }

    fun has(key: String) = key in index

    private fun req(key: String): CborValue =
        index[key] ?: throw ProtocolException("field '$key' hilang")

    fun value(key: String): CborValue = req(key)
    fun optValue(key: String): CborValue? = index[key]?.takeUnless { it == CborValue.Null }

    fun long(key: String): Long = when (val v = req(key)) {
        is CborValue.Uint -> v.v
        else -> throw ProtocolException("field '$key' bukan uint")
    }

    fun int(key: String): Int = long(key).also {
        if (it > Int.MAX_VALUE) throw ProtocolException("field '$key' terlalu besar")
    }.toInt()

    fun optLong(key: String): Long? = optValue(key)?.let { long(key) }

    fun str(key: String): String = (req(key) as? CborValue.Text)?.v
        ?: throw ProtocolException("field '$key' bukan teks")

    fun optStr(key: String): String? = optValue(key)?.let { str(key) }

    fun bool(key: String): Boolean = (req(key) as? CborValue.Bool)?.v
        ?: throw ProtocolException("field '$key' bukan bool")

    fun optBool(key: String): Boolean? = optValue(key)?.let { bool(key) }

    fun f32(key: String): Float = toFloat(req(key), key)

    fun optF32(key: String): Float? = optValue(key)?.let { toFloat(it, key) }

    private fun toFloat(v: CborValue, key: String): Float = when (v) {
        is CborValue.Flt -> v.v.toFloat()
        // Rust dapat menuliskan float bulat sebagai integer? tidak, tetapi bersikap toleran.
        is CborValue.Uint -> v.v.toFloat()
        else -> throw ProtocolException("field '$key' bukan float")
    }

    fun bytes(key: String): ByteArray = (req(key) as? CborValue.Bytes)?.v
        ?: throw ProtocolException("field '$key' bukan bytes")

    fun id(key: String): Id16 = Id16.of(bytes(key))

    fun list(key: String): List<CborValue> = (req(key) as? CborValue.Arr)?.v
        ?: throw ProtocolException("field '$key' bukan array")

    fun fields(key: String): Fields = asFields(req(key), key)

    fun <T> listOf(key: String, f: (Fields) -> T): List<T> =
        list(key).map { f(asFields(it, key)) }

    fun strings(key: String): List<String> = list(key).map {
        (it as? CborValue.Text)?.v ?: throw ProtocolException("elemen '$key' bukan teks")
    }

    fun floats(key: String): List<Float> = list(key).map { toFloat(it, key) }

    companion object {
        fun asFields(v: CborValue, what: String): Fields =
            Fields(v as? CborValue.Map ?: throw ProtocolException("'$what' bukan map"))
    }
}

/** Identifier 16 byte (device id, host id, session id). */
class Id16 private constructor(val bytes: ByteArray) {
    fun toHex(): String = bytes.joinToString("") { "%02x".format(it) }

    override fun equals(other: Any?) = other is Id16 && bytes.contentEquals(other.bytes)
    override fun hashCode() = bytes.contentHashCode()
    override fun toString() = "Id16(${toHex()})"

    companion object {
        fun of(b: ByteArray): Id16 {
            if (b.size != 16) throw ProtocolException("id harus 16 byte, dapat ${b.size}")
            return Id16(b.copyOf())
        }

        fun fromHex(s: String): Id16? {
            if (s.length != 32) return null
            val out = ByteArray(16)
            for (i in 0 until 16) {
                out[i] = s.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte() ?: return null
            }
            return Id16(out)
        }

        fun random(): Id16 = Id16(ByteArray(16).also { java.security.SecureRandom().nextBytes(it) })
    }
}
