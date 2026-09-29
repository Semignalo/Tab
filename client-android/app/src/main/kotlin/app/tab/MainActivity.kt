package app.tab

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.tab.ui.TabRoot
import app.tab.ui.kit.TabTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as TabApplication
        setContent {
            TabTheme {
                TabRoot(app)
            }
        }
    }
}
