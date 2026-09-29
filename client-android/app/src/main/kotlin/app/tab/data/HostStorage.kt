package app.tab.data

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tab.model.Id16
import app.tab.net.HostRecord
import app.tab.net.HostStorage
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Enkripsi token di penyimpanan. Produksi memakai Android Keystore; test memakai versi polos. */
interface TokenCipher {
    fun encrypt(plain: ByteArray): String
    fun decrypt(encoded: String): ByteArray
}

/**
 * AES-256-GCM dengan kunci di Android Keystore: kunci tidak pernah keluar dari perangkat keras
 * aman, sehingga token tersimpan tidak bisa disalin lewat backup atau akses berkas biasa.
 */
class KeystoreTokenCipher(private val alias: String = "tab.token.v1") : TokenCipher {
    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return gen.generateKey()
    }

    override fun encrypt(plain: ByteArray): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        val ct = c.doFinal(plain)
        return Base64.encodeToString(c.iv + ct, Base64.NO_WRAP)
    }

    override fun decrypt(encoded: String): ByteArray {
        val raw = Base64.decode(encoded, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw, 0, 12))
        return c.doFinal(raw, 12, raw.size - 12)
    }
}

/** (De)serialisasi daftar host ke JSON; token selalu lewat [TokenCipher]. Murni, mudah dites. */
object HostCodec {
    fun encode(hosts: List<HostRecord>, cipher: TokenCipher): String {
        val arr = JSONArray()
        for (h in hosts) {
            arr.put(
                JSONObject()
                    .put("hid", h.hid.toHex())
                    .put("name", h.name)
                    .put("pk", h.publicKey.toHex())
                    .put("token", h.token?.let { cipher.encrypt(it) } ?: JSONObject.NULL)
                    .put("addr", h.address)
                    .put("port", h.port),
            )
        }
        return arr.toString()
    }

    fun decode(json: String, cipher: TokenCipher): List<HostRecord> {
        val arr = JSONArray(json)
        val out = ArrayList<HostRecord>()
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            val hid = Id16.fromHex(o.getString("hid")) ?: continue
            val token = if (o.isNull("token")) null else try {
                cipher.decrypt(o.getString("token"))
            } catch (_: Exception) {
                // Kunci Keystore hilang (mis. data dipulihkan ke perangkat lain): anggap belum
                // dipasangkan, jangan crash.
                null
            }
            out += HostRecord(hid, o.getString("name"), o.getString("pk").fromHex(), token, o.getString("addr"), o.getInt("port"))
        }
        return out
    }

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }
    private fun String.fromHex() = ByteArray(length / 2) { substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}

class PrefsHostStorage(context: Context, private val cipher: TokenCipher = KeystoreTokenCipher()) : HostStorage {
    private val prefs: SharedPreferences = context.getSharedPreferences("tab", Context.MODE_PRIVATE)

    @Synchronized
    override fun deviceId(): Id16 {
        prefs.getString(KEY_DEVICE, null)?.let { s -> Id16.fromHex(s)?.let { return it } }
        val id = Id16.random()
        prefs.edit().putString(KEY_DEVICE, id.toHex()).apply()
        return id
    }

    @Synchronized
    override fun hosts(): List<HostRecord> {
        val json = prefs.getString(KEY_HOSTS, null) ?: return emptyList()
        return try {
            HostCodec.decode(json, cipher)
        } catch (_: Exception) {
            emptyList()
        }
    }

    @Synchronized
    override fun save(record: HostRecord) {
        val next = hosts().filter { it.hid != record.hid } + record
        prefs.edit().putString(KEY_HOSTS, HostCodec.encode(next, cipher)).apply()
    }

    @Synchronized
    override fun remove(hid: Id16) {
        prefs.edit().putString(KEY_HOSTS, HostCodec.encode(hosts().filter { it.hid != hid }, cipher)).apply()
    }

    fun guideSeen(): Boolean = prefs.getBoolean(KEY_GUIDE, false)
    fun setGuideSeen() = prefs.edit().putBoolean(KEY_GUIDE, true).apply()

    private companion object {
        const val KEY_DEVICE = "device_id"
        const val KEY_HOSTS = "hosts"
        const val KEY_GUIDE = "guide_seen"
    }
}
