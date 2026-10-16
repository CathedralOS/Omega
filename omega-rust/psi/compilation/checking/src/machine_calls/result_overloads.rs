/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

use diagnostics::Diagnostic;
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::type_identity::{CallableIdentityId, CallableParametersId, CallablePathId};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct MachineOverloadGroup {
    provisional_identity: CallableIdentityId,
    overloads: Vec<(SymbolHandle, CallableIdentityId)>,
}

/// Invocation-local grouping of top-level named machine overloads by family.
/// The identities themselves are interned by the program; this index only
/// buckets them, so an entry state resolves to its family in one map read.
/// Source order is retained inside each family, and no index survives this
/// validation invocation.
struct MachineOverloadIndex {
    entry_to_group: HashMap<u32, usize>,
    groups: Vec<MachineOverloadGroup>,
}

impl MachineOverloadIndex {
    fn new(program: &dyn crate::CheckingInput) -> Self {
        let mut entry_to_group = HashMap::default();
        let mut family_to_group: HashMap<(CallablePathId, CallableParametersId), usize> =
            HashMap::default();
        let mut groups: Vec<MachineOverloadGroup> = Vec::new();

        for machine in program.machines() {
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let identity = program.machine_callable_identity(machine);
            if !identity.is_valid() {
                continue;
            }
            let family = (
                program.callable_identity_path(identity),
                program.callable_identity_parameters(identity),
            );
            let group_index = if let Some(index) = family_to_group.get(&family).copied() {
                index
            } else {
                let index = groups.len();
                family_to_group.insert(family, index);
                groups.push(MachineOverloadGroup {
                    provisional_identity: identity,
                    overloads: Vec::new(),
                });
                index
            };
            groups[group_index].overloads.push((entry.symbol, identity));
            entry_to_group
                .entry(entry.symbol.arena_index())
                .or_insert(group_index);
        }

        Self {
            entry_to_group,
            groups,
        }
    }

    fn group(&self, entry: SymbolHandle) -> Option<&MachineOverloadGroup> {
        self.entry_to_group
            .get(&entry.arena_index())
            .and_then(|index| self.groups.get(*index))
    }

    fn contains_entry(&self, entry: SymbolHandle) -> bool {
        self.entry_to_group.contains_key(&entry.arena_index())
    }
}

fn expression_owner_source_spans(program: &dyn crate::CheckingInput) -> HashMap<u32, SourceSpan> {
    let mut owners = HashMap::default();
    for machine in program.machines() {
        let Some(source_span) = program.symbols.symbol_source_span(machine.symbol) else {
            continue;
        };
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    record_expression_owner(program, root, source_span, &mut owners);
                }
            }
        }
    }
    owners
}

fn record_expression_owner(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    source_span: SourceSpan,
    owners: &mut HashMap<u32, SourceSpan>,
) {
    if !expression.is_valid() || owners.contains_key(&expression.arena_index()) {
        return;
    }
    owners.insert(expression.arena_index(), source_span);
    crate::expression_traversal::children(
        program,
        program.expression_table.expression(expression),
        |child| record_expression_owner(program, child, source_span, owners),
    );
}

/// Bind concrete named callable calls after type/domain normalization, when
/// the expected result qualification is available. Early symbol resolution
/// keeps binding same-named declarations to the first symbol; this pass
/// replaces that provisional symbol with the exact machine, trait-requirement,
/// or unspelled boundary-operator result overload before downstream consumers.
mod transition_calls;

