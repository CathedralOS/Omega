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

    pub(super) fn bind_reference_return_backings(
        &self,
        parameters: &[StructuralParameterDeclaration],
        signature: &StructuralResultDeclaration,
        arguments: &BTreeMap<PlaceId, TerminalStructuralValue>,
    ) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        signature
            .reference_sources
            .iter()
            .map(|origin| {
                if origin.source.access != StructuralAccess::MutableBorrow {
                    return Err(invalid());
                }
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.place == origin.source.place)
                    .ok_or_else(invalid)?;
                let leaf_type = resolve_structural_path_type(
                    &self.structural_types,
                    signature.structural_type,
                    &origin.path,
                )?;
                let referent_type = self.mutable_primitive_referent(leaf_type)?;
                let expected = if origin.source.path.is_empty() {
                    if parameter.access != StructuralAccess::MutableBorrow
                        || parameter.structural_type != referent_type
                    {
                        return Err(invalid());
                    }
                    arguments.get(&parameter.place).ok_or_else(invalid)?
                } else {
                    let Some((StructuralPathSegment::Referent, carrier_path)) =
                        origin.source.path.split_last()
                    else {
                        return Err(invalid());
                    };
                    if parameter.access != StructuralAccess::Owned
                        || parameter.multiplicity != StructuralMultiplicity::Affine
                        || carrier_path.is_empty()
                        || carrier_path
                            .iter()
                            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                    {
                        return Err(invalid());
                    }
                    let carrier = resolve_structural_arguments(
                        &self.structural_types,
                        arguments,
                        &[StructuralArgument {
                            place: parameter.place,
                            path: carrier_path.to_vec(),
                            access: StructuralAccess::Owned,
                        }],
                    )?
                    .pop()
                    .ok_or_else(invalid)?;
                    if self.mutable_primitive_referent(carrier.structural_type)? != referent_type {
                        return Err(invalid());
                    }
                    self.reference_referents
                        .get(&StructuralRuntimePlace::from(&carrier))
                        .ok_or_else(invalid)?
                };
                if expected.structural_type != referent_type
                    || !expected.qualifications.is_empty()
                    || !expected.path.is_empty()
                {
                    return Err(invalid());
                }
                // Retain only the contract's expected backing, not another
                // owner or lineage. The callee may move or repack its ingress
                // carrier before returning, deleting its original binding.
                Ok(expected.clone())
            })
            .collect()
    }

    pub(super) fn validate_reference_return(
        &self,
        signature: &StructuralResultDeclaration,
        carrier: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        if signature.reference_sources.is_empty() {
            return if matches!(
                self.structural_types
                    .get(&signature.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Reference { .. })
            ) {
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            } else {
                Ok(())
            };
        }
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if self.call_stack.is_empty() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference results require an explicit loan carrier interface",
            ));
        }
        if !carrier.path.is_empty()
            || self
                .reference_referents
                .keys()
                .filter(|key| {
                    key.opaque_identity == carrier.opaque_identity
                        && key.path.starts_with(&carrier.path)
                })
                .count()
                != signature.reference_sources.len()
        {
            return Err(invalid());
        }
        let Some(SuspendedCall {
            result:
                SuspendedCallResult::Structural {
                    expected_reference_backings,
                    ..
                },
            ..
        }) = self.call_stack.last()
        else {
            return Err(invalid());
        };
        if expected_reference_backings.len() != signature.reference_sources.len() {
            return Err(invalid());
        }
        // The verifier reconstructs the complete typed result roster. Runtime
        // preflight compares each captured backing against the callee's actual
        // bound ingress before the return charge. The ordinary return then moves
        // this same owner identity; its descriptor subtree is never discarded.
        for (ordinal, origin) in signature.reference_sources.iter().enumerate() {
            if signature.reference_sources[..ordinal]
                .iter()
                .any(|previous| previous.path == origin.path)
                || origin
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                || origin.source.access != StructuralAccess::MutableBorrow
            {
                return Err(invalid());
            }
            let leaf_type = resolve_structural_path_type(
                &self.structural_types,
                signature.structural_type,
                &origin.path,
            )?;
            let referent_type = self.mutable_primitive_referent(leaf_type)?;
            let expected = &expected_reference_backings[ordinal];
            let mut key = StructuralRuntimePlace::from(carrier);
            key.path.extend_from_slice(&origin.path);
            let actual = self.reference_referents.get(&key).ok_or_else(invalid)?;
            if expected.structural_type != referent_type || expected != actual {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
