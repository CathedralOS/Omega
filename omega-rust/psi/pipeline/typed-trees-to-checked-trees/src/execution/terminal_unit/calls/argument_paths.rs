//! Checked scalar arguments and projected argument paths: field and index
//! paths, projected call support, contract mentions and crash expressions.

use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_type_identity, parameter_root_symbol,
    structural_access_for_type_reference, terminal_field_identity,
    type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
    MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, ProofFact, SignatureContractKind, StateParameter,
    SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees, is_reference,
};

pub(crate) fn checked_call_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
    boundary: bool,
) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(argument_ordinal, parameter)| {
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                }
            };
            let computations = &facts.values.scalar_computations;
            let roots = computations
                .roots
                .iter()
                .filter(|(_, root)| {
                    root.state == caller_state
                        && root.statement_ordinal == coordinate.statement_index
                        && root.role == role
                })
                .map(|(_, root)| root)
                .collect::<Vec<_>>();
            if let [root] = roots.as_slice() {
                if !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).primitive_type != parameter.primitive_type
                    || facts
                        .values
                        .scalar_expressions
                        .expressions
                        .iter()
                        .any(|expression| {
                            expression.state == caller_state
                                && expression.statement_ordinal == coordinate.statement_index
                                && expression.role == role
                        })
                {
                    return None;
                }
                return Some(checked_trees::CheckedCallScalarArgument::Computation(
                    root.root,
                ));
            }
            if !roots.is_empty() {
                return None;
            }
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (crate::values::scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| checked_trees::CheckedCallScalarArgument::Pure(expression.clone()))
        })
        .collect()
}

/// Proof-only erased actuals of an in-module Unit call: the retained proof
/// terms recorded under `ErasedUnitCallArgument`, in the callee's dense
/// erased proof-formal order. Each term's constructed (or forwarded) data
/// identity must match the formal's canonical `Nat`-style identity.
pub(crate) fn checked_call_erased_proof_arguments(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[checked_trees::CheckedErasedProofParameterPlan],
) -> Option<Vec<checked_trees::CheckedProofTerm>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).ok()?,
            };
            let term =
                facts
                    .values
                    .proof_terms
                    .term_at(caller_state, coordinate.statement_index, role)?;
            proof_term_type_matches(program, caller_state, term, &parameter.type_identity)
                .then(|| term.clone())
        })
        .collect()
}

/// The construction or forwarded formal carries exactly the formal's declared
/// proof-only data identity.
fn proof_term_type_matches(
    program: &TypedTrees,
    caller_state: SymbolHandle,
    term: &checked_trees::CheckedProofTerm,
    type_identity: &str,
) -> bool {
    match term {
        checked_trees::CheckedProofTerm::Construction {
            type_identity: actual,
            ..
        } => actual == type_identity,
        checked_trees::CheckedProofTerm::Formal { parameter_symbol } => {
            crate::semantic::calls::find_state(program, caller_state).is_some_and(|state| {
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.symbol == *parameter_symbol
                        && base_type_identity(program, parameter.type_reference, &[]).as_deref()
                            == Some(type_identity)
                })
            })
        }
        // A bare scalar leaf never satisfies a proof-formal position: scalar
        // leaves only occur nested inside a construction.
        checked_trees::CheckedProofTerm::Scalar(_) => false,
    }
}

/// Proof-only erased actuals of an in-module Unit call: the retained pure
/// expressions recorded under `ErasedUnitCallArgument`, in the callee's dense
/// erased-formal order.
pub(crate) fn checked_call_erased_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = CheckedScalarExpressionRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).ok()?,
            };
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (crate::values::scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| checked_trees::CheckedCallScalarArgument::Pure(expression.clone()))
        })
        .collect()
}

fn checked_nonempty_field_path(path: &[CheckedUnitStructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
}

fn checked_literal_index_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Option<&[CheckedUnitStructuralPathSegment]> {
    let field_count = path
        .iter()
        .position(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))?;
    let (fields, indexes) = path.split_at(field_count);
    (!indexes.is_empty()
        && fields
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        && indexes
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_))))
    .then_some(fields)
}

