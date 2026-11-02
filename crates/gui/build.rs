fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/Hostmask.ico");
    println!("cargo:rerun-if-changed=../../windows/Hostmask.manifest");
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("../../assets/icons/Hostmask.ico")
            .set_manifest_file("../../windows/Hostmask.manifest")
            .set("ProductName", "Hostmask")
            .set("FileDescription", "Hostmask hosts-file set manager")
            .set("OriginalFilename", "Hostmask.exe")
            .compile()
            .expect("could not embed the Windows application icon and UAC manifest");
    }
}
