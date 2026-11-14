/*

End-to-end CLI check that a Build can ask for a Product declaration by path
(`omega --check`).

`BuildProduct::entry(&builder.product, "support::setup::launch", slot)` asks
which machine a path names and hands back a description a root binding can
use (`wiki/spec/build/scoped_execution.md`, "Selecting product declarations
without executing them"). The path is read in the package of the Build
source that asked: a path through a Product dependency alias names a public
declaration of that dependency, and a Build-scope edge of the same alias
grants nothing to the Product.

The two kept programs differ only in the scope of the dependency edge their
Build registers under the alias `support`. With a Product edge the entry is
found and the program compiles. With a Build edge the alias names no Product
dependency, so the path names nothing and the Build is refused for it.

*/

use std::path::PathBuf;
use std::process::Command;

fn check(fixture: &str) -> (bool, String) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures")
        .join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn an_entry_described_through_a_product_dependency_alias_compiles() {
    let (compiled, stderr) = check("pass/build/product_entry_query_product_scope_alias");
    assert!(compiled, "{stderr}");
}

#[test]
fn an_entry_asked_through_a_build_scope_alias_names_nothing() {
    let (compiled, stderr) = check("fail/build/product_entry_query_build_scope_alias");
    assert!(!compiled);
    assert!(
        stderr.contains(
            "product entry `support::setup::launch` names no machine in this build occurrence's package"
        ),
        "{stderr}"
    );
}
