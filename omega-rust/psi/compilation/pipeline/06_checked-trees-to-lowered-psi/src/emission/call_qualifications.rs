//! Normal-call memberships have one publication inventory. Complete scalar and
//! byte-field invariants belong to their exact receiving declaration; additional
//! nominal memberships require a catalog entry and an occurrence publication.

use crate::emission::reference_formations::ReferenceFormations;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::shared::lowering_error::LoweringError;

/// Keep why a declared field does not carry the complete checked obligation.
/// Normal publication remains responsible for every such membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationRepresentation {
    Complete,
    RequiresPublication(DeclarationRepresentationRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationRepresentationRefusal {
    NotCallEnsures,
    NotDomainMembership,
    NoStoragePlace,
    NoRootType,
    NoLeafType,
    NoExactDeclaredDomain {
        domain: SymbolHandle,
        instance: language_semantics::SemanticDomainId,
        same_symbol: bool,
        same_instance: bool,
    },
    DuplicateDeclaredDomain,
    NoDomainInstance,
    IncompletePredicateTheory,
    EstablishmentRoute,
    RequiredSubjectMembership,
    ContentCustody,
    NotDeclaredByteField,
    UnknownDeclaredBytePredicate,
    MissingDeclaredBytePredicate,
    NotScalarPredicate,
    NoScalarRestriction,
    NoScalarDomainPredicate,
    ScalarPredicateNotEntailed,
}

impl DeclarationRepresentation {
    pub(crate) fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }
}

pub(crate) fn for_call<'a>(
    checked: &'a CheckedTrees,
    machine: SymbolHandle,
    coordinate: SourceCallCoordinate,
    mut has_home: impl FnMut(facts::PlaceRoot) -> bool + 'a,
) -> impl Iterator<Item = &'a facts::Fact> {
    let point = facts::ProgramPoint::CallEnsures {
        machine_symbol: machine,
        state_symbol: coordinate.state,
        statement_index: coordinate.statement_index,
        call_ordinal: coordinate.call_ordinal,
    };
    checked
        .facts
        .semantic
        .facts
        .at_point(point)
        .into_iter()
        .filter(move |fact| {
            fact.origin == facts::FactOrigin::CallEnsures
                && fact.point == point
                && fact
                    .place
                    .as_place()
                    .is_some_and(|place| has_home(checked.facts.semantic.places.get(place).root))
                && fact.payload.domain_membership().is_some()
        })
}

/// Independently reconstruct the complete selected domain, including every
/// predicate conjunct, before discharging its publication in declared storage.
/// Equal scalar widths or an unrelated restriction never establish this test.
pub(crate) fn declaration_represents(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    fact: &facts::Fact,
) -> Result<DeclarationRepresentation, LoweringError> {
    use DeclarationRepresentation::{Complete, RequiresPublication};
    use DeclarationRepresentationRefusal as Refusal;
    let program = &checked.typed;
    let facts::ProgramPoint::CallEnsures {
        state_symbol,
        statement_index,
        ..
    } = fact.point
    else {
        return Ok(RequiresPublication(Refusal::NotCallEnsures));
    };
    let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
        return Ok(RequiresPublication(Refusal::NotDomainMembership));
    };
    let Some(place) = fact.place.as_place() else {
        return Ok(RequiresPublication(Refusal::NoStoragePlace));
    };
    let place = checked.facts.semantic.places.get(place);
    let segments = checked
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments);
    let Some(root_type) = validation::canonical_place_type_reference(
        program,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    ) else {
        return Ok(RequiresPublication(Refusal::NoRootType));
    };
    let Some(reference) =
        validation::qualification_storage_type_reference(program, root_type, segments)
    else {
        return Ok(RequiresPublication(Refusal::NoLeafType));
    };
    let constraints =
        validation::field_domains::declared_domain_constraint_records(program, reference);
    let mut same_symbol = false;
    let mut same_instance = false;
    let mut selected = None;
    for constraint in constraints {
        same_symbol |= constraint.symbol == domain_symbol;
        same_instance |= constraint.semantic_id == semantic_domain;
        if constraint.symbol == domain_symbol
            && constraint.semantic_id == semantic_domain
            && selected.replace(constraint).is_some()
        {
            return Ok(RequiresPublication(Refusal::DuplicateDeclaredDomain));
        }
    }
    let Some(constraint) = selected else {
        return Ok(RequiresPublication(Refusal::NoExactDeclaredDomain {
            domain: domain_symbol,
            instance: semantic_domain,
            same_symbol,
            same_instance,
        }));
    };
    let Some(definition) = program.domain_definition_for_instance(domain_symbol, semantic_domain)
    else {
        return Ok(RequiresPublication(Refusal::NoDomainInstance));
    };
    if !validation::field_domains::domain_predicate_theory_is_complete(program, definition) {
        return Ok(RequiresPublication(Refusal::IncompletePredicateTheory));
    }
    if !definition.establishment_routes.is_empty() {
        return Ok(RequiresPublication(Refusal::EstablishmentRoute));
    }
    if !typed_trees::domain::required_subject_memberships(program, definition)
        .is_some_and(|required| required.is_empty())
    {
        return Ok(RequiresPublication(Refusal::RequiredSubjectMembership));
    }
    if checked
        .facts
        .qualifications
        .content
        .for_semantic_domain(semantic_domain)
        .is_some()
    {
        return Ok(RequiresPublication(Refusal::ContentCustody));
    }
    if let Some(predicate) =
        validation::field_domains::domain_byte_predicate(program, domain_symbol)
    {
        // Inline byte fields retain their complete predicate in the field
        // declaration. Reattaching that represented invariant as a nominal
        // projected membership would strengthen the earlier call operation,
        // although its public result correctly contains no such row. Whole
        // view roots and stored pointer crossings retain their ordinary
        // qualification publication; neither has this inline field contract.
        // Qualified fixed arrays keep their ordinary array storage; the
        // borrowed byte-slice carrier is a different presentation.
        if !matches!(segments.last(), Some(facts::PlaceSegment::Field { .. }))
            || !matches!(
                constraint.subject,
                typed_trees::types::DomainConstraintSubject::Declared
            )
            || !matches!(
                program.type_reference_table.type_reference(
                    validation::peel_declared_constraints(program, reference)
                ),
                typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
                    if checked.primitive_type_reference(*element_type)
                        == Some(typed_trees::types::PrimitiveType::U8)
            )
            || validation::stores_reference_shape(program, reference, &[])
        {
            return Ok(RequiresPublication(Refusal::NotDeclaredByteField));
        }
        let declared =
            typed_trees::byte_predicates::type_reference_domain_predicates(program, reference);
        if !declared.iter().all(|(_, predicate)| predicate.is_some()) {
            return Ok(RequiresPublication(Refusal::UnknownDeclaredBytePredicate));
        }
        return Ok(
            if declared
                .iter()
                .any(|(_, candidate)| *candidate == Some(predicate))
            {
                Complete
            } else {
                RequiresPublication(Refusal::MissingDeclaredBytePredicate)
            },
        );
    }
    let Some(primitive) = checked.primitive_type_reference(reference) else {
        return Ok(RequiresPublication(Refusal::NotScalarPredicate));
    };
    let Some(restriction) =
        crate::expression_preparation::qualifications::retained_scalar_restriction(
            checked, reference,
        )
    else {
        return Ok(RequiresPublication(Refusal::NoScalarRestriction));
    };
    let Some(predicate) = validation::scalar_domain_value_predicate(
        program,
        constraint,
        checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: primitive,
        },
    ) else {
        return Ok(RequiresPublication(Refusal::NoScalarDomainPredicate));
    };
    let required = crate::shared::structural_types::lower_field_type(
        identities,
        checked,
        &checked_trees::CheckedUnitStructuralFieldType::RefinedScalar {
            primitive,
            predicate,
        },
        &[],
    )?;
    let declared =
        crate::shared::structural_types::lower_field_type(identities, checked, restriction, &[])?;
    Ok(
        if terminal_semantics::scalar_field_entails(&declared, &required) {
            Complete
        } else {
            RequiresPublication(Refusal::ScalarPredicateNotEntailed)
        },
    )
}

