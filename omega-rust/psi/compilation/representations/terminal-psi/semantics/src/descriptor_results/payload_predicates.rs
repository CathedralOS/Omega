//! Predicate-only memberships follow actual current integer/Boolean payload contents.
//! Observations are snapshots of typed constructor/store operands, not facts
//! supplied by a receiving copy. Unknown contents stay unknown. The ordinary
//! history worklist owns mutation, predecessor intersection and value capture;
//! this query uses the existing predicate-denotation owner to check the theory.
//! Neither Content nor establishment-route authority can be introduced here.
use super::PayloadMembershipContract;
use crate::descriptor_lifetimes::DescriptorPathSegment;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, PropositionContext, ScalarTerm, ScalarType,
    StructuralPlaceKind,
};
use std::borrow::{Borrow, Cow};
use terminal_psi::{StructuralDomainDeclaration, StructuralTypeDeclaration};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadScalarObservation {
    pub path: Vec<DescriptorPathSegment>,
    /// A known executable scalar expression. An entry snapshot identity below
    /// does not manufacture such an expression or a numeric premise.
    pub value: Option<ScalarTerm>,
    /// Equality-only provenance of an unknown immutable entry value. This is
    /// disposable history data, not serialized SSA or an address/permission.
    pub entry: Option<PayloadEntryScalarIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadEntryScalarIdentity {
    pub root: semantic_vocabulary::PlaceId,
    pub path: Vec<DescriptorPathSegment>,
    pub scalar_type: ScalarType,
}

impl PayloadScalarObservation {
    /// Meet the two independent observations at one typed location. A new read
    /// may provide an expression without changing its original entry snapshot.
    /// Neither component survives a conflicting or absent incoming component.
    pub(super) fn intersect(retained: &mut Vec<Self>, incoming: &[Self]) -> bool {
        let mut changed = false;
        retained.retain_mut(|observation| {
            let mut rows = incoming.iter().filter(|row| row.path == observation.path);
            let Some(row) = rows.next() else {
                changed = true;
                return false;
            };
            if rows.next().is_some() {
                changed = true;
                return false;
            }
            if observation.value != row.value && observation.value.take().is_some() {
                changed = true;
            }
            if observation.entry != row.entry && observation.entry.take().is_some() {
                changed = true;
            }
            observation.value.is_some() || observation.entry.is_some()
        });
        changed
    }
}

pub(super) fn canonical_path(
    path: &[DescriptorPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    path.iter()
        .map(|segment| match segment {
            DescriptorPathSegment::Field(field) => {
                Some(CanonicalStructuralPathSegment::Field(*field))
            }
            DescriptorPathSegment::FixedIndex(index) => {
                Some(CanonicalStructuralPathSegment::FixedIndex(*index))
            }
            DescriptorPathSegment::Case(case) => Some(CanonicalStructuralPathSegment::Case(*case)),
            DescriptorPathSegment::Referent => Some(CanonicalStructuralPathSegment::Referent),
        })
        .collect()
}

pub(super) fn observation_type(
    types: &[StructuralTypeDeclaration],
    carrier: semantic_vocabulary::StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<ScalarType> {
    let path = crate::descriptor_lifetimes::descriptor_path(types, carrier, path)?;
    crate::primitive_read_projection_type(types.iter(), carrier, &path)
}

pub(super) fn close(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
    close_with_shared_requirements::<Proposition>(types, domains, contract, &[], None);
}

/// Most payloads have no eligible byte predicate. Keep the state's shared
/// theory borrowed until a predicate actually reads it; that reader still
/// receives the complete ordered premises for its context, equations and goal.
pub(super) fn close_with_shared_requirements<Shared: Borrow<Proposition>>(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
    shared: &[Shared],
    edge_predicate: Option<&Proposition>,
) {
    let mut requirements = None;
    for domain in domains {
        if domain.carrier != contract.carrier
            || domain.content_projection.is_some()
            || !domain.establishment_routes.is_empty()
            || contract.qualifications.contains(&domain.id)
        {
            continue;
        }
        let Some(predicate) =
            introduction_predicate(types, domains, domain, &contract.qualifications)
        else {
            continue;
        };
        let requirements = requirements.get_or_insert_with(|| {
            if shared.is_empty() && edge_predicate.is_none() {
                Cow::Borrowed(contract.scalar_requirements.as_slice())
            } else {
                let mut premises = contract.scalar_requirements.clone();
                super::payload_history::extend_canonical_premises(&mut premises, shared);
                // Preserve the pre-normalization proof input of predicate edges:
                // ordered local/shared union, then the whole predicate if new.
                if let Some(predicate) = edge_predicate
                    && !premises.contains(predicate)
                {
                    premises.push(predicate.clone());
                }
                Cow::Owned(premises)
            }
        });
        let mut value_types = Vec::new();
        for observation in &contract.scalar_observations {
            if let Some(value) = &observation.value {
                collect_value_types(value, &mut value_types, 0);
            }
        }
        for premise in requirements.iter() {
            collect_proposition_types(premise, &mut value_types);
        }
        let Ok(context) = PropositionContext::from_value_types_and_places(
            value_types,
            [(
                predicate.subject,
                StructuralPlaceKind::Parameter { position: 0 },
            )],
        ) else {
            continue;
        };
        // These premises came from entry obligations, completed calls or
        // selected edges. Their unconditional equalities may rewrite captured
        // scalar operands just as the independently reconstructed field
        // equations do; the requested domain supplies neither roster.
        let mut equations = requirements.to_vec();
        for observation in &contract.scalar_observations {
            let Some(value) = &observation.value else {
                continue;
            };
            let Some(path) = canonical_path(&observation.path) else {
                continue;
            };
            let scalar_type = value.scalar_type();
            if observation_type(types, contract.carrier, &observation.path) != Some(scalar_type) {
                continue;
            }
            let observed = match scalar_type {
                ScalarType::Integer(integer) => {
                    ScalarTerm::integer_field_path(predicate.subject, path, integer)
                }
                ScalarType::Boolean => ScalarTerm::boolean_field_path(predicate.subject, path),
                // IEEE equality is not a snapshot equation for NaN.
                ScalarType::IeeeFloat(_) => continue,
            };
            equations.push(Proposition::Equal(observed, value.clone()));
        }
        visit_proposition_terms(&predicate.proposition, &mut |term| {
            if let Some(value) = observed_byte_predicate(types, contract, predicate.subject, term) {
                equations.push(Proposition::Equal(term.clone(), ScalarTerm::Boolean(value)));
            }
        });
        let checked = proof_admission::check_predicate_denotations_with_value_equalities(
            &context,
            &predicate.proposition,
            requirements,
            &equations,
        );
        if checked.is_ok_and(|checked| {
            super::payload_entry::prove(&context, checked.goal(), checked.requirements(), 0)
                .is_some_and(|proof| checked.check_certificate(&context, &proof).is_ok())
        }) {
            contract.qualifications.push(domain.id);
        }
    }
    contract.qualifications.sort();
    contract.qualifications.dedup();
}

fn collect_value_types(
    term: &ScalarTerm,
    output: &mut Vec<(semantic_vocabulary::ValueId, ScalarType)>,
    depth: usize,
) {
    visit_scalar_terms(
        term,
        &mut |term| {
            if let ScalarTerm::Value { id, scalar_type } = term {
                output.push((*id, *scalar_type));
            }
        },
        depth,
    );
}

fn visit_scalar_terms(term: &ScalarTerm, visit: &mut impl FnMut(&ScalarTerm), depth: usize) {
    if depth > 64 {
        return;
    }
    visit(term);
    for operand in term.operands() {
        visit_scalar_terms(operand, visit, depth + 1);
    }
}

fn collect_proposition_types(
    proposition: &Proposition,
    output: &mut Vec<(semantic_vocabulary::ValueId, ScalarType)>,
) {
    visit_proposition_terms(proposition, &mut |term| {
        if let ScalarTerm::Value { id, scalar_type } = term {
            output.push((*id, *scalar_type));
        }
    });
}

fn visit_proposition_terms(proposition: &Proposition, visit: &mut impl FnMut(&ScalarTerm)) {
    use semantic_vocabulary::IntegerMathTerm;
    enum Pending<'a> {
        Proposition(&'a Proposition),
        Math(&'a IntegerMathTerm),
    }
    let mut pending = vec![Pending::Proposition(proposition)];
    while let Some(item) = pending.pop() {
        match item {
            Pending::Proposition(proposition) => match proposition {
                semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                    visit_scalar_terms(left, visit, 0);
                    visit_scalar_terms(right, visit, 0);
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    pending.push(Pending::Math(left));
                    pending.push(Pending::Math(right));
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    pending.extend(children.iter().map(Pending::Proposition))
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(Pending::Proposition(premise));
                    pending.push(Pending::Proposition(conclusion));
                }
                _ => {}
            },
            Pending::Math(term) => match term {
                IntegerMathTerm::Embed(term) => visit_scalar_terms(term, visit, 0),
                IntegerMathTerm::MathValue { source_type, value } => visit(&ScalarTerm::value(
                    *value,
                    ScalarType::Integer(*source_type),
                )),
                _ => pending.extend(term.operands().into_iter().map(Pending::Math)),
            },
        }
    }
}

/// Only complete, typed current bytes determine a content predicate. A view,
/// unknown element or retired observation supplies no literal content evidence.
fn observed_byte_predicate(
    types: &[StructuralTypeDeclaration],
    contract: &PayloadMembershipContract,
    subject: semantic_vocabulary::PlaceId,
    term: &ScalarTerm,
) -> Option<bool> {
    let ScalarTerm::ByteSequencePredicate {
        predicate,
        root,
        path,
    } = term
    else {
        return None;
    };
    if *root != subject {
        return None;
    }
    let selected = crate::canonical_structural_path_tip(types.iter(), contract.carrier, path)?;
    let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = selected.shape else {
        return None;
    };
    if !crate::byte_predicate_carrier(types.iter(), selected.id)
        || length > contract.scalar_observations.len() as u64
    {
        return None;
    }
    let mut bytes = Vec::new();
    for index in 0..length {
        let mut element_path = path.clone();
        element_path.push(CanonicalStructuralPathSegment::FixedIndex(index));
        let mut matching = contract.scalar_observations.iter().filter(|observation| {
            canonical_path(&observation.path).as_ref() == Some(&element_path)
        });
        let observation = matching.next()?;
        let observed = observation.value.as_ref()?;
        if matching.next().is_some()
            || observation_type(types, contract.carrier, &observation.path)
                != Some(observed.scalar_type())
        {
            return None;
        }
        let ScalarTerm::Integer { value, .. } = observed else {
            return None;
        };
        let byte = match value {
            semantic_vocabulary::IntegerValue::Unsigned(value) => u8::try_from(*value).ok()?,
            semantic_vocabulary::IntegerValue::Signed(value) => u8::try_from(*value).ok()?,
        };
        bytes.push(byte);
    }
    Some(predicate.holds_for(&bytes))
}

pub(super) fn terminal_scalar_literal(
    machine: &terminal_psi::TerminalMachine,
    value: semantic_vocabulary::ValueId,
) -> Option<ScalarTerm> {
    use terminal_psi::OperationKind;
    if let Some(literal) = machine.operations().find_map(|operation| {
        let result = operation.result.scalar_ref()?;
        if result.id != value {
            return None;
        }
        match operation.kind {
            OperationKind::BooleanConstant { value } => Some(ScalarTerm::Boolean(value)),
            _ => None,
        }
    }) {
        return Some(literal);
    }
    let (scalar_type, literal) = crate::scalar_constant_value(value, |value| {
        crate::machine_scalar_constant_origin(machine, value)
    })?;
    let ScalarType::Integer(integer) = scalar_type else {
        return None;
    };
    Some(ScalarTerm::Integer {
        scalar_type: integer,
        value: literal,
    })
}

/// Expand completed scalar denotations, whose operation obligations remain
/// independently checked. Reads remain immutable SSA snapshots; their
/// relationship to current storage belongs to payload history below.
pub fn scalar_payload_expression(
    value: semantic_vocabulary::ValueId,
    origin: &impl Fn(semantic_vocabulary::ValueId) -> Option<(ScalarType, Option<ScalarTerm>)>,
) -> Option<ScalarTerm> {
    fn expand(
        term: &mut ScalarTerm,
        origin: &impl Fn(semantic_vocabulary::ValueId) -> Option<(ScalarType, Option<ScalarTerm>)>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<()> {
        if depth > 64 {
            return None;
        }
        *remaining = remaining.checked_sub(1)?;
        if let ScalarTerm::Value { id, scalar_type } = term {
            let (actual, denotation) = origin(*id)?;
            if actual != *scalar_type {
                return None;
            }
            if let Some(denotation) = denotation {
                *term = denotation;
                return expand(term, origin, remaining, depth + 1);
            }
            return Some(());
        }
        for operand in term.operands_mut() {
            expand(operand, origin, remaining, depth + 1)?;
        }
        Some(())
    }
    let (scalar_type, _) = origin(value)?;
    let mut term = ScalarTerm::value(value, scalar_type);
    expand(&mut term, origin, &mut 4096, 0)?;
    Some(term)
}

/// One immutable scalar namespace per descriptor-machine adaptation. Expanding
/// each stored operand and edge predicate previously rebuilt the same complete
/// type table. Retaining it here changes preparation, not expression expansion:
/// literal recovery, definition order, failure and recursion budgets stay intact.
pub(super) struct TerminalScalarExpressions<'machine> {
    machine: &'machine terminal_psi::TerminalMachine,
    types: std::collections::BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
}

impl<'machine> TerminalScalarExpressions<'machine> {
    pub(super) fn new(machine: &'machine terminal_psi::TerminalMachine) -> Self {
        let types: std::collections::BTreeMap<_, _> = machine
            .parameters
            .iter()
            .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .map(|value| (value.id, value.scalar_type))
            .collect();
        Self { machine, types }
    }

    pub(super) fn expression(&self, value: semantic_vocabulary::ValueId) -> Option<ScalarTerm> {
        let machine = self.machine;
        let types = &self.types;
        scalar_payload_expression(value, &|value| {
            let scalar_type = *types.get(&value)?;
            if let Some(literal) = terminal_scalar_literal(machine, value) {
                return Some((scalar_type, Some(literal)));
            }
            let denotation = machine
                .operations()
                .find(|operation| {
                    operation
                        .result
                        .scalar_ref()
                        .is_some_and(|result| result.id == value)
                })
                .and_then(|operation| {
                    crate::goal_free_scalar_leaf_semantics(operation, types)
                        .ok()
                        .flatten()
                        .map(|semantics| semantics.result_equation().clone())
                        .or_else(|| {
                            crate::proof_bearing_scalar_leaf_semantics(operation, types)
                                .ok()
                                .flatten()
                                .map(|semantics| semantics.result_equation().clone())
                        })
                })
                .and_then(|equation| match equation {
                    Proposition::Equal(_, right) => Some(right),
                    _ => None,
                });
            Some((scalar_type, denotation))
        })
    }
}

pub(crate) fn terminal_scalar_snapshots(
    machine: &terminal_psi::TerminalMachine,
    values: &[semantic_vocabulary::ValueId],
) -> Vec<(semantic_vocabulary::ValueId, ScalarTerm)> {
    let expressions = TerminalScalarExpressions::new(machine);
    values
        .iter()
        .filter_map(|value| {
            expressions
                .expression(*value)
                .map(|expression| (*value, expression))
        })
        .collect()
}

/// Prove every still-missing value obligation on this exact subject. Existing
/// memberships cover their own authenticated prerequisites; an unsupplied
/// issuer or content obligation cannot be replaced by predicate truth.
pub(super) fn introduction_predicate(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    domain: &StructuralDomainDeclaration,
    supplied: &[semantic_vocabulary::StructuralDomainId],
) -> Option<terminal_psi::StructuralDomainPredicate> {
    let subject = domain.predicate.as_ref()?.subject;
    let mut clauses = Vec::new();
    for missing in domain.missing_membership_requirements(domains, supplied)? {
        if missing.content_projection.is_some() || !missing.establishment_routes.is_empty() {
            return None;
        }
        let predicate = missing.predicate.as_ref()?;
        if !crate::domain_predicate_is_closed(types.iter(), missing.carrier, predicate) {
            return None;
        }
        let proposition = crate::substitution::substitute_proposition_structural_places(
            &predicate.proposition,
            &std::collections::BTreeMap::from([(predicate.subject, (subject, Vec::new()))]),
        );
        if proposition != Proposition::Truth {
            clauses.push(proposition);
        }
    }
    clauses.sort();
    clauses.dedup();
    let proposition = match clauses.len() {
        0 => Proposition::Truth,
        1 => clauses.pop()?,
        _ => Proposition::Conjunction(clauses),
    };
    Some(terminal_psi::StructuralDomainPredicate {
        subject,
        proposition,
    })
}
