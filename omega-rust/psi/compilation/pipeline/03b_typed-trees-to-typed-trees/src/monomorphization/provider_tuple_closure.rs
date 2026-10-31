//! Capture tuples wait until every independently supplied operand is closed.
use super::{Candidate, TypedTrees};
fn bindings_are_complete(provider: &Candidate<'_>) -> bool {
    !(provider.conflicted
        || provider.type_bindings.iter().any(Option::is_none)
        || provider.const_bindings.iter().any(Option::is_none)
        || provider.machine_bindings.iter().any(Option::is_none)
        || provider.evidence_bindings.iter().any(Option::is_none))
}

/// An independently supplied operand is open while its carrier still names a
/// caller binder, a machine parameter, or an unresolved conformance
/// application; admission must wait for the tuple that closes it.
pub(super) fn operands_are_open(program: &TypedTrees, provider: &Candidate<'_>) -> bool {
    provider
        .type_bindings
        .iter()
        .chain(&provider.const_bindings)
        .flatten()
        .any(|reference| super::contextual_calls::operand_is_open(program, *reference))
        || provider.machine_bindings.iter().flatten().any(|argument| {
            matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::MachineParameter
            ) || argument.application.is_some()
        })
        || provider.evidence_bindings.iter().flatten().any(|argument| {
            validation::conformance_applications::close_conformance_application(program, argument)
                .is_err()
        })
}

pub(super) fn operands_are_closed(program: &TypedTrees, provider: &Candidate<'_>) -> bool {
    bindings_are_complete(provider) && !operands_are_open(program, provider)
}
