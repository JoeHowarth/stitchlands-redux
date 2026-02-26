use std::env;
use std::process::Command;

#[test]
fn steel_renders_without_fallback_when_registry_present() {
    let Some(rimworld_data) = env::var_os("RIMWORLD_DATA_DIR") else {
        eprintln!("skipping: set RIMWORLD_DATA_DIR to run steel smoke test");
        return;
    };
    let Some(typetree_registry) = env::var_os("RIMWORLD_TYPETREE_REGISTRY") else {
        eprintln!("skipping: set RIMWORLD_TYPETREE_REGISTRY to run steel smoke test");
        return;
    };

    let bin = env!("CARGO_BIN_EXE_stitchlands-redux");
    let output = Command::new(bin)
        .arg("--rimworld-data")
        .arg(rimworld_data)
        .arg("--typetree-registry")
        .arg(typetree_registry)
        .arg("render")
        .arg("--thingdef")
        .arg("Steel")
        .arg("--no-window")
        .env("RUST_LOG", "info")
        .output()
        .expect("run stitchlands-redux smoke command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("selected def: Steel"),
        "missing selection log\nstderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("using checker fallback"),
        "steel still fell back\nstderr:\n{}",
        stderr
    );
}
