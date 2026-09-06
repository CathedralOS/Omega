use diagnostics::Diagnostic;
use language_semantics::{MachineSupplyMode, ReferenceAccess};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod receiver;

#[derive(Clone)]
struct WriteOnlyRoot {
    symbol: SymbolHandle,
    receiver_machine: SymbolHandle,
    name: String,
    referee: TypeReferenceHandle,
    is_parameter: bool,
}

pub(crate) fn validate_checked_write_only_slice(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut roots = program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    let TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::WriteOnly,
                        ..
                    } = program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    else {
                        return None;
                    };
                    Some(WriteOnlyRoot {
                        symbol: parameter.symbol,
                        receiver_machine: if parameter.is_self {
                            machine.symbol
                        } else {
                            SymbolHandle::invalid()
                        },
                        name: parameter.name.as_str().to_owned(),
                        referee: *referee,
                        is_parameter: true,
                    })
                })
                .collect::<Vec<_>>();
            roots.extend(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| {
                        let StatementNode::LocalData(local) = statement else {
                            return None;
                        };
                        let TypeReferenceNode::Reference {
                            referee,
                            access: ReferenceAccess::WriteOnly,
                            ..
                        } = program
                            .type_reference_table
                            .type_reference(local.type_reference)
                        else {
                            return None;
                        };
                        Some(WriteOnlyRoot {
                            symbol: local.symbol,
                            receiver_machine: SymbolHandle::invalid(),
                            name: local.name.as_str().to_owned(),
                            referee: *referee,
                            is_parameter: false,
                        })
                    }),
            );
            if roots.is_empty() {
                continue;
            }

            if !matches!(machine.supply_mode, MachineSupplyMode::CheckedBody) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` declares `&write`, but the current milestone proves non-observation only for checked Omega bodies; boundary, accepted, requirement, and external-provider declarations require an admitted write-only boundary claim",
                    machine.name,
                    state.name,
                )));
            }

            for root in &roots {
                if !is_supported_checked_referee(program, root.referee)
                    && receiver::record(program, root).is_none()
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` parameter `{}` uses `&write` with `{}`; the current checked slice supports unrestricted primitive scalars, recursively literal fixed arrays whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums, forwarding-only byte slices, non-generic invariant-free checked records, and closed material `[copy]` sums as atomic whole values",
                        machine.name,
                        state.name,
                        root.name,
                        program.display_type_reference_with_constraints(root.referee),
                    )));
                }
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                validate_statement(program, machine, state, statement, &roots, diagnostics);
            }
        }
    }
}

fn is_unrestricted_scalar(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    !name.as_str().starts_with("Atomic")
        && program.primitive_type_reference(type_reference).is_some()
}

fn fixed_unrestricted_write_only_array_shape(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, usize)> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    is_unrestricted_write_only_array_element(program, *element_type)
        .then_some((*element_type, *length))
}

fn fixed_unrestricted_write_only_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    fixed_unrestricted_write_only_array_shape(program, type_reference).map(|(_, length)| length)
}

fn literal_fixed_array_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::FixedArray {
        length: FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    Some(*length)
}

fn is_supported_checked_referee(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_unrestricted_scalar(program, type_reference)
        || fixed_unrestricted_write_only_array_length(program, type_reference).is_some()
        || is_byte_slice(program, type_reference)
        || write_only_record(program, type_reference).is_some()
        || is_unrestricted_write_only_sum(program, type_reference)
}

fn is_byte_slice(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Slice { element_type }
            if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}

fn is_write_only_length_metadata(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
    roots: &[WriteOnlyRoot],
) -> bool {
    if member.case_variant.is_some() || member.member.as_str() != "len" {
        return false;
    }

    if direct_write_only_root(program, member.receiver, roots).is_some_and(|root| {
        is_byte_slice(program, root.referee)
            || fixed_unrestricted_write_only_array_length(program, root.referee).is_some()
    }) {
        return true;
    }

    write_only_record_field_type(program, member.receiver, roots)
        .and_then(|field_type| literal_fixed_array_length(program, field_type))
        .is_some()
}

