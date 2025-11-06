use std::env;
use std::path::PathBuf;

fn main() {
    // Compile lalrpop grammar files to OUT_DIR for cargo publish compatibility
    lalrpop::process_root().unwrap();

    let espresso_src = PathBuf::from("espresso-src");

    println!("cargo:rerun-if-changed=espresso-src");

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
    build
        .flag("-std=c11") // Ensure C11 standard for _Thread_local support
        .flag_if_supported("-w"); // Suppress warnings from C code

    // Add sanitizer flags if requested via environment
    if let Ok(cflags) = env::var("CFLAGS") {
        if cflags.contains("-fsanitize=address") {
            build
                .flag("-fsanitize=address")
                .flag("-fno-omit-frame-pointer");
        }
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

    // Compile
    build.compile("espresso");

    // Generate bindings
    let bindings = bindgen::Builder::default()
        .header("espresso-src/thread_local_accessors.h")
        .clang_arg(format!("-I{}", espresso_src.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Whitelist the functions and types we want to expose
        .allowlist_function("espresso")
        .allowlist_function("read_pla")
        .allowlist_function("new_PLA")
        .allowlist_function("free_PLA")
        .allowlist_function("fprint_pla")
        .allowlist_function("PLA_summary")
        .allowlist_function("cube_setup")
        .allowlist_function("setdown_cube")
        .allowlist_function("sf_new")
        .allowlist_function("sf_free")
        .allowlist_function("sf_save")
        .allowlist_function("set_new")
        .allowlist_function("set_free")
        .allowlist_function("complement")
        .allowlist_function("simplify")
        .allowlist_function("expand")
        .allowlist_function("irredundant")
        .allowlist_function("reduce")
        .allowlist_function("minimize_exact")
        .allowlist_function("verify")
        .allowlist_function("sf_addset")
        .allowlist_function("sf_active")
        .allowlist_function("complement")
        .allowlist_function("cube1list")
        .allowlist_function("cube2list")
        .allowlist_function("set_clear")
        .allowlist_type("PLA_t")
        .allowlist_type("pPLA")
        .allowlist_type("set_family_t")
        .allowlist_type("pset_family")
        .allowlist_type("pset")
        .allowlist_type("cost_t")
        .allowlist_var("F_type")
        .allowlist_var("FD_type")
        .allowlist_var("FR_type")
        .allowlist_var("FDR_type")
        // Debug flags
        .allowlist_var("EXPAND")
        .allowlist_var("ESSEN")
        .allowlist_var("IRRED")
        .allowlist_var("REDUCE")
        .allowlist_var("SPARSE")
        .allowlist_var("GASP")
        .allowlist_var("SHARP")
        .allowlist_var("MINCOV")
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