pub fn resolve_named_result_overloads(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    transition_calls::normalize_boundary_transition_calls(program);
    let expected_calls = collect_expected_expression_calls(program);
    let expression_owner_source_spans = expression_owner_source_spans(program);
    let machine_overloads = MachineOverloadIndex::new(program);
    let mut diagnostics = Vec::new();

    let expression_updates = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            let expected = expected_calls
                .iter()
                .find_map(|(candidate, expected)| (*candidate == handle).then_some(*expected));
            selected_named_expression_callable_symbol(
                program,
                &machine_overloads,
                call,
                expression_owner_source_spans
                    .get(&handle.arena_index())
                    .copied()
                    .unwrap_or_else(|| program.expression_table.source_span(handle)),
                expected,
                &mut diagnostics,
            )
            .map(|selected| (handle, selected))
        })
        .collect::<Vec<_>>();

    let mut statement_updates = Vec::new();
    let mut operator_statement_updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(call) = statement.as_call() else {
                    continue;
                };
                if let Some(selected) = selected_named_statement_callable_symbol(
                    program,
                    &machine_overloads,
                    call,
                    program
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .unwrap_or(call.source_span),
                    None,
                    &mut diagnostics,
                ) {
                    if let Some(operator) = program.operator_definition_by_symbol(selected) {
                        if !operator.return_type.is_valid() {
                            statement_updates.push((state.statement_nodes, index, selected));
                            continue;
                        }
                        let returns_non_unit = !matches!(
                            program
                                .type_reference_table
                                .type_reference(operator.return_type),
                            TypeReferenceNode::Unit
                        );
                        if returns_non_unit && !call.discards_result {
                            let path = program.operator_path_string(operator.name);
                            diagnostics.push(Diagnostic::error(format!(
                                "call to named requirement `{path}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {path}(...);`",
                                program.display_type_reference_with_constraints(operator.return_type)
                            )));
                        } else {
                            operator_statement_updates.push((
                                state.symbol,
                                state.statement_nodes,
                                index,
                                call.clone(),
                                selected,
                                returns_non_unit,
                            ));
                        }
                    } else {
                        statement_updates.push((state.statement_nodes, index, selected));
                    }
                }
            }
        }
    }

    let transition_updates = selected_transition_call_symbols(program, &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (target, selected) in transition_updates {
        let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            unreachable!("collected transition target changed shape");
        };
        path.symbol = selected;
    }
    for (handle, selected) in expression_updates {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected expression call changed shape");
        };
        call.target_symbol = selected;
    }
    for (statements, index, selected) in statement_updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[index]
        else {
            unreachable!("collected statement call changed shape");
        };
        call.target_symbol = selected;
    }
    for (state_symbol, statements, index, call, selected, returns_value) in
        operator_statement_updates
    {
        let source_span = call.source_span;
        let authored_call_selection = call.authored_call_selection;
        let receiver_members = program
            .statement_table
            .name_path_members(call.receiver)
            .to_vec();
        let arguments = program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec();
        let receiver = if receiver_members.is_empty() {
            ExpressionHandle::invalid()
        } else {
            let mut members = arena::HandleSpan::empty();
            for member in receiver_members {
                program
                    .expression_table
                    .push_name_path_member(&mut members, member);
            }
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols: arena::HandleSpan::empty(),
                    head_symbol: call.receiver_root_symbol,
                    symbol: call.receiver_symbol,
                }))
        };
        let arguments = program
            .expression_table
            .insert_expression_handles(arguments);
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_machine_application: call.authored_machine_application,
                    requirement_application: Default::default(),
                    exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                    receiver,
                    target_symbol: selected,
                    target: call.target,
                    static_machine_parameter: call.static_machine_parameter,
                    static_requirement_dispatch: None,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: None,
                    private_layout_operation: None,
                    layout_query: None,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }));
        program
            .expression_table
            .set_source_span(expression, source_span);
        if let Some(occurrence) = authored_call_selection {
            program
                .expression_table
                .attach_authored_selection_occurrences(expression, [occurrence]);
        }
        // Unit has no value carrier. Keep the selected call in statement
        // position so its writes and guarantees reach subsequent statements
        // without manufacturing a local initialized by a nonexistent value.
        if !returns_value {
            program.statement_table.statements_mut(statements)[index] =
                StatementNode::Expression(expression);
            continue;
        }
        let type_reference = program
            .operator_definition_by_symbol(selected)
            .expect("selected statement operator disappeared")
            .return_type;
        let generated_name = format!(
            "__discarded_named_requirement#{}#{index}",
            statements.start().arena_index()
        );
        let symbol = program.symbols.insert_generated_root_from(
            state_symbol,
            SymbolKind::Local,
            &generated_name,
        );
        program.statement_table.statements_mut(statements)[index] =
            StatementNode::LocalData(TableLocalData {
                role: language_core::LocalBindingRole::Value,
                symbol,
                name: typed_trees::name::Identifier::generated(generated_name),
                type_reference,
                initial_value: expression,
                is_mutable: false,
                type_is_inferred: true,
                is_operand_capture: false,
                relevance: language_core::BindingRelevance::Relevant,
            });
    }
    Ok(())
}

