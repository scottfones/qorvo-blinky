use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    // Put memory.x on the linker search path.
    let out = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out.join("memory.x"), include_bytes!("memory.x"))?;
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");

    // Linker scripts for every binary and test artifact.
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
    println!("cargo:rustc-link-arg=--nmagic");

    // embedded-test's linker script, only for test binaries.
    println!("cargo:rustc-link-arg-tests=-Tembedded-test.x");

    Ok(())
}
