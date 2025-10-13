fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("windows") {
        extern crate embed_resource;

        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let version_parts: Vec<&str> = version.split('.').collect();

        let major = version_parts.first().unwrap_or(&"0");
        let minor = version_parts.get(1).unwrap_or(&"0");
        let patch = version_parts.get(2).unwrap_or(&"0");

        let version_comma = format!("{major},{minor},{patch},0");

        let year = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / (365 * 24 * 60 * 60)
            + 1970;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let icon_path = std::path::Path::new(&manifest_dir).join("resources/icon.ico");

        let rc_content = format!(
            r#"#define IDI_ICON1 101
IDI_ICON1 ICON "{}"
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
END"#,
            icon_path.display()
        );

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let rc_path = std::path::Path::new(&out_dir).join("resources.rc");
        std::fs::write(&rc_path, rc_content).expect("Failed to write .rc file");

        embed_resource::compile(rc_path, embed_resource::NONE)
            .manifest_optional()
            .expect("Failed to compile .rc file");

        println!("cargo:rerun-if-changed=Cargo.toml");
        println!("cargo:rerun-if-changed=resources/icon.ico");
    }
}
