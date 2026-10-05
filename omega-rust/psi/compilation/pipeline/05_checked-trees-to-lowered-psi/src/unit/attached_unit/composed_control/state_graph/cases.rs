//! Reconstruct closed-case dispatch from the authored result and payload paths.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{CheckedUnitEffectOperationPlan, Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan, edges};
use checked_trees::data::DataMember;
use checked_trees::statement::{StatementNode, TransitionGuardNode};
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
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } = &state.terminator
    else {
        return unsupported("Unit case has no structural subject");
    };
    if subject.access != checked_trees::CheckedStructuralAccess::Owned || !subject.path.is_empty() {
        return unsupported("Unit case subject is not whole owned custody");
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
            (
                CaseSubject::Place(parameter.symbol),
                parameter.type_reference,
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
            let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit case result local missing");
            };
            if local.is_mutable
                || result.multiplicity != checked.type_multiplicity(local.type_reference)
            {
                return unsupported("Unit case local is mutable or has mismatched multiplicity");
            }
            (
                CaseSubject::Place(local.symbol),
                local.type_reference,
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
    let TypeReferenceNode::Named {
        symbol: type_symbol,
        ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("Unit case subject is not a closed declaration");
    };
    let declaration = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *type_symbol)
        .ok_or(LoweringError::Unsupported(
            "Unit case subject declaration absent",
        ))?;
    if !matches!(
        checked.type_multiplicity(reference),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) || checked.normalized_type_identity(reference).as_str() != subject.type_identity
        || !(crate::content_classes::plain_owned_with_numeric_constraints(checked,
            reference) || validation::has_owned_or_shared_view_contents(&checked.typed, reference))
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
    tail: &[StatementNode],
    ordinal: usize,
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
    let needs_discard = declaration.properties.multiplicity == Multiplicity::Affine;
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
    let dispatched_parameter = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            state
                .structural_parameters
                .get(parameter_index as usize)
                .filter(|parameter| parameter.multiplicity == Multiplicity::Affine)
                .map(|parameter| parameter.position)
        }
        _ => None,
    };
    let declared = checked.data_members(declaration);
    let mut previous_case_order = None;
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
        if successor_plan.is_some_and(|successor| {
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
        let source_offset = (case.statement_ordinal() as usize)
            .checked_sub(ordinal)
            .ok_or(LoweringError::Unsupported(
                "Unit case source ordinal precedes dispatch",
            ))?;
        let statement = tail.get(source_offset).ok_or(LoweringError::Unsupported(
            "Unit case source ordinal exceeds dispatch",
        ))?;
        let StatementNode::Transition(transition) = statement else {
            return unsupported("Unit case tail contains an effect");
        };
        let variant = declared
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.path_identity() == case.case_identity => {
                    Some(variant)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported("Unit case names an absent case"))?;
        match transition.guard {
            TransitionGuardNode::When(_) => {}
            TransitionGuardNode::Always if source_offset + 1 == tail.len() => {}
            _ => return unsupported("Unit case guard is not an exact case test"),
        }
        let declaration_position = declared.iter().position(|member| {
            matches!(member, DataMember::Variant(candidate) if candidate.symbol == variant.symbol)
        }).ok_or(LoweringError::Unsupported("Unit case declaration position missing"))?;
        let order = (source_offset, declaration_position);
        if previous_case_order.is_some_and(|previous| previous >= order) {
            return unsupported("Unit case roster no longer follows authored dispatch order");
        }
        previous_case_order = Some(order);
        if let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
            checked_trees::CheckedConditionalReturnArm::Call {
                operands,
                operation,
            },
        ) = &case.outcome
        {
            super::admission::validate_arm_call(
                checked,
                plan,
                source,
                state,
                transition,
                case.statement_ordinal() as usize,
                operands,
                operation,
            )?;
            super::result_custody::local_discards(checked, owner, source, state, None)?;
            continue;
        }
        let (
            Some(successor),
            checked_trees::statement::TransitionTargetNode::Named { arguments, .. },
        ) = (
            successor_plan,
            checked.statement_table.transition_target(transition.target),
        )
        else {
            if let (
                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm),
                checked_trees::statement::TransitionTargetNode::Value(_),
            ) = (
                &case.outcome,
                checked.statement_table.transition_target(transition.target),
            ) {
                if mutable_reference_result {
                    let checked_trees::CheckedConditionalReturnArm::Structural(operation) = arm
                    else {
                        return unsupported(
                            "Unit case reference return lost its structural producer",
                        );
                    };
                    super::guarded::validate_reference_return(plan, state, operation)?;
                }
                super::result_custody::local_discards(checked, owner, source, state, None)?;
                continue;
            }
            return unsupported("Unit case outcome and authored arm disagree");
        };
        let target = plan
            .states
            .iter()
            .find(|target| target.state == successor.target_state)
            .ok_or(LoweringError::Unsupported("Unit case target missing"))?;
        let arguments = checked.statement_table.expression_handles(*arguments);
        for payload in &case.payloads {
            let parameter = target
                .scalar_parameters
                .get(payload.target_scalar_parameter_index as usize)
                .ok_or(LoweringError::Unsupported("Unit case payload slot invalid"))?;
            if !checked
                .data_payload_fields(variant)
                .iter()
                .any(|field| field.path_identity() == payload.field_identity)
            {
                return unsupported("Unit case payload field missing");
            }
            let argument_position = parameter.source_position as usize;
            arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload source missing",
                ))?;
        }
        edges::validate_bindings(
            checked,
            plan,
            source,
            state,
            transition,
            successor,
            successor.statement_ordinal as usize,
            &case.payloads,
            dispatched_parameter,
        )?;
        super::result_custody::local_discards(checked, owner, source, state, Some(successor))?;
    }
    if tail.iter().enumerate().any(|(offset, _)| {
        !cases
            .iter()
            .any(|case| case.statement_ordinal() as usize == ordinal + offset)
    }) {
        return unsupported("Unit case omitted an authored arm");
    }
    Ok(())
}
