use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rustc-check-cfg=cfg(cups2)");
    println!("cargo:rustc-check-cfg=cfg(cups3)");
    println!("cargo:rustc-check-cfg=cfg(cups2_new_enum)");

    let force_cups2 = env::var_os("CARGO_FEATURE_FORCE_CUPS2").is_some();
    let force_cups3 = env::var_os("CARGO_FEATURE_FORCE_CUPS3").is_some();

    if force_cups2 && force_cups3 {
        panic!("features force-cups2 and force-cups3 cannot be enabled together");
    }

    let (library, cups_cfg) = if force_cups2 {
        (
            pkg_config::probe_library("cups").expect("Failed to find cups with pkg-config"),
            "cups2",
        )
    } else if force_cups3 {
        (
            pkg_config::probe_library("cups3").expect("Failed to find cups3 with pkg-config"),
            "cups3",
        )
    } else if let Ok(library) = pkg_config::probe_library("cups3") {
        (library, "cups3")
    } else {
        (
            pkg_config::probe_library("cups")
                .expect("Failed to find cups3 or cups with pkg-config"),
            "cups2",
        )
    };

    println!("cargo:rustc-cfg={cups_cfg}");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allow CUPS types and functions
        .allowlist_function("cups.*")
        .allowlist_type("cups_.*")
        .allowlist_var("CUPS_.*")
        // Allow HTTP types and functions (needed for http_t)
        .allowlist_function("http.*")
        .allowlist_type("http_.*")
        .allowlist_var("HTTP_.*")
        // Allow IPP types and functions
        .allowlist_function("ipp.*")
        .allowlist_type("ipp_.*")
        .allowlist_var("IPP_.*")
        // CUPS 3 headers take `FILE *` parameters. Rust only passes the pointer around,
        // so keep the struct opaque instead of generating glibc's internal fields.
        .opaque_type("_IO_FILE");

    // Lets wrapper.h include cups/dnssd.h, which only CUPS 3 has.
    if cups_cfg == "cups3" {
        builder = builder.clang_arg("-DCUPS_RS_CUPS3");
    }

    for include_path in library.include_paths {
        builder = builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = builder.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bindings_path = out_path.join("bindings.rs");
    bindings
        .write_to_file(&bindings_path)
        .expect("Couldn't write bindings!");

    // cups@master has started tagging cups_dest_flags_e like CUPS 3 does, so bindgen
    // prefixes its constants the same way even on a cups2 build.
    if cups_cfg == "cups2" {
        let generated =
            std::fs::read_to_string(&bindings_path).expect("Couldn't read bindings back");
        if generated.contains("cups_dest_flags_e_CUPS_DEST_FLAGS_NONE") {
            println!("cargo:rustc-cfg=cups2_new_enum");
        }
    }
}
