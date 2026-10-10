//! Decide ordinary structural call custody from the semantic call and
//! ownership facts. A result binding records a value, not authority. Every
//! consumed input claim crosses at its owned argument; every linear claim path
//! of the declared result type binds the caller's claim for the claim the
//! callee returns there. The binding is by path because the callee may return
//! different claims at one path on different exits; a matching type or an
//! empty producer record cannot establish custody either way.
//!
//! Stage 04 decides each ordinary structural call's custody here once and
//! records it in the call plan; lowering reads that record and never decides
//! it again. These source facts are not portable PCC openings.

use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralPathQualification, CheckedStructuralResultClaimPlan,
    CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralPathSegment, FlowCallFact, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeSource,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

mod declaration_predicates;

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
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    let Some(membership) = fact.as_membership() else {
        return false;
    };
    let Some(symbol) =
        crate::expression_traversal::single_segment_name_symbol(program, membership.value)
    else {
        return false;
    };
    let Some(parameter) = program.state_parameter_by_symbol(state, symbol) else {
        return false;
    };
    let reference = program
        .type_reference_table
        .dereferenced_type_reference(parameter.type_reference);
    if program.primitive_type_reference(reference).is_some() {
        return false;
    }
    let Some(domain) = program.domain_definition(membership.domain_symbol) else {
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
    program: &dyn crate::CheckingInput,
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
        let reference = program
            .type_reference_table
            .dereferenced_type_reference(parameter.type_reference);
        if program.primitive_type_reference(reference).is_some() {
            if !parameter.is_mutable {
                expected.extend(
                    crate::scalar_interval_domains(program, reference)
                        .into_iter()
                        .chain(crate::scalar_type_tags(program, reference))
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
                    let path = program.expression_table.name_path_node(membership.value)?;
                    let Some(symbol) = crate::expression_traversal::single_segment_name_symbol(
                        program,
                        membership.value,
                    ) else {
                        return None;
                    };
                    let domain = program.domain_definition(membership.domain_symbol)?;
                    // A scalar parameter's interval membership is carried as its
                    // closed interval; a scalar tag is carried by the qualified
                    // Terminal signature itself and needs no row. The
                    // qualification comparison below still requires either to
                    // restate the declared type exactly. A tag compares by its
                    // applied identity, which differs from its declaration's
                    // when the family takes arguments (`Indexed<A>`).
                    let parameter = program.state_parameter_by_symbol(state, path.symbol);
                    let tag = parameter.is_some_and(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                            && crate::scalar_type_tags(program, parameter.type_reference)
                                .contains(&(membership.domain_symbol, membership.semantic_domain))
                    });
                    actual.push((
                        symbol,
                        if tag {
                            membership.semantic_domain.0
                        } else {
                            domain.semantic_id.0
                        },
                    ));
                    if let Some(parameter) = parameter
                        && let Some(primitive) =
                            program.primitive_type_reference(parameter.type_reference)
                        && !tag
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
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::types::PrimitiveType;
    if parameter.is_const
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
    predicates: &mut Vec<StateScalarContractRow>,
) -> Option<()> {
    if !program
        .primitive_type_reference(parameter.type_reference)
        .is_some_and(|primitive| primitive.is_integer())
    {
        return Some(());
    }
    for (minimum, maximum, end_inclusive) in
        crate::value_custody::type_shapes::range_constraints(program, parameter.type_reference)
    {
        if program.arithmetic_domain_for_type_reference(parameter.type_reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return None;
        }
        predicates.push(StateScalarContractRow::Interval {
            parameter: parameter.symbol,
            minimum: crate::closed_integer_range_bound(program, minimum)?,
            maximum: crate::closed_integer_range_maximum(program, maximum, end_inclusive)?,
        });
    }
    Some(())
}

/// The closed interval of the exactly-interval domain constraint on
/// `reference` that `membership` restates.
fn scalar_interval_bounds(
    program: &dyn crate::CheckingInput,
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

/// Declared value qualifications accompany a carrier; they are not part of
/// its storage identity and are never inferred from incidental initializer facts.
pub struct StructuralResultContract {
    pub carrier: TypeReferenceHandle,
    pub qualifications: Vec<SemanticDomainId>,
}

/// Split an owned value's declared contract without granting any qualification.
/// Establishment and call checking remain the owners of those judgments.
pub fn structural_result_contract(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Result<StructuralResultContract, &'static str> {
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
    Ok(StructuralResultContract {
        carrier: reference,
        qualifications,
    })
}

/// Exact whole-result qualification row, excluding reference presentation.
pub fn structural_result_qualifications(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Vec<SemanticDomainId>, &'static str> {
    Ok(structural_result_contract(program, reference)?.qualifications)
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
    program: &dyn crate::CheckingInput,
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
/// emits finite record/fixed-index paths and rejects malformed spans.
pub fn structural_result_projected_qualifications(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<CheckedStructuralPathQualification>, &'static str> {
    structural_qualification_contract(program, type_reference).map(|(rows, _)| rows)
}

/// Inventory the same exact source contract for path rows and domain theories.
/// Theory registration describes a domain; it never proves membership. Closed
/// instances supply substituted members, and active references bound recursion.
pub fn structural_qualification_contract(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<
    (
        Vec<CheckedStructuralPathQualification>,
        Vec<(SemanticDomainId, TypeReferenceHandle)>,
    ),
    &'static str,
> {
    fn collect(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        is_root: bool,
        path: &mut Vec<CheckedUnitStructuralPathSegment>,
        output: &mut Vec<CheckedStructuralPathQualification>,
        carriers: &mut Vec<(SemanticDomainId, TypeReferenceHandle)>,
        active: &mut Vec<TypeReferenceHandle>,
    ) -> Result<(), &'static str> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, is_root, path, output, carriers, active)
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
                    // A complete interval domain on a primitive is represented
                    // by its exact storage declaration, like a refined record
                    // scalar field. Carrying it again as a nominal membership
                    // row would discard that distinction at array loan seams.
                    // Routed and window-scoped memberships retain their rows.
                    if program
                        .primitive_type_reference(reference)
                        .is_some_and(|primitive| {
                            crate::exact_declared_domain_carrier_interval(
                                program, primitive, domain,
                            )
                            .is_some()
                        })
                    {
                        continue;
                    }
                    if !carriers.contains(&(domain.semantic_id, reference)) {
                        carriers.push((domain.semantic_id, reference));
                    }
                    if !is_root || !path.is_empty() {
                        output.push(CheckedStructuralPathQualification {
                            path: path.clone(),
                            domain: domain.semantic_id,
                        });
                    }
                }
                collect(program, *base_type, is_root, path, output, carriers, active)
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
                    carriers,
                    active,
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
                        qualified.path.splice(0..0, path.iter().cloned());
                        output.push(qualified);
                    }
                }
                Ok(())
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let Some(record) = program.closed_data_definition(reference) else {
                    return Ok(());
                };
                if active.contains(&reference) {
                    return Ok(());
                }
                active.push(reference);
                for member in program.data_members(record) {
                    let Some(field) = member.as_field() else {
                        continue;
                    };
                    if field.relevance.is_erased()
                        || program
                            .primitive_type_reference(field.type_reference)
                            .is_some()
                        // Byte fields embed their predicate contract in the
                        // field's carrier, like refined scalar fields. They
                        // are not nominal structural children, and duplicating
                        // that contract as a structural path would erase this
                        // represented/nominal boundary.
                        || crate::byte_sequence_carrier(program, field.type_reference, &[])
                            .is_some()
                    {
                        continue;
                    }
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        field.path_identity(),
                    ));
                    collect(
                        program,
                        field.type_reference,
                        false,
                        path,
                        output,
                        carriers,
                        active,
                    )?;
                    path.pop();
                }
                active.pop();
                Ok(())
            }
            _ => Ok(()),
        }
    }
    let mut output = Vec::new();
    let mut carriers = Vec::new();
    collect(
        program,
        type_reference,
        true,
        &mut Vec::new(),
        &mut output,
        &mut carriers,
        &mut Vec::new(),
    )?;
    output.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.domain.0.cmp(&right.domain.0))
    });
    output.dedup();
    Ok((output, carriers))
}

