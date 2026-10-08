//! Admit a closed-case dispatch the case emitter can lower: a whole, plain,
//! non-linear sum subject (an owned or borrowed parameter, a retained local,
//! or the dispatch's one computed call), arms that return a value or a call
//! or jump to a successor that does not take the consumed subject, payload
//! fields the selected case declares, and exit disposal that names only the
//! subject. Stage 04 built the arms from the authored transitions; this does
//! not rejoin them to source.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{CheckedUnitEffectOperationPlan, Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan, edges};
use checked_trees::data::DataMember;
use checked_trees::types::TypeReferenceNode;

/// The sum a Unit case dispatch reads.
#[derive(Clone, Copy)]
enum CaseSubject {
    /// A parameter or an earlier immutable local, named by every arm.
    Place(symbols::SymbolHandle),
    /// The dispatch's one authored call, evaluated once at the first arm's
    /// statement; every other copy of it reads that one result.
    Computed,
}

impl CaseSubject {
    fn place(self) -> Option<symbols::SymbolHandle> {
        match self {
            Self::Place(symbol) => Some(symbol),
            Self::Computed => None,
        }
    }
}

/// The call whose result a computed case dispatch holds: the call planned for
/// the dispatch subject's binding. `None` when the dispatch inspects a place.
pub(super) fn computed_subject(
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Result<Option<&checked_trees::CheckedUnitCallPlan>, LoweringError> {
    if !matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ClosedSum {
            subject_kind: checked_trees::CheckedClosedSumSubjectKind::Computed,
            ..
        }
    ) {
        return Ok(None);
    }
    let mut calls = state.operations.iter().filter_map(|operation| {
        let call = operation.call()?;
        (call.structural_result()?.binding_ordinal == binding_ordinal).then_some(call)
    });
    let Some(call) = calls.next() else {
        return Ok(None);
    };
    if calls.next().is_some() {
        return unsupported("Unit case subject call is duplicated");
    }
    // As for a named structural result, this composed dispatch does not
    // settle boundary claim transfers.
    if call.is_boundary() && !call.claim_transfers().is_empty() {
        return unsupported("Unit case subject call transfers boundary claims");
    }
    Ok(Some(call))
}

fn result_source<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<
    (
        CaseSubject,
        &'a checked_trees::data::DataDefinition,
        language_semantics::PermissionProvenance,
    ),
    LoweringError,
> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum {
        subject,
        subject_type,
        ..
    } = &state.terminator
    else {
        return unsupported("Unit case has no structural subject");
    };
    // An owned subject is whole custody the dispatch consumes; a shared or
    // mutable loan parameter is inspected in place.
    let inspected = match subject.access {
        checked_trees::CheckedStructuralAccess::Owned => false,
        checked_trees::CheckedStructuralAccess::SharedBorrow
        | checked_trees::CheckedStructuralAccess::MutableBorrow => true,
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
            return unsupported("Unit case subject is a write-only loan");
        }
    };
    if inspected
        && subject.path.is_empty()
        && !matches!(
            subject.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        )
    {
        return unsupported("Unit case loan subject is not a parameter");
    }
    let (symbol, reference, provenance) = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let retained = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported("Unit case parameter missing"))?;
            let parameter = checked
                .state_parameters(source)
                .get(retained.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit case source parameter missing",
                ))?;
            let reference = if inspected && subject.path.is_empty() {
                let TypeReferenceNode::Reference { referee, .. } = checked
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    return unsupported("Unit case loan subject is not a reference");
                };
                *referee
            } else {
                parameter.type_reference
            };
            (
                CaseSubject::Place(parameter.symbol),
                reference,
                language_semantics::PermissionProvenance::Unknown,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } if let Some(call) = computed_subject(state, binding_ordinal)? => {
            if call.structural_result().is_none() {
                return unsupported("Unit case subject call lost its result");
            }
            let signature =
                crate::declarations::target_signature(checked, machine, call.target.state)?;
            (
                CaseSubject::Computed,
                signature.return_type,
                language_semantics::PermissionProvenance::Unknown,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            let result = state
                .operations
                .iter()
                .filter_map(super::result_custody::result)
                .find(|result| result.binding_ordinal == binding_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "Unit case result producer missing",
                ))?;
            if !result.local.is_named() {
                return unsupported("Unit case result local missing");
            }
            (
                CaseSubject::Place(result.local.symbol),
                result.local.declared_type,
                language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state.state,
                    source: language_semantics::PermissionEventSource::Statement {
                        statement_index: result.statement_index as usize,
                    },
                },
            )
        }
        _ => return unsupported("Unit case subject has unsupported ownership"),
    };
    let reference = if subject.path.is_empty() {
        reference
    } else {
        *subject_type
    };
    let TypeReferenceNode::Named {
        symbol: type_symbol,
        ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("Unit case subject is not a closed declaration");
    };
    let declaration =
        checked
            .data_definition_by_symbol(*type_symbol)
            .ok_or(LoweringError::Unsupported(
                "Unit case subject declaration absent",
            ))?;
    if !matches!(
        checked.type_multiplicity(reference),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) || checked.normalized_type_identity(reference).as_str() != subject.type_identity
        || !(crate::content_classes::plain_owned_with_numeric_constraints(checked,
            reference) || validation::has_contents(&checked.typed, reference, validation::ContentsRequirement::OwnedOrSharedView))
        // A structural payload may stay unread in its case: the whole type
        // graph's plain owned contents leave it no cleanup beyond the
        // subject's own disposal. Only scalar payloads are bound, and
        // `case_emission` refuses an argument that reads any other kind.
        || checked
            .data_members(declaration)
            .iter()
            .any(|member| matches!(member, DataMember::Field(_)))
    {
        return unsupported("Unit case lacks exact plain non-linear sum custody");
    }
    Ok((symbol, declaration, provenance))
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases, .. } =
        &state.terminator
    else {
        return unsupported("Unit case terminator missing");
    };
    // A fused roster's case dispatch lives under the state owner's machine:
    // custody rows and entry parameters are scoped to it, not the composed
    // plan's dispatch machine.
    let owner = super::owner(checked, state.state)?;
    let (dispatch_subject, declaration, expected_provenance) =
        result_source(checked, owner, source, state)?;
    // As on an ordinary return, a preceding owned call transfers a whole
    // affine parameter's cleanup to its callee. Keep projected parameters and
    // every unconsumed owner on the existing case-edge custody checks.
    let consumed_parameters = state
        .operations
        .iter()
        .flat_map(|operation| operation.call_structural_arguments())
        .filter(|argument| {
            argument.access == checked_trees::CheckedStructuralAccess::Owned
                && argument.path.is_empty()
        })
        .filter_map(|argument| argument.source_parameter_index())
        .collect::<Vec<_>>();
    let consumed_affine_parameter = |index: usize| {
        state
            .structural_parameters
            .get(index)
            .is_some_and(|parameter| {
                parameter.access == checked_trees::CheckedStructuralAccess::Owned
                    && parameter.multiplicity == Multiplicity::Affine
                    && u32::try_from(index)
                        .ok()
                        .is_some_and(|index| consumed_parameters.contains(&index))
            })
    };
    // Every parameter beside the dispatched subject must need no cleanup on
    // an arm that leaves it unconsumed. A shared or mutable loan is restored
    // at return, and an owned unrestricted value, such as the by-value `[copy]`
    // `other: Duration` of `Duration::is_less_than`, carries no drop
    // obligation. Affine or linear owned parameters and write-only loans need
    // a routed disposition that this reconstruction does not supply.
    if state
        .structural_parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| {
            !(matches!(subject.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } if parameter_index as usize == index))
                && !consumed_affine_parameter(index)
                && (parameter.multiplicity != Multiplicity::Unrestricted
                    || !matches!(
                        parameter.access,
                        checked_trees::CheckedStructuralAccess::Owned
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                            | checked_trees::CheckedStructuralAccess::MutableBorrow
                    ))
        })
    {
        return unsupported("Unit case has unrelated owned parameter cleanup");
    }
    // An inspected loan disposes nothing.
    let needs_discard = subject.access == checked_trees::CheckedStructuralAccess::Owned
        && declaration.properties.multiplicity == Multiplicity::Affine;
    let mut has_result_discard = false;
    let mut consumed_drops = Vec::new();
    for event in checked
        .facts
        .flow
        .ownership
        .state_exit_cleanup_roots(owner, state.state)
        .ok_or(LoweringError::Unsupported(
            "Unit case exit cleanup is inconsistent",
        ))?
    {
        if !subject.path.is_empty()
            && dispatch_subject
                .place()
                .is_some_and(|symbol| event.root == facts::PlaceRoot::Symbol(symbol))
        {
            // A projected observation leaves this owner's disposition to the
            // ordinary selected-edge cleanup plan.
            continue;
        }
        if dispatch_subject
            .place()
            .is_none_or(|symbol| event.root != facts::PlaceRoot::Symbol(symbol))
        {
            if let Some(source_position) = checked
                .state_parameters(source)
                .iter()
                .position(|parameter| event.root == facts::PlaceRoot::Symbol(parameter.symbol))
            {
                let index = state
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.position as usize == source_position)
                    .ok_or(LoweringError::Unsupported(
                        "Unit case cleanup parameter is absent",
                    ))?;
                if !consumed_affine_parameter(index)
                    || event.access != language_semantics::PermissionAccess::Owned
                    || event.multiplicity != Multiplicity::Affine
                    || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
                    || event.provenance != language_semantics::PermissionProvenance::Unknown
                    || event.obligation_live
                    || !event.segments.is_empty()
                    || consumed_drops.contains(&index)
                {
                    return unsupported("Unit case discards an unrelated entry parameter");
                }
                consumed_drops.push(index);
            }
            continue;
        }
        if !needs_discard
            || event.access != language_semantics::PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || event.provenance != expected_provenance
            || has_result_discard
            || event.obligation_live
            || !event.segments.is_empty()
        {
            return unsupported("Unit case affine exit custody escaped its result local");
        }
        has_result_discard = true;
    }
    // A computed subject is an anonymous temporary with no exit event: the
    // dispatch consumes it on every case edge.
    if has_result_discard != (needs_discard && dispatch_subject.place().is_some()) {
        return unsupported("Unit case result disposal disagrees with its source multiplicity");
    }
    // An affine parameter subject's state-exit drop is this dispatch's
    // consumption. Edge cleanup evidence names the parameter among those
    // dying on each case edge, and the edge rejoin takes that row as the
    // dispatch rather than as a no-code discard.
    let _dispatched_parameter = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            state
                .structural_parameters
                .get(parameter_index as usize)
                .filter(|parameter| {
                    parameter.access == checked_trees::CheckedStructuralAccess::Owned
                        && parameter.multiplicity == Multiplicity::Affine
                })
                .map(|parameter| parameter.position)
        }
        _ => None,
    };
    let declared = checked.data_members(declaration);
    let mutable_reference_result =
        crate::reference_custody::mutable_borrow_parts(checked, source.return_type).is_some();
    for case in cases {
        let successor_plan = match &case.outcome {
            checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => Some(successor),
            checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                if !case.payloads.is_empty() {
                    return unsupported("Unit case return arm holds a payload binding");
                }
                match arm {
                    checked_trees::CheckedConditionalReturnArm::Crash { .. } => {
                        return unsupported("case completion has no authored crash selection");
                    }
                    checked_trees::CheckedConditionalReturnArm::Unit { .. } => {
                        // Case return production currently establishes a value.
                        // Unit case completion must also discharge the selected
                        // sum subject; conditional return custody cannot assert it.
                        return unsupported("Unit case completion lacks subject disposal custody");
                    }
                    checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                        if !matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                                | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                        ) {
                            return unsupported("Unit case return arm lost its producer");
                        }
                    }
                    checked_trees::CheckedConditionalReturnArm::Scalar {
                        primitive_type, ..
                    } => {
                        let expected = checked_trees::CheckedControlResultPlan::Scalar {
                            primitive_type: *primitive_type,
                        };
                        if expected != plan.result {
                            return unsupported("Unit case return arm changed the result type");
                        }
                    }
                    // The selected case edge disposes its subject before
                    // entering the ordinary call-and-completion block.
                    checked_trees::CheckedConditionalReturnArm::Call { .. } => {}
                }
                None
            }
        };
        if subject.access == checked_trees::CheckedStructuralAccess::Owned && successor_plan.is_some_and(|successor| {
            successor.transfers.iter().any(|transfer| {
                match (&subject.source, &transfer.source) {
                    (
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index,
                        },
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index },
                    ) => parameter_index == index,
                    (
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: subject,
                        },
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                            binding_ordinal,
                        },
                    ) => subject == binding_ordinal,
                    _ => false,
                }
            })
        }) {
            return unsupported("Unit case transfers its consumed subject");
        }
        let variant = declared
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.path_identity() == case.case_identity => {
                    Some(variant)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported("Unit case names an absent case"))?;
        let Some(successor) = successor_plan else {
            // A returned mutable reference keeps its structural producer; a
            // call arm returns its callee's reference.
            if mutable_reference_result
                && let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) = &case.outcome
            {
                match arm {
                    checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                        super::guarded::validate_reference_return(checked, plan, state, operation)?
                    }
                    checked_trees::CheckedConditionalReturnArm::Call { .. } => {}
                    _ => {
                        return unsupported(
                            "Unit case reference return lost its structural producer",
                        );
                    }
                }
            }
            continue;
        };
        for payload in &case.payloads {
            if !checked
                .data_payload_fields(variant)
                .iter()
                .any(|field| field.path_identity() == payload.field_identity)
            {
                return unsupported("Unit case payload field missing");
            }
        }
        let target = plan
            .state(successor.target_state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph successor target is missing",
            ))?;
        edges::admit_transfers(state, successor, target)?;
    }
    Ok(())
}