/// A terminal arm naming a machine is a call whose normal result completes the
/// enclosing machine (state_contracts.md, terminal calls in transition arms),
/// so its expected result is the enclosing result. Resolution leaves a
/// receiverless family name unbound when several overloads share it; select
/// the member here by the same result-domain rule a value-position call uses.
/// The selection names the owning machine, as a resolved machine target does,
/// so a selected self-call stays the constant-stack backedge it spells.
fn selected_transition_call_symbols(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(typed_trees::statement::TransitionTargetHandle, SymbolHandle)> {
    let mut updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let typed_trees::statement::TransitionTargetNode::Named {
                        path,
                        source_span,
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    if path.symbol.is_valid() {
                        continue;
                    }
                    let [name] = program.statement_table.name_path_members(path.members) else {
                        continue;
                    };
                    let Some(entry) = named_machine_overload_family_symbol(
                        program,
                        name.as_str(),
                        *source_span,
                        state.return_type.is_valid().then_some(state.return_type),
                        diagnostics,
                    ) else {
                        continue;
                    };
                    if let Some(owner) = program.machine_holding_state(entry) {
                        updates.push((target, owner.symbol));
                    }
                }
            }
        }
    }
    updates
}

/// The boundary-operator overloads in `candidates` sharing
/// `provisional_identity`'s family, restricted to candidates `source_span`
/// may see.
fn family_boundary_overloads<'program>(
    program: &'program dyn crate::CheckingInput,
    candidates: impl Iterator<Item = &'program typed_trees::operator::OperatorDefinition>,
    source_span: SourceSpan,
    provisional_identity: CallableIdentityId,
) -> Vec<(SymbolHandle, CallableIdentityId)> {
    candidates
        .filter(|operator| operator.is_boundary && operator.spelling.is_none())
        .filter(|operator| {
            program
                .symbols
                .source_reference_can_see_symbol(source_span, operator.symbol)
        })
        .filter_map(|operator| {
            let identity = program.operator_callable_identity(operator);
            program
                .callable_identities_share_family(identity, provisional_identity)
                .then_some((operator.symbol, identity))
        })
        .collect()
}

/// The provisional-identity overload admission shared by the statement and
/// expression named-callable paths: resolves the family boundary overloads
/// visible from `source_span` and reports whether they cover every visible
/// candidate.
fn visible_boundary_overload_family<'program>(
    program: &'program dyn crate::CheckingInput,
    candidates: &[&'program typed_trees::operator::OperatorDefinition],
    source_span: SourceSpan,
    provisional: &'program typed_trees::operator::OperatorDefinition,
) -> (Vec<(SymbolHandle, CallableIdentityId)>, usize) {
    let provisional_identity = program.operator_callable_identity(provisional);
    let visible_candidate_count = candidates
        .iter()
        .filter(|operator| {
            program
                .symbols
                .source_reference_can_see_symbol(source_span, operator.symbol)
        })
        .count();
    let overloads = family_boundary_overloads(
        program,
        candidates.iter().copied(),
        source_span,
        provisional_identity,
    );
    (overloads, visible_candidate_count)
}

fn selected_named_statement_callable_symbol(
    program: &dyn crate::CheckingInput,
    machine_overloads: &MachineOverloadIndex,
    call: &typed_trees::statement::TableCall,
    source_span: SourceSpan,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if let Some(resolution) = named_callable_resolution(
        program,
        machine_overloads,
        call.target_symbol,
        call.receiver.is_empty(),
        call.target.as_str(),
        source_span,
        expected_result,
        diagnostics,
    ) {
        return resolution;
    }
    if let Some(provisional) = program.operators().iter().find(|operator| {
        operator.symbol == call.target_symbol && operator.is_boundary && operator.spelling.is_none()
    }) {
        let provisional_identity = program.operator_callable_identity(provisional);
        let overloads = family_boundary_overloads(
            program,
            program.operators().iter(),
            source_span,
            provisional_identity,
        );
        if overloads.len() == 1 {
            return Some(provisional.symbol);
        }
        return select_overload_symbol(
            program,
            provisional_identity,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        );
    }

    let candidates = typed_trees::operator::named_statement_call_candidates(program, call);
    if let Some(provisional) = candidates.first() {
        let (overloads, visible_candidate_count) =
            visible_boundary_overload_family(program, &candidates, source_span, provisional);
        if overloads.len() == visible_candidate_count {
            return select_unbound_overload_family_symbol(
                program,
                &overloads,
                expected_result,
                diagnostics,
                "requirement",
            );
        }
    }

    selected_named_callable_symbol(
        program,
        machine_overloads,
        call.target_symbol,
        source_span,
        expected_result,
        diagnostics,
    )
}

