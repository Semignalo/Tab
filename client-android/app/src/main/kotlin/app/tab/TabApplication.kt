package app.tab

import android.app.Application
import android.os.Build
import app.tab.data.PrefsHostStorage
import app.tab.net.DeviceProfile
import app.tab.net.TabClient
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * Pemegang klien tunggal. Sambungan hidup selama proses hidup — bukan selama satu Activity —
 * sehingga memutar layar atau berpindah aplikasi sesaat tidak memutus sesi.
 */
class TabApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    lateinit var storage: PrefsHostStorage
        private set
    lateinit var client: TabClient
        private set
    lateinit var discovery: DiscoveryModel
        private set

    override fun onCreate() {
        super.onCreate()
        storage = PrefsHostStorage(this)
        val dm = resources.displayMetrics
        val version = runCatching { packageManager.getPackageInfo(packageName, 0).versionName }.getOrNull() ?: "0"
        client = TabClient(
            scope = scope,
            storage = storage,
            device = DeviceProfile(
                name = Build.MODEL ?: "Android",
                platformVersion = Build.VERSION.RELEASE ?: "",
                appVersion = version,
                screenWidth = (dm.widthPixels / dm.density).toInt(),
                screenHeight = (dm.heightPixels / dm.density).toInt(),
                dpi = dm.density,
            ),
        )
        discovery = DiscoveryModel(storage.deviceId())

        // Buka aplikasi → langsung tersambung ke host yang sudah dipasangkan; user tidak
        // pernah mengetik alamat IP.
        storage.hosts().firstOrNull { it.token != null }?.let { client.connect(it.hid) }
    }
}
