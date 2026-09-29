pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "tab-client"

include(
    ":app",
    ":core:model",
    ":core:net",
    ":feature:trackpad",
    ":feature:deck",
    ":feature:monitor",
    ":feature:music",
    ":feature:clock",
)
