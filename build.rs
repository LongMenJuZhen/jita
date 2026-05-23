// 用于编译 slint 图形界面定义文件
fn main() {
    slint_build::compile("gui/main.slint").unwrap();

    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        println!("cargo:rustc-link-arg=/FORCE:MULTIPLE");
        println!("cargo:rustc-link-arg=/FORCE:UNRESOLVED");
    }
}