/// Translate an authored place into the checked structural path a call
/// argument retains, and report the exact type that path projects to. Every
/// admitted fixed index is checked against its literal array length, so the
/// returned path names storage the place actually contains. Root and field
/// resolution belong to `canonical_place_type_reference`, which already owns
/// the two `self` spellings.
///
/// Both call lanes share this owner. What does not belong here is the admission
/// decision: which segment shapes a lane permits, and which rule the projected
/// type must satisfy against the target parameter. The ordinary transitive lane
/// admits exact mutable byte-view presentations as well as equal normalized
/// identity; each call owner retains its own access and path restrictions.
pub(crate) fn projected_argument_path(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path_impl(program, state_symbol, statement_index, place, None)
}

/// `projected_argument_path` with runtime-index admission for the borrowed
/// receiver lane. A runtime `Index` segment retains `RuntimeIndex` — the
/// checked selector's dense scalar position and the inclusive bounds its
/// retained integer entry range publishes — only while
/// `bounded_runtime_index` proves `0 <= index < extent` against the enclosing
/// fixed array's literal extent. Every other caller keeps rejecting runtime
/// indexes through `projected_argument_path`.
pub(crate) fn projected_borrowed_receiver_path(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path_impl(
        program,
        state_symbol,
        statement_index,
        place,
        Some((facts, machine)),
    )
}

fn projected_argument_path_impl(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    runtime_index_bounds: Option<(&CheckFacts, SymbolHandle)>,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    let mut path = Vec::with_capacity(place.segments.len());
    for (position, segment) in place.segments.iter().enumerate() {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                path.push(CheckedUnitStructuralPathSegment::Field(
                    terminal_field_identity(program, *symbol)?,
                ));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                let container = crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                let container = crate::flow::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &container,
                )?;
                if *index >= fixed_array_literal_length(program, container)? {
                    return None;
                }
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
            }
            facts::PlaceSegment::Index { expression } => {
                let (facts, machine) = runtime_index_bounds?;
                let container = crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                let container = crate::flow::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &container,
                )?;
                let extent = fixed_array_literal_length(program, container)?;
                let (selector, minimum, maximum) = bounded_runtime_index(
                    program,
                    facts,
                    machine,
                    state_symbol,
                    *expression,
                    extent,
                )?;
                path.push(CheckedUnitStructuralPathSegment::RuntimeIndex {
                    selector,
                    minimum,
                    maximum,
                });
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Case { .. } => {
                return None;
            }
        }
    }
    let projected =
        crate::flow::canonical_place_type_reference(program, state_symbol, statement_index, place)?;
    Some((projected, path))
}

/// The bounds evidence an authored runtime index must publish before a
/// `RuntimeIndex` segment may retain it. The selector expression must be one
/// bare direct scalar parameter of the caller's entry state, and that
/// parameter's retained closed integer entry range must prove
/// `0 <= index < extent` for the fixed array it indexes. The returned
/// selector is the parameter's dense scalar position — the same coordinate
/// `CheckedScalarExpression::Parameter` and the Terminal caller's
/// `parameters` vector share — and the inclusive endpoints restate the range
/// row at the parameter's declared signedness, so terminal verification
/// replays the published row rather than trusting this segment.
fn bounded_runtime_index(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    extent: usize,
) -> Option<(
    u32,
    semantic_vocabulary::IntegerValue,
    semantic_vocabulary::IntegerValue,
)> {
    let machine = crate::lookup::machine_by_symbol(program, machine)?;
    // Retained integer entry ranges publish on the machine's entry state, so
    // a bounded runtime selector proves only there.
    if program.machine_states(machine).first()?.symbol != state_symbol {
        return None;
    }
    let state = crate::semantic::calls::find_state(program, state_symbol)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameters = program.state_parameters(state);
    let authored_position = crate::values::parameter_position(program, name, parameters)?;
    let authored = &parameters[authored_position];
    let primitive_type = program.primitive_type_reference(authored.type_reference)?;
    // `CheckedScalarExpression::Parameter` positions skip erased primitive
    // formals while the retained range roster counts every primitive
    // parameter; both namespaces derive from this one authored order.
    let selector = parameters[..authored_position]
        .iter()
        .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
        .count();
    let roster_position = parameters[..authored_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .count();
    let requirement = facts
        .contract_plans
        .for_machine(machine.symbol)?
        .closed_scalar_values
        .integer_entry_ranges()?
        .iter()
        .find(|requirement| {
            requirement.position == roster_position && requirement.primitive_type == primitive_type
        })?;
    // A negative minimum fails `to_u64`, and the normalized inclusive maximum
    // must fit strictly below the declared extent.
    let minimum = requirement.minimum.value_bignum()?.to_u64()?;
    let maximum = requirement.maximum.value_bignum()?.to_u64()?;
    if maximum >= u64::try_from(extent).ok()? {
        return None;
    }
    let endpoint = |value: u64| {
        if primitive_type.is_signed_integer() {
            semantic_vocabulary::IntegerValue::Signed(i128::from(value))
        } else {
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(value))
        }
    };
    Some((
        u32::try_from(selector).ok()?,
        endpoint(minimum),
        endpoint(maximum),
    ))
}

