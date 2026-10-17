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
//! Boundary calls retain their exact qualified requirement application as a
//! static provider-slot demand. They have only ordinary authored arguments;
//! provider selection, package admission and runtime permissions stay separate.
//! This pass resolves qualified/dynamic applications without manufacturing,
//! capturing or removing a source routing operand.

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
    program: &dyn crate::CheckingInput,
) -> Result<Vec<DynamicImplementationSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, root) in program.tables.expression_roots(statement) {
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
    program: &dyn crate::CheckingInput,
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
    let Some(conformance) = program.conformance_by_symbol(implementation) else {
        return;
    };
    let implementation_name = program.symbols.name(implementation);
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
    /// Complete outer-to-inner applied telescopes. A declaring trait's
    /// composite argument can still name an outer trait's parameter.
    pub type_substitutions: Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
    pub substitution_scope_starts: Vec<usize>,
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
    program: &dyn crate::CheckingInput,
) -> Result<Vec<DynamicSlotDemand>, Vec<Diagnostic>> {
    let mut demands: Vec<(
        typed_trees::type_identity::TypeIdentityId,
        DynamicSlotDemand,
    )> = Vec::new();
    let mut push = |demand: DynamicSlotDemand| {
        let identity = program.normalized_type_identity(demand.selector_type);
        if !demands.iter().any(|(existing, known)| {
            *existing == identity && known.requirement == demand.requirement
        }) {
            demands.push((identity, demand));
        }
    };
    for selection in collect_dynamic_implementation_selections(program)? {
        let Some(implementation) = program.conformance_by_symbol(selection.implementation) else {
            continue;
        };
        let Some((selector_trait, selector_arguments)) =
            dynamic_selector_application(program, selection.selector_type)
        else {
            continue;
        };
        let Some(selector_definition) = program.trait_definition_by_symbol(selector_trait) else {
            continue;
        };
        for row in &implementation.rows {
            let Some(application) = typed_trees::trait_definition::AppliedTraitScope::new(
                program,
                selector_definition,
                typed_trees::trait_definition::TraitApplicationArguments {
                    lexical_owner: selection.machine,
                    enclosing_data_applications: &[],
                    arguments: selector_arguments,
                    lifetimes: &[],
                },
            )
            .and_then(|scope| scope.declaring_scope(program, row.declaring_trait)) else {
                continue;
            };
            push(DynamicSlotDemand {
                selector_type: selection.selector_type,
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                type_substitutions: application.substitutions,
                substitution_scope_starts: application.substitution_scope_starts,
            });
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call()
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
                for (_, root) in program.tables.expression_roots(statement) {
                    collect_call_demands_in(program, machine, state, root, &mut visited, &mut push);
                }
            }
        }
    }
    Ok(demands.into_iter().map(|(_, demand)| demand).collect())
}

fn collect_call_demands_in(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
        declaring_trait: call.application.declaration.symbol,
        requirement: call.signature.symbol,
        type_substitutions: call.application.substitutions,
        substitution_scope_starts: call.application.substitution_scope_starts,
    })
}

