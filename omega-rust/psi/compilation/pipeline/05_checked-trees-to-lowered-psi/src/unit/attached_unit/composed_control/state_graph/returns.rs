//! Ordinary structural graph results, with exact authored field custody, and
//! primitive scalar results. A scalar-result state completes with the
//! ordinary single-state completion: the binding its final expression's
//! operation produced, or its value-only exit roster (`validate_scalar`).
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralFieldType, StructuralResultDeclaration,
};
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, Multiplicity, Operation,
    OperationKind, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachineResult, ValueDeclaration,
    allocate_dense, direct_expression_contains_short_circuit, emit_direct_expression,
    lookup_type_id, obligation_id, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use checked_trees::{CheckedControlResultPlan, data::DataMember, expression::ExpressionNode};

pub(super) fn signature_matches(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    result: &CheckedControlResultPlan,
) -> bool {
    match result {
        CheckedControlResultPlan::Unit => matches!(
            checked
                .type_reference_table
                .type_reference(source.return_type),
            checked_trees::types::TypeReferenceNode::Unit
        ),
        // A primitive result carries no refinement beyond an arithmetic
        // policy or one closed integer range, which the contract publishes
        // as a normal-return guarantee (`callable::scalar_guarantees`).
        CheckedControlResultPlan::Scalar { primitive_type } => {
            checked.primitive_type_reference(source.return_type) == Some(*primitive_type)
                && (matches!(
                    checked
                        .type_reference_table
                        .type_reference(source.return_type),
                    checked_trees::types::TypeReferenceNode::Named { .. }
                ) || validation::is_arithmetic_policy_only_integer(
                    &checked.typed,
                    source.return_type,
                ) || validation::closed_scalar_result_range(&checked.typed, source.return_type)
                    .is_some())
        }
        CheckedControlResultPlan::Structural(result) => {
            // A borrowed `&[T]` view result names the peeled slice carrier in
            // its identity — the contents live in the caller's frame, so the
            // owned-contents custody clause does not apply.
            // Slice views use their element-view carrier. Named records and
            // primitive referents retain the reference shell and its affine
            // permission; neither becomes owned referent storage.
            if validation::reference_result_custody::mutable_borrowed_parts(
                &checked.typed,
                source.return_type,
            )
            .is_some()
            {
                // Only a slice view has a peeled carrier here.
                let identity = crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                    checked,
                    source.return_type,
                )
                .map(|carrier| checked.normalized_type_identity(carrier).into_string())
                .unwrap_or_else(|| {
                    checked
                        .normalized_type_identity(source.return_type)
                        .into_string()
                });
                return identity.as_str() == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            if let Some(slice) = borrowed_view_slice(checked, source.return_type) {
                return checked.normalized_type_identity(slice).as_str() == result.type_identity
                    && validation::reference_result_custody::result_multiplicity(
                        &checked.typed,
                        source.return_type,
                    ) == result.multiplicity
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            // A `&mut T` result is alias custody, not an owned structural
            // claim: the plan identity spells the whole reference type, the
            // binding itself is affine, and no owned qualification rows
            // exist to compare. The caller-side loan the `EstablishReference`
            // source rejoins is verified at its own emission, not here.
            if validation::reference_result_custody::mutable_borrowed_parts(
                &checked.typed,
                source.return_type,
            )
            .is_some()
            {
                return checked
                    .normalized_type_identity(source.return_type)
                    .as_str()
                    == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && result.qualifications.is_empty()
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            // A shared `&'a V` result spells the same whole-reference
            // identity convention: `add_view_type` registers under the
            // reference's own identity with the affine view-loan
            // multiplicity the checking side mints; the named referent
            // stays in the caller's frame and the binding carries loan,
            // not contents.
            if validation::reference_result_custody::shared_borrowed_parts(
                &checked.typed,
                source.return_type,
            )
            .is_some()
            {
                return checked
                    .normalized_type_identity(source.return_type)
                    .as_str()
                    == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            let Ok(carrier) = crate::unit::attached_unit::parameters::structural_carrier_type(
                checked,
                source.return_type,
            ) else {
                return false;
            };
            checked
                .normalized_type_identity(carrier)
                .as_str()
                == result.type_identity
                && checked.type_multiplicity(source.return_type) == result.multiplicity
                && validation::structural_result_qualifications(&checked.typed, source.return_type).ok().as_ref() == Some(&result.qualifications)
                && validation::structural_result_projected_qualifications(
                    &checked.typed,
                    source.return_type,
                )
                .ok()
                .as_ref()
                    == Some(&result.projected_qualifications)
                // Plain storage classification excludes linear roots by design.
                // Their admission instead requires exact entry/call/return claims.
                && (result.multiplicity == Multiplicity::Linear
                    || (result.multiplicity == Multiplicity::Unrestricted
                        && validation::has_plain_owned_contents_with_represented_constraints(&checked.typed, source.return_type))
                    || validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    carrier,
                ) || validation::has_owned_or_shared_view_contents(&checked.typed, carrier))
        }
    }
}

/// The slice a shared-borrow `&[T]` result refers to, peeling constraint and
/// reference shells. Mutable and write-only borrows stay with their own
/// custody families.
fn borrowed_view_slice(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let mut referee = loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            checked_trees::types::TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } => break *referee,
            _ => return None,
        }
    };
    // Constraint shells inside the borrow qualify the referent's storage, not
    // the loan's shape: `&([u8] in Utf8)` refers to a slice exactly as `&[u8]`
    // does, and the domain rides the carrier the plan spells.
    loop {
        match checked.type_reference_table.type_reference(referee) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                referee = *base_type
            }
            checked_trees::types::TypeReferenceNode::Slice { .. } => return Some(referee),
            _ => return None,
        }
    }
}

