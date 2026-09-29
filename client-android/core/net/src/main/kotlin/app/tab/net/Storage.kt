package app.tab.net

import app.tab.model.Id16

/**
 * Host yang pernah dipasangkan: kunci publik yang **dipin** dan token resume.
 *
 * Kunci publik yang dipin tidak boleh berubah diam-diam: bila host yang menjawab discovery
 * membawa kunci lain untuk `hid` yang sama, klien menolaknya (bisa jadi host palsu).
 */
class HostRecord(
    val hid: Id16,
    val name: String,
    val publicKey: ByteArray,
    /** `null` setelah token dicabut: perangkat harus dipasangkan ulang. */
    val token: ByteArray?,
    val address: String,
    val port: Int,
) {
    fun copy(
        name: String = this.name,
        token: ByteArray? = this.token,
        address: String = this.address,
        port: Int = this.port,
    ) = HostRecord(hid, name, publicKey, token, address, port)

    // Token tidak pernah ikut ke log.
    override fun toString() = "HostRecord($name, $address:$port, ${if (token != null) "dipasangkan" else "belum dipasangkan"})"
}

interface HostStorage {
    /** UUID acak yang dibuat sekali saat instalasi. */
    fun deviceId(): Id16
    fun hosts(): List<HostRecord>
    fun host(hid: Id16): HostRecord? = hosts().firstOrNull { it.hid == hid }
    fun save(record: HostRecord)
    fun remove(hid: Id16)
}

/** Untuk test dan pratinjau. */
class MemoryHostStorage(private val device: Id16 = Id16.random()) : HostStorage {
    private val map = LinkedHashMap<Id16, HostRecord>()
    override fun deviceId() = device
    override fun hosts(): List<HostRecord> = synchronized(map) { map.values.toList() }
    override fun save(record: HostRecord) { synchronized(map) { map[record.hid] = record } }
    override fun remove(hid: Id16) { synchronized(map) { map.remove(hid) } }
}
