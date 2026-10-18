fn main() {
    let config=slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("ui/app-window.slint",config).expect("Slint build failed");
    println!("cargo:rerun-if-changed=windows/app.rc");
    println!("cargo:rerun-if-changed=windows/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref()==Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref()==Ok("msvc") {
        let output=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("notes-ai.res");
        let status=std::process::Command::new("rc.exe").arg("/nologo")
            .arg(format!("/fo{}",output.display())).arg("windows/app.rc").status()
            .expect("Windows SDK rc.exe is required to embed the application icon");
        assert!(status.success(),"Windows resource compilation failed");
        println!("cargo:rustc-link-arg={}",output.display());
        println!("cargo:rustc-link-arg=/STACK:8000000");
    }
}
