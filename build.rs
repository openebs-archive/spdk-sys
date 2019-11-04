extern crate bindgen;
extern crate cc;

use bindgen::callbacks::{MacroParsingBehavior, ParseCallbacks};
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{self, Error, ErrorKind, Result, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Debug)]
struct MacroCallback {
    macros: Arc<RwLock<HashSet<String>>>,
}

impl ParseCallbacks for MacroCallback {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        self.macros.write().unwrap().insert(name.into());

        if name == "IPPORT_RESERVED" {
            return MacroParsingBehavior::Ignore;
        }

        MacroParsingBehavior::Default
    }
}

/// We don't have luxury of using pkg-config for detecting if the library
/// is installed on the system. Try a small C program to test if the lib
/// is there.
fn find_spdk_lib(out_path: &PathBuf) -> Result<()> {
    let c_file = out_path.join("test_libspdk.c");
    let o_file = out_path.join("test_libspdk.o");

    {
        let mut file = File::create(&c_file)?;
        file.write_all(b"int main() { return 0; }")?;
    }

    let output = cc::Build::new()
        .get_compiler()
        .to_command()
        .arg("-o")
        .arg(o_file)
        .arg(c_file)
        .arg("-L./build")
        .arg("-lm")
        .arg("-lspdk_fat")
        .output()
        .expect("Failed to execute cc");

    if !output.status.success() {
        io::stderr().write_all(&output.stderr).unwrap();
        Err(Error::new(
            ErrorKind::Other,
            "spdk_fat library not found
    Hint: Likely you need to install it to the system at first.
          Look at build.sh script in spdk-sys repo.",
        ))
    } else {
        Ok(())
    }
}

fn main() {
    #![allow(unreachable_code)]
    #[cfg(not(target_arch = "x86_64"))]
    panic!("spdk-sys crate is only for x86_64 cpu architecture");
    #[cfg(not(target_os = "linux"))]
    panic!("spdk-sys crate works only on linux");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    if let Err(err) = find_spdk_lib(&out_path) {
        panic!("{}", err);
    }

    let macros = Arc::new(RwLock::new(HashSet::new()));
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        // If we did not use private interfaces those would not be needed.
        // All needed headers should be in /usr/local/include.
        .clang_arg("-Ispdk/include")
        .clang_arg("-Ispdk/lib")
        .clang_arg("-Ispdk/module")
        .rustfmt_bindings(true)
        .whitelist_function("^spdk.*")
        .whitelist_function("*.aio.*")
        .whitelist_function("*.iscsi.*")
        .whitelist_function("*.crypto_disk.*")
        .whitelist_function("*.lvs.*")
        .whitelist_function("*.lvol.*")
        .whitelist_var("^NVMF.*")
        .whitelist_var("^SPDK.*")
        .whitelist_var("^spdk.*")
        .trust_clang_mangling(false)
        .layout_tests(false)
        .derive_default(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .generate_inline_functions(true)
        .parse_callbacks(Box::new(MacroCallback {
            macros: macros.clone(),
        }))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("libspdk.rs"))
        .expect("Couldn't write bindings!");

    // spdk lib
    println!(
        "cargo:rustc-link-search={}",
        format!("{}/build", std::env::current_dir().unwrap().display())
    );
    println!("cargo:rustc-link-lib=spdk_fat");

    // OS libs
    // depending on distro/version -- this search path might be needed
    println!("cargo:rustc-link-search=native=/usr/lib64/iscsi");
    // if you add a library here then also add it in build.sh
    println!("cargo:rustc-link-lib=ibverbs");
    println!("cargo:rustc-link-lib=rdmacm");
    println!("cargo:rustc-link-lib=aio");
    println!("cargo:rustc-link-lib=iscsi");
    println!("cargo:rustc-link-lib=numa");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=rt");
    println!("cargo:rustc-link-lib=uuid");
    println!("cargo:rustc-link-lib=crypto");

    println!("cargo:rerun-if-changed=build.rs");
}
