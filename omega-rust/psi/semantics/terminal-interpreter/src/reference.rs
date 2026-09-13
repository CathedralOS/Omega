//! Owned reference carriers preserve borrowed primitive backing across moves.
//!
//! A descriptor's key names its current owner and record-field path; its value
//! names the original referent. Record construction relocates only the key.
//! Disposal removes descriptor subtrees, not the storage they permit access to.
//! Loan ancestry and disposal order are independently checked before execution.

use super::*;
use terminal_psi::{Operation, StructuralResultDeclaration};

/// Disposition ends carrier custody, never the captured primitive backing.
/// Ownership transfers deliberately keep using ordinary map removal instead.
pub(super) fn discard_structural_value(
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    referents: &mut BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    place: PlaceId,
) -> Option<TerminalStructuralValue> {
    let value = values.remove(&place)?;
    // Carrier paths belong to the discarded owner. Captured backing values
    // are map payloads, not descendants to dispose along with that owner.
    referents.retain(|carrier, _| {
        carrier.opaque_identity != value.opaque_identity || !carrier.path.starts_with(&value.path)
    });
    Some(value)
}

impl TerminalExecution {
    fn mutable_primitive_referent(
        &self,
        reference_type: StructuralTypeId,
    ) -> Result<StructuralTypeId, TerminalInterpretError> {
        let Some(StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow,
        }) = self
            .structural_types
            .get(&reference_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "reference execution requires a mutable primitive referent",
            ));
        };
        if !self
            .structural_types
            .get(referent)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
            })
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "reference execution requires a mutable primitive referent",
            ));
        }
        Ok(*referent)
    }

    pub(super) fn establish_reference(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let referent_type = self.mutable_primitive_referent(result.structural_type)?;
        // Both primitive locals and ingress borrows already name their backing.
        // An explicit Referent projection reborrows that backing through a live
        // carrier; it never establishes ownership of the carrier's contents.
        let referent = self
            .resolve_reference_call_arguments(std::slice::from_ref(source))?
            .pop()
            .ok_or_else(invalid)?;
        if source.access != StructuralAccess::MutableBorrow
            || (!source.path.is_empty()
                && source.path.last() != Some(&StructuralPathSegment::Referent))
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || referent.structural_type != referent_type
            || !referent.qualifications.is_empty()
            || !referent.path.is_empty()
            || !self
                .structural_primitive_storage
                .contains_key(&StructuralRuntimePlace::from(&referent))
            || self.structural_values.contains_key(&result.place)
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == result.place)
        {
            return Err(invalid());
        }
        let carrier = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        self.reference_referents
            .insert(StructuralRuntimePlace::from(&carrier), referent);
        self.structural_values.insert(result.place, carrier);
        self.live_affine_frontier.insert(StructuralAffineDiscard {
            place: result.place,
            path: Vec::new(),
            structural_type: result.structural_type,
        });
        Ok(())
    }

    pub(super) fn release_reference(
        &mut self,
        operation: &Operation,
        source: PlaceId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let carrier = self.structural_values.get(&source).ok_or_else(invalid)?;
        self.mutable_primitive_referent(carrier.structural_type)?;
        let key = StructuralRuntimePlace::from(carrier);
        let frontier = StructuralAffineDiscard {
            place: source,
            path: Vec::new(),
            structural_type: carrier.structural_type,
        };
        if operation.result != OperationResult::Unit
            || !carrier.path.is_empty()
            || !self.reference_referents.contains_key(&key)
            || !self.live_affine_frontier.contains(&frontier)
        {
            return Err(invalid());
        }
        discard_structural_value(
            &mut self.structural_values,
            &mut self.reference_referents,
            source,
        );
        self.live_affine_frontier.remove(&frontier);
        Ok(())
    }

    pub(super) fn resolve_reference_call_arguments(
        &self,
        arguments: &[StructuralArgument],
    ) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
        if arguments
            .iter()
            .all(|argument| !argument.path.contains(&StructuralPathSegment::Referent))
        {
            return resolve_structural_arguments(
                &self.structural_types,
                &self.structural_values,
                arguments,
            );
        }
        arguments
            .iter()
            .map(|argument| {
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    argument.path.split_last()
                else {
                    let mut resolved = resolve_structural_arguments(
                        &self.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(argument),
                    )?;
                    return resolved
                        .pop()
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed);
                };
                let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
                let owner = self
                    .structural_values
                    .get(&argument.place)
                    .ok_or_else(invalid)?;
                // Resolve the carrier's owned field path first. Only the final
                // Referent crosses into captured storage; it is never an offset
                // within the record or permission to move that storage.
                let carrier = resolve_structural_arguments(
                    &self.structural_types,
                    &self.structural_values,
                    &[StructuralArgument {
                        place: argument.place,
                        path: carrier_path.to_vec(),
                        access: StructuralAccess::Owned,
                    }],
                )?
                .pop()
                .ok_or_else(invalid)?;
                let referent_type = self.mutable_primitive_referent(carrier.structural_type)?;
                let referent = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&carrier))
                    .ok_or_else(invalid)?;
                if argument.access == StructuralAccess::Owned
                    || referent.structural_type != referent_type
                    || !self
                        .live_affine_frontier
                        .contains(&StructuralAffineDiscard {
                            place: argument.place,
                            path: Vec::new(),
                            structural_type: owner.structural_type,
                        })
                {
                    return Err(invalid());
                }
                Ok(referent.clone())
            })
            .collect()
    }

    pub(super) fn validate_reference_return(
        &self,
        signature: &StructuralResultDeclaration,
        carrier: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        if !matches!(
            self.structural_types
                .get(&signature.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Reference { .. })
        ) {
            return if signature.reference_sources.is_empty() {
                Ok(())
            } else {
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            };
        }
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let referent_type = self.mutable_primitive_referent(signature.structural_type)?;
        if self.call_stack.is_empty() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference results require an explicit loan carrier interface",
            ));
        }
        let [origin] = signature.reference_sources.as_slice() else {
            return Err(invalid());
        };
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| {
                parameter.place == origin.source.place
                    && parameter.structural_type == referent_type
                    && parameter.access == StructuralAccess::MutableBorrow
            })
            .ok_or_else(invalid)?;
        let expected = self
            .structural_values
            .get(&parameter.place)
            .ok_or_else(invalid)?;
        let actual = self
            .reference_referents
            .get(&StructuralRuntimePlace::from(carrier))
            .ok_or_else(invalid)?;
        if !origin.path.is_empty()
            || !origin.source.path.is_empty()
            || origin.source.access != StructuralAccess::MutableBorrow
            || !carrier.path.is_empty()
            || expected != actual
        {
            return Err(invalid());
        }
        Ok(())
    }
}
