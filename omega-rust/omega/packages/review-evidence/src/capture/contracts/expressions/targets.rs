//! The checked declarations one contract call target can name.
//!
//! A target resolves over four declaration species: a realized machine state,
//! a compile-time machine parameter's structural signature, a trait
//! requirement, or an operator (including a domain's operators). Both the
//! value telescope and the static telescope of the call are read from the one
//! declaration that declares the target, so this join is the single owner of
//! that enumeration; callers project the half they need.

use crate::capture::PackageReviewInput;
use symbols::SymbolHandle;

/// One candidate declaration for a contract call target, retaining both of its
/// telescopes. `conformance_slots` counts the static conformance-bound slots a
/// realized machine adds after its own type-parameter telescope.
pub(crate) struct ContractCallTargetTelescopes<'a> {
    pub(crate) value_parameters: &'a [typed_trees::signature::StateParameter],
    pub(crate) static_parameters: &'a [typed_trees::data::TypeParameter],
    pub(crate) conformance_slots: usize,
}

/// Every checked declaration whose telescope can declare `target`, in
/// declaration-species order. A well-formed call target is declared exactly
/// once here; callers decide how an empty or ambiguous join is refused.
pub(crate) fn contract_call_target_telescopes<'a>(
    compilation: &'a PackageReviewInput<'_>,
    target: SymbolHandle,
) -> Vec<ContractCallTargetTelescopes<'a>> {
    let mut candidates = Vec::new();
    for (machine, state) in compilation.machine_states_named(target) {
        candidates.push(ContractCallTargetTelescopes {
            value_parameters: compilation.state_parameters(state),
            static_parameters: compilation.machine_type_parameters(machine),
            conformance_slots: machine.conformance_bounds.len(),
        });
    }
    if let Some((_, signature)) = compilation.machine_parameter_signature(target) {
        candidates.push(ContractCallTargetTelescopes {
            value_parameters: compilation.state_signature_parameters(signature),
            static_parameters: compilation.state_signature_type_parameters(signature),
            conformance_slots: 0,
        });
    }
    for definition in compilation.traits() {
        for signature in compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target)
        {
            candidates.push(ContractCallTargetTelescopes {
                value_parameters: compilation.state_signature_parameters(signature),
                static_parameters: compilation.state_signature_type_parameters(signature),
                conformance_slots: 0,
            });
        }
    }
    for operator in compilation
        .operators()
        .iter()
        .chain(
            compilation
                .domain_definitions()
                .iter()
                .flat_map(|domain| compilation.domain_operators(domain)),
        )
        .filter(|operator| operator.symbol == target)
    {
        candidates.push(ContractCallTargetTelescopes {
            value_parameters: compilation.operator_parameters(operator),
            static_parameters: compilation.operator_type_parameters(operator),
            conformance_slots: 0,
        });
    }
    candidates
}
