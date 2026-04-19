use std::env;
use std::process::Command;

#[test]
fn fixture_scene_builds() {
    let Some(rimworld_data) = env::var_os("RIMWORLD_DATA_DIR") else {
        eprintln!("skipping: set RIMWORLD_DATA_DIR to run fixture smoke test");
        return;
    };
    let Some(typetree_registry) = env::var_os("RIMWORLD_TYPETREE_REGISTRY") else {
        eprintln!("skipping: set RIMWORLD_TYPETREE_REGISTRY to run fixture smoke test");
        return;
    };

    let bin = env!("CARGO_BIN_EXE_stitchlands-redux");
    let output = Command::new(bin)
        .arg("--rimworld-data")
        .arg(rimworld_data)
        .arg("--typetree-registry")
        .arg(typetree_registry)
        .arg("fixture")
        .arg("fixtures/v2/move_lane.ron")
        .arg("--no-window")
        .env("RUST_LOG", "info")
        .output()
        .expect("run stitchlands-redux fixture smoke command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("fixture scene built"),
        "missing fixture build log\nstderr:\n{}",
        stderr
    );
}
