fn main() {
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let configured = std::env::var("VERRUCKT_DEFAULT_CLOUD_URL")
            .or_else(|_| std::env::var("VERRUCKT_CLOUD_PRODUCTION_URL"))
            .unwrap_or_default();
        let value = configured.trim().to_ascii_lowercase();
        let valid = value.starts_with("https://")
            || value.starts_with("http://localhost")
            || value.starts_with("http://127.0.0.1");
        if !valid {
            panic!(
                "release build requires VERRUCKT_DEFAULT_CLOUD_URL (or VERRUCKT_CLOUD_PRODUCTION_URL) to be an https:// endpoint; localhost is allowed only for development"
            );
        }
    }
    #[cfg(feature = "desktop")]
    tauri_build::build()
}
