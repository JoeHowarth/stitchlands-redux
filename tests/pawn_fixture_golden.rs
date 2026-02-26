use std::env;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn pawn_fixture_matches_golden_when_enabled() {
    let enabled = env::var_os("RIMWORLD_ENABLE_SCREENSHOT_GOLDEN").is_some();
    if !enabled {
        eprintln!(
            "skipping: set RIMWORLD_ENABLE_SCREENSHOT_GOLDEN=1 to run pawn fixture golden test"
        );
        return;
    }

    let Some(rimworld_data) = env::var_os("RIMWORLD_DATA_DIR") else {
        eprintln!("skipping: set RIMWORLD_DATA_DIR to run pawn fixture golden test");
        return;
    };
    let Some(golden_path) = env::var_os("RIMWORLD_PAWN_GOLDEN") else {
        eprintln!("skipping: set RIMWORLD_PAWN_GOLDEN to run pawn fixture golden test");
        return;
    };

    let golden_path = PathBuf::from(golden_path);
    if !golden_path.exists() {
        eprintln!(
            "skipping: RIMWORLD_PAWN_GOLDEN does not exist: {}",
            golden_path.display()
        );
        return;
    }

    let mut actual_path = env::temp_dir();
    actual_path.push(format!(
        "stitchlands-pawn-fixture-{}.png",
        std::process::id()
    ));

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stitchlands-redux"));
    cmd.arg("--rimworld-data")
        .arg(rimworld_data)
        .arg("--pawn-fixture")
        .arg("--map-width")
        .arg("16")
        .arg("--map-height")
        .arg("16")
        .arg("--screenshot")
        .arg(&actual_path)
        .env("RUST_LOG", "info");

    if let Some(registry) = env::var_os("RIMWORLD_TYPETREE_REGISTRY") {
        cmd.arg("--typetree-registry").arg(registry);
    }

    let output = cmd.output().expect("run stitchlands-redux pawn fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("pawn fixture scene built"),
        "missing fixture log\nstderr:\n{}",
        stderr
    );

    let actual = image::open(&actual_path)
        .expect("open actual screenshot")
        .to_rgba8();
    let golden = image::open(&golden_path)
        .expect("open golden screenshot")
        .to_rgba8();

    assert_eq!(
        actual.dimensions(),
        golden.dimensions(),
        "screenshot dimensions differ: actual={:?} golden={:?}",
        actual.dimensions(),
        golden.dimensions()
    );

    let max_diff = env::var("RIMWORLD_PAWN_GOLDEN_MAX_DIFF")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let mut total = 0f64;
    let mut count = 0usize;
    for (a, b) in actual.pixels().zip(golden.pixels()) {
        for i in 0..4 {
            total += (a.0[i] as f64 - b.0[i] as f64).abs();
            count += 1;
        }
    }
    let mean_abs_diff = (total / count as f64) as f32;

    assert!(
        mean_abs_diff <= max_diff,
        "mean pixel diff too high: actual={} allowed={} (actual={}, golden={})",
        mean_abs_diff,
        max_diff,
        actual_path.display(),
        golden_path.display()
    );
}
