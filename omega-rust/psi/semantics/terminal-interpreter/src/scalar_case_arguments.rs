//! Ordinary calls carry an established tag and payload, not an opaque value
//! fabricated from its sum type. Preparation snapshots only the selected actual;
//! the existing call transaction commits its owned transfer after preflight.
//! Shared lending is bounded to complete scalar-only sums: this runtime has no
//! mutable case binding or scalar-case store. Neither the callee nor an admitted
//! alias can change this payload during the call. This is not permission to
//! snapshot arbitrary shared structural backing.

use super::*;

impl TerminalExecution {
    pub(super) fn prepare_scalar_case_argument(
        &self,
        callee: &ExecutableMachine,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<TerminalScalarCaseValue, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let value = self
            .scalar_case_values
            .get(&argument.place)
            .ok_or_else(invalid)?;
        if !argument.path.is_empty()
            || argument.access != parameter.access
            || !matches!(
                argument.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || value.structural_type != parameter.structural_type
            || self.structural_values.contains_key(&argument.place)
            || self.scalar_array_values.contains_key(&argument.place)
            || self.byte_sequence_values.contains_key(&argument.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
            || callee
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
            || callee
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(invalid());
        }
        let source = self.owned_block_source(argument.place);
        let exact_source = source.is_some_and(|source| {
            source.structural_type == parameter.structural_type
                && source.qualifications.is_empty()
                && source.projected_qualifications.is_empty()
                && if parameter.access == StructuralAccess::Owned {
                    source.access == StructuralAccess::Owned
                        && source.multiplicity == parameter.multiplicity
                } else {
                    source.access != StructuralAccess::WriteOnlyBorrow
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                }
        }) || if parameter.access == StructuralAccess::Owned {
            self.owned_result_matches(argument.place, parameter, true)
        } else {
            let mut owned = parameter.clone();
            owned.access = StructuralAccess::Owned;
            [
                StructuralMultiplicity::Unrestricted,
                StructuralMultiplicity::Affine,
            ]
            .iter()
            .any(|multiplicity| {
                owned.multiplicity = *multiplicity;
                self.owned_result_matches(argument.place, &owned, true)
            }) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        };
        if !exact_source
            || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
        {
            return Err(invalid());
        }
        if parameter.access == StructuralAccess::Owned
            && parameter.multiplicity == StructuralMultiplicity::Affine
        {
            let root = StructuralAffineDiscard {
                place: argument.place,
                path: Vec::new(),
                structural_type: parameter.structural_type,
            };
            // Reject a stale/partial owner before the call cursor or caller
            // frame changes. Commit will consume this exact root once.
            if self
                .live_affine_frontier
                .iter()
                .filter(|entry| entry.place == argument.place)
                .ne(std::iter::once(&root))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Sum { cases },
            ..
        }) = self.structural_types.get(&value.structural_type)
        else {
            return Err(invalid());
        };
        if !cases.iter().all(|case| {
            case.fields.iter().all(|field| {
                field.relevance == terminal_psi::BindingRelevance::Relevant
                    && field.field_type.scalar_type().is_some()
            })
        }) {
            return Err(invalid());
        }
        let selected = cases
            .iter()
            .find(|case| case.id == value.result_case)
            .ok_or_else(invalid)?;
        if selected.fields.len() != value.fields.len()
            || selected
                .fields
                .iter()
                .zip(&value.fields)
                .any(|(field, (identity, value))| {
                    field.id != *identity
                        || field.field_type.scalar_type() != Some(value.scalar_type())
                })
        {
            return Err(invalid());
        }
        Ok(value.clone())
    }

    pub(super) fn consume_affine_call_argument(
        &self,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        argument: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(value) = self.scalar_case_values.get(&argument.place) {
            let root = StructuralAffineDiscard {
                place: argument.place,
                path: Vec::new(),
                structural_type: value.structural_type,
            };
            if !argument.path.is_empty() || !frontier.remove(&root) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            Ok(())
        } else {
            consume_affine_projection(
                &self.structural_types,
                &self.structural_values,
                frontier,
                argument,
            )
        }
    }
}

pub(super) fn bind_call_affine_frontier(
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    cases: &BTreeMap<PlaceId, TerminalScalarCaseValue>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    bind_affine_frontier_types(parameters, |place| {
        match (values.get(&place), cases.get(&place)) {
            (Some(value), None) => Some(value.structural_type),
            (None, Some(value)) => Some(value.structural_type),
            _ => None,
        }
    })
}