/// Domain qualifications a view result declares on its own carrier:
/// constraints above the borrow directly, and on a byte view the
/// inside-borrow domains too — the lowered view type peels the reference so
/// `&[u8] in Utf8` registers `Utf8` under the slice identity the result
/// spells. Other referent domains stay inside the loan.
fn view_result_qualifications(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Option<Vec<language_semantics::SemanticDomainId>> {
    validation::view_result_carrier_qualifications(&checked.typed, reference).ok()
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedControlResultPlan::Structural(_) = &plan.result else {
        return unsupported("case return has no structural result signature");
    };
    let CheckedComposedUnitControlTerminatorPlan::ReturnCase { result } = &state.terminator else {
        return unsupported("case return terminator absent");
    };
    validate_case(checked, source, state, ordinal, result)
}

pub(super) fn validate_case(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
    result: &checked_trees::CheckedStructuralCaseReturnPlan,
) -> Result<(), LoweringError> {
    let checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity,
        fields,
    } = result;
    if *statement_ordinal as usize != ordinal {
        return unsupported("case return statement moved");
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(source.return_type)
    else {
        return unsupported("case result is not a nominal sum");
    };
    let expression =
        crate::expression_preparation::source_custody::guarded_exits::completion_expression(
            checked, source, ordinal,
        )?;
    let (case_symbol, authored) = match checked.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_symbol == *symbol => (
            literal.case_symbol.ok_or(LoweringError::Unsupported(
                "case constructor has no selected case",
            ))?,
            checked.expression_table.struct_fields(literal.fields),
        ),
        ExpressionNode::Name(path)
            if path.head_symbol == *symbol
                && checked
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
        {
            (path.symbol, &[][..])
        }
        _ => return unsupported("case return does not construct its declared result"),
    };
    let declaration = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
        .ok_or(LoweringError::Unsupported(
            "case result declaration missing",
        ))?;
    let variant = checked
        .data_members(declaration)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == case_symbol => Some(variant),
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "case return selected a foreign case",
        ))?;
    let identity = variant.path_identity();
    let declarations = checked.data_payload_fields(variant);
    if identity != *case_identity
        || authored.len() != fields.len()
        || fields.len() != declarations.len()
    {
        return unsupported("case return field or case roster drifted");
    }
    for (field_index, (authored, field)) in authored.iter().zip(fields).enumerate() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.symbol == authored.field_symbol)
            .ok_or(LoweringError::Unsupported(
                "case return field has a foreign declaration",
            ))?;
        let identity = declaration.path_identity();
        if field.field_ordinal as usize != field_index
            || identity != field.field_identity
            || checked.primitive_type_reference(declaration.type_reference)
                != Some(field.primitive_type)
            || fields[..field_index]
                .iter()
                .any(|previous| previous.field_identity == field.field_identity)
        {
            return unsupported("case return field identity or evaluation order drifted");
        }
        let (binding, retained) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                *statement_ordinal,
                CheckedScalarExpressionRole::ReturnCaseField {
                    field_ordinal: field.field_ordinal,
                },
            )
            .ok_or(LoweringError::Unsupported(
                "case return field lost its checked expression",
            ))?;
        if binding.expression != authored.value
            || binding.destination.is_valid()
            || retained != &field.expression
        {
            return unsupported("case return field expression disagrees with source");
        }
        crate::expression_preparation::source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(field.primitive_type)?,
        )?;
    }
    Ok(())
}

