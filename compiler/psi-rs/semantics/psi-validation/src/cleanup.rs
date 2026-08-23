use psi_diagnostics::Diagnostic;
use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee, TerminationInterface};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_cleanup_machine_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines().iter().filter(|machine| {
        machine.attached_data.is_some() && machine.name.as_str().ends_with("::drop")
    }) {
        validate_cleanup_machine(program, machine, diagnostics);
    }
}

fn validate_cleanup_machine(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(_attached_name) = machine.attached_data.as_ref() else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` has no entry state; reserved cleanup has shape `drop(&mut self) -> ()`",
            machine.name
        )));
        return;
    };
    let parameters = program.state_parameters(entry);
    let receiver_is_exact = matches!(parameters, [receiver]
    if receiver.is_self
        && receiver.is_mutable
        && !receiver.is_const
            && mutable_reference_targets(
                program,
                receiver.type_reference,
                machine.symbol,
            ));
    if !receiver_is_exact {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must have exactly the receiver `&mut self` and no positional parameters",
            machine.name
        )));
    }
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare method-local lifetime or type parameters",
            machine.name
        )));
    }
    if entry.return_type.is_valid()
        && !matches!(
            program
                .type_reference_table
                .type_reference(entry.return_type),
            TypeReferenceNode::Unit
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must return Unit; fallible cleanup is an explicit consuming machine",
            machine.name
        )));
    }
    if machine.suspends || machine.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must be non-suspending and nonblocking",
            machine.name
        )));
    }
    if program
        .machine_contracts(machine)
        .iter()
        .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare a crash outcome",
            machine.name
        )));
    }
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        && !matches!(
            machine.termination_plan.interface,
            TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless cleanup machine `{}` must publish `terminates;`",
            machine.name
        )));
    }
}

fn mutable_reference_targets(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expected_self_symbol: psi_symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            mutable_reference_targets(program, *base_type, expected_self_symbol)
        }
        TypeReferenceNode::Reference {
            referee, access, ..
        } if access.is_exclusive() => named_type_matches(program, *referee, expected_self_symbol),
        _ => false,
    }
}

fn named_type_matches(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expected_self_symbol: psi_symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_matches(program, *base_type, expected_self_symbol)
        }
        TypeReferenceNode::Named { symbol, name } => {
            *symbol == expected_self_symbol && name.as_str() == "Self"
        }
        _ => false,
    }
}
