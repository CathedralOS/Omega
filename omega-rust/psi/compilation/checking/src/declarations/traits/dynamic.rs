//! Runtime selection of named implementations (`dyn Trait<Args>`).
//!
//! A `dyn Trait<Args>` value selects one named implementation of exactly that
//! trait application at runtime. It packages no data instance: copying,
//! storing or forwarding a selector borrows nothing, and every data value is
//! an ordinary argument of the call it reaches. `Impl as dyn Trait<Args>` is
//! the only constructor; it names the implementation, never a data place, so
//! there is no implementation search over a value's type. A call names the
//! requirement through its trait and passes the selector first:
//! `Trait::member(selector, data...)`. The selector is the dispatch operand,
//! not a requirement parameter
//! ([dynamic dispatch](../../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)).
//!
//! Rebinding, joins, storage and forwarding are ordinary value flow of the
//! selector; this module only fixes which implementation each conversion
//! selects. Call checking (`machine_calls::calls::generic_requirement`)
//! instantiates the requirement under the selector's trait application the
//! same way an evidence binder's bound does.
//!
//! The receiver-call binding below is older and separate: a call through a
//! `Binding<R>` service value binds its exact requirement target here.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One `Impl as dyn Trait<Args>` conversion: the exact named implementation it
/// selects for the selector type at `occurrence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicImplementationSelection {
    /// The conversion expression itself.
    pub occurrence: ExpressionHandle,
    pub machine: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    /// The selected named implementation (a `Conformance` symbol).
    pub implementation: symbols::SymbolHandle,
    /// The `dyn Trait<Args>` type the conversion produces.
    pub selector_type: TypeReferenceHandle,
    pub target_trait: symbols::SymbolHandle,
}

/// Fix the implementation every `Impl as dyn Trait<Args>` conversion selects.
/// The implementation must be a closed implementation of exactly the
/// selector's trait application: a selection never widens, narrows or
/// re-targets the application it was declared for.
pub fn collect_dynamic_implementation_selections(
    program: &TypedTrees,
) -> Result<Vec<DynamicImplementationSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_selections_in(
                        program,
                        machine,
                        state,
                        root,
                        &mut visited,
                        &mut selections,
                        &mut diagnostics,
                    );
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