/// Replays a complete identity-preserving return of one whole owned parameter.
/// Other parameters may be consumed separately, but every output must continue
/// this parameter's exact entry path and every entry claim below it must return.
/// The source ownership analysis supplies the outcome theorem; this join checks
/// its exact custody, rather than inferring a theorem from matching carriers.
pub fn reconstruct_structural_parameter_return_claims(
    program: &dyn crate::CheckingInput,
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
        .machine(machine)
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle), &'static str> {
    let mut path = Vec::with_capacity(segments.len());
    let mut payload_fields: Option<&[typed_trees::data::DataField]> = None;
    for segment in segments {
        structural_result_qualifications(program, reference)?;
        reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        match (
            segment,
            program.type_reference_table.type_reference(reference),
        ) {
            (
                facts::PlaceSegment::Field { symbol },
                TypeReferenceNode::Named { symbol: owner, .. },
            ) => {
                let definition =
                    crate::declarations::data_queries::data_definition_by_symbol(program, *owner)
                        .ok_or("structural claim field has no exact data owner")?;
                let field = if let Some(fields) = payload_fields.take() {
                    fields.iter().find(|field| field.symbol == *symbol)
                } else {
                    program.data_members(definition).iter().find_map(|member| {
                        let field = member.as_field()?;
                        (field.symbol == *symbol).then_some(field)
                    })
                }
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
                facts::PlaceSegment::Case { variant },
                TypeReferenceNode::Named { symbol: owner, .. },
            ) if payload_fields.is_none() => {
                let definition = program
                    .data_definition_by_symbol(*owner)
                    .ok_or("structural claim case has no exact data owner")?;
                let selected = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .find(|candidate| candidate.symbol == *variant)
                    .ok_or("structural claim case differs from its typed owner")?;
                path.push(CheckedUnitStructuralPathSegment::Case(
                    selected.path_identity(),
                ));
                payload_fields = Some(program.data_payload_fields(selected));
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                },
            ) if payload_fields.is_none() && *index < *length => {
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).map_err(|_| "structural claim index exceeds u64")?,
                ));
                reference = *element_type;
            }
            _ => return Err("structural claim has an unsupported or mistyped path"),
        }
    }
    if payload_fields.is_some() {
        return Err("structural claim ends at a case namespace rather than a value");
    }
    structural_result_qualifications(program, reference)?;
    Ok((path, reference))
}