/// Why `requirement` cannot be called through `dyn Trait`: its signature or
/// contract is outside the local dynamic surface. `None` when it can.
pub(crate) fn dynamic_requirement_ineligibility(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
/// Report the dynamic-receiver call error `receiver_type_reference` carries,
/// when any — the `.and_then`-push-return spine the call-site validators share.
/// Returns true when a diagnostic was emitted.
pub(crate) fn report_dynamic_receiver_call_error(
    program: &dyn crate::CheckingInput,
    receiver_type_reference: Option<TypeReferenceHandle>,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(error) = receiver_type_reference
        .and_then(|type_reference| dynamic_receiver_call_error(program, type_reference, target))
    else {
        return false;
    };
    diagnostics.push(Diagnostic::error(error));
    true
}

pub(crate) fn dynamic_receiver_call_error(
    program: &dyn crate::CheckingInput,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<String> {
    let (trait_symbol, _) = dynamic_selector_application(program, receiver_type)?;
    let trait_name = program.symbols.name(trait_symbol);
    Some(format!(
        "a `dyn {trait_name}` selector is a dispatch operand, not a receiver; call `{trait_name}::{target}(selector, ...)`"
    ))
}

/// A static qualifier names a declaration scope, never a runtime place or
/// stored trait value. Ordinary selectors instead supply their exact declared
/// type. Both routes retain the same applied inheritance environment; neither
/// selected providers nor a same-spelled data carrier can supply one.
pub fn qualified_trait_scope<'program>(
    program: &'program dyn crate::CheckingInput,
    lexical_owner: symbols::SymbolHandle,
    namespace: symbols::SymbolHandle,
    value_type: Option<TypeReferenceHandle>,
) -> Option<typed_trees::trait_definition::AppliedTraitScope<'program>> {
    program
        .trait_definition_by_symbol(namespace)
        .map(typed_trees::trait_definition::AppliedTraitScope::abstract_scope)
        .or_else(|| {
            value_type.and_then(|reference| {
                typed_trees::trait_definition::AppliedTraitScope::from_type_reference(
                    program,
                    reference,
                    lexical_owner,
                )
            })
        })
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
                .indexed_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    let members = program.statement_table.name_path_members(call.receiver);
                    // The lexical declaration, not a repeated leaf spelling,
                    // owns the path. Namespace qualification has no value root.
                    let receiver = members.split_first().and_then(|(first, members)| {
                        let root = crate::receiver_projections::root(
                            program,
                            state,
                            statement_index,
                            call.receiver_root_symbol,
                            first,
                        )?;
                        members.iter().try_fold(root, |receiver, member| {
                            crate::receiver_projections::field(program, receiver, member)
                        })
                    });
                    let dynamic_target = qualified_trait_scope(
                        program,
                        machine.symbol,
                        call.receiver_symbol,
                        receiver.as_ref().map(|receiver| receiver.type_reference),
                    )
                    .and_then(|scope| {
                        resolved_dynamic_requirement_symbol(
                            program,
                            scope,
                            program
                                .trait_definition_by_symbol(call.receiver_symbol)
                                .is_some(),
                            call.target.as_str(),
                            call.target_symbol,
                            call.source_span,
                            &mut diagnostics,
                        )
                    });
                    let target = dynamic_target.or_else(|| {
                        crate::value_custody::placed_views::statement_call_target(
                            program, machine, state, call,
                        )
                    });
                    if let Some(target) = target {
                        let endpoint = receiver
                            .as_ref()
                            .filter(|_| dynamic_target.is_some())
                            .map_or_else(symbols::SymbolHandle::invalid, |receiver| {
                                receiver.symbol
                            });
                        if endpoint.is_valid()
                            && call.receiver_symbol.is_valid()
                            && call.receiver_symbol != endpoint
                        {
                            diagnostics.push(
                                Diagnostic::error(
                                    "projected call receiver disagrees with its exact declared field",
                                )
                                .with_source_span(call.source_span),
                            );
                        }
                        statement_updates.push((
                            state.statement_nodes,
                            statement_index,
                            endpoint,
                            target,
                        ));
                    }
                }

                for (_, root) in program.tables.expression_roots(statement) {
                    collect_dynamic_expression_call_updates(
                        program,
                        machine,
                        state,
                        statement_index,
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
            unreachable!("collected qualified expression call changed shape")
        };
        call.target_symbol = requirement;
    }
    for (statements, statement_index, endpoint, requirement) in statement_updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[statement_index]
        else {
            unreachable!("collected dynamic statement call changed shape")
        };
        if endpoint.is_valid() {
            call.receiver_symbol = endpoint;
        }
        call.target_symbol = requirement;
    }

    Ok(())
}

