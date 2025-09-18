
fn main() {
    #[cfg(target_os = "windows")]
    {
        extern crate embed_resource;

        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let version_parts: Vec<&str> = version.split('.').collect();

        let major = version_parts.get(0).unwrap_or(&"0");
        let minor = version_parts.get(0).unwrap_or(&"0");
        let patch = version_parts.get(0).unwrap_or(&"0");

        let version_comma = format!("{},{},{},0", major, minor, patch);

        let year = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / (365 * 24 * 60 * 60) + 1970; // Approx year

        let rc_content = format!(r#"#define IDI_ICON1 101
IDI_ICON1 ICON "./resources/icon.ico"
1 VERSIONINFO
FILEVERSION {version_comma}
PRODUCTVERSION {version_comma}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "morgverd"
            VALUE "FileDescription", "SMS-API interface terminal"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "sms-terminal"
            VALUE "LegalCopyright", "Copyright (C) {year}"
            VALUE "OriginalFilename", "sms-terminal.exe"
            VALUE "ProductName", "SMS Terminal"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END"#);

        let our_dir = std::env::var("OUT_DIR").unwrap();
        let rc_path = std::path::Path::new(&our_dir).join("resources.rc");
        std::fs::write(&rc_path, rc_content).expect("Failed to write .rc file");

        // Embed the generated resource file
        embed_resource::compile(rc_path, embed_resource::NONE)
            .manifest_optional()
            .expect("Failed to compile .rc file");

        println!("cargo:rerun-if-changed=Cargo.toml");
    }
}