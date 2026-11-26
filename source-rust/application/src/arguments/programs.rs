//! Bind one invocation's root capability ceiling, executable and file limits.
use package_compilation::build_execution::{BuildProgramBinding, BuildProgramInvocation};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

pub(super) fn take(
    argument: &OsStr,
    arguments: &mut super::Arguments,
    programs: &mut BuildProgramInvocation,
) -> Result<bool, String> {
    match argument.to_str() {
        Some("--allow") => {
            let name = super::take_text(arguments, "--allow")?;
            if name != "ProgramLaunch" {
                return Err(format!("unsupported root Build service `{name}`"));
            }
            super::set_flag(&mut programs.allow, "--allow ProgramLaunch")?;
        }
        Some("--program") => {
            let selector = super::take_text(arguments, "--program")?;
            if programs
                .bindings
                .iter()
                .any(|binding| binding.selector == selector)
            {
                return Err(format!("duplicate program grant for `{selector}`"));
            }
            let executable = PathBuf::from(super::take_value(arguments, "--program")?);
            programs
                .bindings
                .push(BuildProgramBinding::new(selector, executable)?);
        }
        Some(option @ ("--program-read" | "--program-write" | "--program-timeout")) => {
            let binding = programs
                .bindings
                .last_mut()
                .ok_or_else(|| format!("{option} must follow a --program binding"))?;
            let value = super::take_text(arguments, option)?;
            if option == "--program-timeout" {
                let milliseconds = value
                    .parse::<u64>()
                    .ok()
                    .filter(|&value| value > 0)
                    .ok_or("--program-timeout requires a positive millisecond count")?;
                if binding
                    .timeout
                    .replace(Duration::from_millis(milliseconds))
                    .is_some()
                {
                    return Err("duplicate timeout for one program binding".into());
                }
            } else {
                binding.add_path(value.into_bytes(), option == "--program-write")?;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(super) fn finish(programs: &mut BuildProgramInvocation) -> Result<(), String> {
    if !programs.bindings.is_empty() {
        if !programs.allow {
            return Err("program bindings require the root --allow ProgramLaunch ceiling".into());
        }
        programs.worker = Some(
            std::env::current_exe()
                .map_err(|why| format!("cannot locate the program supervisor: {why}"))?,
        );
    }
    Ok(())
}
