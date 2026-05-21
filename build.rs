fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("lang");
    slint_build::compile_with_config("ui/main.slint", config).unwrap();

    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        println!("cargo:rustc-link-arg=/FORCE:MULTIPLE");
        println!("cargo:rustc-link-arg=/FORCE:UNRESOLVED");
    }
}