/// Resolve one closed nominal data definition whose shape is known without
/// substitution and whose authored domain cannot couple replacement to prior
/// content. Record traversal and atomic sum replacement apply their own shape
/// judgments below.
fn closed_write_only_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    closed_write_only_data_by_symbol(program, *symbol)
}

fn closed_write_only_data_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataDefinition> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    (definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && definition.lifetime_parameters.is_empty()
        && program.data_type_parameters(definition).is_empty()
        && definition.quotient.is_none()
        && definition.where_facts.is_empty()
        && !definition.zero_gated)
        .then_some(definition)
}

/// The first aggregate traversal rung is deliberately nominal and closed. A
/// record may contain wider siblings without making them writable; final-leaf
/// eligibility is checked separately at the exact assignment target.
fn write_only_record(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let definition = closed_write_only_data(program, type_reference)?;
    (DataDefinition::shape_kind_from_members(program.data_members(definition))
        == DataShapeKind::Record)
        .then_some(definition)
}

fn is_unrestricted_write_only_record(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    write_only_record(program, type_reference).is_some_and(|definition| {
        definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
    })
}

/// A closed material `[copy]` sum may be displaced only as one whole value.
/// The incoming value supplies its complete tag and payload, so this judgment
/// neither observes nor projects the prior case. Erased payload occurrences
/// remain outside the runtime replacement carrier.
fn is_unrestricted_write_only_sum(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    closed_write_only_data(program, type_reference).is_some_and(|definition| {
        definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
            && DataDefinition::shape_kind_from_members(program.data_members(definition))
                == DataShapeKind::Enum
            && program.data_members(definition).iter().all(|member| {
                let DataMember::Variant(variant) = member else {
                    return false;
                };
                program
                    .data_payload_fields(variant)
                    .iter()
                    .all(|field| !field.relevance.is_erased())
            })
    })
}

/// Fixed-array aggregate elements stay a closed runtime shape: the existing
/// primitive scalars plus eligible unrestricted records or sums whose direct
/// runtime occurrences are all material. A recursively literal fixed array of
/// the same eligible elements is also one atomic element of its enclosing
/// array. Generic/qualified shells do not enter through this judgment, and
/// aggregate elements remain atomic.
fn is_unrestricted_write_only_array_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    is_unrestricted_scalar(program, type_reference)
        || fixed_unrestricted_write_only_array_shape(program, type_reference).is_some()
        || write_only_record(program, type_reference).is_some_and(|definition| {
            definition.properties.multiplicity
                == language_semantics::Multiplicity::Unrestricted
                && program.data_members(definition).iter().all(|member| {
                    matches!(member, DataMember::Field(field) if !field.relevance.is_erased())
                })
        })
        || is_unrestricted_write_only_sum(program, type_reference)
}

fn whole_root_replacement_is_supported(program: &TypedTrees, root: &WriteOnlyRoot) -> bool {
    is_unrestricted_scalar(program, root.referee)
        || fixed_unrestricted_write_only_array_length(program, root.referee).is_some()
        || is_unrestricted_write_only_record(program, root.referee)
        || is_unrestricted_write_only_sum(program, root.referee)
        || receiver::record(program, root).is_some_and(|definition| {
            definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
        })
}