fn collect_selections_in(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
    selections: &mut Vec<DynamicImplementationSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    crate::expression_traversal::children(program, node, |child| {
        collect_selections_in(
            program,
            machine,
            state,
            child,
            visited,
            selections,
            diagnostics,
        )
    });
    let ExpressionNode::Cast(cast) = node else {
        return;
    };
    let TypeReferenceNode::DynamicTrait {
        symbol: target_trait,
        name: trait_name,
        arguments,
    } = program
        .type_reference_table
        .type_reference(cast.target_type)
    else {
        return;
    };
    let implementation = match program.expression_table.expression(cast.value) {
        ExpressionNode::Name(name)
            if name.symbol.is_valid()
                && program.symbols.get(name.symbol).kind == symbols::SymbolKind::Conformance =>
        {
            name.symbol
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` converts a value to `dyn {trait_name}`; a selector is constructed from a named implementation (`Implementation as dyn {trait_name}<...>`) and the data is passed to each call",
                cast.display_name(&program.expression_table)
            )));
            return;
        }
    };
    let Some(conformance) = program
        .conformances()
        .iter()
        .find(|conformance| conformance.symbol == implementation)
    else {
        return;
    };
    let implementation_name = program.symbols.name(implementation);
    if program.closed_conformance_rows(conformance).is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "`{implementation_name}` declares no complete row map and cannot be selected at runtime; declare it with a `{{ ... }}` implementation block"
        )));
        return;
    }
    let selector_arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let implementation_arguments = program
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let same_application = conformance.trait_symbol == *target_trait
        && selector_arguments.len() == implementation_arguments.len()
        && selector_arguments.iter().zip(implementation_arguments).all(
            |(selector, implementation)| {
                program.normalized_type_identity(*selector)
                    == program.normalized_type_identity(*implementation)
            },
        );
    if !same_application {
        let display =
            |handle: TypeReferenceHandle| program.type_reference_table.display_name(handle);
        let implemented = format!(
            "{}<{}>",
            conformance.trait_name,
            implementation_arguments
                .iter()
                .map(|argument| display(*argument))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let selected = format!(
            "dyn {trait_name}<{}>",
            selector_arguments
                .iter()
                .map(|argument| display(*argument))
                .collect::<Vec<_>>()
                .join(", ")
        );
        diagnostics.push(Diagnostic::error(format!(
            "`{implementation_name}` implements `{implemented}`, not `{selected}`; a selector names exactly the trait application its implementation satisfies"
        )));
        return;
    }
    selections.push(DynamicImplementationSelection {
        occurrence: expression,
        machine: machine.symbol,
        state: state.symbol,
        implementation,
        selector_type: cast.target_type,
        target_trait: *target_trait,
    });
}

/// One requirement a dynamic caller reaches through one selector
/// application: what stage 04 instantiates into a
/// [`typed_trees::dynamic_traits::DynamicSlotSignature`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSlotDemand {
    /// The `dyn Trait<Args>` selector type.
    pub selector_type: TypeReferenceHandle,
    pub declaring_trait: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    /// The declaring trait's arguments under the selector's application; an
    /// inherited requirement's declaring trait receives the arguments its
    /// inheritance edge composes.
    pub trait_arguments: Vec<TypeReferenceHandle>,
}

/// Every requirement the program's selections and selector calls reach.
///
/// A selection demands its implementation's complete row map, because every
/// row is a table slot whether or not a call names it. A call demands the
/// requirement it invokes, which covers a helper that only forwards a
/// selector it was given. Demands are unique by selector type identity and
/// requirement. Collection diagnostics are returned unchanged; call checking
/// reports a malformed selector call.
pub fn collect_dynamic_slot_demands(
    program: &TypedTrees,
) -> Result<Vec<DynamicSlotDemand>, Vec<Diagnostic>> {
    let mut demands: Vec<(String, DynamicSlotDemand)> = Vec::new();
    let mut push = |demand: DynamicSlotDemand| {
        let identity = program
            .normalized_type_identity(demand.selector_type)
            .into_string();
        if !demands.iter().any(|(existing, known)| {
            *existing == identity && known.requirement == demand.requirement
        }) {
            demands.push((identity, demand));
        }
    };
    for selection in collect_dynamic_implementation_selections(program)? {
        let Some(implementation) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == selection.implementation)
        else {
            continue;
        };
        let Some((selector_trait, selector_arguments)) =
            dynamic_selector_application(program, selection.selector_type)
        else {
            continue;
        };
        let Some(selector_definition) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == selector_trait)
        else {
            continue;
        };
        for row in program
            .closed_conformance_rows(implementation)
            .unwrap_or_default()
        {
            let Some(trait_arguments) = super::arguments_for_declaring_trait(
                program,
                selector_definition,
                selector_arguments,
                row.declaring_trait,
                &mut Vec::new(),
            ) else {
                continue;
            };
            push(DynamicSlotDemand {
                selector_type: selection.selector_type,
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                trait_arguments,
            });
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement
                    && let Some(demand) = dynamic_call_demand(
                        program,
                        machine,
                        state,
                        call.receiver_symbol,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    )
                {
                    push(demand);
                }
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_call_demands_in(program, machine, state, root, &mut visited, &mut push);
                }
            }
        }
    }
    Ok(demands.into_iter().map(|(_, demand)| demand).collect())
}

fn collect_call_demands_in(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
    push: &mut impl FnMut(DynamicSlotDemand),
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    crate::expression_traversal::children(program, node, |child| {
        collect_call_demands_in(program, machine, state, child, visited, push)
    });
    if let ExpressionNode::Call(call) = node
        && let Some(demand) = dynamic_expression_call_demand(program, machine, state, call)
    {
        push(demand);
    }
}

/// [`dynamic_call_demand`] for an expression-position call.
pub fn dynamic_expression_call_demand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<DynamicSlotDemand> {
    dynamic_call_demand(
        program,
        machine,
        state,
        crate::machine_calls::calls::dynamic_requirement::expression_trait_receiver(
            program,
            call.receiver,
        ),
        call.target_symbol,
        program.expression_table.expression_handles(call.arguments),
    )
}

/// The slot one call reaches when it is `Trait::member(selector, ...)`.
/// Any other call, or a selector call checking rejects, demands nothing.
pub fn dynamic_call_demand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
) -> Option<DynamicSlotDemand> {
    let call = crate::machine_calls::calls::dynamic_requirement::dynamic_selector_call(
        program,
        machine,
        Some(state),
        receiver,
        target,
        arguments,
    )
    .ok()??;
    Some(DynamicSlotDemand {
        selector_type: call.selector_type,
        declaring_trait: call.trait_definition.symbol,
        requirement: call.signature.symbol,
        trait_arguments: call.bound.arguments,
    })
}

/// Why `requirement` cannot be called through `dyn Trait`: its signature or
/// contract is outside the local dynamic surface. `None` when it can.
pub(crate) fn dynamic_requirement_ineligibility(
    program: &TypedTrees,
    declaring_trait: &TraitDefinition,
    requirement: &StateSignature,
) -> Option<&'static str> {
    if let Err(reason) = program.dynamic_signature_eligibility(declaring_trait, requirement) {
        return Some(match reason {
            DynamicSignatureIneligibility::BoundaryRequirement => {
                "boundary-machine requirements are not local dynamic calls"
            }
            DynamicSignatureIneligibility::RequirementLocalGenerics => {
                "the requirement has requirement-local generic parameters outside one finite family"
            }
        });
    }
    program
        .dynamic_non_signature_eligibility(requirement)
        .err()
        .map(|reason| match reason {
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::PrivateContractIdentity => {
                "its public contract names an identity a dynamic caller cannot see"
            }
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::UnprojectableResultLifetime => {
                "its result names a borrow lifetime no argument binds"
            }
            typed_trees::dynamic_traits::DynamicNonSignatureIneligibility::UnfittableEnvelope => {
                "its operational envelope (installation-bound reach or native callback parameters) cannot be selected at runtime"
            }
        })
}

/// The trait and arguments of a `dyn Trait<Args>` selector type.
pub(crate) fn dynamic_selector_application(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, &[TypeReferenceHandle])> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_selector_application(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait {
            symbol, arguments, ..
        } => Some((
            *symbol,
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        )),
        _ => None,
    }
}

/// A receiver-style call on a selector (`selector.member(...)`) has no
/// target: the selector is a dispatch operand, so the call names the
/// requirement through its trait and passes the selector first.
pub(crate) fn dynamic_receiver_call_error(
    program: &TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<String> {
    let (trait_symbol, _) = dynamic_selector_application(program, receiver_type)?;
    let trait_name = program.symbols.name(trait_symbol);
    Some(format!(
        "a `dyn {trait_name}` selector is a dispatch operand, not a receiver; call `{trait_name}::{target}(selector, ...)`"
    ))
}

pub fn resolve_dynamic_call_targets(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    resolve_selected_dynamic_call_targets(
        program,
        &typed_trees::references::DeclarationSelection::all(),
    )
}

/// Bind only calls owned by the selected declarations, publishing their exact
/// targets into the same canonical expression and statement arenas.
pub fn resolve_selected_dynamic_call_targets(
    program: &mut TypedTrees,
    domain: &typed_trees::references::DeclarationSelection,
) -> Result<(), Vec<Diagnostic>> {
    let mut expression_updates = Vec::new();
    let mut statement_updates = Vec::new();
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        if !domain.contains(machine.symbol) {
            continue;
        }
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if let StatementNode::Call(call) = statement {
                    let members = program.statement_table.name_path_members(call.receiver);
                    // A projected receiver belongs to its entire place, not
                    // the final field spelling in the caller's attachment.
                    // This pass also binds Binding<R> calls: selecting another
                    // same-named record here would overwrite the exact target
                    // already established by projected-call resolution.
                    let receiver_type = match members {
                        [receiver] => {
                            crate::machine_calls::calls::declared_receiver_type_reference(
                                program,
                                machine,
                                state,
                                receiver.as_str(),
                            )
                        }
                        [] => None,
                        _ => {
                            let path = members
                                .iter()
                                .map(|member| member.as_str().to_owned())
                                .collect::<Vec<_>>();
                            crate::value_custody::places::declared_member_path_type(
                                program,
                                machine,
                                Some(state),
                                &path,
                            )
                        }
                    };
                    let dynamic_target = receiver_type.and_then(|receiver_type| {
                        resolved_dynamic_requirement_symbol(
                            program,
                            receiver_type,
                            call.target.as_str(),
                            &mut diagnostics,
                        )
                    });
                    let target = dynamic_target.or_else(|| {
                        crate::value_custody::placed_views::statement_call_target(
                            program, machine, state, call,
                        )
                    });
                    if let Some(target) = target {
                        statement_updates.push((state.statement_nodes, statement_index, target));
                    }
                }

                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_dynamic_expression_call_updates(
                        program,
                        machine,
                        state,
                        root,
                        &mut expression_updates,
                        &mut diagnostics,
                    );
                }
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (expression, requirement) in expression_updates {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            unreachable!("collected dynamic expression call changed shape")
        };
        call.target_symbol = requirement;
    }
    for (statements, statement_index, requirement) in statement_updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[statement_index]
        else {
            unreachable!("collected dynamic statement call changed shape")
        };
        call.target_symbol = requirement;
    }

    Ok(())
}

fn collect_dynamic_expression_call_updates(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    updates: &mut Vec<(ExpressionHandle, symbols::SymbolHandle)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    macro_rules! visit {
        ($child:expr) => {
            collect_dynamic_expression_call_updates(
                program,
                machine,
                state,
                $child,
                updates,
                diagnostics,
            )
        };
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit!(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    visit!(pattern);
                }
                visit!(arm.value);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit!(*element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            visit!(atomic.value);
            visit!(atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            visit!(binary.left);
            visit!(binary.right);
        }
        ExpressionNode::Cast(cast) => visit!(cast.value),
        ExpressionNode::Call(call) => {
            if let Some(receiver_type) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                call.receiver,
            ) && let Some(requirement) = resolved_dynamic_requirement_symbol(
                program,
                receiver_type,
                call.target.as_str(),
                diagnostics,
            ) && !updates
                .iter()
                .any(|(candidate, _)| *candidate == expression)
            {
                updates.push((expression, requirement));
            }
            visit!(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit!(*argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            visit!(indexed.collection);
            visit!(indexed.index);
        }
        ExpressionNode::Member(member) => visit!(member.receiver),
        ExpressionNode::Borrow(inner) => visit!(inner.target),
        ExpressionNode::Range(range) => {
            visit!(range.start);
            visit!(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit!(field.value);
            }
        }
        ExpressionNode::Unary(unary) => visit!(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn resolved_dynamic_requirement_symbol(
    program: &TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<symbols::SymbolHandle> {
    let trait_symbol = receiver_trait_symbol(program, receiver_type)?;
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)?;
    let mut matches = Vec::new();
    collect_dynamic_requirements_named(
        program,
        trait_definition,
        target,
        &mut Vec::new(),
        &mut matches,
    );
    match matches.as_slice() {
        [(_, requirement)] => Some(requirement.symbol),
        [] => None,
        many => {
            let declaring_trait = many[0].0.symbol;
            if many
                .iter()
                .all(|(candidate, _)| candidate.symbol == declaring_trait)
            {
                // Same-trait result overloads use the first declaration only
                // as a provisional family key. The ordinary overload pass runs
                // immediately afterward and selects the exact result identity.
                return Some(many[0].1.symbol);
            }
            let declarations = many
                .iter()
                .map(|(declaring_trait, requirement)| {
                    format!("{}::{}", declaring_trait.name, requirement.name)
                })
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic::error(format!(
                "dynamic call `{}::{target}` is ambiguous across inherited requirements: {declarations}",
                trait_definition.name
            )));
            None
        }
    }
}

fn receiver_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    if let Some(requirement) =
        typed_trees::service::exact_bound_service_requirement(program, type_reference)
    {
        return Some(requirement);
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => receiver_trait_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            receiver_trait_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. }
            if program
                .traits()
                .iter()
                .any(|definition| definition.symbol == *symbol) =>
        {
            Some(*symbol)
        }
        _ => None,
    }
}

fn collect_dynamic_requirements_named<'program>(
    program: &'program TypedTrees,
    trait_definition: &'program TraitDefinition,
    target: &str,
    visited: &mut Vec<symbols::SymbolHandle>,
    matches: &mut Vec<(&'program TraitDefinition, &'program StateSignature)>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);

    for requirement in program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|requirement| requirement.name.as_str() == target)
    {
        if !matches
            .iter()
            .any(|(_, candidate)| candidate.symbol == requirement.symbol)
        {
            matches.push((trait_definition, requirement));
        }
    }
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == parent.symbol)
        else {
            continue;
        };
        collect_dynamic_requirements_named(program, parent_trait, target, visited, matches);
    }
}
