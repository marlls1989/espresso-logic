fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "wasm32" {
        println!("cargo:rustc-link-arg=-s");
        println!("cargo:rustc-link-arg=EXPORTED_FUNCTIONS=[\"_minimise_expressions\",\"_free_string\",\"_main\"]");
        println!("cargo:rustc-link-arg=-s");
        println!("cargo:rustc-link-arg=EXPORTED_RUNTIME_METHODS=[\"ccall\",\"cwrap\",\"UTF8ToString\",\"stringToUTF8\"]");
    }
}

