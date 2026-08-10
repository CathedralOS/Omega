use crate::data::lower_type_parameter;
use crate::domain::lower_proof_facts;
use crate::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::state::lower_state;
use crate::type_reference::lower_type_reference_into_table;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    let mut typed_machine = typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::name::lower_name(&machine.name),
        attached_data: machine.attached_data.as_ref().map(crate::name::lower_name),
        // Copied, never re-derived.
        supply_mode: machine.supply_mode,
        // TPR2: copied, never re-derived (populated at syntax->resolved).
        // TPR4 slice 3: an implementation satisfying a requirement that
        // authored `terminates;` INHERITS the published guarantee (see
        // inherit_requirement_guarantee below).
        termination_plan: inherit_requirement_guarantee(lowerer, machine),
        service_reach_row: machine.service_reach_row,
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(crate::name::lower_name)
            .collect(),
        type_parameters: psi_arena::HandleSpan::empty(),
        owned_data: psi_arena::HandleSpan::empty(),
        satisfies: psi_arena::HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        decreases: psi_arena::HandleSpan::empty(),
        decrease_order: psi_arena::HandleSpan::empty(),
        decrease_view_arguments: psi_arena::HandleSpan::empty(),
        decrease_range: typed::expression::ExpressionHandle::invalid(),
        service_reaches: psi_arena::HandleSpan::empty(),
        invokes: psi_arena::HandleSpan::empty(),
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: psi_arena::HandleSpan::empty(),
        states: psi_arena::HandleSpan::empty(),
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(machine.type_parameters)
    {
        let type_parameter = lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_machine_type_parameter(&mut typed_machine, type_parameter);
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
        let mut arguments = psi_arena::HandleSpan::empty();
        for argument in lowerer
            .source_trees
            .child_type_references(conformance.arguments)
        {
            let argument =
                crate::type_reference::lower_type_reference_into_table(lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        lowerer.typed_trees.push_machine_trait_conformance(
            &mut typed_machine,
            typed::machine::TraitConformance {
                symbol: conformance.symbol,
                name: crate::name::lower_name(&conformance.name),
                arguments,
                requirement: conformance
                    .requirement
                    .as_ref()
                    .map(crate::name::lower_name),
                alias: conformance.alias.as_ref().map(crate::name::lower_name),
                via: conformance.via.clone(),
            },
        );
    }

    for bound in &machine.conformance_bounds {
        let mut arguments = Vec::new();
        for argument in lowerer.source_trees.child_type_references(bound.arguments) {
            arguments.push(lower_type_reference_into_table(lowerer, argument)?);
        }
        typed_machine
            .conformance_bounds
            .push(typed::machine::GenericConformanceBound {
                subject: bound.subject,
                subject_name: crate::name::lower_name(&bound.subject_name),
                carrier: bound.carrier,
                carrier_name: crate::name::lower_name(&bound.carrier_name),
                arguments,
                conformance: bound.conformance,
                conformance_name: bound.conformance_name.as_ref().map(crate::name::lower_name),
            });
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
    // TPR3: argumented-view arguments lower exactly like the subjects.
    let mut view_arguments = Vec::new();
    for argument in lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(machine.decrease_view_arguments)
    {
        let argument = lower_expression_handle(lowerer, *argument)?;
        view_arguments.push(argument);
    }
    typed_machine.decrease_view_arguments = lowerer
        .typed_trees
        .expression_table
        .insert_expression_handles(view_arguments);
    // TPR3 slice 3: the rank-range constraint (invalid = absent).
    if machine.decrease_range.is_valid() {
        typed_machine.decrease_range = lower_expression_handle(lowerer, machine.decrease_range)?;
    }
    for member in lowerer
        .source_trees
        .machine_decrease_order(machine.decrease_order)
    {
        lowerer.typed_trees.decrease_orders.append_to_span(
            &mut typed_machine.decrease_order,
            crate::name::lower_name(member),
        );
    }

    for service in lowerer.source_trees.machine_service_reaches(machine) {
        let service = crate::name::lower_name(service);
        lowerer
            .typed_trees
            .push_machine_service_reach(&mut typed_machine, service);
    }

    for binding in lowerer.source_trees.machine_invokes(machine) {
        lowerer
            .typed_trees
            .push_machine_invoke(&mut typed_machine, crate::name::lower_name(binding));
    }

    for contract in lowerer.source_trees.machine_contracts(machine) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_machine_contract(
            &mut typed_machine,
            typed::signature::SignatureContract {
                kind: lower_contract_kind(&contract.kind),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    for state in lowerer.source_trees.machine_state_handles(machine.states) {
        let state = lowerer.source_trees.machine_state(*state);
        // The state body's value-typing scope lets `==` lowering find an
        // operand's data type for structural-equality expansion.
        lowerer.equality_scope = Some(crate::equatable::EqualityScope::for_state(
            lowerer.source_trees,
            machine,
            state,
        ));
        let state = lower_state(lowerer, machine.attached_data.as_ref(), state)?;
        lowerer.equality_scope = None;
        lowerer
            .typed_trees
            .push_machine_state(&mut typed_machine, state);
    }

    // #66 Phase 1 (returns) NOT YET: synthesizing an `ensures result in Domain`
    // for a `-> T in Domain` return type is VACUOUS today -- the contract
    // entailment proves arithmetic ensures (polynomials over `result`) but does
    // not PROVE a domain-MEMBERSHIP ensures from a machine body, so the obligation
    // is silently skipped (a fail canary returning a raw value compiled). Returns
    // need a direct return-membership check (the #40 return-range parallel, but on
    // the membership engine), not a desugar. Deferred to avoid shipping a vacuous
    // (unsound) obligation. Params (above) work because their `requires` is checked
    // at CALL sites, which is robust.

    Ok(typed_machine)
}

/// TPR4 slice 3 (decision 23): "an implementation satisfying a requirement
/// inherits the requirement's guarantee and premises. It does not repeat
/// `terminates;`; a textual `terminates by ...` on the implementation
/// supplies only the witness needed to discharge the inherited claim."
/// The inheritance happens HERE (the resolved->typed machine lowering),
/// where the conformance edge and the requirement's signature flag are both
/// in reach -- so the TPR3-migrated checker's plan gate then enforces the
/// inherited claim for free (a cyclic inheritor without a witness fails
/// with the missing-witness diagnostic). Requirement matching mirrors the
/// conformance validator's carrier model: an explicitly named requirement,
/// or the machine's own SIMPLE name (free machines conform
/// machine-by-machine; attached machines' whole-trait conformance matches
/// requirements by simple name). An authored guarantee is never overwritten.
fn inherit_requirement_guarantee(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
) -> psi_language_semantics::MachineTerminationPlan {
    use psi_language_semantics::{TerminationGuarantee, TerminationInterface};

    let mut plan = machine.termination_plan.clone();
    if matches!(
        &plan.interface,
        TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
    ) {
        return plan;
    }
    let simple_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());
    for conformance in lowerer
        .source_trees
        .machine_trait_conformances(machine.satisfies)
    {
        let Some(trait_definition) = lowerer
            .source_trees
            .traits
            .iter()
            .find(|definition| definition.symbol == conformance.symbol)
        else {
            continue;
        };
        let required_name = conformance
            .requirement
            .as_ref()
            .map(|requirement| requirement.as_str())
            .unwrap_or(simple_name);
        let inherited = lowerer
            .source_trees
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .find(|requirement| requirement.name.as_str() == required_name);
        if let Some(requirement) = inherited {
            plan.interface = TerminationInterface::Published(if requirement.terminates_guarantee {
                TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                }
            } else {
                TerminationGuarantee::NoGuarantee
            });
            break;
        }
    }
    plan
}

fn lower_contract_kind(
    kind: &resolved::signature::SignatureContractKind,
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
        resolved::signature::SignatureContractKind::Crashes { cause } => {
            typed::signature::SignatureContractKind::Crashes {
                cause: match cause {
                    resolved::signature::CrashCause::Trap => typed::signature::CrashCause::Trap,
                    resolved::signature::CrashCause::Abort => typed::signature::CrashCause::Abort,
                },
            }
        }
    }
}
