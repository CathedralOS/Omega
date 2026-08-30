use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_source(name: &str, source: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-inspect-terminal-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create inspect-terminal fixture directory");
    let path = directory.join("main.omg");
    std::fs::write(&path, source).expect("write inspect-terminal fixture");
    path
}

fn inspect(machine: &str, source: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "inspect-terminal",
            "--machine",
            machine,
            source.to_str().expect("UTF-8 temporary source path"),
        ])
        .output()
        .expect("run omega inspect-terminal")
}

fn remove_fixture(path: PathBuf) {
    let directory = path.parent().expect("fixture has a parent");
    std::fs::remove_dir_all(directory).expect("remove inspect-terminal fixture");
}

#[test]
fn exact_ranked_u32_countdown_reports_its_replayed_fixed_fuel_ceiling() {
    let source = temporary_source(
        "ranked-u32",
        r#"
            data Token { value: i32; }
            data Root {}

            machine Root::countdown(token: Token, remaining: u32)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(token, remaining - 1)
                    _ -> done(token)
                }
                state done(token: Token) {}
            }
        "#,
    );

    let output = inspect("Root::countdown", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "ranked fixed-fuel inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_machine=Root::countdown"));
    assert!(stdout.contains("ceiling_units=25769803775"), "{stdout}");
}

#[test]
fn acyclic_inspection_keeps_the_ordinary_fixed_fuel_path() {
    let source = temporary_source(
        "acyclic",
        r#"
            data Token { value: i32; }
            data Root {}

            machine Root::forward(token: Token) {
                transition { _ -> done(token) }
                state done(token: Token) {}
            }
        "#,
    );

    let output = inspect("Root::forward", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "acyclic fixed-fuel inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("selected_machine=Root::forward"));
    assert!(stdout.contains("fixed_fuel "), "{stdout}");
}

#[test]
fn unsupported_wider_ranked_countdown_fails_closed() {
    let source = temporary_source(
        "ranked-u64",
        r#"
            data Token { value: i32; }
            data Root {}

            machine Root::countdown(token: Token, remaining: u64)
            terminates by remaining -> Nat::Descending;
            {
                transition remaining > 0 {
                    true -> countdown(token, remaining - 1)
                    _ -> done(token)
                }
                state done(token: Token) {}
            }
        "#,
    );

    let output = inspect("Root::countdown", &source);
    remove_fixture(source);

    assert!(!output.status.success(), "wider ranked slice was accepted");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot lower terminal machine")
            || stderr.contains("cannot verify terminal machine")
            || stderr.contains("cannot derive ranked fixed fuel"),
        "unexpected rejection: {stderr}"
    );
}
