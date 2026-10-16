fn main() {
    #[cfg(windows)]
    {
        let icon =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/icons/icon.ico");

        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon.to_str().expect("icon path is valid UTF-8"));
        res.set("ProductName", "PAWS");
        res.set("FileDescription", "PAWS - Answers Web Sessions");
        res.compile().expect("failed to embed Windows resources");
    }

    println!("cargo:rerun-if-changed=../packaging/icons/icon.ico");
}
