import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "io.github.mustafabayramguven.readcrab"
    compileSdk = 35
    defaultConfig {
        applicationId = "io.github.mustafabayramguven.readcrab"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets.getByName("main") {
        // cargo-ndk çıktısı src/main/jniLibs'e gider (varsayılan konum)
        java.srcDir(layout.buildDirectory.dir("generated/uniffi/kotlin")) // CI'da üretilen UniFFI Kotlin binding
        assets.srcDir(layout.buildDirectory.dir("webAssets"))             // web/ yazı tipleri ve CSS (kopya depoya girmez)
    }
}

kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }

// Okuyucu için web/ altından yalnız gerekenler assets köküne (=> /static/...) gider
val syncWebAssets by tasks.registering(Sync::class) {
    from(rootProject.projectDir.resolve("../web")) {
        include("frame.css", "reader-fonts.css", "fonts/**")
        exclude("fonts/roboto-flex-*")
    }
    into(layout.buildDirectory.dir("webAssets"))
}
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }
    .configureEach { dependsOn(syncWebAssets) }

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.05.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.navigation:navigation-compose:2.8.9")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.work:work-runtime-ktx:2.10.1")
    implementation("androidx.webkit:webkit:1.12.1")
    implementation("io.coil-kt.coil3:coil-compose:3.2.0")
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    testImplementation("junit:junit:4.13.2")
}
