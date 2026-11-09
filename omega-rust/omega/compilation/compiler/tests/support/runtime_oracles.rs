//! Authored corpus exit oracles, shared by execution admission and recording gates.

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy)]
enum ExpectedExit {
    Code(i32),
    Trap,
}

impl ExpectedExit {
    fn text(self) -> String {
        match self {
            Self::Code(code) => code.to_string(),
            Self::Trap => "trap".to_owned(),
        }
    }
}

fn header_exit(main: &Path) -> Option<ExpectedExit> {
    let source = std::fs::read_to_string(main).ok()?;
    source.lines().rev().find_map(|line| {
        let comment = line.trim().strip_prefix("//")?.trim();
        comment
            .strip_prefix("Expected exit:")?
            .trim()
            .trim_end_matches('.')
            .parse::<i32>()
            .ok()
            .map(ExpectedExit::Code)
    })
}

fn selected_exit(
    main: &Path,
    target: &str,
    interpreted: bool,
) -> Result<(Option<ExpectedExit>, bool), String> {
    let directory = main
        .parent()
        .ok_or("exit oracle has no fixture directory")?;
    let interpreter = directory.join("expected_interpreter_exit.txt");
    let host = directory.join(format!("expected_native_exit.{target}.txt"));
    let portable = directory.join("expected_native_exit.txt");
    let explicit_interpreter = interpreted && interpreter.is_file();
    let path = if explicit_interpreter {
        interpreter
    } else if host.is_file() {
        host
    } else {
        portable
    };
    let expected = match std::fs::read_to_string(&path) {
        Ok(text) if text.trim() == "trap" => Some(ExpectedExit::Trap),
        Ok(text) => Some(ExpectedExit::Code(text.trim().parse::<i32>().map_err(
            |error| format!("invalid exit oracle {}: {error}", path.display()),
        )?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => header_exit(main),
        Err(error) => return Err(format!("read exit oracle {}: {error}", path.display())),
    };
    Ok((expected, explicit_interpreter))
}

fn matches_exit(
    expected: ExpectedExit,
    target: &str,
    interpreted: bool,
    explicit_interpreter: bool,
    facts: &BTreeMap<&str, &str>,
) -> bool {
    let fact = |key| facts.get(key).copied();
    match expected {
        ExpectedExit::Code(expected) => {
            let key = if interpreted {
                "interpreter-exit"
            } else {
                "exit"
            };
            let Some(mut actual) = fact(key).and_then(|value| value.parse::<i32>().ok()) else {
                return false;
            };
            if interpreted {
                if matches!(
                    fact("interpreter"),
                    Some("trapped" | "declined" | "unchecked")
                ) {
                    return false;
                }
                if !explicit_interpreter && !target.starts_with("windows_") {
                    actual &= 255;
                }
            } else if facts.contains_key("signal") {
                return false;
            }
            actual == expected
        }
        ExpectedExit::Trap if interpreted => {
            fact("interpreter") == Some("trapped") && !facts.contains_key("interpreter-exit")
        }
        ExpectedExit::Trap => match target {
            "windows_x86_64" => {
                fact("exit") == Some("-1073741795") && !facts.contains_key("signal")
            }
            "linux_x86_64" | "macos_x86_64" => {
                fact("exit") == Some("signal") && fact("signal") == Some("4")
            }
            "linux_arm64" | "macos_arm64" => {
                fact("exit") == Some("signal") && fact("signal") == Some("5")
            }
            _ => false,
        },
    }
}

fn engines_agree(facts: &BTreeMap<&str, &str>, target: &str) -> bool {
    if facts.get("interpreter") == Some(&"disagrees") {
        return false;
    }
    facts
        .get("exit")
        .and_then(|value| value.parse::<i32>().ok())
        .is_some_and(|code| {
            matches_exit(ExpectedExit::Code(code), target, false, false, facts)
                && matches_exit(ExpectedExit::Code(code), target, true, false, facts)
        })
        || (matches_exit(ExpectedExit::Trap, target, false, false, facts)
            && matches_exit(ExpectedExit::Trap, target, true, false, facts))
}

// Before target discovery, read only the numeric header. Once a target and
// observations are available, select its file oracle and check the actual exit.
pub(super) fn exit_oracle(
    main: &Path,
    target: Option<&str>,
    interpreted: bool,
    observed: Option<&[(String, String)]>,
) -> Result<Option<String>, String> {
    let (expected, explicit_interpreter) = match target {
        Some(target) => selected_exit(main, target, interpreted)?,
        None => (header_exit(main), false),
    };
    if let Some(observed) = observed {
        let target = target.ok_or("observed exit has no selected target")?;
        let mut facts = BTreeMap::new();
        for (key, value) in observed {
            if facts.insert(key.as_str(), value.as_str()).is_some() {
                return Err(format!("duplicate execution fact {key}"));
            }
        }
        if let Some(expected) = expected {
            if !matches_exit(expected, target, interpreted, explicit_interpreter, &facts) {
                let engine = if interpreted { "interpreter" } else { "native" };
                return Err(format!(
                    "{engine} execution did not satisfy authored exit {}",
                    expected.text()
                ));
            }
            if facts.contains_key("exit")
                && (facts.contains_key("interpreter-exit")
                    || facts.get("interpreter") == Some(&"trapped"))
                && !engines_agree(&facts, target)
            {
                return Err("native and interpreter executions did not agree".to_owned());
            }
        }
    }
    Ok(expected.map(ExpectedExit::text))
}
