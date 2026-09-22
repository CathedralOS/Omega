//! The gates a statement call passes before its target resolves: a named
//! conformance requirement, a declared receiver, an `asm#` intrinsic and
//! the banned self-entry call.

use super::{
    CallScope, generic_requirement, user_asm_contract, validate_asm_operand_constraint,
    validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

/// A call whose receiver names a conformance requirement: its result use
/// and arguments validate against the requirement's signature. Reports
/// whether the call was such a requirement call (or failed to resolve as
/// one).
pub(super) fn validate_named_conformance_call(
    scope: &CallScope<'_>,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        writable_roots,
        value_env,
        ..
    } = *scope;
    match generic_requirement::named_conformance_requirement(
        program,
        current_machine,
        call.receiver_symbol,
        call.target_symbol,
    ) {
        Ok(Some(requirement)) => {
            validate_result_use(
                program,
                call,
                requirement.signature.name.as_str(),
                requirement.signature.return_type,
                diagnostics,
            );
            generic_requirement::validate_named_conformance_arguments(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                &requirement,
                writable_roots,
                diagnostics,
            );
            return true;
        }
        Err(error) => {
            diagnostics.push(Diagnostic::error(error));
            return true;
        }
        Ok(None) => {}
    }
    false
}

/// The receiver's first member must be a declared receiver in the current
/// state unless the path names an operator namespace; reports whether it
/// is.
pub(super) fn receiver_is_declared(
    scope: &CallScope<'_>,
    receiver_members: &[typed_trees::name::Identifier],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let CallScope {
        program,
        call,
        current_machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        ..
    } = *scope;
    let namespace: Vec<_> = receiver_members
        .iter()
        .map(|member| member.as_str())
        .collect();
    if let Some(first) = receiver_members.first()
        && !crate::value_custody::locals::is_named_operator_namespace(
            program,
            &[call.receiver_symbol],
            &namespace,
            call.target_symbol,
            call.target.as_str(),
            arguments.len(),
        )
    {
        let root = call.receiver_root_symbol;
        if !root.is_valid()
            || !current_state.is_some_and(|state| {
                crate::value_custody::locals::state_value_root_is_known(
                    program,
                    current_machine,
                    state,
                    writable_roots.statements,
                    machine_symbols,
                    symbols,
                    root,
                    first.as_str(),
                )
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}` uses `{}`, which is not a declared receiver in this state",
                current_machine.name.as_str(), first.as_str(),
            )));
            return false;
        }
    }
    true
}

/// An `asm#` intrinsic statement: a known-contract instruction with a
/// fixed operand count whose operands validate against the instruction's
/// contract.
pub(super) fn validate_asm_statement_call(
    scope: &CallScope<'_>,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        ..
    } = *scope;
    let register_write =
        language_core::inline_assembly::AsmControlRegister::from_write_intrinsic_name(
            call.target.as_str(),
        )
        .and_then(|register| register.write_mnemonic())
        .or_else(|| {
            language_core::inline_assembly::AsmSystemRegister::from_write_intrinsic_name(
                call.target.as_str(),
            )
            .and_then(|register| register.write_mnemonic())
        });
    let (source_mnemonic, expected_arguments) = match register_write {
        Some(mnemonic) => (mnemonic, 1),
        None => match call.target.as_str() {
            "asm#hlt" => ("hlt", 0),
            "asm#port_out" => ("out", 2),
            "asm#lfence" => ("lfence", 0),
            "asm#sfence" => ("sfence", 0),
            "asm#mfence" => ("mfence", 0),
            "asm#cli" => ("cli", 0),
            "asm#sti" => ("sti", 0),
            "asm#serialize" => ("serialize", 0),
            "asm#isb" => ("isb", 0),
            "asm#pause" => ("pause", 0),
            "asm#yield" => ("yield", 0),
            "asm#wbinvd" => ("wbinvd", 0),
            "asm#invd" => ("invd", 0),
            "asm#wbnoinvd" => ("wbnoinvd", 0),
            "asm#nop" => ("nop", 0),
            "asm#wfe" => ("wfe", 0),
            "asm#wfi" => ("wfi", 0),
            "asm#sev" => ("sev", 0),
            "asm#sevl" => ("sevl", 0),
            "asm#popfq" => ("popfq", 1),
            "asm#wrmsr" => ("wrmsr", 2),
            other => {
                diagnostics.push(Diagnostic::error(format!(
                    "asm intrinsic `{other}` is not a statement form"
                )));
                return;
            }
        },
    };
    if arguments.len() != expected_arguments {
        diagnostics.push(Diagnostic::error(format!(
            "asm intrinsic `{}` takes {} operand(s), found {}",
            call.target,
            expected_arguments,
            arguments.len()
        )));
        return;
    }
    if register_write.is_some() || matches!(source_mnemonic, "out" | "popfq" | "wrmsr") {
        let contract = user_asm_contract(source_mnemonic);
        for (operand, constraint) in arguments.iter().zip(contract.operands.iter()) {
            validate_asm_operand_constraint(
                program,
                current_machine,
                current_state,
                source_mnemonic,
                *operand,
                *constraint,
                diagnostics,
            );
        }
    }
}

/// A statement-position `self.<entry>(..)` call is tail recursion spelled
/// as a call and is rejected; reports whether the call was one.
pub(super) fn self_entry_call_is_banned(
    scope: &CallScope<'_>,
    receiver_members: &[typed_trees::name::Identifier],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let CallScope {
        call,
        current_machine,
        ..
    } = *scope;
    if !matches!(receiver_members, [receiver] if receiver.is_self_receiver()) {
        return false;
    }
    let machine_entry_name = current_machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(current_machine.name.as_str());
    if call.target.as_str() == machine_entry_name {
        diagnostics.push(Diagnostic::error(format!(
            "`self.{}(..)` as a STATEMENT calls the enclosing machine's own entry -- tail recursion spelled as a call, which Omega does not support (machine call cycles are banned; stack size must be predictable). Write the repetition as states: transition to a sub-state or loop back with a bare `-> {}(..)` arm",
            call.target.as_str(),
            call.target.as_str(),
        )));
        return true;
    }
    false
}