/// Rejoin a scalar-result state's completion with its authored tail. An exit
/// roster is the ordinary single-state one and must begin exactly at this
/// state's terminator. A binding completion returns the final expression
/// through the operation that binds it (whose value `body::validate`
/// rejoins), or a final name through the earlier immutable binding it reads.
pub(super) fn validate_scalar(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    completion: &checked_trees::CheckedScalarReturnPlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedControlResultPlan::Scalar { primitive_type } = plan.result else {
        return unsupported("scalar graph return has no scalar result signature");
    };
    let binding = match completion {
        checked_trees::CheckedScalarReturnPlan::Exits(exits) => {
            if exits.primitive_type != primitive_type
                || crate::unit::attached_unit::scalar_completion::control::validate_exits(
                    checked,
                    state.state,
                    exits,
                )? != ordinal
            {
                return unsupported("scalar graph exits drifted from their authored tail");
            }
            return Ok(());
        }
        checked_trees::CheckedScalarReturnPlan::Binding(binding) => binding,
    };
    let statements = checked.statement_table.statements(source.statement_nodes);
    if binding.primitive_type != primitive_type {
        return unsupported("scalar graph return has no final authored expression");
    }
    let expression = match statements.get(ordinal..) {
        Some([checked_trees::statement::StatementNode::Expression(expression)]) => {
            Some(*expression)
        }
        // An unconditional terminal machine call at the closing transition
        // completes with the result it binds there
        // (state_contracts.md, "Terminal calls in transition arms").
        Some([checked_trees::statement::StatementNode::Transition(_)])
            if binding.statement_index as usize == ordinal
                && crate::emission::call_source_custody::authored::linear_terminal_invocation(
                    checked,
                    state.state,
                    binding.statement_index,
                )? =>
        {
            None
        }
        _ => return unsupported("scalar graph return has no final authored expression"),
    };
    // A scalar local or an ordinary scalar call produces the returned binding;
    // a terminal machine call is necessarily that call.
    let mut producers = state.operations.iter().filter(|operation| match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
            expression.is_some() && result == binding
        }
        CheckedUnitEffectOperationPlan::Call(call) => {
            !call.is_boundary() && call.scalar_result() == Some(binding)
        }
        _ => false,
    });
    if producers.next().is_none() || producers.next().is_some() {
        return unsupported("scalar graph return has no unique producer");
    }
    if binding.statement_index as usize == ordinal {
        return Ok(());
    }
    let (Some(expression), Some(checked_trees::statement::StatementNode::LocalData(local))) =
        (expression, statements.get(binding.statement_index as usize))
    else {
        return unsupported("scalar graph return name has no immutable binding");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("scalar graph return reads neither its producer nor a local");
    };
    if local.is_mutable
        || !local.symbol.is_valid()
        || name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || checked.primitive_type_reference(local.type_reference) != Some(binding.primitive_type)
    {
        return unsupported("scalar graph return name differs from its exact binding");
    }
    Ok(())
}

