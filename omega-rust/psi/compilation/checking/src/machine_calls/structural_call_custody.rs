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

mod declaration_predicates;
#[cfg(test)]
mod tests;

pub use declaration_predicates::{
    DeclarationPredicateRow, declaration_facts_carried, machine_declaration_predicates,
    state_declaration_predicates,
};

/// One scalar row a state's `requires` contracts leave for a proof lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateScalarContractRow {
    /// An authored `requires` expression; the caller owns its closed lowering.
    Expression(typed_trees::expression::ExpressionHandle),
    /// A scalar parameter's declared range or exactly-interval domain:
    /// `minimum <= parameter <= maximum`. Both spellings require the same
    /// independently checked arrival predicate.
    Interval {
        parameter: SymbolHandle,
        minimum: numerics::bignum::BigInt,
        maximum: numerics::bignum::BigInt,
    },
    /// An internal state's IEEE parameter declared in the compiler-known
    /// `Finite` value domain: `-largest <= parameter <= largest` in its exact
    /// format (`state_finite_predicate`). The closed scalar qualification
    /// catalog holds no predicate-bearing domain, so every arrival proves the
    /// row instead. A machine's entry parameter keeps no such row.
    Finite { parameter: SymbolHandle },
}

/// Whether one `requires` fact only restates a structural parameter's declared
/// qualification: `text in Utf8` for `text: &[u8; 8] in Utf8`. The parameter
/// declaration carries that membership into every lowering, and the caller's
/// argument presentation proves it, so it is no runtime entry requirement.
/// A scalar parameter's membership is an interval requirement and stays out.
pub fn restates_structural_parameter_qualification(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    let typed_trees::domain::ProofFact::Membership(membership) = fact else {
        return false;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(membership.value) else {
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
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)
    else {
        return false;
    };
    let mut reference = parameter.type_reference;
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    if program.primitive_type_reference(reference).is_some() {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == membership.domain_symbol)
    else {
        return false;
    };
    structural_result_qualifications(program, reference)
        .is_ok_and(|domains| domains.contains(&domain.semantic_id))
}

/// Split a state's authored `requires` contracts into the membership rows that
/// restate parameter domain qualifications and the scalar rows a proof lane
/// may carry. Membership rows must still match the exact declared
/// qualifications. Declared integer ranges and exactly-interval memberships
/// become interval rows; every other contract shape remains out of scope.
pub fn structural_state_contract_scalar_predicates(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<StateScalarContractRow>> {
    let mut expected = Vec::new();
    let mut predicates = Vec::new();
    for parameter in program.state_parameters(state) {
        // These are mandatory arrival contracts, not inference hints. Keeping
        // them on the shared roster lets both plan production and replay see
        // the same rows, and Terminal proves initial and cyclic arrivals before
        // allowing the body's arithmetic to use the bounds. Never recover the
        // range from a particular body expression or a successful source check.
        retain_declared_integer_ranges(program, parameter, &mut predicates)?;
        if finite_arrival_parameter(program, state, parameter) {
            predicates.push(StateScalarContractRow::Finite {
                parameter: parameter.symbol,
            });
        }
        let mut reference = parameter.type_reference;
        if let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *referee;
        }
        if program.primitive_type_reference(reference).is_some() {
            if !parameter.is_self && !parameter.is_mutable {
                expected.extend(
                    crate::scalar_interval_domains(program, reference)
                        .into_iter()
                        .map(|(_, domain)| (parameter.symbol, domain.0)),
                );
            }
            continue;
        }
        let Ok(domains) = structural_result_qualifications(program, reference) else {
            return None;
        };
        expected.extend(
            domains
                .into_iter()
                .map(|domain| (parameter.symbol, domain.0)),
        );
    }
    let mut actual = Vec::new();
    for contract in program.state_contracts(state) {
        // Authored rows carry their clause token span; only the synthesized
        // qualification restatements are zero-token. Both are admissible —
        // authored memberships must restate the declared qualifications
        // exactly, and authored expressions become scalar predicates.
        if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
            return None;
        }
        let facts = program.proof_facts.span(contract.facts)?;
        if facts.is_empty() {
            return None;
        }
        // A requires clause is a conjunction, not necessarily one fact.
        // Keep every row in authored order so each reaches the state's
        // entry obligations; an unsupported later row rejects the whole
        // contract rather than silently retaining just its first conjunct.
        for fact in facts {
            match fact {
                typed_trees::domain::ProofFact::Membership(membership) => {
                    let ExpressionNode::Name(path) =
                        program.expression_table.expression(membership.value)
                    else {
                        return None;
                    };
                    if path.head_symbol != path.symbol
                        || program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            != 1
                    {
                        return None;
                    }
                    let domain = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)?;
                    actual.push((path.symbol, domain.semantic_id.0));
                    // A scalar parameter's interval membership is carried as its
                    // closed interval; the qualification comparison below still
                    // requires it to restate the declared type exactly.
                    if let Some(parameter) = program
                        .state_parameters(state)
                        .iter()
                        .find(|parameter| parameter.symbol == path.symbol)
                        && let Some(primitive) =
                            program.primitive_type_reference(parameter.type_reference)
                    {
                        let (minimum, maximum) = scalar_interval_bounds(
                            program,
                            parameter.type_reference,
                            membership,
                            primitive,
                        )?;
                        predicates.push(StateScalarContractRow::Interval {
                            parameter: parameter.symbol,
                            minimum,
                            maximum,
                        });
                    }
                }
                typed_trees::domain::ProofFact::Expression(expression) => {
                    predicates.push(StateScalarContractRow::Expression(*expression));
                }
                typed_trees::domain::ProofFact::Proposition(_) => return None,
            }
        }
    }
    expected.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    actual.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    (actual == expected).then_some(predicates)
}

