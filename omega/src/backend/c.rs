use std::collections::HashMap;

use crate::ast::expr::Expr;
use crate::ast::item::{Item, Machine, Platform};
use crate::ast::stmt::Stmt;
use crate::diagnostics::Diagnostic;

pub fn emit_c(items: &[Item]) -> Result<String, Diagnostic> {
    let main = find_main_machine(items)?;
    let main_state = main
        .states
        .iter()
        .find(|state| state.name == "Main")
        .ok_or_else(|| Diagnostic::error("machine main is missing state Main"))?;

    let platforms = items
        .iter()
        .filter_map(|item| match item {
            Item::Platform(platform) => Some((platform.name.as_str(), platform)),
            _ => None,
        })
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
        let Stmt::CommandCall(call) = statement else {
            continue;
        };

        let Some(receiver_type) = contains.get(call.receiver.as_str()) else {
            return Err(Diagnostic::error(format!(
                "unknown command receiver `{}`",
                call.receiver
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
                let text = expect_string_arg(&call.args, "WriteLine")?;
                output.push_str("    puts(\"");
                output.push_str(&escape_c_string(text));
                output.push_str("\");\n");
            }
            ("Console", "ExitProcess") => {
                let code = expect_integer_arg(&call.args, "ExitProcess")?;
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

fn find_main_machine(items: &[Item]) -> Result<&Machine, Diagnostic> {
    items
        .iter()
        .find_map(|item| match item {
            Item::Machine(machine) if machine.name == "main" => Some(machine),
            _ => None,
        })
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

fn expect_string_arg<'a>(args: &'a [Expr], command: &str) -> Result<&'a str, Diagnostic> {
    match args {
        [Expr::String(value)] => Ok(value),
        _ => Err(Diagnostic::error(format!(
            "{command} expects one string argument"
        ))),
    }
}

fn expect_integer_arg(args: &[Expr], command: &str) -> Result<i64, Diagnostic> {
    match args {
        [Expr::Integer(value)] => Ok(*value),
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