/// Shared slice descriptors borrow contents without inheriting owned claims.
/// Their unrestricted multiplicity remains distinct from the affine lifetime
/// obligation of a materialized named-reference carrier.
pub fn shared_slice_view_result(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    crate::slice_view_shell(program, reference, &[])
        .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared)
}

/// Decide one ordinary structural call's custody from its exact call facts.
/// The custody the plan already carries is not read. Operand and source
/// checking own every value binding and projection; this judgment owns the
/// claim correspondence, not call sequencing.
/// The authored argument roster of a structural call occurrence: its
/// authored expression when present, else the authored statement the
/// coordinate names — each refusing a target drift — arity-checked
/// against the destination parameter roster.
fn structural_authored_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &FlowCallFact,
    source_state: &typed_trees::state::State,
    coordinate: &CheckedUnitCallCoordinate,
    target_state: SymbolHandle,
    parameter_count: usize,
) -> Result<&'program [typed_trees::expression::ExpressionHandle], &'static str> {
    let authored_arguments = if call.authored_expression.is_valid() {
        let ExpressionNode::Call(authored) = program
            .expression_table
            .expression(call.authored_expression)
        else {
            return Err("structural call occurrence has no authored expression");
        };
        if authored.target_symbol != target_state {
            return Err("structural call expression target changed");
        }
        program
            .expression_table
            .expression_handles(authored.arguments)
    } else {
        match program
            .statement_table
            .statements(source_state.statement_nodes)
            .get(coordinate.statement_index as usize)
        {
            Some(StatementNode::Call(authored)) => {
                if authored.target_symbol != target_state {
                    return Err("structural call statement target changed");
                }
                program
                    .statement_table
                    .expression_handles(authored.arguments)
            }
            // A terminal call in a transition arm names the machine (or its
            // entry state) as the arm's target; its result completes the
            // caller.
            Some(StatementNode::Transition(transition)) => {
                let names_target = |symbol: SymbolHandle| {
                    symbol == target_state
                        || program.machine_by_symbol(symbol).is_some_and(|machine| {
                            program
                                .machine_states(machine)
                                .first()
                                .is_some_and(|entry| entry.symbol == target_state)
                        })
                };
                let mut named = [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                    .filter_map(
                        |target| match program.statement_table.transition_target(target) {
                            typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                arguments,
                                ..
                            } if names_target(path.symbol) => Some(*arguments),
                            _ => None,
                        },
                    );
                let arguments = named
                    .next()
                    .ok_or("structural call occurrence has no authored statement")?;
                if named.next().is_some() {
                    return Err("structural transition call names its target twice");
                }
                program.statement_table.expression_handles(arguments)
            }
            _ => return Err("structural call occurrence has no authored statement"),
        }
    };
    if authored_arguments.len() != parameter_count {
        return Err("structural call has no exact explicit argument roster");
    }
    Ok(authored_arguments)
}