/// The storage duties of this call's actual references. Select the exact
/// formation records once per invocation, before examining its returned facts.
pub(crate) struct CallSourceRequirements<'checked> {
    sources: Vec<(facts::PlaceRoot, &'checked [facts::PlaceSegment], usize)>,
}

impl<'checked> CallSourceRequirements<'checked> {
    pub(crate) fn prepare(
        checked: &'checked CheckedTrees,
        machine: SymbolHandle,
        coordinate: SourceCallCoordinate,
        formations: &ReferenceFormations,
    ) -> Self {
        let borrow = &checked.facts.borrow;
        let mut sources = Vec::new();
        if let Some(call) = borrow.call_at(
            machine,
            coordinate.state,
            coordinate.statement_index,
            coordinate.call_ordinal,
        ) {
            let call = borrow.calls.get(call);
            let retained = call
                .input_reference_loans
                .map_or(&[][..], |span| borrow.loan_suppliers.span_or_empty(span));
            for access in borrow.argument_accesses.span_or_empty(call.accesses) {
                if let Some(index) = formations.find(coordinate.statement_index, access.expression)
                {
                    sources.push((
                        access.root,
                        borrow.access_segments.span_or_empty(access.segments),
                        index,
                    ));
                }
            }
            for loan in retained {
                let loan = borrow.loans.get(*loan);
                if let Some(index) =
                    formations.find(loan.statement_index, loan.formation_expression)
                {
                    sources.push((
                        loan.root,
                        borrow.access_segments.span_or_empty(loan.segments),
                        index,
                    ));
                }
            }
        }
        Self { sources }
    }

    /// A fact already owed at the supplying endpoint needs no new qualification
    /// on its enclosing owner. Only recorded standing duties qualify here;
    /// additional input observations never become return promises.
    pub(crate) fn represents(
        &self,
        checked: &CheckedTrees,
        formations: &ReferenceFormations,
        fact: &facts::Fact,
    ) -> bool {
        let Some((_, _, _, domain)) = fact.payload.domain_membership() else {
            return false;
        };
        let Some(place) = fact.place.as_place() else {
            return false;
        };
        let place = checked.facts.semantic.places.get(place);
        let segments = checked
            .facts
            .semantic
            .place_segments
            .span_or_empty(place.segments);
        self.sources.iter().any(|(root, prefix, formation)| {
            if *root != place.root || !segments.starts_with(prefix) {
                return false;
            }
            let selected = &segments[prefix.len()..];
            formations
                .requirements(*formation)
                .iter()
                .any(|requirement| {
                    requirement.semantic_domain == domain
                        && requirement.path.len() == selected.len()
                        && requirement
                            .path
                            .iter()
                            .zip(selected)
                            .all(|(required, actual)| {
                                required == actual
                                    || (*required
                                        == validation::field_domains::WHOLE_ELEMENT_EXTENT
                                        && matches!(
                                            actual,
                                            facts::PlaceSegment::FixedIndex { .. }
                                                | facts::PlaceSegment::Index { .. }
                                        ))
                            })
                })
        })
    }
}
