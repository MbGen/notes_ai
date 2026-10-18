plugins { id("com.android.application") }
android {
    namespace = "com.notesai.phone"
    compileSdk = 35
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    defaultConfig {
        applicationId = "com.notesai.phone"
        minSdk = 26
        targetSdk = 35
        versionCode = 4
        versionName = "0.4"
    }
}

dependencies {
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
