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
    if validate {
        return Ok(());
    }

    let mut command_args = vec![
        "test".to_owned(),
        "--locked".to_owned(),
        "-p".to_owned(),
        package,
    ];
    if target.is_empty() {
        command_args.push("--lib".to_owned());
    } else {
        command_args.push("--test".to_owned());
        command_args.push(target);
    }
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

    let root = repo_root();
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