pub(super) fn validate_structural(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    let (
        CheckedControlResultPlan::Structural(signature),
        CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result },
    ) = (&plan.result, &state.terminator)
    else {
        return unsupported("structural return has no result signature");
    };
    if signature.type_identity != result.type_identity
        || signature.multiplicity != result.multiplicity
    {
        return unsupported("structural return changed its declared result custody");
    }
    let statement = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(ordinal)
        .ok_or(LoweringError::Unsupported(
            "structural return has no authored completion",
        ))?;
    // A returning tail is either the bare expression or an unconditional
    // `(expression)` transition arm carrying it on the same statement.
    let expression = match statement {
        checked_trees::statement::StatementNode::Expression(expression) => expression,
        checked_trees::statement::StatementNode::Transition(transition)
            if transition.guard == checked_trees::statement::TransitionGuardNode::Always
                && transition.exit == checked_trees::statement::TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            let checked_trees::statement::TransitionTargetNode::Value(expression) =
                checked.statement_table.transition_target(transition.target)
            else {
                return unsupported("structural return has no authored completion");
            };
            expression
        }
        _ => {
            return unsupported("structural return has no authored completion");
        }
    };
    match result.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            let mut producers = state.operations.iter().filter_map(|operation| {
                let result = match operation {
                    // A `&mut` view tail return's producer is the reference
                    // establishment minted at the transition.
                    CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
                    | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. }
                    | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. } => {
                        Some(result)
                    }
                    _ => super::result_custody::result(operation),
                }?;
                (result.binding_ordinal == binding_ordinal).then_some((operation, result))
            });
            let (operation, binding) = producers.next().ok_or(LoweringError::Unsupported(
                "structural return producer absent",
            ))?;
            if producers.next().is_some()
                || binding.type_identity != result.type_identity
                || binding.multiplicity != result.multiplicity
                || binding.statement_index as usize > ordinal
            {
                return unsupported("structural return producer custody drifted");
            }
            if binding.statement_index as usize == ordinal {
                if operation
                    .call()
                    .is_some_and(|call| call.structural_result().is_some())
                {
                    crate::unit::attached_unit::structural_calls::validate_custody(
                        checked,
                        super::owner(checked, state.state)?,
                        state.state,
                        operation,
                    )?;
                    crate::emission::call_source_custody::validate_operation(
                        checked,
                        super::owner(checked, state.state)?,
                        state.state,
                        operation,
                        &state.structural_parameters,
                    )?;
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                ) {
                    crate::expression_preparation::source_custody::structural::validate(
                        checked,
                        super::owner(checked, state.state)?,
                        state.state,
                        operation,
                    )?;
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                ) {
                    super::super::super::byte_locals::validate(checked, state.state, operation)?;
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                ) {
                    super::super::super::view_ranges::binding_site(
                        checked,
                        state.state,
                        operation,
                    )?;
                } else if let CheckedUnitEffectOperationPlan::EstablishReference {
                    source, ..
                } = operation
                {
                    super::super::super::reference_results::validate_composed_establishment(
                        checked,
                        super::owner(checked, state.state)?,
                        state,
                        binding,
                        source,
                        *expression,
                    )?;
                } else {
                    return unsupported(
                        "structural return tail producer requires exact value custody",
                    );
                }
            } else {
                let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
                    .statement_table
                    .statements(source.statement_nodes)
                    .get(binding.statement_index as usize)
                else {
                    return unsupported("structural return does not name its produced local");
                };
                if !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
                {
                    return unsupported("structural return exchanged its produced local");
                }
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural return parameter absent",
                ))?;
            let declaration = checked
                .state_parameters(source)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural return source parameter absent",
                ))?;
            if parameter.type_identity != result.type_identity
                || parameter.multiplicity != result.multiplicity
                || !matches!(
                    parameter.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == declaration.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("structural return exchanged its owned parameter");
            }
        }
        // A literal-backed `&[u8]` return carries its bytes as the whole
        // ingress: the authored completion is that same string literal —
        // nothing else could have exchanged or weakened the loan.
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { ref bytes } => {
            if !matches!(checked.expression_table.expression(*expression), ExpressionNode::String(literal) if literal.as_ref() == bytes.as_slice())
            {
                return unsupported("structural return exchanged its literal source");
            }
        }
        _ => return unsupported("structural return requires whole established custody"),
    }
    Ok(())
}

