//! Review needs the parameters of the exact function a contract calls.
//! Machines and mathematical functions have different application rules; keep
//! the declaration kind so each uses its own checker.

use crate::capture::SourceDeclarationInput;
use symbols::SymbolHandle;

pub(crate) enum ContractCallDeclaration<'a> {
    Runtime {
        value_parameters: &'a [typed_trees::signature::StateParameter],
        static_parameters: &'a [typed_trees::data::TypeParameter],
        conformance_slots: usize,
    },
    Mathematical(&'a typed_trees::mathematical::MathematicalDefinition),
}

/// Find every declaration with this exact symbol. The caller rejects missing
/// or duplicate declarations instead of choosing one by traversal order.
pub(crate) fn contract_call_target_declarations<'a>(
    compilation: &'a SourceDeclarationInput<'_>,
    target: SymbolHandle,
) -> Vec<ContractCallDeclaration<'a>> {
    let mut candidates = Vec::new();
    for (machine, state) in compilation.machine_states_named(target) {
        candidates.push(ContractCallDeclaration::Runtime {
            value_parameters: compilation.state_parameters(state),
            static_parameters: compilation.machine_type_parameters(machine),
            conformance_slots: machine.conformance_bounds.len(),
        });
    }
    if let Some((_, signature)) = compilation.machine_parameter_signature(target) {
        candidates.push(ContractCallDeclaration::Runtime {
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
            candidates.push(ContractCallDeclaration::Runtime {
                value_parameters: compilation.state_signature_parameters(signature),
                static_parameters: compilation.state_signature_type_parameters(signature),
                conformance_slots: 0,
            });
        }
    }
    for operator in compilation
        .all_operators()
        .filter(|operator| operator.symbol == target)
    {
        candidates.push(ContractCallDeclaration::Runtime {
            value_parameters: compilation.operator_parameters(operator),
            static_parameters: compilation.operator_type_parameters(operator),
            conformance_slots: 0,
        });
    }
    candidates.extend(
        compilation
            .mathematical_definitions()
            .iter()
            .filter(|definition| definition.symbol == target)
            .map(ContractCallDeclaration::Mathematical),
    );
    candidates
}
