//! Reconstruct closed-case dispatch from the authored result and payload paths.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{CheckedUnitEffectOperationPlan, Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan, edges};
pub(super) use crate::expression_preparation::source_custody::dispatch_subjects::case_test;
use crate::expression_preparation::source_custody::dispatch_subjects::repeats_subject;
use checked_trees::data::DataMember;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use checked_trees::types::TypeReferenceNode;

fn root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    symbol: symbols::SymbolHandle,
) -> bool {
    matches!(checked.expression_table.expression(expression), ExpressionNode::Name(path)
        if path.head_symbol == symbol
            && path.symbol == symbol
            && checked.expression_table.name_path_members(path.members).len() == 1)
}

/// The sum a Unit case dispatch reads, reconstructed from source.
#[derive(Clone, Copy)]
enum CaseSubject {
    /// A parameter or an earlier immutable local, named by every arm.
    Place(symbols::SymbolHandle),
    /// The dispatch's one authored call, evaluated once at the first arm's
    /// statement. Every arm's guard subject and payload receiver is a parser
    /// copy of it: the same call and arguments carrying the same authored
    /// selection occurrence.
    Computed(ExpressionHandle),
}

impl CaseSubject {
    fn names(self, checked: &CheckedTrees, expression: ExpressionHandle) -> bool {
        match self {
            Self::Place(symbol) => root(checked, expression, symbol),
            Self::Computed(subject) => repeats_subject(checked, subject, expression),
        }
    }

    fn place(self) -> Option<symbols::SymbolHandle> {
        match self {
            Self::Place(symbol) => Some(symbol),
            Self::Computed(_) => None,
        }
    }
}

