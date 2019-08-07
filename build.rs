extern crate bindgen;
extern crate cc;
extern crate glob;

use bindgen::callbacks::{MacroParsingBehavior, ParseCallbacks};
use glob::glob;
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{self, Write, Error, ErrorKind, Result},
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

/// Create a wrapper.h file containing includes for all public spdk header
/// files, which can be used as input for bindgen.
fn create_wrapper_h(out_path: &PathBuf) -> Result<String> {
    let headers: Vec<String> = glob("spdk/include/spdk/*.h")
        .expect("wrong glob pattern")
        .map(|e| format!(
                "#include <spdk/{}>",
                e.unwrap().file_name().unwrap().to_str().unwrap()
        ))
        .collect();

    let h_file = out_path.join("wrapper.h");
    let mut file = File::create(&h_file)?;
    file.write_all(headers.join("\n").as_bytes())?;
    Ok(h_file.to_str().unwrap().to_string())
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

    let wrapper_h = match create_wrapper_h(&out_path) {
        Ok(val) => val,
        Err(err) => panic!("Failed to create wrapper file with headers: {}", err),
    };

    let macros = Arc::new(RwLock::new(HashSet::new()));
    let bindings = bindgen::Builder::default()
        .header(wrapper_h)
        .clang_arg("-Ispdk/include")
        .rustfmt_bindings(true)
        .trust_clang_mangling(false)
        .layout_tests(false)
        .derive_default(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .generate_inline_functions(true)
        .ctypes_prefix("libc")
        .parse_callbacks(Box::new(MacroCallback {
            macros: macros.clone(),
        }))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("libspdk.rs"))
        .expect("Couldn't write bindings!");

    // spdk lib
    println!("cargo:rustc-link-lib=spdk_fat");

    // OS libs
    // depending on distro/version -- this search path might be needed
    println!("cargo:rustc-link-search=native=/usr/lib64/iscsi");
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
    println!("cargo:rerun-if-changed=wrapper.h");
}
