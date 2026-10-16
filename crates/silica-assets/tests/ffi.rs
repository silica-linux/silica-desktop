//! The C header is valid C. (The smoke test of the functions is in `src/ffi/c_api_tests.rs`,
//! where `unsafe` is allowed.)

#[test]
fn the_header_is_valid_c() {
    let header = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("include/silica_assets.h");
    for compiler in ["cc", "gcc", "clang"] {
        let Ok(output) = std::process::Command::new(compiler)
            .args([
                "-fsyntax-only",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-std=c99",
                "-x",
                "c",
            ])
            .arg(&header)
            .output()
        else {
            continue;
        };
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    eprintln!("no C compiler found: header syntax not checked");
}
