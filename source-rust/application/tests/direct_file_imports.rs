//! Exercise a direct call when a file, a value and a type share one name.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn the_file_type_and_value_each_select_their_own_member() {
    let fixture = support::repository_root().join("samples/cli/probes/direct_file_imports");
    let scratch = TemporaryDirectory::create("direct-file-imports", false).unwrap();
    std::fs::create_dir(scratch.path().join("m")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.args(["run", "--both"]);
    std::fs::write(scratch.path().join("main.omg"), r#"
use m/value-cell.omg as cell;
use m/b.omg;

data Reader { read: i32; }
data cell { case Stored; case Supplied; }

machine read(cell: Reader, item: |b|::B, kind: cell) -> i32 {
    transition kind {
        cell::Stored -> |cell|::read(item.value),
        cell::Supplied -> cell.read
    }
}

machine main() crashes Trap {
    let item: |b|::B = |b|::B { value: |cell|::A { value: 7 } };
    let local: Reader = Reader { read: 9 };
    transition read(local, item, cell::Stored) == 7 && read(local, item, cell::Supplied) == 9 {
        true -> done(), _ -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#).unwrap();
    command.args(["--build-input", "main.omg"]);
    for file in ["build.omg", "m/b.omg", "m/value-cell.omg"] {
        std::fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
        command.args(["--build-input", file]);
    }
    let output = command
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}