/// The computed subject a case dispatch's result binding holds: the call
/// planned at the first arm's own statement, whose authored source is that
/// arm's guard subject. `None` when the binding is not that call.
pub(super) fn computed_subject<'plan>(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &'plan CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Result<Option<(ExpressionHandle, &'plan checked_trees::CheckedUnitCallPlan)>, LoweringError> {
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
    let Some(result) = call.structural_result() else {
        return Ok(None);
    };
    let statements = checked.statement_table.statements(source.statement_nodes);
    if !matches!(
        statements.get(result.statement_index as usize),
        Some(StatementNode::Transition(_))
    ) {
        return Ok(None);
    }
    let subject =
        crate::expression_preparation::source_custody::dispatch_subjects::evaluated_case_subject(
            checked,
            statements,
            result.statement_index as usize,
        )
        .ok_or(LoweringError::Unsupported(
            "Unit case subject call has no computed case test",
        ))?;
    let located = crate::emission::call_source_custody::authored::locate_source(
        checked,
        state.state,
        call.coordinate,
    )?;
    // The subject's placement does not change call authority: body replay
    // validates its exact boundary target, receiver, buffers and occurrence
    // through the ordinary call admission. As for a named structural result,
    // this composed dispatch does not settle boundary claim transfers.
    if (call.is_boundary() && !call.claim_transfers().is_empty())
        || call.coordinate.statement_index != result.statement_index
        || call.coordinate.call_ordinal != 0
        || located.source_site != Some(checked_trees::NominalMachineUseSite::Expression(subject))
        || statements[..result.statement_index as usize]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return unsupported("Unit case subject call is not the dispatch's first authored subject");
    }
    Ok(Some((subject, call)))
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
            if retained.access != checked_trees::CheckedStructuralAccess::Owned
                || !matches!(
                    retained.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
                || retained.multiplicity != checked.type_multiplicity(parameter.type_reference)
                || !retained.qualifications.is_empty()
                || retained.type_identity != subject.type_identity
            {
                return unsupported("Unit case parameter custody drifted");
            }
            (
                CaseSubject::Place(parameter.symbol),
                parameter.type_reference,
                language_semantics::PermissionProvenance::Unknown,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } if let Some((subject_expression, call)) =
            computed_subject(checked, source, state, binding_ordinal)? =>
        {
            let Some(result) = call.structural_result() else {
                return unsupported("Unit case subject call lost its result");
            };
            let signature = crate::emission::call_source_custody::authored::target_signature(
                checked,
                machine,
                call.target.state,
            )?;
            if result.type_identity != subject.type_identity
                || result.multiplicity != checked.type_multiplicity(signature.return_type)
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
            {
                return unsupported("Unit case subject call result drifted");
            }
            (
                CaseSubject::Computed(subject_expression),
                signature.return_type,
                language_semantics::PermissionProvenance::Unknown,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            let mut matching = state
                .operations
                .iter()
                .filter_map(super::result_custody::result)
                .filter(|result| result.binding_ordinal == binding_ordinal);
            let result = matching.next().ok_or(LoweringError::Unsupported(
                "Unit case result producer missing",
            ))?;
            if matching.next().is_some()
                || result.type_identity != subject.type_identity
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                )
            {
                return unsupported("Unit case result producer duplicated or drifted");
            }
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
        || !(validation::has_plain_owned_contents_with_numeric_constraints(
            &checked.typed,
            reference,
        ) || validation::has_owned_or_shared_view_contents(&checked.typed, reference))
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

pub(super) fn validate_markers(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    end: usize,
) -> Result<usize, LoweringError> {
    if !matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
    ) {
        return Ok(0);
    }
    let (subject, declaration, _) = result_source(checked, machine, source, state)?;
    let statements = checked.statement_table.statements(source.statement_nodes);
    // Marker lets are parsed as the contiguous run immediately ahead of the
    // dispatch's first arm statement — the same convention the checker uses
    // (04_typed-trees' `closed_sum` finds them by trailing run). Operation
    // counts cannot locate the window: one authored statement may plan
    // several effect operations, so `bindings + operations` overruns `end`.
    let start = statements[..end]
        .iter()
        .rposition(|statement| {
            !matches!(statement, StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__arm_destructure#V="))
        })
        .map_or(0, |index| index + 1);
    let markers = statements
        .get(start..end)
        .ok_or(LoweringError::Unsupported("Unit case body roster drifted"))?;
    // A computed subject has no place for a marker to re-read; its payload
    // reads are the subject copies `validate` rejoins arm by arm.
    let Some(result_symbol) = subject.place() else {
        if !markers.is_empty() {
            return unsupported("Unit case computed subject carries destructure markers");
        }
        return Ok(0);
    };
    let mut spelled_fields = Vec::new();
    for statement in markers {
        let StatementNode::LocalData(local) = statement else {
            return unsupported("Unit case skipped a source effect");
        };
        let Some(encoded) = local.name.as_str().strip_prefix("__arm_destructure#V=") else {
            return unsupported("Unit case skipped a non-marker local");
        };
        let Some((variant_name, fields)) = encoded.split_once('#') else {
            return unsupported("Unit case marker has no declared case");
        };
        let variant = checked
            .data_members(declaration)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == variant_name => {
                    Some(variant)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "Unit case marker names an unknown case",
            ))?;
        let Some((fields, suffix)) = fields.split_once("~subject=") else {
            return unsupported("Unit case marker lost its subject");
        };
        let fields = fields
            .trim_end_matches('#')
            .split('#')
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        let (subject, rest) = suffix
            .split_once('#')
            .map_or((suffix, false), |(subject, tail)| {
                (subject, tail == "~rest")
            });
        let declared = checked.data_payload_fields(variant);
        if local.is_mutable
            || !root(checked, local.initial_value, result_symbol)
            || subject.parse::<u32>().is_err()
            || fields.iter().enumerate().any(|(position, field)| {
                fields[..position].contains(field)
                    || !declared
                        .iter()
                        .any(|declaration| declaration.name.as_str() == *field)
            })
            || (!rest && fields.len() != declared.len())
        {
            return unsupported("Unit case destructure marker disagrees with source declaration");
        }
        for field_name in fields {
            let field = declared
                .iter()
                .find(|field| field.name.as_str() == field_name)
                .ok_or(LoweringError::Unsupported(
                    "Unit case destructure field missing",
                ))?;
            let field_identity = field.path_identity();
            let spelling = (variant.path_identity(), field_identity);
            if spelled_fields.contains(&spelling) {
                return unsupported("Unit case duplicated a destructure marker field");
            }
            spelled_fields.push(spelling);
        }
    }
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &state.terminator
    else {
        return unsupported("Unit case marker lost its terminator");
    };
    // A marker establishes available fields, not mandatory transfers. Payloads
    // of plain owned contents may remain unused; result_source checks that
    // restriction independently, and validate retains any affine disposal
    // obligation.
    // Every actual transfer still needs its exact authored destructure below.
    if cases.iter().any(|case| {
        case.payloads.iter().any(|payload| {
            !spelled_fields.iter().any(|(variant, field)| {
                variant == &case.case_identity && field == &payload.field_identity
            })
        })
    }) {
        return unsupported("Unit case destructure and payload rosters disagree");
    }
    Ok(markers.len())
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    tail: &[StatementNode],
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } = &state.terminator
    else {
        return unsupported("Unit case terminator missing");
    };
    // A fused roster's case dispatch lives under the state owner's machine:
    // custody rows and entry parameters are scoped to it, not the composed
    // plan's dispatch machine.
    let owner = super::owner(checked, state.state)?;
    let (dispatch_subject, declaration, expected_provenance) =
        result_source(checked, owner, source, state)?;
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
    super::result_custody::validate_disposition_roster(checked, owner, source, state)?;
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == owner
                && event.state_symbol == state.state
                && event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
        })
    {
        if dispatch_subject
            .place()
            .is_none_or(|symbol| event.root != facts::PlaceRoot::Symbol(symbol))
        {
            if checked
                .state_parameters(source)
                .iter()
                .any(|parameter| event.root == facts::PlaceRoot::Symbol(parameter.symbol))
            {
                return unsupported("Unit case discards an unrelated entry parameter");
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
    let mut subject_reads = Vec::new();
    let declared = checked.data_members(declaration);
    if cases.is_empty()
        || tail.is_empty()
        || cases.len() < tail.len()
        || cases.len() != declared.len()
    {
        return unsupported("Unit case result or complete case roster drifted");
    }
    let mut previous_case_order = None;
    let mutable_reference_result = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        source.return_type,
    )
    .is_some();
    if mutable_reference_result {
        super::guarded::validate_reference_roster(plan)?;
    }
    for (offset, case) in cases.iter().enumerate() {
        let successor_plan = match &case.outcome {
            checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => Some(successor),
            checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                if !case.payloads.is_empty() {
                    return unsupported("Unit case return arm holds a payload binding");
                }
                match arm {
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
        let (tested_subject, variant) = match transition.guard {
            TransitionGuardNode::When(guard) => {
                let (tested, variant_symbol) = case_test(checked, guard).ok_or(
                    LoweringError::Unsupported("Unit case guard lost its source identity"),
                )?;
                let variant = declared
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == variant_symbol => {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit case guard names another sum",
                    ))?;
                (Some(tested), variant)
            }
            TransitionGuardNode::Always if source_offset + 1 == tail.len() => {
                let variant = declared
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.path_identity() == case.case_identity =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit case fallback names absent case",
                    ))?;
                if tail[..source_offset]
                    .iter()
                    .any(|statement| match statement {
                        StatementNode::Transition(transition) => match transition.guard {
                            TransitionGuardNode::When(guard) => case_test(checked, guard)
                                .is_some_and(|(_, selected)| selected == variant.symbol),
                            _ => true,
                        },
                        _ => true,
                    })
                {
                    return unsupported("Unit case fallback overlaps an earlier case");
                }
                (None, variant)
            }
            _ => return unsupported("Unit case guard is not an exact case test"),
        };
        let declaration_position = declared.iter().position(|member| {
            matches!(member, DataMember::Variant(candidate) if candidate.symbol == variant.symbol)
        }).ok_or(LoweringError::Unsupported("Unit case declaration position missing"))?;
        let order = (source_offset, declaration_position);
        if previous_case_order.is_some_and(|previous| previous >= order) {
            return unsupported("Unit case roster no longer follows authored dispatch order");
        }
        previous_case_order = Some(order);
        if let Some(tested) = tested_subject {
            subject_reads.push(tested);
        }
        if tested_subject.is_some_and(|tested| !dispatch_subject.names(checked, tested))
            || variant.path_identity() != case.case_identity
            || cases[..offset]
                .iter()
                .any(|previous| previous.case_identity == case.case_identity)
        {
            return unsupported("Unit case source or declaration roster drifted");
        }
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
                checked_trees::statement::TransitionTargetNode::Value(expression),
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
                    super::guarded::validate_reference_return(
                        checked,
                        plan,
                        source,
                        state,
                        *expression,
                        case.statement_ordinal(),
                        operation,
                    )?;
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
        for (payload_ordinal, payload) in case.payloads.iter().enumerate() {
            let parameter = target
                .scalar_parameters
                .get(payload.target_scalar_parameter_index as usize)
                .ok_or(LoweringError::Unsupported("Unit case payload slot invalid"))?;
            let field = checked
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.path_identity() == payload.field_identity)
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload field missing",
                ))?;
            let argument_position = (parameter.source_position as usize)
                .checked_sub(
                    target
                        .structural_parameters
                        .iter()
                        .filter(|candidate| {
                            candidate.is_self && candidate.position < parameter.source_position
                        })
                        .count(),
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload argument position invalid",
                ))?;
            let argument = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload source missing",
                ))?;
            let valid_path = match checked.expression_table.expression(*argument) {
                // A computed subject's payload is read through the parser's
                // case-qualified member of a subject copy.
                ExpressionNode::Member(member)
                    if let CaseSubject::Computed(subject) = dispatch_subject =>
                {
                    subject_reads.push(member.receiver);
                    repeats_subject(checked, subject, member.receiver)
                        && member.member == field.name
                        && (member.member_symbol == field.symbol
                            || !member.member_symbol.is_valid())
                        && member
                            .case_variant
                            .as_ref()
                            .is_some_and(|name| name.as_str() == variant.name.as_str())
                }
                ExpressionNode::Member(member) => {
                    dispatch_subject.names(checked, member.receiver)
                        && member.member == field.name
                        && (member.member_symbol == field.symbol
                            || (!member.member_symbol.is_valid()
                                && member
                                    .case_variant
                                    .as_ref()
                                    .is_some_and(|name| name.as_str() == variant.name.as_str())))
                        && member
                            .case_variant
                            .as_ref()
                            .is_none_or(|name| name.as_str() == variant.name.as_str())
                }
                ExpressionNode::Name(path) => {
                    dispatch_subject
                        .place()
                        .is_some_and(|symbol| path.head_symbol == symbol)
                        && path.symbol == field.symbol
                        && checked
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            == 2
                        && checked
                            .expression_table
                            .name_path_member_symbols(path.member_symbols)
                            .last()
                            == Some(&field.symbol)
                }
                _ => false,
            };
            if !valid_path
                || parameter.primitive_type != payload.primitive_type
                || checked.primitive_type_reference(field.type_reference)
                    != Some(payload.primitive_type)
                || case.payloads[..payload_ordinal].iter().any(|previous| {
                    previous.target_scalar_parameter_index == payload.target_scalar_parameter_index
                })
            {
                return unsupported("Unit case payload path, type, or target slot drifted");
            }
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
    if let CaseSubject::Computed(subject) = dispatch_subject {
        // Every flow row that copies the subject at the dispatch is read from
        // its one evaluation: the subject itself, a guard's subject, or a
        // transferred payload's receiver. Any other copy would be a second
        // evaluation the dispatch cannot account for.
        let mut flow_states = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, flow)| {
                flow.machine_symbol == plan.machine && flow.state_symbol == state.state
            });
        let (Some((_, flow)), None) = (flow_states.next(), flow_states.next()) else {
            return unsupported("Unit case computed subject has no exact flow state");
        };
        if checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .filter(|call| call.statement_index >= ordinal)
            .filter(|call| repeats_subject(checked, subject, call.authored_expression))
            .any(|call| {
                call.authored_expression != subject
                    && !subject_reads.contains(&call.authored_expression)
            })
        {
            return unsupported("Unit case computed subject is evaluated outside its dispatch");
        }
    }
    Ok(())
}
