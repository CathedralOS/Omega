//! Compile or run one selected Rust test through Cargo (`cargo xtask focused-test`).

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::repo_root;

fn is_valid_package_or_target(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_valid_filter(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':')
}

fn unit_target(targets: &[serde_json::Value]) -> Result<Vec<String>, String> {
    let has_kind = |target: &serde_json::Value, kind: &str| {
        target["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|value| value == kind))
    };
    if targets
        .iter()
        .any(|target| has_kind(target, "lib") || has_kind(target, "proc-macro"))
    {
        return Ok(vec!["--lib".to_owned()]);
    }
    let binaries: Vec<_> = targets
        .iter()
        .filter(|target| has_kind(target, "bin"))
        .filter_map(|target| target["name"].as_str())
        .collect();
    match binaries.as_slice() {
        [name] => Ok(vec!["--bin".to_owned(), (*name).to_owned()]),
        _ => Err("package needs a library or one unambiguous binary unit-test target".to_owned()),
    }
}

fn selected_unit_target(root: &std::path::Path, package: &str) -> Result<Vec<String>, u8> {
    let result = (|| {
        let output = Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version=1"])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("read test targets: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "read test targets: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        let selected = metadata["packages"]
            .as_array()
            .and_then(|packages| packages.iter().find(|entry| entry["name"] == package))
            .ok_or_else(|| format!("unknown test package {package}"))?;
        unit_target(
            selected["targets"]
                .as_array()
                .ok_or("package has no targets")?,
        )
    })();
    result.map_err(|error| {
        eprintln!("focused-test: {error}");
        1u8
    })
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut prepare = false;
    let mut validate = false;
    for arg in args {
        match arg.as_str() {
            "--prepare" => prepare = true,
            "--validate" => validate = true,
            "-h" | "--help" => {
                println!("usage: cargo xtask focused-test [--validate | --prepare]");
                return Ok(());
            }
            other => {
                eprintln!("focused-test: unknown argument `{other}`");
                return Err(2);
            }
        }
    }
    if prepare && validate {
        eprintln!("focused-test: `--prepare` and `--validate` are mutually exclusive");
        return Err(2);
    }

    let package = std::env::var("TEST_PACKAGE").unwrap_or_default();
    let selected = std::env::var("TEST_FILTER").unwrap_or_default();
    let target = std::env::var("TEST_TARGET").unwrap_or_default();

    if !is_valid_package_or_target(&package) {
        eprintln!("Select one workspace package");
        return Err(1);
    }
    if !is_valid_filter(&selected) {
        eprintln!("Select a nonempty Rust test name or module");
        return Err(1);
    }
    if !target.is_empty() && !is_valid_package_or_target(&target) {
        eprintln!("Invalid integration test target");
        return Err(1);
    }
    let root = repo_root();
    let target_args = if target.is_empty() {
        selected_unit_target(&root, &package)?
    } else {
        vec!["--test".to_owned(), target]
    };
    if validate {
        return Ok(());
    }

    let mut command_args = vec![
        "test".to_owned(),
        "--locked".to_owned(),
        "-p".to_owned(),
        package,
    ];
    command_args.extend(target_args);
    if prepare {
        command_args.push("--no-run".to_owned());
        command_args.push(selected);
    } else {
        command_args.push(selected);
        command_args.push("--".to_owned());
        command_args.push("--nocapture".to_owned());
        command_args.push("--test-threads=1".to_owned());
    }

    let sha = std::env::var("GITHUB_SHA").unwrap_or_else(|_| "HEAD".to_owned());
    println!("Source: {sha}");
    println!("Command: mbx {}", command_args.join(" "));

    let _ = Command::new("rustc")
        .args(["--version", "--verbose"])
        .current_dir(&root)
        .status();

    let mut child = Command::new("mbx")
        .args(&command_args)
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            eprintln!("focused-test: failed to start mbx (mbx is required; run `bash tools/agent_setup.sh`): {e}");
            1u8
        })?;

    let mut passed = 0usize;
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            println!("{line}");
            if let Some(rest) = line.strip_prefix("test result: ok. ")
                && let Some(num_str) = rest.split_whitespace().next()
                && let Ok(n) = num_str.parse::<usize>()
            {
                passed += n;
            }
        }
    }

    let status = child.wait().map_err(|_| 1u8)?;
    if !status.success() {
        return Err(status.code().unwrap_or(1) as u8);
    }
    if prepare {
        println!("Compilation completed; no tests were executed.");
    } else if passed == 0 {
        eprintln!("The filter executed no passing tests");
        return Err(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn unit_selection_preserves_library_scope_and_supports_binary_only_packages() {
        assert_eq!(
            unit_target(&[
                json!({"kind": ["lib"], "name": "library"}),
                json!({"kind": ["bin"], "name": "cli"})
            ])
            .unwrap(),
            ["--lib"]
        );
        assert_eq!(
            unit_target(&[
                json!({"kind": ["bin"], "name": "cli"}),
                json!({"kind": ["test"], "name": "integration"})
            ])
            .unwrap(),
            ["--bin", "cli"]
        );
        assert!(
            unit_target(&[
                json!({"kind": ["bin"], "name": "one"}),
                json!({"kind": ["bin"], "name": "two"})
            ])
            .is_err()
        );
        assert!(unit_target(&[json!({"kind": ["test"], "name": "integration"})]).is_err());
    }
}