/// The shared owner under the ordinary rule: the projected type must carry the
/// target parameter's exact normalized identity.
pub(crate) fn projected_argument_path_with_identity(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    target_identity: &str,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (projected, path) = projected_argument_path(program, state_symbol, statement_index, place)?;
    (base_type_identity(program, projected, &[])? == target_identity).then_some(path)
}

/// The literal element count of a fixed array reached through any number of
/// domain constraints and borrows. A length that is not an authored literal
/// bounds no fixed index here.
fn fixed_array_literal_length(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    while let TypeReferenceNode::Constrained { base_type, .. }
    | TypeReferenceNode::Reference {
        referee: base_type, ..
    } = program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    Some(*length)
}

/// A range loan stays a projection of its original fixed-array backing. The
/// call owns its lifetime, so no independently live mutable descriptor is
/// manufactured. Runtime endpoints need separate extent evidence; this route
/// retains only canonical fixed bounds and rechecks the actual array extent.
pub(crate) fn fixed_byte_array_range_path(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (facts::PlaceSegment::FixedRange { start, end }, fields) = place.segments.split_last()?
    else {
        return None;
    };
    if !fields
        .iter()
        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
    {
        return None;
    }
    let backing = crate::flow::CanonicalPlace {
        root: place.root,
        segments: fields.to_vec(),
    };
    let (backing_type, mut path) =
        projected_argument_path(program, state, statement_index, &backing)?;
    let extent = fixed_array_literal_length(program, backing_type)?;
    if extent == 0
        || start > end
        || *end > extent
        || !super::boundary_admission::fixed_byte_array_view_is_admitted(
            program,
            backing_type,
            target,
        )
    {
        return None;
    }
    path.push(CheckedUnitStructuralPathSegment::FixedByteRange {
        start: u64::try_from(*start).ok()?,
        end: u64::try_from(*end).ok()?,
    });
    Some(path)
}

