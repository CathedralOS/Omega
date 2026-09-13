//! Reconstruct ordinary whole-result claim custody from the semantic call and
//! ownership facts. A result binding records a value, not authority. The call's
//! returned frontier must continue the exact claims transferred into its callee;
//! a matching type or an empty producer record cannot establish that relation.
//!
//! This is shared producer-side replay. Terminal independently verifies the
//! resulting claim bijection; these source facts are not portable PCC openings.

use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralPathQualification, CheckedStructuralReturnedClaimTransferPlan,
    CheckedUnitClaimTransferPlan, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeSource,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Implicit membership clauses only restate the exact declared parameter row.
/// Authored preconditions still require their ordinary proof-carrying route.
pub fn structural_state_contracts_are_parameter_qualifications(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    let mut expected = Vec::new();
    for parameter in program.state_parameters(state) {
        let mut reference = parameter.type_reference;
        if let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *referee;
        }
        // Scalar ranges retain their existing numeric proof route. They are
        // not structural-domain memberships and must not be reclassified here.
        if program.primitive_type_reference(reference).is_some() {
            continue;
        }
        let Ok(domains) = structural_result_qualifications(program, reference) else {
            return false;
        };
        expected.extend(
            domains
                .into_iter()
                .map(|domain| (parameter.symbol, domain.0)),
        );
    }
    let mut actual = Vec::new();
    for contract in program.state_contracts(state) {
        if contract.token_count != 0
            || contract.kind != typed_trees::signature::SignatureContractKind::Requires
        {
            return false;
        }
        let [typed_trees::domain::ProofFact::Membership(membership)] =
            program.proof_facts.span_or_empty(contract.facts)
        else {
            return false;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(membership.value)
        else {
            return false;
        };
        if path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            return false;
        };
        actual.push((path.symbol, domain.semantic_id.0));
    }
    expected.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    actual.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    actual == expected
}

/// Exact whole-result qualification row, excluding reference presentation.
pub fn structural_result_qualifications(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<Vec<SemanticDomainId>, &'static str> {
    let mut qualifications = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        let retained = program.type_reference_table.constraints(*constraints);
        if retained.len() != constraints.len() {
            return Err("structural result qualification span is stale");
        }
        for constraint in retained {
            let TypeConstraintNode::Domain(domain) = constraint else {
                return Err("structural result has an unsupported qualification");
            };
            if !domain.semantic_id.is_valid() {
                return Err("structural result qualification has no exact identity");
            }
            qualifications.push(domain.semantic_id);
        }
        reference = *base_type;
    }
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. }
    ) {
        return Err("structural result custody cannot manufacture an owned reference");
    }
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    Ok(qualifications)
}

/// Reconstruct exact nested domain rows from a source structural type. Root
/// constraints remain owned by `structural_result_qualifications`; this route
/// only emits nonempty fixed-index paths and rejects malformed spans.
pub fn structural_result_projected_qualifications(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<CheckedStructuralPathQualification>, &'static str> {
    fn collect(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        is_root: bool,
        path: &mut Vec<CheckedUnitStructuralPathSegment>,
        output: &mut Vec<CheckedStructuralPathQualification>,
    ) -> Result<(), &'static str> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, is_root, path, output)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let retained = program.type_reference_table.constraints(*constraints);
                if retained.len() != constraints.len() {
                    return Err("projected qualification span is stale");
                }
                for constraint in retained {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    if !domain.semantic_id.is_valid() {
                        return Err("projected qualification has no exact identity");
                    }
                    if !is_root || !path.is_empty() {
                        output.push(CheckedStructuralPathQualification {
                            path: path.clone(),
                            domain: domain.semantic_id,
                        });
                    }
                }
                collect(program, *base_type, is_root, path, output)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return Err("projected qualification array length is not literal");
                };
                let mut element_rows = Vec::new();
                collect(
                    program,
                    *element_type,
                    false,
                    &mut Vec::new(),
                    &mut element_rows,
                )?;
                if element_rows.is_empty() {
                    return Ok(());
                }
                output
                    .try_reserve(
                        element_rows
                            .len()
                            .checked_mul(*length)
                            .ok_or("projected qualification count overflow")?,
                    )
                    .map_err(|_| "projected qualification storage exhausted")?;
                for index in 0..*length {
                    let index = u64::try_from(index)
                        .map_err(|_| "projected qualification index overflow")?;
                    for row in &element_rows {
                        let mut qualified = row.clone();
                        qualified
                            .path
                            .insert(0, CheckedUnitStructuralPathSegment::FixedIndex(index));
                        output.push(qualified);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    let mut output = Vec::new();
    collect(program, type_reference, true, &mut Vec::new(), &mut output)?;
    output.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.domain.0.cmp(&right.domain.0))
    });
    output.dedup();
    Ok(output)
}

