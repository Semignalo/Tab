plugins {
    alias(libs.plugins.kotlin.jvm)
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    api(project(":core:model"))
    api(libs.kotlinx.coroutines.core)
    implementation(libs.bouncycastle)
    testImplementation(libs.junit)
    testImplementation(libs.kotlin.test)
    testImplementation(libs.kotlinx.coroutines.test)
}

tasks.test {
    // Uji lintas-bahasa menjalankan host Rust sungguhan (`cargo build -p tab-host --bins` dulu).
    val debug = rootProject.projectDir.resolve("../target/debug")
    val exe = debug.resolve("tab-testhost.exe").takeIf { it.exists() } ?: debug.resolve("tab-testhost")
    systemProperty("tab.testhost", exe.absolutePath)
    testLogging {
        events("failed", "skipped")
        showStandardStreams = false
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}