pub(crate) fn ordinary_projected_call_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    arguments: &[CheckedUnitStructuralArgumentPlan],
    allow_field_path_projection: bool,
) -> bool {
    if arguments.iter().all(|argument| argument.path.is_empty()) {
        return true;
    }
    // A retained result carrier presents its borrowed referent, not a moved
    // aggregate field. Exact producing local/loan custody was rejoined by the
    // shared result argument builder and is independently replayed in lowering.
    if arguments.iter().all(|argument| {
        argument.path.is_empty()
            || (argument
                .source_structural_result_binding_ordinal()
                .is_some()
                && argument.path.last() == Some(&CheckedUnitStructuralPathSegment::Referent)
                && argument.path[..argument.path.len() - 1]
                    .iter()
                    .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
                && argument.access == CheckedStructuralAccess::MutableBorrow)
    }) {
        return true;
    }

    let caller_source_parameters = program.state_parameters(caller_state);
    let target_source_parameters = program.state_parameters(target_state);
    let target_parameters = target_source_parameters
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && !(parameter.is_self && is_reference(program, parameter.type_reference))
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if target_parameters.len() != arguments.len() {
        return false;
    }
    // A live result binding lends a record subtree through an exclusive
    // borrow: the binding itself is not moved, the checked loan evidence was
    // already rejoined by the result-argument builder, and lowering replays
    // the same projected place against its own result roster.
    if target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                if argument.path.is_empty() {
                    return true;
                }
                argument
                    .source_structural_result_binding_ordinal()
                    .is_some()
                    && !target.is_self
                    && argument.path.iter().all(|segment| {
                        matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
                    })
                    && matches!(
                        argument.access,
                        CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::WriteOnlyBorrow
                    )
                    && structural_access_for_type_reference(program, target.type_reference)
                        == Some(argument.access)
            })
    {
        return true;
    }
    let has_content_evidence = |machine, state| {
        facts
            .qualifications
            .content
            .identity_reshuffles
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
            || facts
                .qualifications
                .content
                .partition_compositions
                .iter()
                .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
    };
    if has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol)
        || arguments
            .iter()
            .zip(&target_parameters)
            .any(|(argument, parameter)| {
                !argument.path.is_empty()
                    && target_contract_mentions_projected_parameter(
                        program,
                        facts,
                        target_machine,
                        target_state,
                        parameter,
                    )
            })
    {
        return false;
    }

    // Argument construction has already rejoined each canonical place, exact
    // referent type, and authored loan. Borrowed projections compose per
    // argument; neither the number of sibling loans nor the caller's state
    // count grants or removes access to those places.
    if target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                if argument.path.is_empty() {
                    return true;
                }
                let Some(source) = argument
                    .source_parameter_index()
                    .and_then(|index| caller_parameters.get(index as usize))
                else {
                    return false;
                };
                if !source.qualifications.is_empty()
                    || target.is_self
                    || caller_source_parameters
                        .get(source.position as usize)
                        .is_none()
                    || structural_access_for_type_reference(program, target.type_reference)
                        != Some(argument.access)
                {
                    return false;
                }
                let field_path = checked_nonempty_field_path(&argument.path);
                let indexed_fields = checked_literal_index_path(&argument.path);
                let static_path = argument.path.iter().all(|segment| {
                    matches!(
                        segment,
                        CheckedUnitStructuralPathSegment::Field(_)
                            | CheckedUnitStructuralPathSegment::FixedIndex(_)
                            | CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                    )
                });
                match argument.access {
                    CheckedStructuralAccess::SharedBorrow => {
                        source.multiplicity == Multiplicity::Unrestricted
                            && crate::checks::type_multiplicity(program, target.type_reference)
                                == Multiplicity::Unrestricted
                            && static_path
                    }
                    CheckedStructuralAccess::MutableBorrow => {
                        source.access == CheckedStructuralAccess::MutableBorrow
                            && ((source.multiplicity == Multiplicity::Unrestricted && static_path)
                                || ((field_path
                                    || indexed_fields.is_some_and(|fields| !fields.is_empty()))
                                    && byte_sequence_carrier(program, target.type_reference, &[])
                                        == Some(
                                            checked_trees::CheckedByteSequenceCarrier::BorrowedView,
                                        )))
                    }
                    CheckedStructuralAccess::WriteOnlyBorrow => {
                        matches!(
                            source.access,
                            CheckedStructuralAccess::MutableBorrow
                                | CheckedStructuralAccess::WriteOnlyBorrow
                        ) && source.multiplicity == Multiplicity::Unrestricted
                            && static_path
                    }
                    CheckedStructuralAccess::Owned => false,
                }
            })
    {
        return true;
    }
    // A dying-continuation consumer may mix several projected operands with
    // ordinary whole operands. Each projected anonymous temporary rejoins its
    // own result through `binding_ordinal`, and the appended call
    // continuation keeps each owner's residual rows separately. A projected
    // owned parameter joins the same operand shape; its untouched complement
    // stays with the caller and publishes on the caller's return edge
    // instead of the call continuation.
    if allow_field_path_projection
        && target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments.iter().any(|argument| {
            (argument
                .source_structural_result_binding_ordinal()
                .is_some()
                || argument.source_parameter_index().is_some())
                && argument.access == CheckedStructuralAccess::Owned
                && !argument.path.is_empty()
        })
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                let result_projection = argument
                    .source_structural_result_binding_ordinal()
                    .is_some();
                // The projected parameter root must itself be claim-free
                // owned affine: its complement publishes residual rows on
                // the caller's return edge, which has no vocabulary for
                // borrowed or qualified custody.
                let parameter_projection = !result_projection
                    && !argument.path.is_empty()
                    && argument.source_parameter_index().is_some_and(|index| {
                        caller_parameters.get(index as usize).is_some_and(|source| {
                            source.access == CheckedStructuralAccess::Owned
                                && source.multiplicity == Multiplicity::Affine
                                && source.qualifications.is_empty()
                        })
                    });
                if result_projection || parameter_projection {
                    return argument.access == CheckedStructuralAccess::Owned
                        && !argument.path.is_empty()
                        && argument.path.iter().all(|segment| {
                            matches!(
                                segment,
                                CheckedUnitStructuralPathSegment::Field(_)
                                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
                            )
                        })
                        && !target.is_self
                        && crate::checks::type_multiplicity(program, target.type_reference)
                            == Multiplicity::Affine
                        && !type_graph_requires_nominal_drop(program, target.type_reference);
                }
                argument.path.is_empty()
            })
        && program.machine_states(caller_machine).len() == 1
        && program.machine_states(target_machine).len() == 1
        && facts
            .contract_plans
            .for_machine(caller_machine.symbol)
            .is_some()
        && facts
            .contract_plans
            .for_machine(target_machine.symbol)
            .is_some()
    {
        return true;
    }
    if arguments.len() != 1 {
        return false;
    }
    // Only the partial-cleanup owner enables projections from its exact
    // earlier result roster. Ordinary whole-value sequencing leaves it off.
    let result_projection = allow_field_path_projection
        && arguments[0]
            .source_structural_result_binding_ordinal()
            .is_some()
        && arguments[0].access == CheckedStructuralAccess::Owned;
    let caller_parameter = if result_projection {
        None
    } else {
        // A projected owned parameter names its root by index; the caller
        // may carry any number of structural siblings because the residual
        // complement publishes on the caller's return edge, not the call.
        let Some(parameter_index) = arguments[0].source_parameter_index() else {
            return false;
        };
        caller_parameters.get(parameter_index as usize)
    };
    if !result_projection
        && caller_parameter
            .and_then(|parameter| {
                usize::try_from(parameter.position)
                    .ok()
                    .and_then(|position| caller_source_parameters.get(position))
            })
            .is_none()
    {
        return false;
    };

    let owned_affine_projection = allow_field_path_projection
        && (result_projection
            || caller_parameter.is_some_and(|parameter| {
                parameter.access == CheckedStructuralAccess::Owned
                    && parameter.multiplicity == Multiplicity::Affine
            }))
        && arguments[0].access == CheckedStructuralAccess::Owned
        && !arguments[0].path.is_empty()
        && arguments[0].path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
            )
        });
    let field_path = checked_nonempty_field_path(&arguments[0].path);
    let literal_index_fields = checked_literal_index_path(&arguments[0].path);
    let literal_index_path = literal_index_fields.is_some();
    let literal_indexed_field_path = literal_index_fields.is_some_and(|fields| !fields.is_empty());
    if field_path && !allow_field_path_projection {
        return false;
    }
    if literal_indexed_field_path && !owned_affine_projection {
        return false;
    }
    if !field_path && !literal_index_path && !owned_affine_projection {
        return false;
    }

    if field_path || owned_affine_projection {
        let [target_parameter] = target_parameters.as_slice() else {
            return false;
        };
        return program.machine_states(caller_machine).len() == 1
            && program.machine_states(target_machine).len() == 1
            && (result_projection
                || caller_parameter.is_some_and(|parameter| {
                    parameter.multiplicity == Multiplicity::Affine
                        && parameter.qualifications.is_empty()
                }))
            && !target_parameter.is_self
            && crate::checks::type_multiplicity(program, target_parameter.type_reference)
                == Multiplicity::Affine
            && !type_graph_requires_nominal_drop(program, target_parameter.type_reference)
            && facts
                .contract_plans
                .for_machine(caller_machine.symbol)
                .is_some()
            && facts
                .contract_plans
                .for_machine(target_machine.symbol)
                .is_some();
    }

    arguments
        .iter()
        .zip(target_parameters)
        .filter(|(argument, _)| !argument.path.is_empty())
        .all(|(_, parameter)| {
            let mut type_reference = parameter.type_reference;
            loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        if !program
                            .type_reference_table
                            .constraints(*constraints)
                            .is_empty()
                        {
                            return false;
                        }
                        type_reference = *base_type;
                    }
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    _ => break,
                }
            }

            let expected_root =
                facts::PlaceRoot::Symbol(parameter_root_symbol(target_machine.symbol, parameter));
            let matching = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == target_machine.symbol
                        && event.state_symbol == target_state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                        && event.root == expected_root
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return false;
            };
            claim.claim_identity != PermissionClaimIdentity::Unknown
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(claim.segments)
                    .is_empty()
        })
}

