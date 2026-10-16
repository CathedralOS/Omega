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
//! requirement stays generic; instantiation only adds type rows and symbols.

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
    for demand in demands {
        let interface_identity = program.normalized_type_identity(demand.selector_type);
        if program
            .dynamic_slot(interface_identity, demand.requirement)
            .is_none()
        {
            let slot = instantiate_slot(program, &demand, interface_identity)?;
            program.dynamic_slots.push(slot);
        }
    }
    retarget_selector_calls(program);
    Ok(())
}

fn instantiate_slot(
    program: &mut TypedTrees,
    demand: &validation::DynamicSlotDemand,
    interface_identity: typed_trees::type_identity::TypeIdentityId,
) -> Result<DynamicSlotSignature, Vec<Diagnostic>> {
    let Some(definition) = program.trait_definition_by_symbol(demand.declaring_trait) else {
        return Err(vec![Diagnostic::error(
            "dynamic slot names no declaring trait",
        )]);
    };
    let Some(signature) = program
        .trait_machine_signature_by_symbol(definition, demand.requirement)
        .cloned()
    else {
        return Err(vec![Diagnostic::error(
            "dynamic slot names no requirement of its declaring trait",
        )]);
    };
    let binders = program
        .trait_type_parameters(definition)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let declaring_arguments = demand
        .substitution_scope_starts
        .last()
        .map_or(&[][..], |start| &demand.type_substitutions[*start..]);
    if binders.len() != declaring_arguments.len() {
        return Err(vec![Diagnostic::error(format!(
            "`dyn {}` supplies {} trait argument(s) for {} parameter(s)",
            definition.name,
            declaring_arguments.len(),
            binders.len()
        ))]);
    }
    let selector_display = program.display_type_reference(demand.selector_type);
    let instantiation_error = |reason: &'static str| {
        vec![Diagnostic::error(format!(
            "requirement `{}` cannot be instantiated for `{selector_display}`: {reason}",
            signature.name
        ))]
    };
    let formals = program.state_signature_parameters(&signature).to_vec();
    let mut parameters = Vec::with_capacity(formals.len() + 1);
    parameters.push(StateParameter {
        name: Identifier::generated_static("selector"),
        type_reference: demand.selector_type,
        ..StateParameter::default()
    });
    for formal in &formals {
        let mut parameter = formal.clone();
        parameter.type_reference =
            instantiate_requirement_type(program, formal.type_reference, demand)
                .map_err(instantiation_error)?;
        parameters.push(parameter);
    }
    let return_type = instantiate_requirement_type(program, signature.return_type, demand)
        .map_err(instantiation_error)?;

    // The slot and its formals are fresh symbols derived from the
    // requirement, so facts keyed by a callee formal never alias the generic
    // requirement's own formals across two selector applications.
    let symbol = program.symbols.insert_generated_root_from(
        demand.requirement,
        program.symbols.get(demand.requirement).kind,
        &format!("{selector_display}::{}", signature.name),
    );
    let names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let origins = std::iter::once(demand.requirement)
        .chain(formals.iter().map(|formal| formal.symbol))
        .collect::<Vec<_>>();
    let children = program.symbols.insert_generated_children_from(
        symbol,
        origins
            .iter()
            .zip(&names)
            .map(|(origin, name)| (*origin, symbols::SymbolKind::Parameter, name.as_str())),
    );
    for (offset, parameter) in (0u32..).zip(parameters.iter_mut()) {
        parameter.symbol = children.handle_at(offset);
    }
    Ok(DynamicSlotSignature {
        symbol,
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
/// Instantiate the inner requirement before substituting its outer scopes.
/// Each telescope is simultaneous: following one replacement through another
/// binder of that same telescope would capture caller-side parameters.
fn instantiate_requirement_type(
    program: &mut TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
    demand: &validation::DynamicSlotDemand,
) -> Result<typed_trees::types::TypeReferenceHandle, &'static str> {
    let mut end = demand.type_substitutions.len();
    for start in demand.substitution_scope_starts.iter().rev() {
        reference = program
            .type_reference_table
            .instantiate(reference, &demand.type_substitutions[*start..end])?;
        end = *start;
    }
    Ok(reference)
}

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
                for (_, root) in program.tables.expression_roots(statement) {
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