/// Resolve `root.record_field...leaf`, where every receiver is an admitted
/// plain record and every selected field is relevant and unconstrained. This
/// is a store-place judgment only: expression traversal still rejects reading
/// the same path, and sum payloads never enter this content-independent walk.
fn write_only_record_field_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<TypeReferenceHandle> {
    let mut cursor = expression;
    let mut members = Vec::new();
    while let ExpressionNode::Member(member) = program.expression_table.expression(cursor) {
        if member.case_variant.is_some() {
            return None;
        }
        members.push(cursor);
        cursor = member.receiver;
    }
    let (root, mut receiver_type, starts_at_receiver) =
        if let Some(root) = direct_write_only_root(program, cursor, roots) {
            (root, root.referee, true)
        } else {
            let (root, field) = receiver::bare_field(program, cursor, roots)?;
            if field.relevance.is_erased() {
                return None;
            }
            if members.is_empty() {
                return Some(field.type_reference);
            }
            (root, field.type_reference, false)
        };
    if members.is_empty() {
        return None;
    }

    for (index, member_handle) in members.iter().rev().enumerate() {
        let ExpressionNode::Member(member) = program.expression_table.expression(*member_handle)
        else {
            unreachable!("member path was collected above")
        };
        let definition = if index == 0 && starts_at_receiver && root.receiver_machine.is_valid() {
            receiver::record(program, root)?
        } else {
            write_only_record(program, receiver_type)?
        };
        let field = if index == 0 && starts_at_receiver && root.receiver_machine.is_valid() {
            receiver::field(program, root, *member_handle)?
        } else {
            program
                .data_members(definition)
                .iter()
                .find_map(|candidate| {
                    let DataMember::Field(field) = candidate else {
                        return None;
                    };
                    ((member.member_symbol.is_valid() && field.symbol == member.member_symbol)
                        || (!member.member_symbol.is_valid()
                            && field.name.as_str() == member.member.as_str()))
                    .then_some(field)
                })?
        };
        if field.relevance.is_erased() {
            return None;
        }
        if index + 1 == members.len() {
            return Some(field.type_reference);
        }
        receiver_type = field.type_reference;
    }
    None
}

/// The final displaced record-path leaf must be an unrestricted primitive, a
/// literal fixed array of eligible unrestricted elements, a whole eligible
/// unrestricted record, or a closed material `[copy]` sum treated atomically.
/// Indexed element stores reuse the same exact path resolver below and apply
/// their own narrower leaf/index gate.
fn write_only_record_field_assignment(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    write_only_record_field_type(program, expression, roots).is_some_and(|field_type| {
        is_unrestricted_scalar(program, field_type)
            || fixed_unrestricted_write_only_array_length(program, field_type).is_some()
            || is_unrestricted_write_only_record(program, field_type)
            || is_unrestricted_write_only_sum(program, field_type)
    })
}

/// Admit one relevant primitive field beneath one literal fixed-array element
/// reached through an otherwise ordinary common-field record path. The array
/// element stays a closed unrestricted record, and the literal index fixes the
/// complete write footprint without observing the referent.
fn write_only_literal_indexed_record_field_assignment(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    let ExpressionNode::Member(final_member) = program.expression_table.expression(expression)
    else {
        return false;
    };
    if final_member.case_variant.is_some() {
        return false;
    }
    let ExpressionNode::Indexed(indexed) =
        program.expression_table.expression(final_member.receiver)
    else {
        return false;
    };
    let Some(collection_type) = write_only_record_field_type(program, indexed.collection, roots)
    else {
        return false;
    };
    let Some((element_type, length)) =
        fixed_unrestricted_write_only_array_shape(program, collection_type)
    else {
        return false;
    };
    let Some(index) = program
        .expression_table
        .constant_integer_value(indexed.index)
        .and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if index >= length {
        return false;
    }
    let Some(definition) = write_only_record(program, element_type) else {
        return false;
    };
    program.data_members(definition).iter().any(|candidate| {
        let DataMember::Field(field) = candidate else {
            return false;
        };
        ((final_member.member_symbol.is_valid() && field.symbol == final_member.member_symbol)
            || (!final_member.member_symbol.is_valid()
                && field.name.as_str() == final_member.member.as_str()))
            && !field.relevance.is_erased()
            && is_unrestricted_scalar(program, field.type_reference)
    })
}