pub fn structural_call_custody(
    program: &dyn crate::CheckingInput,
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
        .machine(caller_machine)
        .ok_or("structural call has no exact caller")?;
    let source_state = program
        .state_in_machine(caller, caller_state)
        .ok_or("structural call has no exact caller state")?;
    let target = program
        .machine(*target_machine)
        .ok_or("structural call has no exact callee")?;
    let destination =
        typed_trees::state::state_by_symbol(program.machine_states(target), *target_state)
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
        || crate::machine_calls::reference_result_custody::has_borrowed_parts(
            program,
            destination.return_type,
        )
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
    let result_frontier = crate::linear_claim_frontier(program, destination.return_type);
    if result_frontier.is_empty() {
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
        if !result_qualifications.is_empty() && !byte_predicated {
            return Err("claim-bearing structural call has no returned claim frontier");
        }
        // Input claims consumed at this occurrence terminate inside the
        // callee: its checked body already accounted for every claim it was
        // admitted with, so a claim-free result owes no returned frontier.
        let claim_transfers = owned_argument_claim_transfers(
            program,
            call,
            source_state,
            coordinate,
            destination,
            *target_state,
            structural_arguments,
            &transfers,
            &facts.flow.ownership.segments,
        )?;
        // A bare `&'a V` shared-view result is itself counted a reference
        // record for custody, but it is no construction — its loan is the
        // shared-borrow join, not captured leaf loans. Only results that
        // actually owe record loans must name exact captured leaves.
        if crate::machine_calls::reference_result_custody::local_owes_record_loans(
            program,
            destination.return_type,
        ) {
            use crate::machine_calls::reference_result_custody::record_call_reception;
            let reception = record_call_reception(
                program,
                facts,
                caller_machine,
                source_state,
                u32::try_from(call.statement_index)
                    .map_err(|_| "reference record call index exceeds u32")?,
                call.authored_expression,
            )
            .ok_or("reference record result has no exact captured leaf loans")?;
            // Received leaves consume checked formation groups. Unreceived
            // projection siblings and anonymous operands stay in the call's
            // temporary carrier until ordinary subtree/operand disposal.
            if !reception.residual_paths.is_empty()
                && !crate::machine_calls::reference_result_custody::transient_call_record_loans(
                    program,
                    facts,
                    caller_machine,
                    source_state,
                    u32::try_from(call.statement_index)
                        .map_err(|_| "reference record call index exceeds u32")?,
                    call.authored_expression,
                    structural_arguments,
                    &reception.residual_paths,
                )
            {
                return Err("reference record result has no exact captured leaf loans");
            }
        }
        let forwarded_reference_sources =
            crate::machine_calls::reference_result_custody::terminal_call_reference_sources(
                program,
                source_state,
                call,
                result,
            )
            .unwrap_or_default();
        let reference_loans = if !forwarded_reference_sources.is_empty() {
            arena::HandleSpan::empty()
        } else if crate::machine_calls::reference_result_custody::has_borrowed_parts(
            program,
            destination.return_type,
        ) {
            crate::machine_calls::reference_result_custody::result_loans(
                program,
                facts,
                caller_machine,
                source_state,
                call,
                result,
            )
            .ok_or("reference result has no exact returned ingress and loan lifetime")?
        } else {
            arena::HandleSpan::empty()
        };
        return Ok(CheckedStructuralCallCustodyPlan {
            reference_loans,
            forwarded_reference_sources,
            result_qualifications,
            claim_transfers,
            result_claims: Vec::new(),
        });
    }

    // The result carries linear claims. Every consumed input crosses at its
    // owned argument; the callee's checked body decides which of them, or
    // which claims it established itself, its returning edge leaves at each
    // claim path of the declared result type. Different exits may place
    // different claims there (checking publishes those alternatives as
    // conditional claim joins), so the result binds by path rather than by
    // one callee lineage: the callee's claim at a path continues as the
    // caller's claim at the same path.
    if crate::machine_calls::reference_result_custody::local_owes_record_loans(
        program,
        destination.return_type,
    ) || crate::machine_calls::reference_result_custody::has_borrowed_parts(
        program,
        destination.return_type,
    ) {
        return Err("claim-bearing structural result also carries a loan");
    }
    let claim_transfers = owned_argument_claim_transfers(
        program,
        call,
        source_state,
        coordinate,
        destination,
        *target_state,
        structural_arguments,
        &transfers,
        &facts.flow.ownership.segments,
    )?;
    let received = received_result_claims(
        program,
        facts,
        caller_machine,
        caller_state,
        source_state,
        call,
    )?;
    let mut result_claims = Vec::with_capacity(result_frontier.len());
    for claim in &result_frontier {
        let identity = if received.is_empty() {
            PermissionClaimIdentity::Unknown
        } else {
            let mut matching = received
                .iter()
                .filter(|(segments, _)| *segments == claim.path.as_slice());
            let (_, identity) = matching
                .next()
                .ok_or("structural result claim has no receiving establishment")?;
            if matching.next().is_some() {
                return Err("structural result claim is received twice");
            }
            *identity
        };
        result_claims.push(CheckedStructuralResultClaimPlan {
            claim: identity,
            path: structural_claim_path(program, destination.return_type, &claim.path)?,
        });
    }
    if !received.is_empty() && received.len() != result_frontier.len() {
        return Err("structural result receives claims outside its result frontier");
    }
    result_claims.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(CheckedStructuralCallCustodyPlan {
        reference_loans: arena::HandleSpan::empty(),
        forwarded_reference_sources: Vec::new(),
        result_qualifications,
        claim_transfers,
        result_claims,
    })
}

