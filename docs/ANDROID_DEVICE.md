# Verruckt / Orbit - Android Physical Device Port Guide

This document describes the architecture, toolchain, signing, deployment, and LAN live development workflow for running the Verruckt / Orbit Tauri 2 application on physical Android devices.

---

## 1. Prerequisites & Toolchain

### 1.1 Development Host (Windows)
- **Node.js**: v20+ / npm v10+
- **JDK**: OpenJDK 17+ (e.g., `Microsoft OpenJDK 17`)
- **Android SDK**: Build-Tools `35.0.0`+, Platforms `android-35` / `android-36`
- **Android NDK**: `27.0.12077973`+
- **Rust**: 1.80+ with target `aarch64-linux-android`
  ```powershell
  rustup target add aarch64-linux-android
  ```

### 1.2 Target Android Device
- **Architecture**: `arm64-v8a` (`aarch64`)
- **OS Version**: Android 7.0+ (API 24+, target API 36)
- **Settings**:
  1. Go to **Settings -> About phone** and tap **Build number** 7 times to enable Developer Options.
  2. In **Developer options**, enable **USB debugging** (and **Install via USB** if present).
  3. When connected via USB, check **Always allow from this computer** on the RSA fingerprint prompt.

---

## 2. Environment Variables & Workaround Strategy

Because Windows paths may contain non-ASCII characters or excessive parallelism may cause NDK clang heap initialization errors (`0xc0000142`), always use the following configurations:

```powershell
# 1. Restrict parallel compiler jobs to prevent Windows heap exhaustion
$env:CARGO_BUILD_JOBS = "4"

# 2. Direct cargo output to pure ASCII path to avoid NDK ld.lld non-ASCII path encoding issues
$env:CARGO_TARGET_DIR = "C:\verruckt-target"

# 3. Explicit toolchain paths
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-17.0.10.7-hotspot"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\27.0.12077973"

# 4. Console code page
chcp 65001
```

> **Warning**: Never use `subst` virtual drives (e.g. `subst V: ...`) with the Tauri CLI. Tauri CLI's asset resolution relies on `pathdiff::diff_paths` which panics when comparing UNC paths and virtual drive letters.

---

## 3. Preflight Environment Check

Run the automated verification script:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/android-check.ps1
```

This verifies:
- Java JDK 17+
- Android SDK platforms & build-tools
- Android NDK clang executable
- Rust target `aarch64-linux-android`
- PC LAN IPv4 address
- ADB connection & connected device properties (`model`, `release`, `abi`)

---

## 4. Release Keystore & Security Design

### 4.1 Keystore Isolation
The production release signing keystore is strictly kept **outside** the source code repository:
- **Keystore Path**: `%USERPROFILE%\.verruckt\android\verruckt-release.jks`
- **Keystore Properties**: `src-tauri/gen/android/keystore.properties`
- **Git Protection**: `keystore.properties` and `*.jks` are strictly listed in both `.gitignore` and `src-tauri/gen/android/.gitignore`.
- **Algorithm**: RSA 2048-bit, SHA256withRSA.

### 4.2 Mobile Safe Secret Storage
On desktop platforms, credentials use the OS Keyring. On Android, `EncryptedSecretStore` provides:
- AES-256-GCM authenticated encryption (`ring::aead`).
- Master key (`secure/vault.key`) and encrypted payload (`secure/vault.enc`) placed in the Android internal app data directory (`/data/data/app.verruckt.desktop/files/secure/`), sandboxed by Linux UID permissions.
- Zero plaintext fallback.

---

## 5. Building the Signed Release APK

```powershell
# In PowerShell:
chcp 65001
$env:CARGO_BUILD_JOBS = "4"
$env:CARGO_TARGET_DIR = "C:\verruckt-target"
$env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-17.0.10.7-hotspot"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\27.0.12077973"

npm run tauri android build -- --apk --target aarch64
```

### 5.1 Signature & Alignment Verification
Verify the generated APK with Android build-tools:

```powershell
$buildTools = "$env:LOCALAPPDATA\Android\Sdk\build-tools\35.0.0"
$apk = "src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk"

# Verify 4-byte zip alignment
& "$buildTools\zipalign.exe" -c -v 4 $apk

# Verify signature scheme v1, v2, v3
& "$buildTools\apksigner.bat" verify --verbose --print-certs $apk
```

Expected output:
- `Verified using v1 scheme (JAR signing): true`
- `Verified using v2 scheme (APK Signature Scheme v2): true`
- `Verified using v3 scheme (APK Signature Scheme v3): true`
- `Signer #1 certificate DN: CN=Verruckt, OU=Mobile, O=Verruckt, ...`

---

## 6. Device Deployment & Launch

Connect phone via USB and run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/android-install.ps1
```

Or execute manually:

```powershell
# 1. Install signed APK
adb install -r src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk

# 2. Launch main activity
adb shell am start -n app.verruckt.desktop/app.verruckt.desktop.MainActivity

# 3. Monitor logcat
adb logcat --pid=$(adb shell pidof app.verruckt.desktop)
```

---

## 7. LAN Wi-Fi Live Development & HMR

Verruckt supports live development over the local Wi-Fi network without relying on `adb reverse`.

### 7.1 Architecture
- **Dev Server**: `http://<PC_LAN_IP>:1420`
- **HMR WebSocket**: `ws://<PC_LAN_IP>:1421`
- **Firewall**: Inbound TCP ports 1420 and 1421 must be allowed through Windows Defender Firewall.
- **Android Manifest**: Cleartext HTTP traffic is explicitly enabled for debug builds (`usesCleartextTraffic = "true"` in debug `buildTypes`).

### 7.2 Starting LAN Dev
Ensure the Android phone and PC are connected to the same Wi-Fi network (or phone is connected to PC mobile hotspot):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/android-dev-lan.ps1
```

Or manually:

```powershell
$lanIp = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -match "以太网|Wi-Fi" }).IPAddress
$env:TAURI_DEV_HOST = $lanIp
npm run tauri android dev -- --host $lanIp
```

---

## 8. Mobile Smoke Validation Checklist

| Item | Test Scenario | Expected Result | Pass/Fail |
|------|---------------|-----------------|-----------|
| 1 | Cold Launch | App launches within 3 seconds, WebView loads dashboard | [ ] |
| 2 | Roadmap View | Graph renders without canvas glitch or memory leak | [ ] |
| 3 | Lesson View | Content markdown / code blocks render correctly | [ ] |
| 4 | Learning Session | Timer, flashcards, and progress tracking update state | [ ] |
| 5 | Composer View | Text input, tags, submit trigger Rust command IPC | [ ] |
| 6 | Soft Keyboard | Screen smoothly resizes when keyboard opens; input remains visible | [ ] |
| 7 | Safe Area | Status bar and navigation bar padding applied (`env(safe-area-inset-*)`) | [ ] |
| 8 | SQLite Persistence | Changes saved to SQLite survive application force-stop and restart | [ ] |
| 9 | Wi-Fi HMR | Vite HMR updates UI components across Wi-Fi within 1s without full reload | [ ] |