/// Replays a complete identity-preserving return of one whole owned parameter.
/// Other parameters may be consumed separately, but every output must continue
/// this parameter's exact entry path and every entry claim below it must return.
/// The source ownership analysis supplies the outcome theorem; this join checks
/// its exact custody, rather than inferring a theorem from matching carriers.
pub fn reconstruct_structural_parameter_return_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
) -> Result<
    Vec<(
        PermissionClaimIdentity,
        Vec<CheckedUnitStructuralPathSegment>,
    )>,
    &'static str,
> {
    let owner = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)
        .ok_or("structural returned claim has no exact machine")?;
    let destination = program
        .machine_states(owner)
        .iter()
        .find(|candidate| candidate.symbol == state)
        .ok_or("structural returned claim has no exact state")?;
    let mut parameters = program
        .state_parameters(destination)
        .iter()
        .filter(|candidate| candidate.symbol == parameter);
    let source = parameters
        .next()
        .ok_or("structural claim outcome names an absent parameter")?;
    if parameters.next().is_some()
        || owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || source.is_self
        || source.is_mutable
        || source.is_const
        || program.type_multiplicity(destination.return_type) != Multiplicity::Linear
        || program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(destination.return_type)
    {
        return Err("structural returned claim differs from its exact whole input contract");
    }
    structural_result_qualifications(program, source.type_reference)?;
    structural_result_qualifications(program, destination.return_type)?;
    let outcomes = returned_claim_outcomes(facts, machine, state)?;
    if outcomes.is_empty() {
        return Err("structural callee has no returned claims");
    }
    let mut paths = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        let FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } = outcome.source
        else {
            return Err("structural returned claim does not continue an input");
        };
        if parameter_symbol != parameter {
            return Err("structural returned claims do not continue the same whole input");
        }
        let input_segments = retained_claim_segments(facts, segments)?;
        let output_segments = retained_claim_segments(facts, outcome.output_segments)?;
        let input_path = structural_claim_path(program, source.type_reference, input_segments)?;
        let output_path = structural_claim_path(program, destination.return_type, output_segments)?;
        if input_path != output_path {
            return Err("structural returned claim changes its typed input path");
        }
        paths.push(output_path);
    }
    paths.sort();
    for pair in paths.windows(2) {
        if pair[1].starts_with(&pair[0]) {
            return Err("structural returned claim paths duplicate or overlap");
        }
    }
    let mut entries = Vec::with_capacity(paths.len());
    for (_, event) in facts.flow.ownership.permissions.iter() {
        if event.machine_symbol != machine
            || event.state_symbol != state
            || event.source != PermissionEventSource::StateEntry
            || event.kind != PermissionEventKind::Establish
            || event.root != facts::PlaceRoot::Symbol(parameter)
            || event.multiplicity != Multiplicity::Linear
            || !event.obligation_live
        {
            continue;
        }
        if event.access != PermissionAccess::Owned
            || event.claim_identity == PermissionClaimIdentity::Unknown
            || entries
                .iter()
                .any(|(identity, _)| *identity == event.claim_identity)
        {
            return Err("structural returned claim has ambiguous entry custody");
        }
        let segments = retained_claim_segments(facts, event.segments)?;
        entries.push((
            event.claim_identity,
            structural_claim_path(program, source.type_reference, segments)?,
        ));
    }
    entries.sort_by(|left, right| left.1.cmp(&right.1));
    if entries.len() != paths.len()
        || entries
            .iter()
            .zip(&paths)
            .any(|((_, entry_path), output_path)| entry_path != output_path)
    {
        return Err("structural returned claims differ from the complete input frontier");
    }
    Ok(entries)
}

fn returned_claim_outcomes(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Result<&[FlowClaimOutcomeEntryFact], &'static str> {
    let mut outcomes = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .filter(|map| map.machine_symbol == machine && map.state_symbol == state);
    let outcome = outcomes
        .next()
        .ok_or("structural callee has no exact claim outcome")?;
    if outcomes.next().is_some() {
        return Err("structural callee has ambiguous claim outcomes");
    }
    let entries = facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome.entries);
    if entries.len() != outcome.entries.len() {
        return Err("structural claim outcome span is stale");
    }
    Ok(entries)
}

fn retained_claim_segments(
    facts: &CheckFacts,
    span: arena::HandleSpan<facts::PlaceSegment>,
) -> Result<&[facts::PlaceSegment], &'static str> {
    let segments = facts.flow.ownership.segments.span_or_empty(span);
    if segments.len() != span.len() {
        return Err("structural claim path span is stale");
    }
    Ok(segments)
}

