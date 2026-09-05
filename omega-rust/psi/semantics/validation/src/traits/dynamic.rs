use diagnostics::Diagnostic;
use source::SourceSpan;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One local dynamic coercion whose complete nominal conformance is fixed in
/// the checked artifact. Runtime descriptor lowering consumes this exact
/// selection rather than rediscovering implementations from names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceSelection {
    pub occurrence: ExpressionHandle,
    pub binding: symbols::SymbolHandle,
    pub binding_name: Identifier,
    pub machine: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
    /// Exact source place repackaged by this coercion. `source_symbol` is the
    /// authored leaf declaration reached through `source_path`, not a
    /// synthesized member-accessor identity. Whole-artifact devirtualization
    /// uses it as the selected realization's receiver instead of treating the
    /// two-word dynamic descriptor as the concrete `self`.
    pub source_symbol: symbols::SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
    pub source_data: symbols::SymbolHandle,
    pub target_trait: symbols::SymbolHandle,
    /// Stable package symbol for the explicitly named conformance.
    pub conformance: Option<symbols::SymbolHandle>,
}

/// One exact move of an already-selected borrowed dynamic descriptor into a
/// field of a local record. This is storage lineage, not a new conformance
/// selection: `selection` remains the sole authority for the descriptor rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicDescriptorStorage {
    /// Exact field-value expression which supplies the descriptor.
    pub occurrence: ExpressionHandle,
    pub machine: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub destination_binding: symbols::SymbolHandle,
    pub destination_name: Identifier,
    pub destination_field: symbols::SymbolHandle,
    pub destination_path: Vec<Identifier>,
    pub source_binding: symbols::SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
    pub selection: DynamicConformanceSelection,
}

