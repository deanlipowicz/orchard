use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-env-changed=R_HOME");
    println!("cargo:rerun-if-env-changed=R_BINARY");
    println!("cargo:rerun-if-changed=wrapper.h");

    let r_home = r_home().unwrap_or_else(|| {
        panic!("R was not found. Install R or set R_HOME/R_BINARY.");
    });
    let include_dir = r_include_dir(&r_home).unwrap_or_else(|| {
        panic!("R headers were not found under {}.", r_home.display());
    });

    println!(
        "cargo:rustc-link-search=native={}",
        r_home.join("lib").display()
    );
    println!("cargo:rustc-link-lib=R");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .allowlist_function("Rf_.*")
        .allowlist_function("R_.*")
        .allowlist_function("SET_.*")
        .allowlist_var("R_.*")
        .allowlist_var("ptr_R_.*")
        .allowlist_type("SEXPREC")
        .allowlist_type("SEXP")
        .allowlist_type("ParseStatus")
        .allowlist_type("SA_TYPE")
        .generate()
        .expect("failed to generate R bindings");

    bindings
        .write_to_file(PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs"))
        .expect("failed to write R bindings");
}

fn r_home() -> Option<PathBuf> {
    env::var_os("R_HOME")
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| env::var_os("R_BINARY").and_then(|r| r_home_from_binary(Path::new(&r))))
        .or_else(|| r_home_from_binary(Path::new("R")))
}

fn r_home_from_binary(r: &Path) -> Option<PathBuf> {
    let output = Command::new(r).arg("RHOME").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn r_include_dir(r_home: &Path) -> Option<PathBuf> {
    let output = Command::new(r_home.join("bin/R"))
        .args(["CMD", "config", "--cppflags"])
        .output()
        .ok()?;
    if output.status.success() {
        let flags = String::from_utf8_lossy(&output.stdout);
        for flag in flags.split_whitespace() {
            if let Some(path) = flag.strip_prefix("-I") {
                let path = PathBuf::from(path);
                if path.join("Rembedded.h").exists() {
                    return Some(path);
                }
            }
        }
    }

    [
        r_home.join("include"),
        PathBuf::from("/usr/share/R/include"),
    ]
    .into_iter()
    .find(|p| p.join("Rembedded.h").exists())
}
