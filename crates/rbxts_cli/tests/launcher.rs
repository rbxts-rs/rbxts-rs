//! The compiler launcher's tests (`templates/compiler/rbxtsc.mjs`, the launcher every project's `compiler/` folder
//! carries): `node tools/test-launcher.mjs`, which runs them against a fake release on a local HTTP server. Skipped
//! with a note without Node, unless tests are required (`CI` or `RBXTS_RS_FIXTURE_REQUIRE=1`).

use std::path::Path;
use std::process::Command;

#[test]
fn launcher() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/test-launcher.mjs");
    let output = match Command::new("node").arg(&script).output() {
        Ok(output) => output,
        Err(e) => return rbxts_test_support::skip("launcher", &format!("cannot run node: {e}")),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "node {}: {}\n{stdout}{stderr}", script.display(), output.status);
}
