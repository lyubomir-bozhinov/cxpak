fn main() {
    // Vendored libgit2 on Windows needs these system libraries linked explicitly
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        for lib in [
            "advapi32", "crypt32", "ole32", "secur32", "ws2_32", "user32",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