fn validate_statement(
    program: &TypedTrees,
    machine_definition: &Machine,
    state_definition: &State,
    statement: &StatementNode,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machine = machine_definition.name.as_str();
    let state = state_definition.name.as_str();
    match statement {
        StatementNode::AssemblyFact(fact) => {
            validate_expression(program, machine, state, fact.expression, roots, diagnostics)
        }
        StatementNode::Assignment(assignment) => {
            if let Some(root) = direct_write_only_root(program, assignment.target, roots) {
                if !whole_root_replacement_is_supported(program, root) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` replaces whole write-only aggregate `{}`; whole-root replacement requires a freely discardable supported root, so replace one eligible leaf through an invariant-free record path or declare and prove an unrestricted material aggregate instead",
                        root.name,
                    )));
                }
                // An admitted whole-value replacement observes no prior
                // content. Aggregate roots additionally satisfy the explicit
                // closed-shape/material/discardability checks above.
            } else if write_only_record_field_assignment(program, assignment.target, roots) {
                // One content-independent common-field-path store. The exact
                // field place is retained by the ordinary checked mutation facts.
            } else if write_only_literal_indexed_record_field_assignment(
                program,
                assignment.target,
                roots,
            ) {
                // One literal fixed-array element and its exact relevant
                // primitive record field form a content-independent place.
            } else if validate_write_only_fixed_array_range_assignment(
                program,
                machine_definition,
                state_definition,
                assignment.target,
                assignment.value,
                roots,
                diagnostics,
            ) {
                // The range-specific gate owns non-observation and RHS shape.
                // Ordinary range validation independently owns order/bounds.
            } else if let Some(index) =
                write_only_element_assignment_index(program, assignment.target, roots)
            {
                // The normal range checker separately proves a dynamic index
                // is in bounds. This gate owns only non-observation: the index
                // expression must not recover information from the write-only
                // referent.
                validate_expression(program, machine, state, index, roots, diagnostics);
            } else if expression_mentions_write_only_root(program, assignment.target, roots) {
                diagnose_unsupported_write_only_assignment_target(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                );
            } else {
                validate_expression(
                    program,
                    machine,
                    state,
                    assignment.target,
                    roots,
                    diagnostics,
                );
            }
            validate_expression(
                program,
                machine,
                state,
                assignment.value,
                roots,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            receiver::validate_statement_call(program, machine, state, call, roots, diagnostics);
            for argument in program.statement_table.expression_handles(call.arguments) {
                validate_call_argument(program, machine, state, *argument, roots, diagnostics);
            }
        }
        StatementNode::Expression(expression) => {
            validate_expression(program, machine, state, *expression, roots, diagnostics)
        }
        StatementNode::LocalData(local) => validate_expression(
            program,
            machine,
            state,
            local.initial_value,
            roots,
            diagnostics,
        ),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                validate_expression(program, machine, state, guard, roots, diagnostics);
            }
            validate_transition_target(
                program,
                machine,
                state,
                transition.target,
                roots,
                diagnostics,
            );
            validate_transition_target(
                program,
                machine,
                state,
                transition.continuation,
                roots,
                diagnostics,
            );
        }
    }
}

/// Validate an exact range replacement: a statically normalized half-open
/// window of a direct fixed array of eligible unrestricted elements, or an
/// eligible common-field path ending in one, replaced by an array literal of
/// exactly the same element width. Returns whether the target was such a range
/// even when another checker owns its eventual rejection.
fn validate_write_only_fixed_array_range_assignment(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    value: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let (root, collection_type) =
        if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
            (root, root.referee)
        } else {
            let Some(collection_type) =
                write_only_record_field_type(program, indexed.collection, roots)
            else {
                return false;
            };
            let Some(root) = mentioned_write_only_root(program, indexed.collection, roots) else {
                return false;
            };
            (root, collection_type)
        };
    let Some((element_type, collection_len)) =
        fixed_unrestricted_write_only_array_shape(program, collection_type)
    else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };

    if range.start.is_valid() {
        validate_expression(
            program,
            machine.name.as_str(),
            state.name.as_str(),
            range.start,
            roots,
            diagnostics,
        );
    }
    if range.end.is_valid() {
        validate_expression(
            program,
            machine.name.as_str(),
            state.name.as_str(),
            range.end,
            roots,
            diagnostics,
        );
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces a write-only fixed-array range with an omitted end; this exact-footprint rung requires a statically known end bound",
            machine.name, state.name,
        )));
        return true;
    }

    let start = if range.start.is_valid() {
        crate::normalize_immutable_integer_bound_to_usize(program, range.start)
    } else {
        Some(0)
    };
    let end =
        crate::normalize_immutable_integer_bound_to_usize(program, range.end).and_then(|end| {
            if range.end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        });
    let (Some(start), Some(end)) = (start, end) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces a write-only fixed-array range whose bounds are not statically known; exact range replacement currently requires literal bounds or finite immutable local-copy aliases",
            machine.name, state.name,
        )));
        return true;
    };

    // The ordinary range checker emits the directed order/bounds diagnostic.
    // Do not add a misleading write-only-shape error for the same invalid range.
    if start > end || end > collection_len {
        return true;
    }
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::ArrayLiteral(_)
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces write-only fixed-array range `{}[{}..{}]` from a non-literal value; the exact range-replacement rung requires an array literal of {} element(s)",
            machine.name,
            state.name,
            root.name,
            start,
            end,
            end - start,
        )));
        return true;
    }
    crate::struct_literals::validate_array_literal_elements_for_shape(
        program,
        machine,
        state,
        value,
        element_type,
        Some(end - start),
        diagnostics,
    );
    true
}

fn write_only_element_assignment_index(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> Option<ExpressionHandle> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let (collection_type, direct_byte_slice) =
        if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
            (root.referee, is_byte_slice(program, root.referee))
        } else {
            (
                write_only_record_field_type(program, indexed.collection, roots)?,
                false,
            )
        };
    if direct_byte_slice {
        return (!matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ))
        .then_some(indexed.index);
    }
    let length = fixed_unrestricted_write_only_array_length(program, collection_type)?;
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(_) => None,
        ExpressionNode::Integer(index) => {
            let index = usize::try_from(index.value_i64()?).ok()?;
            (index < length).then_some(indexed.index)
        }
        _ => Some(indexed.index),
    }
}

/// Admit one exact literal primitive element of either a direct fixed-array
/// root or an already-admitted common-field path only at the direct checked-call
/// boundary. A finite nonempty suffix of literal indexes may traverse
/// recursively literal fixed arrays. Dynamic indices, ranges, and aggregate
/// elements remain excluded.
fn write_only_literal_indexed_direct_call_subloan(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    let mut collection = expression;
    let mut indices = Vec::new();
    while let ExpressionNode::Indexed(indexed) = program.expression_table.expression(collection) {
        if !matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Integer(_)
        ) {
            return false;
        }
        indices.push(indexed.index);
        collection = indexed.collection;
    }
    if indices.is_empty() {
        return false;
    }

    let Some(mut collection_type) = direct_write_only_root(program, collection, roots)
        .filter(|root| root.is_parameter)
        .map(|root| root.referee)
        .or_else(|| write_only_record_field_type(program, collection, roots))
    else {
        return false;
    };
    for index in indices.into_iter().rev() {
        let Some((element_type, length)) =
            fixed_unrestricted_write_only_array_shape(program, collection_type)
        else {
            return false;
        };
        let Some(index) = program
            .expression_table
            .constant_integer_value(index)
            .and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        if index >= length {
            return false;
        }
        collection_type = element_type;
    }
    is_unrestricted_scalar(program, collection_type)
}

fn diagnose_unsupported_write_only_assignment_target(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        && let Some(root) = direct_write_only_root(program, indexed.collection, roots)
        && fixed_unrestricted_write_only_array_length(program, root.referee).is_some()
    {
        let detail = match program.expression_table.expression(indexed.index) {
            ExpressionNode::Range(_) => "range projection is not implemented",
            ExpressionNode::Integer(index) => match index.value_i64() {
                Some(value) if value < 0 => "the index must be non-negative",
                Some(_) => "the literal index is outside the fixed array",
                None => "the literal index is outside the supported index range",
            },
            _ => "the index expression is not an admissible element place",
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine}` state `{state}` writes through unsupported projection of write-only fixed array `{}`; {detail}; whole-array replacement, proven-in-bounds element replacement, and statically normalized closed-range replacement are accepted",
            root.name,
        )));
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{machine}` state `{state}` writes through an unsupported write-only projection; accepted partial stores are a content-independent common-field path through non-generic invariant-free records when every field is relevant and unconstrained and the displaced leaf is an unrestricted primitive, a whole eligible unrestricted record or closed material `[copy]` sum, or a recursively literal fixed array whose ultimate elements are unrestricted primitive scalars or eligible material `[copy]` records or sums, one relevant primitive field beneath a literal fixed-array record element, a proven-in-bounds element or statically normalized closed range of such a fixed array, or a proven-in-bounds element of a direct byte slice; nested array projection, sum case/payload projection, qualified, invariant-dependent, symbolic or open range, take, swap, and read-modify-write operations remain rejected"
    )));
}

fn validate_transition_target(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    target: typed_trees::statement::TransitionTargetHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            receiver::validate_state_transfer(
                program,
                machine,
                state,
                path.symbol,
                roots,
                diagnostics,
            );
            for argument in program.statement_table.expression_handles(*arguments) {
                validate_expression(program, machine, state, *argument, roots, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => {
            validate_expression(program, machine, state, *value, roots, diagnostics)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn validate_call_argument(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression)
        && borrow.access == ReferenceAccess::WriteOnly
        && (write_only_record_field_assignment(program, borrow.target, roots)
            || write_only_literal_indexed_direct_call_subloan(program, borrow.target, roots))
    {
        // This milestone admits the exact projected subloan only at the direct
        // checked-call argument boundary. It does not create a reusable local
        // reference or widen general expression formation.
        return;
    }
    validate_expression(program, machine, state, expression, roots, diagnostics);
}

fn validate_expression(
    program: &TypedTrees,
    machine: &str,
    state: &str,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some(root) = roots
                .iter()
                .find(|root| receiver::mentions_name(program, root, path))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` reads write-only parameter `{}`; `&write` permits replacement or exact `&write` forwarding, never observation",
                    root.name,
                )));
            }
        }
        ExpressionNode::Borrow(borrow) => match borrow.access {
            ReferenceAccess::WriteOnly => {
                if !is_direct_name(program, borrow.target) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` forms `&write` from an unsupported projection or computed expression; the current checked slice supports explicit attenuation of a whole parameter, plus one eligible content-independent common-field path optionally followed by a finite nonempty suffix of in-bounds literal fixed-array indexes only as a direct checked-call argument"
                    )));
                }
            }
            ReferenceAccess::Mutable => {
                if let Some(root) = mentioned_write_only_root(program, borrow.target, roots) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` widens write-only parameter `{}` to `&mut`; forward it explicitly as `&write {}` instead",
                        root.name, root.name,
                    )));
                } else {
                    validate_expression(program, machine, state, borrow.target, roots, diagnostics);
                }
            }
            ReferenceAccess::Shared => {
                validate_expression(program, machine, state, borrow.target, roots, diagnostics)
            }
        },
        ExpressionNode::Member(member) => {
            if is_write_only_length_metadata(program, member, roots) {
                // A direct slice length belongs to its descriptor. A literal
                // fixed-array length belongs to its static type, including when
                // the array is reached only through statically known common
                // fields of plain records. Neither reads referent content. Do
                // not recurse into the receiver: doing so would misclassify this
                // exact metadata read as observation.
            } else if let Some(root) = mentioned_write_only_root(program, member.receiver, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` reads field `{}` from write-only parameter `{}`; an eligible record-field path may be replaced as an assignment target, but write-only projection never grants observation",
                    member.member, root.name,
                )));
            } else {
                validate_expression(program, machine, state, member.receiver, roots, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(root) = mentioned_write_only_root(program, indexed.collection, roots) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{machine}` state `{state}` reads through index projection of write-only parameter `{}`; `&write` permits admitted fixed-array element replacement but never observation",
                    root.name,
                )));
            } else {
                validate_expression(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    roots,
                    diagnostics,
                );
            }
            validate_expression(program, machine, state, indexed.index, roots, diagnostics);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(program, machine, state, *value, roots, diagnostics);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            validate_expression(program, machine, state, atomic.value, roots, diagnostics);
            validate_expression(program, machine, state, atomic.result, roots, diagnostics);
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(program, machine, state, binary.left, roots, diagnostics);
            validate_expression(program, machine, state, binary.right, roots, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            validate_expression(program, machine, state, cast.value, roots, diagnostics)
        }
        ExpressionNode::Call(call) => {
            let nonobserving_receiver = direct_write_only_root(program, call.receiver, roots)
                .is_some_and(|root| receiver::admits_call(program, root, call.target_symbol));
            if call.receiver.is_valid() && !nonobserving_receiver {
                validate_expression(program, machine, state, call.receiver, roots, diagnostics);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                if nonobserving_receiver {
                    validate_call_argument(program, machine, state, *argument, roots, diagnostics);
                } else {
                    validate_expression(program, machine, state, *argument, roots, diagnostics);
                }
            }
        }
        ExpressionNode::Range(range) => {
            validate_expression(program, machine, state, range.start, roots, diagnostics);
            validate_expression(program, machine, state, range.end, roots, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_expression(program, machine, state, field.value, roots, diagnostics);
            }
        }
        ExpressionNode::Unary(unary) => {
            validate_expression(program, machine, state, unary.operand, roots, diagnostics)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn is_direct_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if program.expression_table.name_path_members(path.members).len() == 1
    )
}