pub(crate) fn target_contract_mentions_projected_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let expected_root = parameter_root_symbol(target_machine.symbol, parameter);
    let runtime_arithmetic_requires_are_terminal = facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.uses_structural_proof_gated_arithmetic()
                && contract.crash.structural_runtime_requirements().is_some()
        });
    let authored_contract_mentions_parameter = program
        .state_contracts(target_state)
        .iter()
        .filter(|contract| match contract.kind {
            SignatureContractKind::Crashes { .. } => false,
            SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => false,
            SignatureContractKind::Requires
            | SignatureContractKind::Ensures
            | SignatureContractKind::EnsuresForResultCase { .. } => true,
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let ProofFact::Membership(membership) = fact else {
                return false;
            };
            crate::flow::canonical_place_from_expression_in_state(
                program,
                target_state.symbol,
                0,
                membership.value,
            )
            .is_some_and(|place| {
                place.root == facts::PlaceRoot::Symbol(expected_root)
                    || place.root == facts::PlaceRoot::Symbol(parameter.symbol)
            })
        });
    if authored_contract_mentions_parameter {
        return true;
    }

    facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.published().iter().any(|bucket| {
                bucket.alternative_guards().iter().any(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => false,
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        if matches!(
                            predicate.scalar_expression(),
                            Some(
                                checked_trees::CheckedBooleanExpression::StructuralParameterField {
                                    parameter_position: 0,
                                    path,
                                }
                            ) if !path.is_empty()
                        ) {
                            return false;
                        }
                        if predicate.expression().is_some_and(|expression| {
                            crash_expression_is_nonempty_member_path_from_parameter(expression, 0)
                        }) {
                            return false;
                        }
                        predicate.expression().is_none_or(|expression| {
                            crash_expression_mentions_parameter_outside_member_path(expression, 0)
                        })
                    }
                })
            })
        })
}

