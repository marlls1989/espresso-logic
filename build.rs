// Build script for espresso-logic
//
// This script compiles the C code from the original Espresso implementation
// and generates Rust FFI bindings.
//
// ## WebAssembly/Emscripten Support
//
// To build for WebAssembly, use the wasm32-unknown-emscripten target:
//   cargo build --target wasm32-unknown-emscripten
//
// Requirements:
// - Emscripten SDK installed and activated
// - EMSDK environment variable set (should be set by emsdk_env.sh)
//
// Note: The wasm32-unknown-unknown target is NOT supported due to the
// C code requiring libc functions. Use Emscripten instead.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    // Compile lalrpop grammar files to OUT_DIR for cargo publish compatibility
    lalrpop::process_root().unwrap();

    let espresso_src = PathBuf::from("espresso-src");
    let target = env::var("TARGET").unwrap();
    let is_emscripten = target == "wasm32-unknown-emscripten";

    // Opt-in override of the packed-set word width (BPI), for testing both widths on a single
    // host. Unset (the default) leaves espresso.h's UINTPTR_MAX self-detection alone; cc and
    // bindgen then agree on the native width solely via the target triple (bindgen derives
    // --target from cargo's TARGET env; the emscripten path pins its own triple below).
    println!("cargo:rerun-if-env-changed=ESPRESSO_BPI");
    let bpi_override = env::var("ESPRESSO_BPI").ok();
    if let Some(bpi) = &bpi_override {
        if bpi != "32" && bpi != "64" {
            panic!("ESPRESSO_BPI must be 32 or 64");
        }
    }

    println!("cargo:rerun-if-changed=espresso-src");
    // Regenerate the parser when the grammar changes. Without this, the explicit `rerun-if-changed`
    // above suppresses cargo's default "rerun on any change", so grammar edits would be missed.
    println!("cargo:rerun-if-changed=src/expression/bool_expr.lalrpop");
    // Re-run when the manual clang-args override changes, so toggling it re-discovers (or stops
    // discovering) the resource directory below.
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");

    // Get all C source files except main.c (we'll use this as a library)
    let c_files = vec![
        "black_white.c",
        "canonical.c",
        "cofactor.c",
        "cols.c",
        "compl.c",
        "contain.c",
        "cpu_time.c",
        "cubestr.c",
        "cvrin.c",
        "cvrm.c",
        "cvrmisc.c",
        "cvrout.c",
        "dominate.c",
        "equiv.c",
        "espresso.c",
        "essen.c",
        "essentiality.c",
        "exact.c",
        "expand.c",
        "gasp.c",
        "gimpel.c",
        "globals.c",
        "hack.c",
        "indep.c",
        "irred.c",
        "map.c",
        "matrix.c",
        "mincov.c",
        "opo.c",
        "pair.c",
        "part.c",
        "primes.c",
        "prtime.c",
        "reduce.c",
        "rows.c",
        "set.c",
        "setc.c",
        "sharp.c",
        "sigma.c",
        "signature_exact.c",
        "signature.c",
        "sminterf.c",
        "solution.c",
        "sparse.c",
        "strdup.c",
        "thread_local_accessors.c",
        "unate.c",
        "util_signature.c",
        "verify.c",
    ];

    let mut build = cc::Build::new();

    // Add all C files
    for file in &c_files {
        build.file(espresso_src.join(file));
    }

    // Add include directory
    build.include(&espresso_src);

    // Set compiler flags
    //
    // C11 `_Thread_local` storage is required (see thread_local_accessors.c/.h) regardless of which
    // compiler is driving the build, but the flag spelling differs: GCC/Clang want `-std=c11`, while
    // MSVC's `cl.exe` wants `/std:c11` (and errors out on `-std=c11`). Probe both and let the one the
    // active compiler understands win.
    build
        .flag_if_supported("-std=c11")
        .flag_if_supported("/std:c11")
        .flag_if_supported("-w"); // Suppress warnings from C code

    // Detect and enable AddressSanitizer if requested
    // IMPORTANT: Only enable if Rust is also being compiled with ASan
    // Otherwise linking will fail
    let enable_asan = env::var("RUSTFLAGS")
        .map(|flags| flags.contains("-Z sanitizer=address"))
        .unwrap_or(false)
        || env::var("CARGO_ENCODED_RUSTFLAGS")
            .map(|flags| flags.contains("-Z sanitizer=address"))
            .unwrap_or(false);

    if enable_asan {
        println!("cargo:warning=Building C code with AddressSanitizer enabled for leak detection");
        build
            .flag("-fsanitize=address")
            .flag("-fno-omit-frame-pointer")
            .flag("-g"); // Debug symbols for better stack traces

        // Tell cargo to link with ASan runtime
        println!("cargo:rustc-link-arg=-fsanitize=address");
    } else {
        build.opt_level(2);
    }

    // Support for cargo-zigbuild
    // Zig provides a better C compiler with excellent cross-compilation support
    if env::var("CARGO_CFG_TARGET_ENV").is_ok() {
        // When using zigbuild, Zig's compiler is already configured
        // Just ensure we have the right flags
        build.flag_if_supported("-fno-sanitize=undefined");
    }

    // Special configuration for Emscripten/WebAssembly target
    if is_emscripten {
        println!("cargo:warning=Building for WebAssembly with Emscripten");
        // Emscripten handles compiler selection automatically via emcc wrapper
        // The cc crate will detect and use emcc when TARGET is wasm32-unknown-emscripten
        // Just ensure we have optimization and don't enable features that don't work in WASM

        // `-s KEY=VALUE` is an emcc *link-time* setting, not a compile flag: passing it to `build`
        // here only decorates each `.c -> .o` compile step (a no-op/warning there, since nothing is
        // linked yet). The actual link happens later, when cargo links the final artefact, so the
        // setting must be forwarded to *that* link line instead.
        println!("cargo:rustc-link-arg=-sERROR_ON_UNDEFINED_SYMBOLS=0");

        // Compile the vendored C with the same wasm exception-handling model rust's
        // `wasm32-unknown-emscripten` target links with. The C uses setjmp/longjmp (the recoverable
        // fatal-error trampoline in thread_local_accessors.c); without `-fwasm-exceptions` emcc emits
        // the legacy JS-based `invoke_` SjLj, which the wasm-EH link then rejects ("invoke_ functions
        // exported but exceptions and longjmp are both disabled"). Matching the model routes
        // setjmp/longjmp through wasm exception handling so the crate links and runs on wasm.
        build.flag("-fwasm-exceptions");
    }

    // Compile
    if let Some(bpi) = &bpi_override {
        build.flag(format!("-DBPI={bpi}"));
    }
    build.compile("espresso");

    // Generate bindings
    let mut builder = bindgen::Builder::default()
        .header("espresso-src/thread_local_accessors.h")
        .clang_arg(format!("-I{}", espresso_src.display()));

    if let Some(bpi) = &bpi_override {
        builder = builder.clang_arg(format!("-DBPI={bpi}"));
    }

    // Configure bindgen for Emscripten target
    if is_emscripten {
        // Emscripten provides system headers in its sysroot
        // We need to tell clang (used by bindgen) where to find them
        let sysroot = if let Ok(emsdk) = env::var("EMSDK") {
            // Standard EMSDK installation
            format!("{}/upstream/emscripten/cache/sysroot", emsdk)
        } else if let Ok(emscripten_root) = env::var("EMSCRIPTEN") {
            // Alternative: EMSCRIPTEN variable
            format!("{}/cache/sysroot", emscripten_root)
        } else {
            // Try Homebrew installation on macOS (common case)
            let homebrew_paths = [
                "/opt/homebrew/opt/emscripten/libexec/cache/sysroot", // Apple Silicon
                "/usr/local/opt/emscripten/libexec/cache/sysroot",    // Intel Mac
            ];

            homebrew_paths
                .iter()
                .find(|path| PathBuf::from(path).exists())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    println!("cargo:warning=Could not locate Emscripten sysroot. Set EMSDK environment variable.");
                    String::new()
                })
        };

        if !sysroot.is_empty() {
            builder = builder
                .clang_arg(format!("--sysroot={}", sysroot))
                .clang_arg("-target")
                .clang_arg("wasm32-unknown-emscripten")
                // Additional flags to ensure functions are parsed correctly
                .clang_arg("-fvisibility=default")
                .clang_arg("-D__EMSCRIPTEN__");
        }
    }

    // On systems where libclang is installed under a versioned prefix (for example
    // RHEL's `clang-libs` package, which puts it in /usr/lib64/llvm17/lib without a
    // `clang` driver on PATH), libclang can fail to locate its own builtin headers.
    // The first `#include` while parsing the vendored C then dies with
    // "'stddef.h' file not found". Point bindgen at the resource directory we
    // discover from the loaded library so the vendored C parses on such systems out
    // of the box. An explicit BINDGEN_EXTRA_CLANG_ARGS override wins, and the
    // Emscripten path (which supplies its own sysroot above) is left untouched.
    if !is_emscripten && env::var_os("BINDGEN_EXTRA_CLANG_ARGS").is_none() {
        if let Some(resource_dir) = find_clang_resource_dir() {
            builder = builder.clang_arg(format!("-resource-dir={}", resource_dir.display()));
        }
    }

    let bindings = builder
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allowlist only the FFI surface the wrapper actually calls (PLA I/O is pure Rust, so the
        // `read_pla`/`*_PLA`/`fprint_pla` family and the standalone `simplify`/`expand`/`irredundant`/
        // `reduce`/`verify` passes are not exposed).
        .allowlist_function("espresso")
        .allowlist_function("cube_setup")
        .allowlist_function("setdown_cube")
        .allowlist_function("sf_new")
        .allowlist_function("sf_free")
        .allowlist_function("sf_save")
        .allowlist_function("complement")
        .allowlist_function("minimize_exact")
        .allowlist_function("sf_addset")
        .allowlist_function("cube2list")
        .allowlist_function("set_clear")
        .allowlist_type("set_family_t")
        .allowlist_type("pset_family")
        // `espresso_word` is the width source of truth for the Rust side (native machine word,
        // widened from the historical hardcoded `unsigned int`); `get_bpi` lets Rust confirm cc
        // and bindgen agreed on that width at build time.
        .allowlist_type("espresso_word")
        .allowlist_function("get_bpi")
        // Thread-local accessors (replacing direct global variable access)
        .allowlist_function("get_cube")
        .allowlist_function("get_cdata")
        .allowlist_function("get_debug_ptr")
        .allowlist_function("set_debug")
        .allowlist_function("get_verbose_debug_ptr")
        .allowlist_function("set_verbose_debug")
        .allowlist_function("get_trace_ptr")
        .allowlist_function("set_trace")
        .allowlist_function("get_summary_ptr")
        .allowlist_function("set_summary")
        .allowlist_function("get_remove_essential_ptr")
        .allowlist_function("set_remove_essential")
        .allowlist_function("get_force_irredundant_ptr")
        .allowlist_function("set_force_irredundant")
        .allowlist_function("get_unwrap_onset_ptr")
        .allowlist_function("set_unwrap_onset")
        .allowlist_function("get_single_expand_ptr")
        .allowlist_function("set_single_expand")
        .allowlist_function("get_use_super_gasp_ptr")
        .allowlist_function("set_use_super_gasp")
        .allowlist_function("get_use_random_order_ptr")
        .allowlist_function("set_use_random_order")
        .allowlist_function("get_skip_make_sparse_ptr")
        .allowlist_function("set_skip_make_sparse")
        .allowlist_function("guarded_espresso")
        .allowlist_function("guarded_minimize_exact")
        .allowlist_function("guarded_complement")
        .allowlist_function("guarded_primes")
        // Generate good Rust types
        .derive_default(true)
        .derive_debug(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

/// Locate libclang's resource directory — the one holding the compiler-provided
/// headers (`stddef.h`, `stdarg.h`, …) that its own `#include` resolution needs.
///
/// This is derived from the libclang that bindgen itself will load (via clang-sys),
/// so it works regardless of install prefix — `/usr/lib64`, a versioned
/// `/usr/lib64/llvm17/lib`, a Homebrew cellar, a Nix store path — and needs no
/// `clang` driver on PATH. Returns the directory to pass as `-resource-dir`, or
/// `None` when it cannot be determined, in which case libclang's built-in search is
/// left to resolve the headers as usual.
fn find_clang_resource_dir() -> Option<PathBuf> {
    // Load libclang the same way bindgen will, then ask where the library file sits.
    clang_sys::load().ok()?;
    let library = clang_sys::get_library()?;
    let lib_dir = library.path().parent()?.to_path_buf();

    // The builtin headers live in `<prefix>/lib/clang/<version>/include`. Relative to
    // the library directory (typically `<prefix>/lib`), the `clang` directory is a
    // sibling or one level up.
    [lib_dir.join("clang"), lib_dir.join("..").join("clang")]
        .iter()
        .find_map(|root| newest_versioned_resource_dir(root))
}

/// Given a `.../clang` directory, return the version subdirectory that actually
/// carries `include/stddef.h`, choosing the highest version when several coexist.
fn newest_versioned_resource_dir(root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|dir| dir.join("include").join("stddef.h").is_file())
        .collect();
    candidates.sort_by_key(|dir| version_key(dir));
    candidates.pop()
}

/// Sort key for a resource directory named after its clang version (`17`, `17.0.6`),
/// so the numerically newest sorts last.
fn version_key(dir: &Path) -> Vec<u64> {
    dir.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            name.split('.')
                .map(|part| part.parse::<u64>().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}
