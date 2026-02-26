use std::env;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn v1_fixture_matches_golden_screenshot() {
    let Some(rimworld_data) = env::var_os("RIMWORLD_DATA_DIR") else {
        eprintln!("skipping: set RIMWORLD_DATA_DIR to run v1 golden test");
        return;
    };
    let Some(typetree_registry) = env::var_os("RIMWORLD_TYPETREE_REGISTRY") else {
        eprintln!("skipping: set RIMWORLD_TYPETREE_REGISTRY to run v1 golden test");
        return;
    };

    let out_path =
        env::temp_dir().join(format!("stitchlands-v1-golden-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&out_path);

    let bin = env!("CARGO_BIN_EXE_stitchlands-redux");
    let output = Command::new(bin)
        .arg("--rimworld-data")
        .arg(rimworld_data)
        .arg("--typetree-registry")
        .arg(typetree_registry)
        .arg("fixture")
        .arg("v1")
        .arg("--viewport-width")
        .arg("256")
        .arg("--viewport-height")
        .arg("256")
        .arg("--camera-zoom")
        .arg("8")
        .arg("--no-window")
        .arg("--screenshot")
        .arg(&out_path)
        .env("RUST_LOG", "info")
        .output()
        .expect("run stitchlands-redux v1 golden command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_path = repo_root
        .join("tests")
        .join("golden")
        .join("v1_fixture_256.png");
    let golden = image::open(&golden_path)
        .expect("load golden screenshot")
        .to_rgba8();
    let actual = image::open(&out_path)
        .expect("load output screenshot")
        .to_rgba8();

    assert_eq!(
        golden.dimensions(),
        actual.dimensions(),
        "golden and actual dimensions differ"
    );

    let (w, h) = golden.dimensions();
    let mut diff_pixels = 0usize;
    let mut max_delta = 0u8;
    for (g, a) in golden.pixels().zip(actual.pixels()) {
        let mut pixel_diff = false;
        for c in 0..4 {
            let delta = g[c].abs_diff(a[c]);
            if delta > 0 {
                pixel_diff = true;
            }
            if delta > max_delta {
                max_delta = delta;
            }
        }
        if pixel_diff {
            diff_pixels += 1;
        }
    }

    let total_pixels = (w as usize) * (h as usize);
    let diff_ratio = diff_pixels as f64 / total_pixels as f64;

    assert!(
        diff_ratio <= 0.01 && max_delta <= 16,
        "golden mismatch too large: diff_pixels={} total={} ratio={:.4} max_delta={}\ngolden={}\nactual={}",
        diff_pixels,
        total_pixels,
        diff_ratio,
        max_delta,
        golden_path.display(),
        out_path.display()
    );
}