/// Whether `parameter` of `state` carries its `Finite` value domain as an
/// arrival row: a relevant IEEE scalar of a state other than its machine's
/// entry, declared in `Finite`.
pub fn finite_arrival_parameter(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::types::PrimitiveType;
    if parameter.is_self
        || parameter.is_const
        || parameter.relevance.is_erased()
        || !matches!(
            program.primitive_type_reference(parameter.type_reference),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        )
        || !program.machines().iter().any(|machine| {
            program
                .machine_states(machine)
                .iter()
                .skip(1)
                .any(|internal| internal.symbol == state.symbol)
        })
    {
        return false;
    }
    let mut reference = parameter.type_reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        if program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::Domain(domain)
                        if domain.subject
                            == typed_trees::types::DomainConstraintSubject::Value(
                                language_semantics::value_domain::ValueDomain::Finite,
                            )
                )
            })
        {
            return true;
        }
        reference = *base_type;
    }
    false
}

/// The arrival predicate of a `Finite` row: `-largest <= parameter` and
/// `parameter <= largest` as IEEE comparisons in the parameter's exact
/// format, over its dense scalar position. NaN fails both ordered
/// comparisons and each infinity fails one, so the conjunction holds exactly
/// for finite values, both signed zeros included.
pub fn state_finite_predicate(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::CheckedScalarExpression;
    let parameters = program.state_parameters(state);
    let index = parameters
        .iter()
        .position(|candidate| candidate.symbol == parameter)?;
    if !finite_arrival_parameter(program, state, &parameters[index]) {
        return None;
    }
    let primitive_type = program.primitive_type_reference(parameters[index].type_reference)?;
    let position = parameters[..index]
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
        .count();
    crate::scalar_value_domain_predicate(
        language_semantics::value_domain::ValueDomain::Finite,
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        },
    )
}

/// Read every constrained shell without narrowing endpoints to the host's i64
/// window. References describe storage, not scalar arrival parameters; their
/// invariants remain with the structural storage and loan checks.
fn retain_declared_integer_ranges(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
    predicates: &mut Vec<StateScalarContractRow>,
) -> Option<()> {
    use typed_trees::types::PrimitiveType;
    if !matches!(
        program.primitive_type_reference(parameter.type_reference),
        Some(
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    ) {
        return Some(());
    }
    let mut reference = parameter.type_reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                continue;
            };
            if program.arithmetic_domain_for_type_reference(parameter.type_reference)
                != numerics::arithmetic::ArithmeticDomain::Exact
            {
                return None;
            }
            predicates.push(StateScalarContractRow::Interval {
                parameter: parameter.symbol,
                minimum: crate::closed_integer_range_bound(program, *minimum)?,
                maximum: crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?,
            });
        }
        reference = *base_type;
    }
    Some(())
}