fn selected_named_expression_callable_symbol(
    program: &dyn crate::CheckingInput,
    machine_overloads: &MachineOverloadIndex,
    call: &TableCallExpression,
    source_span: SourceSpan,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if let Some(resolution) = named_callable_resolution(
        program,
        machine_overloads,
        call.target_symbol,
        !call.receiver.is_valid(),
        call.target.as_str(),
        source_span,
        expected_result,
        diagnostics,
    ) {
        return resolution;
    }
    if program.operators().iter().any(|operator| {
        operator.symbol == call.target_symbol && operator.is_boundary && operator.spelling.is_none()
    }) {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            source_span,
            expected_result,
            diagnostics,
        );
    }

    let candidates = typed_trees::operator::named_expression_call_candidates(program, call);
    let Some(provisional) = candidates.first() else {
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            source_span,
            expected_result,
            diagnostics,
        );
    };
    let (overloads, visible_candidate_count) =
        visible_boundary_overload_family(program, &candidates, source_span, provisional);
    if overloads.len() != visible_candidate_count {
        // Path/arity alone still spans distinct parameter overload groups.
        // Leave those calls to ordinary operand-directed resolution.
        return selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            source_span,
            expected_result,
            diagnostics,
        );
    }
    select_unbound_overload_family_symbol(
        program,
        &overloads,
        expected_result,
        diagnostics,
        "requirement",
    )
}

/// The named-callable admission shared by statement and expression calls:
/// a targetless unnamed call resolves through the machine overload family;
/// a provisional target naming a non-operator callable resolves directly.
/// `receiver_missing` is each call shape's own empty-receiver test; the
/// admitted branch's own verdict rides the inner `Option` so a declining
/// family lookup still ends the caller early.
fn named_callable_resolution(
    program: &dyn crate::CheckingInput,
    machine_overloads: &MachineOverloadIndex,
    target_symbol: SymbolHandle,
    receiver_missing: bool,
    target_name: &str,
    source_span: SourceSpan,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Option<SymbolHandle>> {
    if !target_symbol.is_valid() && receiver_missing {
        return Some(named_machine_overload_family_symbol(
            program,
            target_name,
            source_span,
            expected_result,
            diagnostics,
        ));
    }
    if provisional_target_is_named_non_operator_callable(program, machine_overloads, target_symbol)
    {
        return Some(selected_named_callable_symbol(
            program,
            machine_overloads,
            target_symbol,
            source_span,
            expected_result,
            diagnostics,
        ));
    }
    None
}

fn provisional_target_is_named_non_operator_callable(
    program: &dyn crate::CheckingInput,
    machine_overloads: &MachineOverloadIndex,
    target: SymbolHandle,
) -> bool {
    machine_overloads.contains_entry(target)
        || program.traits().iter().any(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|requirement| requirement.symbol == target)
        })
}

fn selected_named_callable_symbol(
    program: &dyn crate::CheckingInput,
    machine_overloads: &MachineOverloadIndex,
    provisional_target: SymbolHandle,
    source_span: SourceSpan,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    if let Some(group) = machine_overloads.group(provisional_target) {
        let visible_overloads = group
            .overloads
            .iter()
            .filter(|(symbol, _)| {
                program
                    .symbols
                    .source_reference_can_see_symbol(source_span, *symbol)
            })
            .cloned()
            .collect::<Vec<_>>();
        return select_overload_symbol(
            program,
            group.provisional_identity,
            &visible_overloads,
            expected_result,
            diagnostics,
            "machine",
        );
    }

    if let Some(provisional_operator) = program.operators().iter().find(|operator| {
        operator.symbol == provisional_target && operator.is_boundary && operator.spelling.is_none()
    }) {
        let provisional_identity = program.operator_callable_identity(provisional_operator);
        let overloads = family_boundary_overloads(
            program,
            program.operators().iter(),
            source_span,
            provisional_identity,
        );
        return select_overload_symbol(
            program,
            provisional_identity,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        );
    }

    let (trait_definition, provisional_requirement) =
        program.trait_machine_signature(provisional_target)?;
    let provisional_identity =
        program.trait_requirement_callable_identity(trait_definition, provisional_requirement);
    let overloads = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|requirement| {
            program
                .symbols
                .source_reference_can_see_symbol(source_span, requirement.symbol)
        })
        .filter_map(|requirement| {
            let identity =
                program.trait_requirement_callable_identity(trait_definition, requirement);
            program
                .callable_identities_share_family(identity, provisional_identity)
                .then_some((requirement.symbol, identity))
        })
        .collect::<Vec<_>>();
    select_overload_symbol(
        program,
        provisional_identity,
        &overloads,
        expected_result,
        diagnostics,
        "requirement",
    )
}