pub(super) fn result(
    plan: &CheckedControlResultPlan,
    reference_sources: &[checked_trees::CheckedReferenceResultSourcePlan],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    catalogs: &mut catalogs::ComposedCatalogs,
    places: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<TerminalMachineResult, LoweringError> {
    let result = match plan {
        CheckedControlResultPlan::Unit => return Ok(TerminalMachineResult::Unit),
        CheckedControlResultPlan::Scalar { primitive_type } => {
            return Ok(TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(&mut catalogs.next_value)?),
                scalar_type: terminal_scalar_type(*primitive_type)?,
            }));
        }
        CheckedControlResultPlan::Structural(result) => result,
    };
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Result,
    });
    // Each arm retains its exact ingress source. The result interface groups
    // alternatives by returned leaf; Terminal replay checks actual exit coverage.
    let mut reference_sources = reference_sources
        .iter()
        .map(|reference| {
            let source = crate::unit::attached_unit::parameters::lower_reference_source(
                &reference.source, &catalogs.structural_types, parameters, &[], &[], &[],
            )?;
            Ok(terminal_psi::StructuralReferenceResultSource {
                path: crate::expression_preparation::bindings::structural_paths::lower_structural_path(&reference.path)?,
                sources: vec![source],
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    reference_sources.sort_by(|left, right| left.path.cmp(&right.path));
    let mut grouped_sources: Vec<terminal_psi::StructuralReferenceResultSource> =
        Vec::with_capacity(reference_sources.len());
    for reference in reference_sources {
        if let Some(group) = grouped_sources.last_mut()
            && group.path == reference.path
        {
            group.sources.extend(reference.sources);
        } else {
            grouped_sources.push(reference);
        }
    }
    for group in &mut grouped_sources {
        group.sources.sort();
        group.sources.dedup();
    }
    Ok(TerminalMachineResult::Structural(
        StructuralResultDeclaration {
            reference_sources: grouped_sources,
            place,
            structural_type: lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
            multiplicity: match result.multiplicity {
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: result
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(
                        &catalogs.domain_ids,
                        *domain,
                        lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications:
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    &result.projected_qualifications,
                    &catalogs.domain_ids,
                    lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
                    &catalogs.structural_types,
                )?,
        },
    ))
}

pub(super) fn emit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    result: &TerminalMachineResult,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    catalogs: &mut catalogs::ComposedCatalogs,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<PlaceId>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ReturnCase {
        result: construction,
    } = &state.terminator
    else {
        return Ok(None);
    };
    emit_case(
        checked,
        state,
        construction,
        result,
        bindings,
        catalogs,
        values,
        next_value,
        operations,
    )
}

pub(super) fn emit_case(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    construction: &checked_trees::CheckedStructuralCaseReturnPlan,
    result: &TerminalMachineResult,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    catalogs: &mut catalogs::ComposedCatalogs,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<PlaceId>, LoweringError> {
    let checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity,
        fields,
    } = construction;
    let result = result.structural().ok_or(LoweringError::Unsupported(
        "case construction result signature missing",
    ))?;
    let declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or(LoweringError::Unsupported(
            "case construction result type missing",
        ))?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("case construction result is not a sum");
    };
    let case = cases
        .iter()
        .find(|case| case.identity == *case_identity)
        .ok_or(LoweringError::Unsupported(
            "case construction declaration missing",
        ))?;
    if fields.len() != case.fields.len() {
        return unsupported("case construction declaration roster drifted");
    }
    let mut evaluated = Vec::new();
    for field in fields {
        let declaration = case
            .fields
            .iter()
            .find(|declaration| declaration.identity == field.field_identity)
            .ok_or(LoweringError::Unsupported(
                "case construction field declaration missing",
            ))?;
        let expression = bindings.expression_at(
            checked,
            state.state,
            *statement_ordinal,
            CheckedScalarExpressionRole::ReturnCaseField {
                field_ordinal: field.field_ordinal,
            },
        )?;
        if declaration.field_type.scalar_type() != Some(expression.scalar_type())
            || direct_expression_contains_short_circuit(&expression)
        {
            return unsupported("case construction needs an exact branch-free scalar operand");
        }
        validate_direct_parameter_types(
            &expression,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let value = emit_direct_expression(&expression, values, next_value, operations);
        let range_obligation = if matches!(
            declaration.field_type,
            StructuralFieldType::BoundedInteger(_)
        ) {
            Some(obligation_id(allocate_dense(
                &mut catalogs.scalar_calls.next_call_obligation,
            )?))
        } else {
            None
        };
        evaluated.push(terminal_psi::ScalarCaseField {
            field: declaration.id,
            value,
            range_obligation,
        });
    }
    evaluated.sort_by_key(|field| field.field);
    let operation = operations.allocate();
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    catalogs.result_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type: result.structural_type,
        },
    });
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: terminal_psi::OperationResult::Structural(
            terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        ),
        kind: OperationKind::EstablishScalarCase {
            result_case: case.id,
            fields: evaluated,
        },
    });
    Ok(Some(place))
}