/// The closed interval of the exactly-interval domain constraint on
/// `reference` that `membership` restates.
fn scalar_interval_bounds(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    membership: &typed_trees::domain::ProofMembershipFact,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                && domain.symbol == membership.domain_symbol
                && domain.semantic_id == membership.semantic_domain
                && let Some(bounds) =
                    crate::exact_declared_domain_carrier_interval(program, primitive, domain)
                        .filter(|(minimum, maximum)| minimum <= maximum)
            {
                return Some(bounds);
            }
        }
        reference = *base_type;
    }
    None
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

/// Every domain a view result's carrier declares. Constraints above the
/// borrow qualify the result shell directly. Constraints inside the borrow
/// qualify the referent — except on a byte view, whose lowered type peels
/// the reference so the referent slice and the result spell one carrier
/// identity: `&[u8] in Utf8` registers `Utf8` on the result's own structural
/// type, making the inside-borrow domain a result qualification rather than
/// a referent claim the loan cannot transport. Other reference referents
/// carry a distinct identity, so their inside-borrow domains stay out.
pub fn view_result_carrier_qualifications(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<Vec<SemanticDomainId>, &'static str> {
    let byte_view =
        crate::value_custody::reference_shells::byte_sequence_carrier(program, reference, &[])
            .is_some();
    let mut qualifications = Vec::new();
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let retained = program.type_reference_table.constraints(*constraints);
                if retained.len() != constraints.len() {
                    return Err("view result qualification span is stale");
                }
                for constraint in retained {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return Err("view result has an unsupported qualification");
                    };
                    if !domain.semantic_id.is_valid() {
                        return Err("view result qualification has no exact identity");
                    }
                    qualifications.push(domain.semantic_id);
                }
                reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } if byte_view => {
                reference = *referee;
            }
            _ => break,
        }
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
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<Vec<CheckedUnitStructuralPathSegment>, &'static str> {
    let (path, reference) = structural_owned_path(program, reference, segments)?;
    if program.primitive_type_reference(reference).is_some()
        || program.type_multiplicity(reference) != Multiplicity::Linear
    {
        return Err("structural claim path does not select linear structural custody");
    }
    Ok(path)
}

/// Reconstruct an owned subtree's path and exact selected type. Resolving a
/// path grants no transfer or claim: callers separately replay those events.
/// References cannot be crossed to manufacture ownership of their referents.
pub fn structural_owned_path(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle), &'static str> {
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
                let definition = crate::program_scope::data_definitions::data_definition_by_symbol(
                    program, *owner,
                )
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
                    field.path_identity(),
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
    Ok((path, reference))
}