/// Select complete nominal conformances for the currently admitted local
/// coercion form: a direct place cast bound to a reference-typed local. Bare
/// `dyn Trait` requires a unique conformance; `dyn Type::Conformance` selects
/// the named declaration exactly.
pub fn collect_dynamic_conformance_selections(
    program: &TypedTrees,
) -> Result<Vec<DynamicConformanceSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let Some((occurrence, cast, dynamic_target)) =
                    normalized_dynamic_coercion(program, local.initial_value)
                else {
                    continue;
                };
                let TypeReferenceNode::DynamicTrait {
                    symbol: target_trait,
                    conformance: exact_conformance,
                    conformance_carrier,
                    conformance_name,
                    ..
                } = dynamic_target
                else {
                    unreachable!("dynamic_trait_reference returns a dynamic-trait node")
                };
                let target_trait = *target_trait;
                let Some(mut source_place) = dynamic_source_place(program, cast.value) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` requires a direct named or member source place",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some(source_type) = crate::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    cast.value,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` has no statically resolved source place type",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some(source_symbol) = crate::places::declared_place_leaf_symbol(
                    program,
                    machine,
                    Some(state),
                    statement_index,
                    cast.value,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` has no exact source declaration identity",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                source_place.symbol = source_symbol;
                let Some((source_data, source_name)) = nominal_data_type(program, source_type)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` requires a concrete nominal data source",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some(trait_definition) = program
                    .traits()
                    .iter()
                    .find(|definition| definition.symbol == target_trait)
                else {
                    continue;
                };
                let nominal_matches = program
                    .conformances()
                    .iter()
                    .filter(|conformance| {
                        program.symbols.source_reference_can_see_symbol(
                            program
                                .symbols
                                .symbol_source_span(machine.symbol)
                                .unwrap_or_else(|| {
                                    program.expression_table.source_span(occurrence)
                                }),
                            conformance.symbol,
                        ) && conformance
                            .carrier_name()
                            .is_some_and(|carrier| carrier.as_str() == source_name)
                            && conformance.trait_name == trait_definition.name
                            && conformance.arguments.is_empty()
                    })
                    .collect::<Vec<_>>();
                let matches = nominal_matches
                    .iter()
                    .copied()
                    .filter(|conformance| {
                        matches!(
                            &conformance.implementation,
                            typed_trees::trait_definition::ConformanceImplementation::Closed { .. }
                        )
                    })
                    .collect::<Vec<_>>();
                if conformance_name.is_some() {
                    let selection_name = conformance_carrier
                        .as_ref()
                        .zip(conformance_name.as_ref())
                        .map(|(carrier, conformance)| format!("{carrier}::{conformance}"))
                        .unwrap_or_else(|| "<invalid named conformance>".to_owned());
                    let Some(exact_symbol) = exact_conformance else {
                        diagnostics.push(Diagnostic::error(format!(
                            "local dynamic coercion selects unresolved named conformance `{selection_name}`"
                        )));
                        continue;
                    };
                    if nominal_matches.iter().any(|candidate| {
                        candidate.symbol == *exact_symbol
                            && matches!(
                                &candidate.implementation,
                                typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
                            )
                    }) {
                        diagnostics.push(Diagnostic::error(format!(
                            "named conformance `{selection_name}` is bodyless and cannot license local dynamic dispatch; declare its complete row map with a conformance block"
                        )));
                        continue;
                    }
                    let Some(selected) = matches
                        .iter()
                        .find(|candidate| candidate.symbol == *exact_symbol)
                    else {
                        diagnostics.push(Diagnostic::error(format!(
                            "local dynamic coercion from `{source_name}` to `dyn {}` cannot use named conformance `{selection_name}`",
                            trait_definition.name
                        )));
                        continue;
                    };
                    let selection = DynamicConformanceSelection {
                        occurrence,
                        binding: local.symbol,
                        binding_name: local.name.clone(),
                        machine: machine.symbol,
                        state: state.symbol,
                        statement_index,
                        source_symbol: source_place.symbol,
                        source_name: source_place.name.clone(),
                        source_path: source_place.path.clone(),
                        source_data,
                        target_trait,
                        conformance: Some(selected.symbol),
                    };
                    if !selections.contains(&selection) {
                        selections.push(selection);
                    }
                    continue;
                }
                match matches.as_slice() {
                    [conformance] => {
                        let selection = DynamicConformanceSelection {
                            occurrence,
                            binding: local.symbol,
                            binding_name: local.name.clone(),
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_index,
                            source_symbol: source_place.symbol,
                            source_name: source_place.name.clone(),
                            source_path: source_place.path.clone(),
                            source_data,
                            target_trait,
                            conformance: conformance.symbol.is_valid().then_some(conformance.symbol),
                        };
                        if !selections.contains(&selection) {
                            selections.push(selection);
                        }
                    }
                    [] => diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion from `{source_name}` to `dyn {}` has no complete nominal conformance",
                        trait_definition.name
                    ))),
                    many => diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion from `{source_name}` to `dyn {}` has {} complete nominal conformances; select one exact named conformance",
                        trait_definition.name,
                        many.len()
                    ))),
                }
            }
        }
    }

    collect_named_conformance_dynamic_rebindings(program, &mut selections, &mut diagnostics);

    // Validate pass-through only after the complete local-selection catalog is
    // known. A call may use a selection authored earlier in its state; no
    // visible-conformance search or source-order guess may reconstruct it.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                validate_dynamic_call_arguments_in_statement(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &selections,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

/// Retain the first bounded aggregate-storage shape: an immutable local record
/// literal field initialized directly from an earlier local `&dyn` selection.
/// More general assignment, nesting, joins, and owned erasure remain separate
/// rungs because each needs different lifetime and mutation custody.
pub fn collect_dynamic_descriptor_storages(
    program: &TypedTrees,
    selections: &[DynamicConformanceSelection],
) -> Vec<DynamicDescriptorStorage> {
    let mut storages = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if local.is_mutable {
                    continue;
                }
                let ExpressionNode::StructLiteral(literal) =
                    program.expression_table.expression(local.initial_value)
                else {
                    continue;
                };
                if literal.case_symbol.is_some() {
                    continue;
                }
                let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == literal.type_symbol)
                else {
                    continue;
                };
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(declared_field) =
                        program.data_members(data).iter().find_map(|member| {
                            let typed_trees::data::DataMember::Field(declared) = member else {
                                return None;
                            };
                            (declared.symbol == field.field_symbol).then_some(declared)
                        })
                    else {
                        continue;
                    };
                    let Some(target_trait) =
                        borrowed_dynamic_trait_symbol(program, declared_field.type_reference)
                    else {
                        continue;
                    };
                    let Some(source) = dynamic_source_place(program, field.value) else {
                        continue;
                    };
                    if source.path.len() != 1 {
                        continue;
                    }
                    let Some(selection) = selections
                        .iter()
                        .filter(|selection| {
                            selection.machine == machine.symbol
                                && selection.state == state.symbol
                                && selection.statement_index < statement_index
                                && selection.target_trait == target_trait
                                && if source.symbol.is_valid() {
                                    selection.binding == source.symbol
                                } else {
                                    selection.binding_name == source.name
                                }
                        })
                        .max_by_key(|selection| selection.statement_index)
                    else {
                        continue;
                    };
                    let storage = DynamicDescriptorStorage {
                        occurrence: field.value,
                        machine: machine.symbol,
                        state: state.symbol,
                        statement_index,
                        destination_binding: local.symbol,
                        destination_name: local.name.clone(),
                        destination_field: declared_field.symbol,
                        destination_path: vec![local.name.clone(), declared_field.name.clone()],
                        source_binding: source.symbol,
                        source_name: source.name,
                        source_path: source.path,
                        selection: selection.clone(),
                    };
                    if !storages.contains(&storage) {
                        storages.push(storage);
                    }
                }
            }
        }
    }
    storages
}

