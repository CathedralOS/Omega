use crate::data::lower_type_parameter;
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
        boundary: machine.boundary,
        // STR3: copied, never re-derived.
        supply_mode: machine.supply_mode,
        // TPR2: copied, never re-derived (populated at syntax->resolved).
        // TPR4 slice 3: an implementation satisfying a requirement that
        // authored `terminates;` INHERITS the published guarantee (see
        // inherit_requirement_guarantee below).
        termination_plan: inherit_requirement_guarantee(lowerer, machine),
        // STR4: copied, never re-derived (the row table copies verbatim at
        // the tree level, so ids stay valid).
        effect_row: machine.effect_row,
        service_reach_row: machine.service_reach_row,
        type_parameters: omega_core::arena::HandleSpan::empty(),
        contains: omega_core::arena::HandleSpan::empty(),
        owned_data: omega_core::arena::HandleSpan::empty(),
        satisfies: omega_core::arena::HandleSpan::empty(),
        terminates: machine.terminates,
        decreases: omega_core::arena::HandleSpan::empty(),
        decrease_order: omega_core::arena::HandleSpan::empty(),
        decrease_view_arguments: omega_core::arena::HandleSpan::empty(),
        decrease_range: typed::expression::ExpressionHandle::invalid(),
        effects: omega_core::arena::HandleSpan::empty(),
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: omega_core::arena::HandleSpan::empty(),
        states: omega_core::arena::HandleSpan::empty(),
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
                requirement: conformance
                    .requirement
                    .as_ref()
                    .map(crate::name::lower_name),
                alias: conformance.alias.as_ref().map(crate::name::lower_name),
                via: conformance.via.clone(),
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

    // (#66 Phase 1 synthesizes additional implicit `requires <param> in Domain`
    // machine contracts AFTER the states are lowered -- see the loop below the
    // states loop, which needs the lowered params to detect Domain constraints.)
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

    // #66/DOM1: every predicate facet on a parameter typed `T in A & B`
    // desugars to an implicit `requires <param> in <domain>` MACHINE contract.
    // Semantic-only facets remain qualifications and never become obligations.
    // Collected first (immutable read of the lowered states/params) then synthesized,
    // to keep the typed-tree borrow disjoint from the contract construction.
    let mut domain_constrained_parameters: Vec<(
        omega_core::symbols::SymbolHandle,
        typed::name::Identifier,
        omega_core::symbols::SymbolHandle,
        String,
    )> = Vec::new();
    for state in lowerer.typed_trees.machine_states(&typed_machine) {
        for parameter in lowerer.typed_trees.state_parameters(state) {
            for (domain_symbol, domain_full_name) in crate::state::predicate_domain_constraints(
                &lowerer.typed_trees,
                parameter.type_reference,
            ) {
                domain_constrained_parameters.push((
                    parameter.symbol,
                    parameter.name.clone(),
                    domain_symbol,
                    domain_full_name,
                ));
            }
        }
    }
    for (param_symbol, param_name, domain_symbol, domain_full_name) in domain_constrained_parameters
    {
        let contract = crate::state::build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
        );
        lowerer
            .typed_trees
            .push_machine_contract(&mut typed_machine, contract);
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
) -> omega_core::semantics::MachineTerminationPlan {
    use omega_core::semantics::TerminationGuarantee;

    let mut plan = machine.termination_plan.clone();
    if plan.published.is_some() {
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
            .any(|requirement| {
                requirement.terminates_guarantee && requirement.name.as_str() == required_name
            });
        if inherited {
            plan.published = Some(TerminationGuarantee::EventualTerminal {
                premises: Vec::new(),
            });
            break;
        }
    }
    plan
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