pub(crate) fn crash_expression_is_nonempty_member_path_from_parameter(
    expression: &checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use checked_trees::CrashPredicateExpression;

    let mut expression = expression;
    let mut nonempty = false;
    while let CrashPredicateExpression::Member { receiver, .. } = expression {
        nonempty = true;
        expression = receiver;
    }
    nonempty
        && matches!(expression, CrashPredicateExpression::Parameter(index) if *index == parameter)
}

pub(crate) fn crash_expression_mentions_parameter_outside_member_path(
    expression: &checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use checked_trees::CrashPredicateExpression;

    match expression {
        CrashPredicateExpression::Parameter(index) => *index == parameter,
        CrashPredicateExpression::Binary { left, right, .. } => {
            crash_expression_mentions_parameter_outside_member_path(left, parameter)
                || crash_expression_mentions_parameter_outside_member_path(right, parameter)
        }
        CrashPredicateExpression::Unary { operand, .. }
        | CrashPredicateExpression::IntegerWiden { operand, .. } => {
            crash_expression_mentions_parameter_outside_member_path(operand, parameter)
        }
        CrashPredicateExpression::Member { receiver, .. } => {
            if crash_expression_is_nonempty_member_path_from_parameter(expression, parameter) {
                false
            } else {
                crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
            }
        }
        CrashPredicateExpression::Indexed { collection, index } => {
            crash_expression_mentions_parameter_outside_member_path(collection, parameter)
                || crash_expression_mentions_parameter_outside_member_path(index, parameter)
        }
        CrashPredicateExpression::Range { start, end, .. } => {
            crash_expression_mentions_parameter_outside_member_path(start, parameter)
                || crash_expression_mentions_parameter_outside_member_path(end, parameter)
        }
        CrashPredicateExpression::Call {
            receiver,
            arguments,
            ..
        } => {
            crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
                || arguments.iter().any(|argument| {
                    crash_expression_mentions_parameter_outside_member_path(argument, parameter)
                })
        }
        CrashPredicateExpression::Invalid
        | CrashPredicateExpression::Opaque(_)
        | CrashPredicateExpression::ContentConservation(_) => true,
        CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Float(_)
        | CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Name(_) => false,
    }
}

pub(crate) fn byte_sequence_literal_argument(
    program: &TypedTrees,
    parameter_type: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if byte_sequence_carrier(program, parameter_type, &[])?
        != checked_trees::CheckedByteSequenceCarrier::BorrowedView
        || structural_access_for_type_reference(program, parameter_type)?
            != CheckedStructuralAccess::SharedBorrow
    {
        return None;
    }
    let ExpressionNode::String(bytes) = program.expression_table.expression(expression) else {
        return None;
    };
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: bytes.to_vec(),
        },
        path: Vec::new(),
        type_identity: byte_sequence_type_identity(program, parameter_type, &[], &[])?,
        access: CheckedStructuralAccess::SharedBorrow,
    })
}