fn select_overload_symbol(
    program: &dyn crate::CheckingInput,
    provisional_identity: CallableIdentityId,
    overloads: &[(SymbolHandle, CallableIdentityId)],
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) -> Option<SymbolHandle> {
    // Result-domain lookup is the disambiguation rule for an authored
    // overload family. A singleton keeps the language's ordinary
    // compatibility/qualification judgment: existing helpers may return a
    // qualified value to an unqualified destination (or establish a
    // qualification after the call) without manufacturing a second overload.
    if overloads.len() <= 1 {
        return None;
    }

    let requested = program.callable_result_dispatch_identity(
        &expected_result
            .filter(|expected| expected.is_valid())
            .map(|expected| program.normalized_result_dispatch_set(expected))
            .unwrap_or_default(),
    );
    let matching = overloads
        .iter()
        .filter(|(_, identity)| program.callable_identity_result_dispatch(*identity) == requested)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [(symbol, _)] => Some(*symbol),
        [] => {
            let requested = if program.callable_result_dispatch_is_empty(requested) {
                "<empty>".to_owned()
            } else {
                program.callable_result_dispatch_text(requested)
            };
            diagnostics.push(Diagnostic::error(format!(
                "named {kind} call `{}` has no overload for result dispatch set `{requested}` and parameter signature `{}`",
                program.callable_path_text(program.callable_identity_path(provisional_identity)),
                program
                    .callable_parameters_text(program.callable_identity_parameters(provisional_identity)),
            )));
            None
        }
        // Declaration validation owns duplicate-set diagnostics and runs
        // immediately after this mutable rebinding pass. Do not relocate that
        // error to whichever call happened to be visited first.
        _ => None,
    }
}

/// The callable symbol for a receiverless call whose authored name spells a
/// result-overload family. Every member owns the declared path, so upstream
/// name binding leaves no provisional symbol to disambiguate from; this pass
/// selects the member by the destination's result dispatch set instead. As in
/// the free-machine entry rule, a member's state named exactly like the call
/// is preferred over its first state.
fn named_machine_overload_family_symbol(
    program: &dyn crate::CheckingInput,
    target_name: &str,
    source_span: SourceSpan,
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SymbolHandle> {
    let target_path = program.callable_path_by_text(target_name);
    let overloads = program
        .machines()
        .iter()
        .filter_map(|machine| {
            let identity = program.machine_callable_identity(machine);
            if program.callable_identity_path(identity) != target_path {
                return None;
            }
            let states = program.machine_states(machine);
            let callable = states
                .iter()
                .find(|state| state.name.as_str() == target_name)
                .or_else(|| states.first())?;
            program
                .symbols
                .source_reference_can_see_symbol(source_span, callable.symbol)
                .then_some((callable.symbol, identity))
        })
        .collect::<Vec<_>>();
    select_unbound_overload_family_symbol(
        program,
        &overloads,
        expected_result,
        diagnostics,
        "machine",
    )
}

/// An unbound call needs a declaration even when its visible family has only
/// one member. Bind that exact member before ordinary type and use checking;
/// result-domain disambiguation applies only when several members remain.
/// Already bound calls keep their original declaration through the ordinary
/// overload selector, including its instantiated trait application identity.
fn select_unbound_overload_family_symbol(
    program: &dyn crate::CheckingInput,
    overloads: &[(SymbolHandle, CallableIdentityId)],
    expected_result: Option<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) -> Option<SymbolHandle> {
    match overloads {
        [] => None,
        [(symbol, _)] => Some(*symbol),
        [(_, provisional_identity), ..] => select_overload_symbol(
            program,
            *provisional_identity,
            overloads,
            expected_result,
            diagnostics,
            kind,
        ),
    }
}

fn collect_expected_expression_calls(
    program: &dyn crate::CheckingInput,
) -> Vec<(ExpressionHandle, TypeReferenceHandle)> {
    let mut expected = Vec::new();

    for (_, expression) in program.expression_table.expression_entries() {
        match expression {
            ExpressionNode::StructLiteral(literal) => {
                let Some(data_definition) = program.data_definitions().iter().find(|definition| {
                    definition.symbol == literal.type_symbol
                        || (!program.symbols.has_source_metadata()
                            && definition.name.as_str() == literal.type_name.as_str())
                }) else {
                    continue;
                };
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) =
                        crate::value_custody::struct_literals::construction_field_type(
                            program,
                            data_definition,
                            literal.case_name.as_ref().map(|name| name.as_str()),
                            field.name.as_str(),
                        )
                    else {
                        continue;
                    };
                    push_expected_call(program, field.value, field_type, &mut expected);
                }
            }
            ExpressionNode::Call(call) => {
                collect_call_argument_expectations(program, call, &mut expected);
            }
            _ => {}
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) =
                            crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        {
                            push_expected_call(program, assignment.value, declared, &mut expected);
                        }
                    }
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && local.type_reference.is_valid() =>
                    {
                        push_expected_call(
                            program,
                            local.initial_value,
                            local.type_reference,
                            &mut expected,
                        );
                    }
                    StatementNode::Call(call) => {
                        let expression_call = TableCallExpression {
                            authored_machine_application: call.authored_machine_application,
                            requirement_application: Default::default(),
                            exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(
                            ),
                            receiver: ExpressionHandle::invalid(),
                            target_symbol: call.target_symbol,
                            target: call.target.clone(),
                            static_machine_parameter: call.static_machine_parameter,
                            static_requirement_dispatch: call.static_requirement_dispatch.clone(),
                            machine_arguments: call.machine_arguments.clone(),
                            quotient_operation: None,
                            private_layout_operation: None,
                            layout_query: None,
                            arguments: call.arguments,
                            evidence_arguments: call.evidence_arguments.clone(),
                            operational_acknowledgement: call.operational_acknowledgement,
                        };
                        collect_call_argument_expectations(
                            program,
                            &expression_call,
                            &mut expected,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            if let typed_trees::statement::TransitionTargetNode::Value(value) =
                                program.statement_table.transition_target(target)
                                && state.return_type.is_valid()
                            {
                                push_expected_call(
                                    program,
                                    *value,
                                    state.return_type,
                                    &mut expected,
                                );
                            }
                        }
                    }
                    StatementNode::Expression(value)
                        if index + 1 == statements.len() && state.return_type.is_valid() =>
                    {
                        push_expected_call(program, *value, state.return_type, &mut expected);
                    }
                    StatementNode::RootBinding(_)
                    | StatementNode::Assembly(_)
                    | StatementNode::AssemblyFact(_)
                    | StatementNode::Expression(_)
                    | StatementNode::LocalData(_) => {}
                }
            }
        }
    }
    expected
}