fn collect_dynamic_expression_call_updates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    before: usize,
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
                before,
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
            let receiver_expression = call.receiver;
            let receiver_type = match crate::receiver_projections::expression(
                program,
                state,
                before,
                receiver_expression,
                &mut Vec::new(),
            ) {
                Ok(Some(receiver)) => Some(receiver.type_reference),
                // A call result can select a service without being a place.
                // The ordinary type query owns those non-place expressions.
                Ok(None) => crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    receiver_expression,
                ),
                Err(errors) => {
                    diagnostics.extend(errors);
                    None
                }
            };
            let namespace = program
                .expression_table
                .name_path_node(receiver_expression)
                .map_or_else(symbols::SymbolHandle::invalid, |path| path.symbol);
            // Preparation can already have closed this target. Its authored
            // qualifier supplies the exact trait application; the abstract
            // namespace cannot stand in for its phantom or inherited arguments.
            let scope = if call.requirement_application.is_valid() {
                program.applied_requirement_scope(machine.symbol, call.requirement_application)
            } else {
                qualified_trait_scope(program, machine.symbol, namespace, receiver_type)
            };
            if let Some(scope) = scope
                && let Some(requirement) = resolved_dynamic_requirement_symbol(
                    program,
                    scope,
                    program.trait_definition_by_symbol(namespace).is_some(),
                    call.target.as_str(),
                    call.target_symbol,
                    program.expression_table.source_span(expression),
                    diagnostics,
                )
                && !updates
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
        typed_trees::literal_expression_pattern!() => {}
    }
}

fn resolved_dynamic_requirement_symbol(
    program: &dyn crate::CheckingInput,
    receiver_scope: typed_trees::trait_definition::AppliedTraitScope<'_>,
    static_namespace: bool,
    target: &str,
    retained_target: symbols::SymbolHandle,
    span: source::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<symbols::SymbolHandle> {
    let trait_definition = receiver_scope.declaration;
    let mut matches = Vec::new();
    collect_dynamic_requirements_named(program, &receiver_scope, target, &mut matches);
    // Static source names a requirement through its declaring trait. Runtime
    // selectors retain the applied ancestor lookup used by dynamic dispatch.
    if static_namespace {
        matches.retain(|(scope, _)| scope.declaration.symbol == trait_definition.symbol);
    }
    // A retained requirement may select an overload in this exact family,
    // but cannot be silently overwritten by an unrelated requirement member.
    let retained_application = program.closed_requirement_application(retained_target);
    let template = retained_application.map_or(retained_target, |application| application.template);
    let retained_matches = matches.iter().any(|(scope, requirement)| {
        requirement.symbol == template
            && retained_application.is_none_or(|application| {
                program.requirement_application_matches_scope(application.signature.symbol, scope)
            })
    });
    if retained_target.is_valid() && !retained_matches {
        diagnostics.push(
            Diagnostic::error("qualified call target belongs to a different requirement")
                .with_source_span(span),
        );
        return None;
    }
    match matches.as_slice() {
        [(_, requirement)] => Some(if retained_application.is_some() {
            retained_target
        } else {
            requirement.symbol
        }),
        [] => None,
        many => {
            let declaring_scope = &many[0].0;
            if many
                .iter()
                .all(|(candidate, _)| candidate.same_application(program, declaring_scope))
            {
                // Preserve an already bound member of this exact family.
                // Initial binding may use the first declaration provisionally;
                // later receiver elaboration must not erase the result overload
                // chosen by ordinary destination binding.
                return Some(if retained_matches {
                    retained_target
                } else {
                    many[0].1.symbol
                });
            }
            let declarations = many
                .iter()
                .map(|(declaring_scope, requirement)| {
                    format!("{}::{}", declaring_scope.declaration.name, requirement.name)
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

fn collect_dynamic_requirements_named<'program>(
    program: &'program dyn crate::CheckingInput,
    receiver_scope: &typed_trees::trait_definition::AppliedTraitScope<'program>,
    target: &str,
    matches: &mut Vec<(
        typed_trees::trait_definition::AppliedTraitScope<'program>,
        &'program StateSignature,
    )>,
) {
    // Application paths, not declaring symbols, determine ambiguity. A diamond
    // may reach one declaration with different phantom arguments.
    receiver_scope.visit(program, &mut |scope| {
        for requirement in program
            .trait_machine_signatures(scope.declaration)
            .iter()
            .filter(|requirement| requirement.name.as_str() == target)
        {
            if !matches.iter().any(|(known, candidate)| {
                candidate.symbol == requirement.symbol && known.same_application(program, scope)
            }) {
                matches.push((scope.clone(), requirement));
            }
        }
    });
}
