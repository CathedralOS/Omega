//! Exact opaque synchronous callbacks shared by reach and operational inference.
//! `invokes` permits entering a binding; it does not select every operation on
//! that binding. A nominal callback telescope supplies the exact requirement.
//! This is the same binder/trait join used by realization's callback roster,
//! including callbacks materialized through private slots rather than direct
//! native parameters. Registration without synchronous invocation adds no row.

use flow_effects::InvocationTarget;
use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use typed_trees::signature::StateSignature;

/// Specialization erases executable binders, not an opaque requirement's
/// published callback ceiling. Its exact specialization receipt retains the
/// original telescope. Reach identities and operational flags are declaration
/// axes; selecting a quieter callback cannot narrow that opaque contract.
/// Checked bodies instead infer their actual calls and never use this rejoin.
pub(super) fn opaque_machine_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> &'program [TypeParameter] {
    match program.machine_specialization(machine.symbol) {
        Some(specialization) => program
            .data_type_parameters
            .span_or_empty(specialization.template_parameters),
        None => program.machine_type_parameters(machine),
    }
}

/// A closed requirement erases executable binders, while its opaque callback
/// ceiling still belongs to the exact authored declaration telescope. Keep
/// the same declaration axis as opaque machine specializations.
pub(super) fn opaque_signature_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    signature: &'program StateSignature,
) -> &'program [TypeParameter] {
    let declaration = program
        .closed_requirement_application(signature.symbol)
        .and_then(|application| program.trait_machine_signature(application.template))
        .map_or(signature, |(_, declaration)| declaration);
    program.state_signature_type_parameters(declaration)
}

pub(super) fn visit(
    program: &dyn crate::CheckingInput,
    parameters: &[TypeParameter],
    invocations: &[InvocationTarget],
    mut consume: impl FnMut(&StateSignature),
) {
    let mut pending = vec![(parameters, invocations.to_vec())];
    let mut visited = Vec::new();
    while let Some((parameters, invocations)) = pending.pop() {
        for parameter in parameters {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let MachineParameterContract::Nominal {
                trait_definition, ..
            } = contract
            else {
                continue;
            };
            if !invocations.contains(&InvocationTarget::Service(*trait_definition))
                || visited.contains(&parameter.symbol)
            {
                continue;
            }
            // Callback contracts can themselves contain callback telescopes.
            // Visit each exact binder once so cyclic declarations terminate;
            // synchronous-cycle validation remains an independent obligation.
            visited.push(parameter.symbol);
            let Some(contract) = program.machine_parameter_contract_view(contract) else {
                continue;
            };
            let signature = contract.signature();
            consume(signature);
            pending.push((
                opaque_signature_parameters(program, signature),
                super::declared_signature_invocations(program, signature),
            ));
        }
    }
}