fn collect_call_argument_expectations(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    let parameters = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .first()
                .filter(|entry| entry.symbol == call.target_symbol)
                .map(|entry| program.state_parameters(entry))
        })
        .or_else(|| {
            program
                .trait_machine_signature(call.target_symbol)
                .map(|(_, requirement)| program.state_signature_parameters(requirement))
        });
    let Some(parameters) = parameters else {
        return;
    };
    let parameters = parameters.iter();
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .copied();
    for (parameter, argument) in parameters.zip(arguments) {
        push_expected_call(program, argument, parameter.type_reference, expected);
    }
}

fn push_expected_call(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    match program.expression_table.expression(value) {
        ExpressionNode::Match(dispatch) => {
            for arm in program.expression_table.match_arms(dispatch.arms) {
                push_expected_call(program, arm.value, expected_type, expected);
            }
        }
        ExpressionNode::Borrow(inner) => {
            push_expected_call(program, inner.target, expected_type, expected)
        }
        ExpressionNode::Atomic(atomic) => {
            push_expected_call(program, atomic.value, expected_type, expected)
        }
        ExpressionNode::Call(_) => {
            if let Some((_, existing)) = expected
                .iter_mut()
                .find(|(candidate, _)| *candidate == value)
            {
                *existing = expected_type;
            } else {
                expected.push((value, expected_type));
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            let Some(element_type) = array_element_type(program, expected_type) else {
                return;
            };
            for element in program
                .expression_table
                .expression_handles(*values)
                .iter()
                .copied()
            {
                push_expected_call(program, element, element_type, expected);
            }
        }
        ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn array_element_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => array_element_type(program, *referee),
        typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
        | typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        typed_trees::types::TypeReferenceNode::Generic { .. }
        | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
        | typed_trees::types::TypeReferenceNode::Named { .. }
        | typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::Unit => None,
    }
}

mod float_conversion_casts;
pub use float_conversion_casts::bind_float_conversion_casts;