fn direct_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            roots
                .iter()
                .find(|root| receiver::matches_name(program, root, path))
        })
        .flatten()
}

fn mentioned_write_only_root<'a>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &'a [WriteOnlyRoot],
) -> Option<&'a WriteOnlyRoot> {
    roots
        .iter()
        .find(|root| expression_mentions_root(program, expression, root))
}

fn expression_mentions_write_only_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
) -> bool {
    mentioned_write_only_root(program, expression, roots).is_some()
}

fn expression_mentions_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    root: &WriteOnlyRoot,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => receiver::mentions_name(program, root, path),
        ExpressionNode::Borrow(value) => expression_mentions_root(program, value.target, root),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_mentions_root(program, *value, root)),
        ExpressionNode::Atomic(atomic) => {
            expression_mentions_root(program, atomic.value, root)
                || expression_mentions_root(program, atomic.result, root)
        }
        ExpressionNode::Binary(binary) => {
            expression_mentions_root(program, binary.left, root)
                || expression_mentions_root(program, binary.right, root)
        }
        ExpressionNode::Cast(cast) => expression_mentions_root(program, cast.value, root),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_mentions_root(program, call.receiver, root))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_mentions_root(program, *argument, root))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_root(program, indexed.collection, root)
                || expression_mentions_root(program, indexed.index, root)
        }
        ExpressionNode::Member(member) => expression_mentions_root(program, member.receiver, root),
        ExpressionNode::Range(range) => {
            expression_mentions_root(program, range.start, root)
                || expression_mentions_root(program, range.end, root)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_mentions_root(program, field.value, root)),
        ExpressionNode::Unary(unary) => expression_mentions_root(program, unary.operand, root),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
