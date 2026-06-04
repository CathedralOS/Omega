use crate::domain::lower_proof_facts;
use crate::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::state::lower_state;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    let mut typed_machine = typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::name::lower_name(&machine.name),
        attached_data: machine.attached_data.as_ref().map(crate::name::lower_name),
        abi: machine.abi.clone(),
        type_parameters: omega_core::arena::HandleSpan::empty(),
        contains: omega_core::arena::HandleSpan::empty(),
        owned_data: omega_core::arena::HandleSpan::empty(),
        satisfies: omega_core::arena::HandleSpan::empty(),
        terminates: machine.terminates,
        decreases: omega_core::arena::HandleSpan::empty(),
        decrease_order: omega_core::arena::HandleSpan::empty(),
        effects: omega_core::arena::HandleSpan::empty(),
        contracts: omega_core::arena::HandleSpan::empty(),
        states: omega_core::arena::HandleSpan::empty(),
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(machine.type_parameters)
    {
        lowerer.typed_trees.push_machine_type_parameter(
            &mut typed_machine,
            typed::data::TypeParameter {
                symbol: parameter.symbol,
                name: crate::name::lower_name(&parameter.name),
            },
        );
    }

    for contained_object in lowerer
        .source_trees
        .machine_contained_objects(machine.contains)
    {
        let contained_object = typed::machine::ContainedObject {
            symbol: contained_object.symbol,
            type_symbol: contained_object.type_symbol,
            name: crate::name::lower_name(&contained_object.name),
            type_name: crate::name::lower_name(&contained_object.type_name),
        };
        lowerer
            .typed_trees
            .push_machine_contained_object(&mut typed_machine, contained_object);
    }

    for owned_data in lowerer.source_trees.machine_owned_data(machine.owned_data) {
        let owned_data = typed::machine::OwnedData {
            symbol: owned_data.symbol,
            name: crate::name::lower_name(&owned_data.name),
            type_reference: lower_type_reference_into_table(lowerer, &owned_data.type_reference)?,
            initial_value: owned_data
                .initial_value
                .is_valid()
                .then(|| lower_expression_handle(lowerer, owned_data.initial_value))
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
        };
        lowerer
            .typed_trees
            .push_machine_owned_data(&mut typed_machine, owned_data);
    }

    for conformance in lowerer
        .source_trees
        .machine_trait_conformances(machine.satisfies)
    {
        lowerer.typed_trees.push_machine_trait_conformance(
            &mut typed_machine,
            typed::machine::TraitConformance {
                symbol: conformance.symbol,
                name: crate::name::lower_name(&conformance.name),
            },
        );
    }

    let mut decreases = Vec::new();
    for decrease in lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(machine.decreases)
    {
        let decrease = lower_expression_handle(lowerer, *decrease)?;
        decreases.push(decrease);
    }
    typed_machine.decreases = lowerer
        .typed_trees
        .expression_table
        .insert_expression_handles(decreases);
    for member in lowerer
        .source_trees
        .machine_decrease_order(machine.decrease_order)
    {
        lowerer.typed_trees.signature_effects.append_to_span(
            &mut typed_machine.decrease_order,
            crate::name::lower_name(member),
        );
    }

    for effect in lowerer.source_trees.machine_effects(machine) {
        let effect = crate::name::lower_name(effect);
        lowerer
            .typed_trees
            .push_machine_effect(&mut typed_machine, effect);
    }

    for contract in lowerer.source_trees.machine_contracts(machine) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_machine_contract(
            &mut typed_machine,
            typed::signature::SignatureContract {
                kind: lower_contract_kind(contract.kind),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    for state in lowerer.source_trees.machine_state_handles(machine.states) {
        let state = lowerer.source_trees.machine_state(*state);
        let state = lower_state(lowerer, state)?;
        lowerer
            .typed_trees
            .push_machine_state(&mut typed_machine, state);
    }

    Ok(typed_machine)
}

fn lower_contract_kind(
    kind: resolved::signature::SignatureContractKind,
) -> typed::signature::SignatureContractKind {
    match kind {
        resolved::signature::SignatureContractKind::Requires => {
            typed::signature::SignatureContractKind::Requires
        }
        resolved::signature::SignatureContractKind::Ensures => {
            typed::signature::SignatureContractKind::Ensures
        }
        resolved::signature::SignatureContractKind::Boundary => {
            typed::signature::SignatureContractKind::Boundary
        }
    }
}
