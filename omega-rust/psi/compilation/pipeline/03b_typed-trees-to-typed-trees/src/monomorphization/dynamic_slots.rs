//! Requirement signatures instantiated for runtime implementation selection.
//!
//! `Trait::member(selector, data...)` invokes whichever implementation the
//! selector names, so its operands are planned against the requirement, not
//! against a selected realization. The requirement is declared over the
//! trait's parameters (`code(subject: &T) -> i32`), while checking and
//! lowering pair each operand with a concrete formal to choose scalar and
//! structural lanes, borrow access, ownership transfer and result carriers.
//!
//! Each reached requirement is therefore instantiated once per selector
//! application into a slot with its own symbol (`dyn Shape<Item>` gives
//! `subject: &Item`), and every selector call is retargeted to its slot the
//! way specialization retargets a generic call to its instance. The slot's
//! roster mirrors the call's operands: the selector first, then the
//! instantiated formals. Every consumer that pairs a call's operands with its
//! target's formals then sees an ordinary bodyless callee of the right arity
//! instead of a generic requirement whose roster omits the selector; threading
//! trait substitutions and a selector offset through each consumer instead
//! would duplicate that pairing at every site. The slot also gives the
//! dispatch catalog the interface's call shape without electing one
//! implementation as representative.
//!
//! This runs once specialization and call rebinding have converged, before
//! typed validation: a generated body can contain a new selection or selector
//! call, and validation must judge the final call edges. The authored
//! requirement stays generic; instantiation adds owned public header graphs and symbols.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::dynamic_traits::DynamicSlotSignature;
use typed_trees::expression::ExpressionNode;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;

/// Instantiate every (selector application, requirement) the program's
/// selections and selector calls reach, then retarget each selector call to
/// its slot.
///
/// An invalid selection or selector call is reported by typed validation;
/// collection failure here defers to that authoritative diagnostic.
pub(crate) fn instantiate_dynamic_slots(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let Ok(demands) = validation::collect_dynamic_slot_demands(program) else {
        return Ok(());
    };
    // Every collected demand belongs to this original payload owner. Prepare
    // it once; fresh slot graphs do not change another demand's substitutions.
    let source = program.clone();
    for demand in demands {
        let interface_identity = program.normalized_type_identity(demand.selector_type);
        if program
            .dynamic_slot(interface_identity, demand.requirement)
            .is_none()
        {
            let slot = instantiate_slot(&source, program, &demand, interface_identity)?;
            let symbol = slot.symbol;
            program.dynamic_slots.push(slot);
            super::finalize_domain_instance_identities(
                program,
                &typed_trees::references::DeclarationSelection::from_symbols([symbol]),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    retarget_selector_calls(program);
    Ok(())
}

fn instantiate_slot(
    source: &TypedTrees,
    program: &mut TypedTrees,
    demand: &validation::DynamicSlotDemand,
    interface_identity: typed_trees::type_identity::TypeIdentityId,
) -> Result<DynamicSlotSignature, Vec<Diagnostic>> {
    let Some(definition) = source.trait_definition_by_symbol(demand.declaring_trait) else {
        return Err(vec![Diagnostic::error(
            "dynamic slot names no declaring trait",
        )]);
    };
    let Some(signature) = source
        .trait_machine_signature_by_symbol(definition, demand.requirement)
        .cloned()
    else {
        return Err(vec![Diagnostic::error(
            "dynamic slot names no requirement of its declaring trait",
        )]);
    };
    let scope = demand.application.bind(source).ok_or_else(|| {
        vec![Diagnostic::error(
            "dynamic slot lost its original applied scope owner",
        )]
    })?;
    if scope.declaration.symbol != demand.declaring_trait {
        return Err(vec![Diagnostic::error(
            "dynamic slot lost its declaring scope",
        )]);
    }
    let selector_display = program.display_type_reference(demand.selector_type);
    let mut selector = StateParameter {
        // The selector is a dispatch operand derived from the requirement,
        // not one of the requirement's authored public formals.
        symbol: demand.requirement,
        name: Identifier::generated_static("selector"),
        type_reference: demand.selector_type,
        ..StateParameter::default()
    };
    let copied = super::signature_cloning::copy_applied_signature(
        source,
        program,
        &signature,
        &scope,
        &[],
        std::slice::from_ref(&selector),
        &format!("{selector_display}::{}", signature.name),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    selector.symbol = copied.leading_parameter_symbols.handle_at(0);
    let contract_fact_copies = copied.contract_fact_copies;
    let applied = copied.signature;
    let symbol = applied.symbol;
    let mut parameters = Vec::with_capacity(applied.parameters.len() + 1);
    parameters.push(selector);
    parameters.extend_from_slice(program.state_signature_parameters(&applied));
    let return_type = applied.return_type;
    let applied_signature = program.trait_machine_signatures.insert(applied);
    Ok(DynamicSlotSignature {
        symbol,
        applied_signature,
        contract_fact_copies,
        interface_identity,
        declaring_trait: demand.declaring_trait,
        requirement: demand.requirement,
        parameters,
        return_type,
    })
}

/// Point every selector call at its slot. A call carrying static machine
/// arguments selects a finite-family row through the same slot: the slot's
/// formals stay the selector and the requirement's instantiated parameters,
/// while the arguments ride on the call as family actuals that call lowering
/// turns into the dispatch's row-selection operand.
fn retarget_selector_calls(program: &mut TypedTrees) {
    let mut statement_targets = Vec::new();
    let mut expression_targets = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = state.statement_nodes;
            for (offset, statement) in program
                .statement_table
                .statements(statements)
                .iter()
                .enumerate()
            {
                if let Some(call) = statement.as_call()
                    && let Some(slot) = slot_for(
                        program,
                        validation::dynamic_call_demand(
                            program,
                            machine,
                            state,
                            call.receiver_symbol,
                            call.target_symbol,
                            program.statement_table.expression_handles(call.arguments),
                        ),
                    )
                {
                    statement_targets.push((statements.handle_at(offset as u32), slot));
                }
                for (_, root) in program.statement_table.expression_roots(statement) {
                    if !root.is_valid() {
                        continue;
                    }
                    let mut tree = Vec::new();
                    validation::expression_traversal::collect_expression_tree(
                        program, root, &mut tree,
                    );
                    for expression in tree {
                        let Some(call) = program.expression_table.call_node(expression) else {
                            continue;
                        };
                        if let Some(slot) = slot_for(
                            program,
                            validation::dynamic_expression_call_demand(
                                program, machine, state, call,
                            ),
                        ) && !expression_targets
                            .iter()
                            .any(|(known, _)| *known == expression)
                        {
                            expression_targets.push((expression, slot));
                        }
                    }
                }
            }
        }
    }
    for (statement, (slot, name)) in statement_targets {
        if let StatementNode::Call(call) = program.statement_table.statement_mut(statement) {
            call.target_symbol = slot;
            call.target = name;
        }
    }
    for (expression, (slot, name)) in expression_targets {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) {
            call.target_symbol = slot;
            call.target = name;
        }
    }
}

fn slot_for(
    program: &dyn ::validation::CheckingInput,
    demand: Option<validation::DynamicSlotDemand>,
) -> Option<(SymbolHandle, Identifier)> {
    let demand = demand?;
    let identity = program.normalized_type_identity(demand.selector_type);
    let slot = program.dynamic_slot(identity, demand.requirement)?;
    Some((
        slot.symbol,
        Identifier::generated(program.symbols.name(slot.symbol).to_owned()),
    ))
}