fn borrowed_dynamic_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            borrowed_dynamic_trait_symbol(program, *base_type)
        }
        _ => None,
    }
}

fn collect_named_conformance_dynamic_rebindings(
    program: &TypedTrees,
    selections: &mut Vec<DynamicConformanceSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let initial_selections = selections.clone();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Assignment(assignment) = statement else {
                    continue;
                };
                let Some(target) = dynamic_source_place(program, assignment.target) else {
                    continue;
                };
                if target.path.len() != 1 {
                    continue;
                }
                let Some(target_symbol) = crate::places::declared_place_leaf_symbol(
                    program,
                    machine,
                    Some(state),
                    statement_index,
                    assignment.target,
                ) else {
                    if initial_selections.iter().any(|selection| {
                        selection.machine == machine.symbol
                            && selection.state == state.symbol
                            && selection.binding_name == target.name
                    }) {
                        diagnostics.push(Diagnostic::error(format!(
                            "dynamic local rebind `{}` has no exact target declaration identity",
                            target.name
                        )));
                    }
                    continue;
                };
                let Some(initial) = initial_selections.iter().find(|selection| {
                    selection.machine == machine.symbol
                        && selection.state == state.symbol
                        && selection.statement_index < statement_index
                        && selection.binding == target_symbol
                }) else {
                    continue;
                };
                let mutable = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(initial.statement_index)
                    .is_some_and(|statement| {
                        matches!(
                            statement,
                            StatementNode::LocalData(local)
                                if local.symbol == initial.binding && local.is_mutable
                        )
                    });
                if !mutable {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` requires the exact mutable local declaration",
                        initial.binding_name
                    )));
                    continue;
                }
                let Some((occurrence, cast, dynamic_target)) =
                    normalized_dynamic_coercion(program, assignment.value)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` requires an exact direct-place named-conformance cast",
                        initial.binding_name
                    )));
                    continue;
                };
                let TypeReferenceNode::DynamicTrait {
                    symbol: target_trait,
                    conformance,
                    ..
                } = dynamic_target
                else {
                    unreachable!("normalized_dynamic_coercion returns a dynamic-trait target")
                };
                if *target_trait != initial.target_trait || conformance.is_none() {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` must retain its exact trait and a named conformance",
                        initial.binding_name
                    )));
                    continue;
                }
                let Some(mut source_place) = dynamic_source_place(program, cast.value) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` requires a direct named or member source place",
                        initial.binding_name
                    )));
                    continue;
                };
                let Some(source_type) = crate::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    cast.value,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` has no statically resolved source place type",
                        initial.binding_name
                    )));
                    continue;
                };
                let Some(source_symbol) = crate::places::declared_place_leaf_symbol(
                    program,
                    machine,
                    Some(state),
                    statement_index,
                    cast.value,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` has no exact source declaration identity",
                        initial.binding_name
                    )));
                    continue;
                };
                source_place.symbol = source_symbol;
                let Some((source_data, _)) = nominal_data_type(program, source_type) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` requires a concrete nominal data source",
                        initial.binding_name
                    )));
                    continue;
                };
                if source_data != initial.source_data {
                    diagnostics.push(Diagnostic::error(format!(
                        "dynamic local rebind `{}` must retain the exact source carrier for this rung",
                        initial.binding_name
                    )));
                    continue;
                }
                selections.push(DynamicConformanceSelection {
                    occurrence,
                    binding: initial.binding,
                    binding_name: initial.binding_name.clone(),
                    machine: initial.machine,
                    state: initial.state,
                    statement_index,
                    source_symbol: source_place.symbol,
                    source_name: source_place.name,
                    source_path: source_place.path,
                    source_data,
                    target_trait: initial.target_trait,
                    conformance: *conformance,
                });
            }
        }
    }
}

fn validate_dynamic_call_arguments_in_statement(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    statement: &StatementNode,
    selections: &[DynamicConformanceSelection],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let StatementNode::Call(call) = statement {
        validate_dynamic_call_arguments(
            program,
            machine,
            state,
            statement_index,
            call.source_span,
            call.target_symbol,
            &call.target,
            call.arguments,
            selections,
            diagnostics,
        );
    }
    for root in crate::calls::statement_value_expression_roots(program, statement) {
        validate_dynamic_call_arguments_in_expression(
            program,
            machine,
            state,
            statement_index,
            root,
            selections,
            diagnostics,
        );
    }
}

fn validate_dynamic_call_arguments_in_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    selections: &[DynamicConformanceSelection],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    macro_rules! visit {
        ($child:expr) => {
            validate_dynamic_call_arguments_in_expression(
                program,
                machine,
                state,
                statement_index,
                $child,
                selections,
                diagnostics,
            )
        };
    }
    match program.expression_table.expression(expression) {
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
            validate_dynamic_call_arguments(
                program,
                machine,
                state,
                statement_index,
                program.expression_table.source_span(expression),
                call.target_symbol,
                &call.target,
                call.arguments,
                selections,
                diagnostics,
            );
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
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_dynamic_call_arguments(
    program: &TypedTrees,
    caller: &Machine,
    caller_state: &State,
    statement_index: usize,
    source_span: SourceSpan,
    target_symbol: symbols::SymbolHandle,
    target_name: &Identifier,
    arguments: arena::HandleSpan<ExpressionHandle>,
    selections: &[DynamicConformanceSelection],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source_span = program
        .symbols
        .symbol_source_span(caller.symbol)
        .unwrap_or(source_span);
    let Some(target_state) = called_state(program, target_symbol, target_name) else {
        return;
    };
    let parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self);
    for (parameter, argument) in parameters.zip(
        program
            .expression_table
            .expression_handles(arguments)
            .iter()
            .copied(),
    ) {
        let Some(TypeReferenceNode::DynamicTrait {
            symbol: target_trait,
            conformance: expected_conformance,
            ..
        }) = dynamic_trait_reference(program, parameter.type_reference)
        else {
            continue;
        };
        if expected_conformance.is_some() {
            continue;
        }

        let source_type =
            crate::places::declared_place_type_raw(program, caller, Some(caller_state), argument)
                .or_else(|| {
                    crate::places::declared_place_type_raw(
                        program,
                        caller,
                        Some(caller_state),
                        strip_mutable(program, argument),
                    )
                });
        let Some(source_type) = source_type else {
            continue;
        };
        if let Some(TypeReferenceNode::DynamicTrait {
            symbol: source_trait,
            ..
        }) = dynamic_trait_reference(program, source_type)
        {
            let passed = passed_dynamic_selection(
                program,
                caller,
                caller_state,
                statement_index,
                argument,
                selections,
            );
            if *source_trait == *target_trait
                && (passed.is_some_and(|selection| selection.target_trait == *target_trait)
                    || is_direct_dynamic_parameter(program, caller_state, argument, *target_trait))
            {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "call to `{target_name}` cannot pass dynamic value to bare parameter `{}` without one earlier exact compatible local conformance selection or dynamic parameter",
                parameter.name
            )));
            continue;
        }
        let Some((_, source_name)) = nominal_data_type(program, source_type) else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == *target_trait)
        else {
            continue;
        };
        let complete_count = program
            .conformances()
            .iter()
            .filter(|conformance| {
                program
                    .symbols
                    .source_reference_can_see_symbol(source_span, conformance.symbol)
                    && conformance
                        .carrier_name()
                        .is_some_and(|carrier| carrier.as_str() == source_name)
                    && conformance.trait_name == trait_definition.name
                    && conformance.arguments.is_empty()
                    && matches!(
                        &conformance.implementation,
                        typed_trees::trait_definition::ConformanceImplementation::Closed { .. }
                    )
            })
            .count();
        match complete_count {
            1 => {}
            0 => diagnostics.push(Diagnostic::error(format!(
                "call to `{target_name}` cannot pass `{source_name}` to bare dynamic parameter `{}`: no complete closed conformance to `{}` is available",
                parameter.name, trait_definition.name
            ))),
            count => diagnostics.push(Diagnostic::error(format!(
                "call to `{target_name}` cannot pass `{source_name}` to bare dynamic parameter `{}`: {count} complete closed conformances to `{}` are available; declare the parameter with one exact named dynamic conformance",
                parameter.name, trait_definition.name
            ))),
        }
    }
}

fn is_direct_dynamic_parameter(
    program: &TypedTrees,
    caller_state: &State,
    argument: ExpressionHandle,
    target_trait: symbols::SymbolHandle,
) -> bool {
    let Some(source) = dynamic_source_place(program, strip_mutable(program, argument)) else {
        return false;
    };
    source.path.len() == 1
        && program
            .state_parameters(caller_state)
            .iter()
            .any(|parameter| {
                !parameter.is_self
                    && parameter.symbol == source.symbol
                    && dynamic_trait_reference(program, parameter.type_reference).is_some_and(
                        |node| {
                            matches!(
                                node,
                                TypeReferenceNode::DynamicTrait {
                                    symbol,
                                    conformance: None,
                                    ..
                                } if *symbol == target_trait
                            )
                        },
                    )
            })
}

fn passed_dynamic_selection<'selections>(
    program: &TypedTrees,
    caller: &Machine,
    caller_state: &State,
    statement_index: usize,
    argument: ExpressionHandle,
    selections: &'selections [DynamicConformanceSelection],
) -> Option<&'selections DynamicConformanceSelection> {
    let source = dynamic_source_place(program, strip_mutable(program, argument))?;
    if source.path.len() != 1 {
        return None;
    }
    selections
        .iter()
        .filter(|selection| {
            selection.machine == caller.symbol
                && selection.state == caller_state.symbol
                && selection.statement_index < statement_index
                && if source.symbol.is_valid() {
                    selection.binding == source.symbol
                } else {
                    selection.binding_name == source.name
                }
        })
        .max_by_key(|selection| selection.statement_index)
}

fn called_state<'program>(
    program: &'program TypedTrees,
    target_symbol: symbols::SymbolHandle,
    target_name: &Identifier,
) -> Option<&'program State> {
    if target_symbol.is_valid() {
        if let Some(state) = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find(|state| state.symbol == target_symbol)
        {
            return Some(state);
        }
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == target_symbol)
        {
            return program.machine_states(machine).first();
        }
    }
    (!program.symbols.has_source_metadata())
        .then(|| {
            program
                .machines()
                .iter()
                .find(|machine| machine.attached_data.is_none() && machine.name == *target_name)
                .and_then(|machine| program.machine_states(machine).first())
        })
        .flatten()
}

/// Bind every call through a typed dynamic receiver to the exact requirement
/// symbol that declares its table slot. Early symbol resolution cannot do this
/// for locals because their declared types are not available until typed trees.
/// This pass runs before validation and checked-fact construction, so no later
/// consumer has to recover a slot from its spelling.
pub fn resolve_dynamic_call_targets(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut expression_updates = Vec::new();
    let mut statement_updates = Vec::new();
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if let StatementNode::Call(call) = statement {
                    let dynamic_target = program
                        .statement_table
                        .name_path_members(call.receiver)
                        .last()
                        .and_then(|receiver| {
                            crate::calls::declared_receiver_type_reference(
                                program,
                                machine,
                                state,
                                receiver.as_str(),
                            )
                        })
                        .and_then(|receiver_type| {
                            resolved_dynamic_requirement_symbol(
                                program,
                                receiver_type,
                                call.target.as_str(),
                                &mut diagnostics,
                            )
                        });
                    let target = dynamic_target.or_else(|| {
                        crate::placed_views::statement_call_target(program, machine, state, call)
                    });
                    if let Some(target) = target {
                        statement_updates.push((state.statement_nodes, statement_index, target));
                    }
                }

                for root in crate::calls::statement_value_expression_roots(program, statement) {
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
            if let Some(receiver_type) =
                crate::places::declared_place_type_raw(program, machine, Some(state), call.receiver)
                && let Some(requirement) = resolved_dynamic_requirement_symbol(
                    program,
                    receiver_type,
                    call.target.as_str(),
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
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
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
        TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
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

#[derive(Debug, Clone)]
struct DynamicSourcePlace {
    symbol: symbols::SymbolHandle,
    name: Identifier,
    path: Vec<Identifier>,
}

fn dynamic_source_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<DynamicSourcePlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => dynamic_source_place(program, atomic.value),
        ExpressionNode::Borrow(inner) => dynamic_source_place(program, inner.target),
        ExpressionNode::Name(name) => {
            let leaf = program
                .expression_table
                .name_path_members(name.members)
                .last()?
                .clone();
            Some(DynamicSourcePlace {
                symbol: name.symbol,
                name: leaf.clone(),
                path: vec![leaf],
            })
        }
        ExpressionNode::Member(member) => {
            let mut source = dynamic_source_place(program, member.receiver)?;
            source.symbol = member.member_symbol;
            source.name = member.member.clone();
            source.path.push(member.member.clone());
            Some(source)
        }
        _ => None,
    }
}

/// Explain why one requirement is absent from a local `dyn Trait` surface.
/// Eligibility is intentionally per requirement: an ineligible sibling does
/// not invalidate calls to the rest of the trait.
pub(crate) fn dynamic_requirement_call_error(
    program: &TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
    target_symbol: symbols::SymbolHandle,
) -> Option<String> {
    let trait_symbol = dynamic_trait_symbol(program, receiver_type)?;
    let dynamic_trait = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)?;
    let mut matches = Vec::new();
    collect_dynamic_requirements_named(
        program,
        dynamic_trait,
        target,
        &mut Vec::new(),
        &mut matches,
    );
    let selected = matches
        .iter()
        .find(|(_, requirement)| requirement.symbol == target_symbol)
        .copied()
        .or_else(|| {
            let [selected] = matches.as_slice() else {
                return None;
            };
            Some(*selected)
        });
    let (declaring_trait, requirement) = selected?;

    let reason = match program
        .dynamic_signature_eligibility(declaring_trait, requirement)
        .err()?
    {
        DynamicSignatureIneligibility::BoundaryRequirement => {
            "boundary-machine requirements are not local dynamic calls"
        }
        DynamicSignatureIneligibility::RequirementLocalGenerics => {
            "the requirement has requirement-local generic parameters"
        }
        DynamicSignatureIneligibility::MissingBorrowedReceiver => {
            "the requirement has no `&self` or `&mut self` receiver"
        }
        DynamicSignatureIneligibility::ByValueReceiver => {
            "the receiver is by value rather than `&self` or `&mut self`"
        }
        DynamicSignatureIneligibility::MultipleReceivers => {
            "the requirement has more than one receiver"
        }
        DynamicSignatureIneligibility::SelfOutsideReceiver => {
            "`Self` appears outside the borrowed receiver"
        }
        DynamicSignatureIneligibility::SelfResult => "`Self` appears in the result type",
    };

    Some(format!(
        "requirement `{}::{}` is absent from `dyn {}`: {reason}",
        declaring_trait.name, requirement.name, dynamic_trait.name
    ))
}

pub(crate) fn dynamic_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_symbol(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

fn dynamic_trait_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&TypeReferenceNode> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_reference(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_reference(program, *base_type)
        }
        dynamic @ TypeReferenceNode::DynamicTrait { .. } => Some(dynamic),
        _ => None,
    }
}

fn nominal_data_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, &str)> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => nominal_data_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => nominal_data_type(program, *base_type),
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol)
            .map(|definition| (definition.symbol, name.as_str())),
        _ => None,
    }
}

/// Recognize the exact local dynamic-coercion shape after removing the one
/// borrow wrapper introduced by `&place as &dyn Carrier::Conformance`.
/// Consumers must share this normalization so validation cannot admit a
/// selection while independently fencing the same receiver shape.
pub(crate) fn normalized_dynamic_coercion(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, &TableCastExpression, &TypeReferenceNode)> {
    let occurrence = strip_mutable(program, expression);
    let ExpressionNode::Cast(cast) = program.expression_table.expression(occurrence) else {
        return None;
    };
    let target = dynamic_trait_reference(program, cast.target_type)?;
    Some((occurrence, cast, target))
}

fn strip_mutable(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    }
}