/// Shared slice descriptors borrow contents without inheriting owned claims.
/// Their unrestricted multiplicity remains distinct from the affine lifetime
/// obligation of a materialized named-reference carrier.
pub fn shared_slice_view_result(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    crate::slice_view_shell(program, reference, &[])
        .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared)
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
    let Some((call, result)) = operation
        .call()
        .filter(|call| !call.is_boundary())
        .and_then(|call| Some((call, call.structural_result()?)))
    else {
        return Err("structural custody requires an ordinary structural call");
    };
    let coordinate = &call.coordinate;
    let target_machine = &call.target.machine;
    let target_state = &call.target.state;
    let structural_arguments = &call.structural_arguments;
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
    let expected_multiplicity = crate::machine_calls::reference_result_custody::result_multiplicity(
        program,
        destination.return_type,
    );
    if target.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || expected_multiplicity != result.multiplicity
    {
        return Err("structural call result differs from the callee contract");
    }
    let result_qualifications = if shared_slice_view_result(program, destination.return_type) {
        // A shared view's result type is its referent's carrier: the
        // byte-predicate domains it declares ride the same structural
        // identity, so the callee's qualifications are the call result's own.
        view_result_carrier_qualifications(program, destination.return_type)?
    } else if crate::machine_calls::reference_result_custody::parts(
        program,
        destination.return_type,
    )
    .is_some()
        || crate::machine_calls::reference_result_custody::shared_borrowed_parts(
            program,
            destination.return_type,
        )
        .is_some()
        || crate::machine_calls::reference_result_custody::mutable_borrowed_parts(
            program,
            destination.return_type,
        )
        .is_some()
    {
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
    // A terminal invocation retains its authored machine symbol. Its executable
    // destination is that machine's entry, never an arbitrary state it owns.
    // Keep the authored symbol below when joining the ownership events.
    let targets_destination = call.target_symbol == *target_state
        || (call.target_symbol == *target_machine
            && program
                .machine_states(target)
                .first()
                .is_some_and(|entry| entry.symbol == *target_state));
    if calls.next().is_some() || !targets_destination {
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
        // Primitive byte predicates describe an independently established
        // byte value, not resource claims — the carrier owns or views the
        // bytes either way. Preserve the exact signature roster when every
        // minted qualification is one of those predicates on a byte
        // carrier; arbitrary qualified or loan-bearing results keep the
        // fence.
        let byte_predicated = result.multiplicity == Multiplicity::Unrestricted
            && crate::byte_sequence_carrier(program, destination.return_type, &[]).is_some()
            && result_qualifications.iter().all(|domain| {
                program
                    .domain_definitions()
                    .iter()
                    .find(|definition| definition.semantic_id == *domain)
                    .and_then(|definition| {
                        typed_trees::byte_predicates::domain_byte_predicate(
                            program,
                            definition.symbol,
                        )
                    })
                    .is_some()
            });
        if (!result_qualifications.is_empty() && !byte_predicated) || !transfers.is_empty() {
            return Err("claim-bearing structural call has no returned claim frontier");
        }
        // A bare `&'a V` shared-view result is itself counted a reference
        // record for custody, but it is no construction — its loan is the
        // shared-borrow join, not captured leaf loans. Only results that
        // actually owe record loans must name exact captured leaves.
        if crate::machine_calls::reference_result_custody::local_owes_record_loans(
            program,
            destination.return_type,
        ) {
            let local_destination = program
                .statement_table
                .statements(source_state.statement_nodes)
                .get(call.statement_index)
                .and_then(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.initial_value == call.authored_expression =>
                    {
                        Some(local)
                    }
                    _ => None,
                });
            if let Some(local) = local_destination {
                if program.normalized_type_identity(local.type_reference)
                    != program.normalized_type_identity(destination.return_type)
                    || crate::machine_calls::reference_result_custody::local_record_loans(
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
            // A nested call result is an anonymous carrier: no local mints
            // leaf loans for it, so each returned leaf must instead replay
            // through the call's exact actual to a live caller-side loan.
            } else if !crate::machine_calls::reference_result_custody::nested_call_result_loans(
                program,
                facts,
                caller_machine,
                source_state,
                u32::try_from(call.statement_index)
                    .map_err(|_| "reference record call index exceeds u32")?,
                call.authored_expression,
            ) {
                return Err("reference record result has no exact captured leaf loans");
            }
        }
        let reference_loan = if crate::machine_calls::reference_result_custody::parts(
            program,
            destination.return_type,
        )
        .is_some()
            || crate::machine_calls::reference_result_custody::mutable_borrowed_parts(
                program,
                destination.return_type,
            )
            .is_some()
            || crate::machine_calls::reference_result_custody::shared_borrowed_parts(
                program,
                destination.return_type,
            )
            .is_some_and(|(referee, _)| program.primitive_type_reference(referee).is_none())
        {
            crate::machine_calls::reference_result_custody::result_loan(
                program,
                facts,
                caller_machine,
                source_state,
                call,
                result,
            )
            .ok_or("reference result has no exact returned ingress and loan lifetime")?
        } else if crate::machine_calls::reference_result_custody::mutable_borrowed_parts(
            program,
            destination.return_type,
        )
        .is_some()
        {
            crate::machine_calls::reference_result_custody::mutable_result_loan(
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
            result_qualifications,
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