/// Resolve each semantic segment against its actual carrier before erasing
/// source handles. Numeric field identities and literal indexes are retained;
/// spelling alone never selects a field from another declaration.
pub fn structural_claim_path(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<Vec<CheckedUnitStructuralPathSegment>, &'static str> {
    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        structural_result_qualifications(program, reference)?;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        match (
            segment,
            program.type_reference_table.type_reference(reference),
        ) {
            (
                facts::PlaceSegment::Field { symbol },
                TypeReferenceNode::Named { symbol: owner, .. },
            ) => {
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *owner)
                    .ok_or("structural claim field has no exact data owner")?;
                let field = program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| {
                        let typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.symbol == *symbol).then_some(field)
                    })
                    .ok_or("structural claim field differs from its typed owner")?;
                if field.relevance != language_core::BindingRelevance::Relevant {
                    return Err("structural claim traverses an erased field");
                }
                path.push(CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                reference = field.type_reference;
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                },
            ) if *index < *length => {
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).map_err(|_| "structural claim index exceeds u64")?,
                ));
                reference = *element_type;
            }
            _ => return Err("structural claim has an unsupported or mistyped path"),
        }
    }
    structural_result_qualifications(program, reference)?;
    if program.primitive_type_reference(reference).is_some()
        || program.type_multiplicity(reference) != Multiplicity::Linear
    {
        return Err("structural claim path does not select linear structural custody");
    }
    Ok(path)
}

