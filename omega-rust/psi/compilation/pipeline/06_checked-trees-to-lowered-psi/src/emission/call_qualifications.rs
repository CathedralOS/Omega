//! Normal-call memberships have one publication inventory. Complete scalar and
//! byte-field invariants belong to their exact receiving declaration; additional
//! nominal memberships require a catalog entry and an occurrence publication.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::shared::lowering_error::LoweringError;

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
) -> Result<bool, LoweringError> {
    let facts::ProgramPoint::CallEnsures {
        state_symbol,
        statement_index,
        ..
    } = fact.point
    else {
        return Ok(false);
    };
    let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
        return Ok(false);
    };
    let Some(place) = fact.place.as_place() else {
        return Ok(false);
    };
    let place = checked.facts.semantic.places.get(place);
    let segments = checked
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments);
    let Some(reference) = validation::canonical_place_type_reference(
        &checked.typed,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: segments.to_vec(),
        },
    ) else {
        return Ok(false);
    };
    let constraints =
        validation::field_domains::declared_domain_constraint_records(&checked.typed, reference);
    let mut selected = constraints.into_iter().filter(|constraint| {
        constraint.symbol == domain_symbol && constraint.semantic_id == semantic_domain
    });
    let Some(constraint) = selected.next() else {
        return Ok(false);
    };
    if selected.next().is_some() {
        return Ok(false);
    }
    let Some(definition) = checked
        .typed
        .domain_definition_for_instance(domain_symbol, semantic_domain)
    else {
        return Ok(false);
    };
    if !validation::field_domains::domain_predicate_theory_is_complete(&checked.typed, definition)
        || !definition.establishment_routes.is_empty()
        || !typed_trees::domain::required_subject_memberships(&checked.typed, definition)
            .is_some_and(|required| required.is_empty())
        || checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(semantic_domain)
            .is_some()
    {
        return Ok(false);
    }
    if let Some(predicate) =
        validation::field_domains::domain_byte_predicate(&checked.typed, domain_symbol)
    {
        // Inline byte fields retain their complete predicate in the field
        // declaration. Reattaching that represented invariant as a nominal
        // projected membership would strengthen the earlier call operation,
        // although its public result correctly contains no such row. Whole
        // view roots and stored pointer crossings retain their ordinary
        // qualification publication; neither has this inline field contract.
        if !matches!(segments.last(), Some(facts::PlaceSegment::Field { .. }))
            || !matches!(
                constraint.subject,
                typed_trees::types::DomainConstraintSubject::Declared
            )
            || validation::byte_sequence_carrier(&checked.typed, reference, &[]).is_none()
            || validation::stores_reference_shape(&checked.typed, reference, &[])
        {
            return Ok(false);
        }
        let declared = typed_trees::byte_predicates::type_reference_domain_predicates(
            &checked.typed,
            reference,
        );
        return Ok(declared.iter().all(|(_, predicate)| predicate.is_some())
            && declared
                .iter()
                .any(|(_, candidate)| *candidate == Some(predicate)));
    }
    let Some(primitive) = checked.primitive_type_reference(reference) else {
        return Ok(false);
    };
    let Some(restriction) =
        crate::expression_preparation::qualifications::retained_scalar_restriction(
            checked, reference,
        )
    else {
        return Ok(false);
    };
    let Some(predicate) = validation::scalar_domain_value_predicate(
        &checked.typed,
        constraint,
        checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: primitive,
        },
    ) else {
        return Ok(false);
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
    Ok(terminal_semantics::scalar_field_entails(
        &declared, &required,
    ))
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
        formations: &[crate::emission::operation_emission::buffer::SourceReferenceRequirements],
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
            for (index, formation) in formations
                .iter()
                .enumerate()
                .filter(|(_, formation)| formation.state == coordinate.state)
            {
                if formation.statement_index == coordinate.statement_index {
                    for access in borrow.argument_accesses.span_or_empty(call.accesses) {
                        if access.expression == formation.expression {
                            sources.push((
                                access.root,
                                borrow.access_segments.span_or_empty(access.segments),
                                index,
                            ));
                        }
                    }
                }
                for loan in retained {
                    let loan = borrow.loans.get(*loan);
                    if loan.statement_index == formation.statement_index
                        && loan.formation_expression == formation.expression
                    {
                        sources.push((
                            loan.root,
                            borrow.access_segments.span_or_empty(loan.segments),
                            index,
                        ));
                    }
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
        formations: &[crate::emission::operation_emission::buffer::SourceReferenceRequirements],
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
            formations[*formation]
                .requirements
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