/// Join each linear claim this call consumes to the owned argument whose
/// place holds it: a named owner, a projection of one, or an anonymous owner
/// its producer completed at this statement. The roster follows the caller's
/// transfer events, so every event lands on exactly one argument; borrowed
/// arguments transfer nothing.
#[allow(clippy::too_many_arguments)]
fn owned_argument_claim_transfers(
    program: &dyn crate::CheckingInput,
    call: &FlowCallFact,
    source_state: &typed_trees::state::State,
    coordinate: &CheckedUnitCallCoordinate,
    destination: &typed_trees::state::State,
    target_state: SymbolHandle,
    structural_arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[&checked_trees::FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
) -> Result<Vec<CheckedUnitClaimTransferPlan>, &'static str> {
    let mut claim_transfers = Vec::with_capacity(transfers.len());
    if transfers.is_empty() {
        return Ok(claim_transfers);
    }
    let parameters = program.state_parameters(destination);
    let authored_arguments = structural_authored_arguments(
        program,
        call,
        source_state,
        coordinate,
        target_state,
        parameters.len(),
    )?;
    let mut accounted = vec![false; transfers.len()];
    for (parameter_position, authored_argument) in authored_arguments.iter().enumerate() {
        if program
            .primitive_type_reference(parameters[parameter_position].type_reference)
            .is_some()
        {
            continue;
        }
        let argument_position = parameters[..parameter_position]
            .iter()
            .filter(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            })
            .count();
        let Some(argument) = structural_arguments.get(argument_position) else {
            return Err("structural claim transfer names an absent argument");
        };
        if argument.access != CheckedStructuralAccess::Owned {
            continue;
        }
        let Some(mut place) = crate::canonical_place_from_expression_in_state(
            program,
            source_state.symbol,
            call.statement_index,
            *authored_argument,
        ) else {
            continue;
        };
        // The checked transfer roster authenticates the actual's owned root.
        // A result ordinal can denote either a named local or an anonymous
        // extraction; it cannot distinguish them by itself. A borrowed
        // extraction moves machine-local claims at its completed expression,
        // while an existing owned home retains its canonical storage root.
        if argument
            .source_structural_result_binding_ordinal()
            .is_some()
            && place.root.as_symbol().is_some()
            && transfers
                .iter()
                .any(|event| event.root == facts::PlaceRoot::Expression(*authored_argument))
        {
            if transfers.iter().any(|event| {
                event.root == place.root
                    && segments
                        .span_or_empty(event.segments)
                        .starts_with(&place.segments)
            }) {
                return Err("owned actual has ambiguous storage and completed-value custody");
            }
            place = crate::CanonicalPlace {
                root: facts::PlaceRoot::Expression(*authored_argument),
                segments: Vec::new(),
            };
        }
        for (event_index, event) in transfers.iter().enumerate() {
            if event.root != place.root
                || !segments
                    .span_or_empty(event.segments)
                    .starts_with(place.segments.as_slice())
                || event.claim_identity == PermissionClaimIdentity::Unknown
                || accounted[event_index]
            {
                continue;
            }
            let root_type = crate::canonical_place_type_reference(
                program,
                source_state.symbol,
                call.statement_index,
                &crate::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )
            .ok_or("owned argument has no canonical root type")?;
            let root_type = crate::assignment_value_type(program, root_type);
            let (event_path, _) =
                structural_owned_path(program, root_type, segments.span_or_empty(event.segments))?;
            let path = event_path
                .strip_prefix(argument.path.as_slice())
                .ok_or("claim is outside the selected owned argument")?
                .to_vec();
            accounted[event_index] = true;
            claim_transfers.push(CheckedUnitClaimTransferPlan {
                claim_identity: event.claim_identity,
                argument_index: u32::try_from(argument_position)
                    .map_err(|_| "structural argument position exceeds u32")?,
                path,
            });
        }
    }
    if accounted.iter().any(|accounted| !accounted) {
        return Err("consumed claim does not cross at an owned whole argument");
    }
    Ok(claim_transfers)
}

