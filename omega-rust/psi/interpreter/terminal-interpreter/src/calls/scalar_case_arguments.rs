//! Ordinary calls carry an established tag and payload, not an opaque value
//! fabricated from its sum type. Preparation snapshots only the selected actual;
//! the existing call transaction commits its owned transfer after preflight.
//! Only the active case chooses its carrier: another variant's structural
//! payload does not turn an established scalar payload into opaque storage.
//! Shared lending remains read-only: this runtime has no mutable packed-case
//! binding or store. Neither the callee nor an admitted alias can change this
//! payload during the call. Structural and shared atomic payloads require
//! their original backing rather than these scalar snapshots.

use crate::custody::bind_affine_frontier_types;
use crate::custody::consume_affine_projection;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{TerminalScalarCaseValue, TerminalStructuralValue};
use semantic_vocabulary::PlaceId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralAffineDiscard;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralFieldType;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralTypeShape;
use terminal_psi::TerminalMachine;

impl TerminalExecution<'_> {
    pub(crate) fn prepare_scalar_case_argument(
        &self,
        callee: &TerminalMachine,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<TerminalScalarCaseValue, TerminalInterpretError> {
        let value = self
            .scalar_case_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
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
            return Err(malformed());
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
            return Err(malformed());
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
        // Validate the selected contents and their declaration predicates,
        // not every inactive variant's hypothetical storage. The prepared
        // carrier partition retains the actual scalar payload through the call.
        let declaration = self
            .program
            .structural_types()
            .get(&value.structural_type)
            .ok_or_else(malformed)?;
        value.validate_against(declaration)?;
        if parameter.access == StructuralAccess::SharedBorrow
            && let StructuralTypeShape::Sum { cases } = &declaration.shape
            && cases
                .iter()
                .find(|case| case.id == value.result_case)
                .is_some_and(|case| {
                    case.fields
                        .iter()
                        .any(|field| matches!(field.field_type, StructuralFieldType::Atomic(_)))
                })
        {
            // Atomic events may mutate through a shared loan. A matching word
            // is not its original cell identity, even when no tag changes.
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "shared atomic case arguments require original cell backing",
            ));
        }
        Ok(value.clone())
    }

    pub(crate) fn consume_affine_call_argument(
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
                &self.program.structural_types(),
                &self.structural_values,
                &self.structural_cases,
                frontier,
                argument,
            )
        }
    }
}

pub(crate) fn bind_call_affine_frontier(
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
