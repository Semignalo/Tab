plugins {
    alias(libs.plugins.kotlin.jvm)
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    testImplementation(libs.junit)
    testImplementation(libs.kotlin.test)
}

tasks.test {
    // Uji kontrak membaca fixture yang dihasilkan sisi Rust.
    systemProperty("tab.fixtures", rootProject.projectDir.resolve("../protocol/fixtures").absolutePath)
}