/// The caller's establishments that receive this call's result: a local
/// initialized by the call, an assigned place, or an anonymous construction
/// containing the call. The receiving join removes only constructor/storage
/// prefixes and leaves the result-relative path with its original identity.
/// An empty roster means no caller place receives the value (a terminal call
/// returned at once).
fn received_result_claims<'facts>(
    program: &dyn crate::CheckingInput,
    facts: &'facts CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    _source_state: &typed_trees::state::State,
    call: &FlowCallFact,
) -> Result<Vec<(&'facts [facts::PlaceSegment], PermissionClaimIdentity)>, &'static str> {
    let mut received = Vec::new();
    for (_, event) in facts.flow.ownership.permissions.iter() {
        if event.machine_symbol != caller_machine
            || event.state_symbol != caller_state
            || event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Linear
            || !event.obligation_live
            || !matches!(event.source,
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::Call { statement_index, .. }
                if statement_index == call.statement_index)
        {
            continue;
        }
        let Some((_, producer, relative)) =
            crate::receiving_value(program, &facts.flow.ownership.segments, event)
        else {
            continue;
        };
        if producer != call.authored_expression {
            continue;
        }
        if event.claim_identity == PermissionClaimIdentity::Unknown {
            return Err("structural result establishment has no claim identity");
        }
        received.push((relative, event.claim_identity));
    }
    Ok(received)
}
