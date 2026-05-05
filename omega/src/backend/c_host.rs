use std::collections::HashMap;

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::expression::Expression;
use crate::ir::machine::Machine;
use crate::ir::platform::Platform;
use crate::ir::statement::Statement;

pub fn emit_c_host_source(program: &Program) -> Result<String, Diagnostic> {
    let main = find_main_machine(program)?;
    let main_state = main
        .states
        .iter()
        .find(|state| state.name == "Main")
        .ok_or_else(|| Diagnostic::error("machine main is missing state Main"))?;

    let platforms = program
        .platforms
        .iter()
        .map(|platform| (platform.name.as_str(), platform))
        .collect::<HashMap<_, _>>();

    let contains = main
        .contains
        .iter()
        .map(|contains| (contains.name.as_str(), contains.type_name.as_str()))
        .collect::<HashMap<_, _>>();

    let mut output = String::new();
    let mut returns_from_main = false;
    output.push_str("#include <stdio.h>\n\n");
    output.push_str("int main(void) {\n");

    for statement in &main_state.statements {
        let Statement::CommandCall(call) = statement else {
            return Err(Diagnostic::error(
                "the C-host MVP can only lower straight-line command calls in state Main for now",
            ));
        };

        let Some(receiver) = call.receiver.as_deref() else {
            return Err(Diagnostic::error(format!(
                "the C-host MVP cannot lower local command `{}` yet",
                call.command
            )));
        };

        let Some(receiver_type) = contains.get(receiver) else {
            return Err(Diagnostic::error(format!(
                "unknown command receiver `{}`",
                receiver
            )));
        };

        let Some(platform) = platforms.get(receiver_type) else {
            return Err(Diagnostic::error(format!(
                "`{receiver_type}` is not a known platform"
            )));
        };

        ensure_platform_command(platform, &call.command)?;

        match (receiver_type.to_owned(), call.command.as_str()) {
            ("Console", "WriteLine") => {
                let text = expect_string_arg(&call.arguments, "WriteLine")?;
                output.push_str("    puts(\"");
                output.push_str(&escape_c_string(text));
                output.push_str("\");\n");
            }
            ("Console", "ExitProcess") => {
                let code = expect_integer_arg(&call.arguments, "ExitProcess")?;
                output.push_str(&format!("    return {code};\n"));
                returns_from_main = true;
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "no C lowering for {}.{}",
                    receiver_type, call.command
                )));
            }
        }
    }

    if !returns_from_main {
        output.push_str("    return 0;\n");
    }

    output.push_str("}\n");

    Ok(output)
}

fn find_main_machine(program: &Program) -> Result<&Machine, Diagnostic> {
    program
        .machines
        .iter()
        .find(|machine| machine.name == "main")
        .ok_or_else(|| Diagnostic::error("missing machine main"))
}

fn ensure_platform_command(platform: &Platform, command_name: &str) -> Result<(), Diagnostic> {
    if platform
        .commands
        .iter()
        .any(|command| command.name == command_name)
    {
        Ok(())
    } else {
        Err(Diagnostic::error(format!(
            "platform `{}` has no command `{command_name}`",
            platform.name
        )))
    }
}

fn expect_string_arg<'a>(
    arguments: &'a [Expression],
    command: &str,
) -> Result<&'a str, Diagnostic> {
    match arguments {
        [Expression::String(value)] => Ok(value),
        _ => Err(Diagnostic::error(format!(
            "{command} expects one string argument"
        ))),
    }
}

fn expect_integer_arg(arguments: &[Expression], command: &str) -> Result<i64, Diagnostic> {
    match arguments {
        [Expression::Integer(value)] => Ok(*value),
        _ => Err(Diagnostic::error(format!(
            "{command} expects one integer argument"
        ))),
    }
}

fn escape_c_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