/// Ignores the recorded custody and reconstructs it from the exact call facts.
/// Existing operand/source validation still checks every value binding and
/// projection; this replay owns the claim correspondence, not call sequencing.
pub fn reconstruct_structural_call_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<CheckedStructuralCallCustodyPlan, &'static str> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        target_machine,
        target_state,
        result,
        structural_arguments,
        ..
    } = operation
    else {
        return Err("structural custody requires an ordinary structural call");
    };
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller_machine)
        .ok_or("structural call has no exact caller")?;
    let source_state = program
        .machine_states(caller)
        .iter()
        .find(|state| state.symbol == caller_state)
        .ok_or("structural call has no exact caller state")?;
    let target = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == *target_machine)
        .ok_or("structural call has no exact callee")?;
    let destination = program
        .machine_states(target)
        .iter()
        .find(|state| state.symbol == *target_state)
        .ok_or("structural call has no exact callee state")?;
    if target.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || crate::reference_result_custody::result_multiplicity(program, destination.return_type)
            != result.multiplicity
    {
        return Err("structural call result differs from the callee contract");
    }
    let result_qualifications =
        if crate::reference_result_custody::parts(program, destination.return_type).is_some() {
            // The reference carrier transports a loan, not owned referent claims.
            // Exact ingress substitution and its weakening are reconstructed below.
            Vec::new()
        } else {
            structural_result_qualifications(program, destination.return_type)?
        };
    let mut states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| {
            state.machine_symbol == caller_machine && state.state_symbol == caller_state
        });
    let state = states
        .next()
        .ok_or("structural call has no semantic state")?;
    if states.next().is_some() {
        return Err("structural call has ambiguous semantic states");
    }
    let retained_calls = facts.flow.control.calls.span_or_empty(state.calls);
    if retained_calls.len() != state.calls.len() {
        return Err("structural call semantic occurrence span is stale");
    }
    let mut calls = retained_calls.iter().filter(|call| {
        call.statement_index == coordinate.statement_index as usize
            && call.call_ordinal == coordinate.call_ordinal as usize
    });
    let call = calls
        .next()
        .ok_or("structural call has no exact semantic occurrence")?;
    if calls.next().is_some() || call.target_symbol != *target_state {
        return Err("structural call target or occurrence differs from source");
    }
    let transfers = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == caller_machine
                && event.state_symbol == caller_state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    if result.multiplicity != Multiplicity::Linear {
        if !result_qualifications.is_empty() || !transfers.is_empty() {
            return Err("claim-bearing structural call has no returned claim frontier");
        }
        if crate::reference_result_custody::is_reference_record(program, destination.return_type) {
            let Some(typed_trees::statement::StatementNode::LocalData(local)) = program
                .statement_table
                .statements(source_state.statement_nodes)
                .get(call.statement_index)
            else {
                return Err("reference record call has no retained local destination");
            };
            if local.initial_value != call.authored_expression
                || program.normalized_type_identity(local.type_reference)
                    != program.normalized_type_identity(destination.return_type)
                || crate::reference_result_custody::local_record_loans(
                    program,
                    facts,
                    caller_machine,
                    source_state,
                    u32::try_from(call.statement_index)
                        .map_err(|_| "reference record call index exceeds u32")?,
                )
                .is_none()
            {
                return Err("reference record result has no exact captured leaf loans");
            }
        }
        let reference_loan =
            if crate::reference_result_custody::parts(program, destination.return_type).is_some() {
                crate::reference_result_custody::result_loan(
                    program,
                    facts,
                    caller_machine,
                    source_state,
                    call,
                    result,
                )
                .ok_or("reference result has no exact returned ingress and loan lifetime")?
            } else {
                arena::Handle::invalid()
            };
        return Ok(CheckedStructuralCallCustodyPlan {
            reference_loan,
            ..Default::default()
        });
    }

    // A whole owner may contain several disjoint claims. The outcome map and
    // entry frontier must agree on their complete identity-preserving path set;
    // source transfers then instantiate that set at this exact call occurrence.
    let outcomes = returned_claim_outcomes(facts, *target_machine, *target_state)?;
    let returned = outcomes
        .first()
        .ok_or("structural callee has no returned claims")?;
    let FlowClaimOutcomeSource::Input {
        parameter_symbol, ..
    } = returned.source
    else {
        return Err("structural returned claim does not continue an input");
    };
    let returned_claims = reconstruct_structural_parameter_return_claims(
        program,
        facts,
        *target_machine,
        *target_state,
        parameter_symbol,
    )?;
    let parameters = program.state_parameters(destination);
    let parameter_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == parameter_symbol)
        .ok_or("structural claim outcome names an absent parameter")?;
    let parameter = &parameters[parameter_position];
    let argument_position = parameters[..parameter_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .count();
    let argument = structural_arguments
        .get(argument_position)
        .ok_or("structural returned claim has no input argument")?;
    if argument.access != CheckedStructuralAccess::Owned || !argument.path.is_empty() {
        return Err("structural returned claim requires an owned whole argument");
    }
    let authored_arguments = if call.authored_expression.is_valid() {
        let ExpressionNode::Call(authored) = program
            .expression_table
            .expression(call.authored_expression)
        else {
            return Err("structural call occurrence has no authored expression");
        };
        if authored.target_symbol != *target_state {
            return Err("structural call expression target changed");
        }
        program
            .expression_table
            .expression_handles(authored.arguments)
    } else {
        let Some(StatementNode::Call(authored)) = program
            .statement_table
            .statements(source_state.statement_nodes)
            .get(coordinate.statement_index as usize)
        else {
            return Err("structural call occurrence has no authored statement");
        };
        if authored.target_symbol != *target_state {
            return Err("structural call statement target changed");
        }
        program
            .statement_table
            .expression_handles(authored.arguments)
    };
    if authored_arguments.len() != parameters.len() {
        return Err("structural call has no exact explicit argument roster");
    }
    let ExpressionNode::Name(path) = program
        .expression_table
        .expression(authored_arguments[parameter_position])
    else {
        return Err("structural returned claim requires an exact whole source root");
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return Err("structural returned claim source is projected");
    }
    if transfers.len() != returned_claims.len() {
        return Err("structural returned claims differ from the complete input transfer set");
    }
    let mut transferred_claims = Vec::with_capacity(transfers.len());
    for transfer in transfers {
        if transfer.root != facts::PlaceRoot::Symbol(path.symbol)
            || transfer.claim_identity == PermissionClaimIdentity::Unknown
            || transferred_claims
                .iter()
                .any(|(identity, _)| *identity == transfer.claim_identity)
        {
            return Err("structural returned claim source differs from its transferred lineage");
        }
        let segments = retained_claim_segments(facts, transfer.segments)?;
        let transfer_path = structural_claim_path(program, parameter.type_reference, segments)?;
        transferred_claims.push((transfer.claim_identity, transfer_path));
    }
    transferred_claims.sort_by(|left, right| left.1.cmp(&right.1));
    let argument_index =
        u32::try_from(argument_position).map_err(|_| "structural argument position exceeds u32")?;
    let mut custody = CheckedStructuralCallCustodyPlan {
        reference_loan: arena::Handle::invalid(),
        result_qualifications,
        claim_transfers: Vec::with_capacity(returned_claims.len()),
        returned_claim_transfers: Vec::with_capacity(returned_claims.len()),
    };
    for ((callee_claim, returned_path), (caller_claim, transfer_path)) in
        returned_claims.into_iter().zip(transferred_claims)
    {
        if returned_path != transfer_path {
            return Err("structural returned claim path differs from its source transfer");
        }
        custody.claim_transfers.push(CheckedUnitClaimTransferPlan {
            claim_identity: caller_claim,
            argument_index,
        });
        custody
            .returned_claim_transfers
            .push(CheckedStructuralReturnedClaimTransferPlan {
                callee_claim,
                caller_claim,
                path: returned_path,
            });
    }
    Ok(custody)
}
