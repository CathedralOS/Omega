//! Owned reference carriers preserve borrowed backing across moves.
//!
//! A descriptor's key names its current owner and record-field path; its value
//! names the original referent. Record construction relocates only the key.
//! Disposal removes descriptor subtrees, not the storage they permit access to.
//! Loan ancestry and disposal order are independently checked before execution.
use crate::custody::project_structural_value;
use crate::custody::resolve_structural_path_type;
use crate::errors::TerminalInterpretError;
use crate::execution::SuspendedCall;
use crate::execution::SuspendedCallResult;
use crate::execution::TerminalExecution;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::OperationResult;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralAffineDiscard;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;
use terminal_psi::{Operation, StructuralResultDeclaration};

/// Disposition ends carrier custody, never the captured primitive backing.
/// Ownership transfers deliberately keep using ordinary map removal instead.
pub(crate) fn discard_structural_value(
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    referents: &mut BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    element_views: &mut BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    place: PlaceId,
) -> Option<TerminalStructuralValue> {
    let value = values.remove(&place)?;
    // Carrier paths belong to the discarded owner. Captured backing values
    // are map payloads, not descendants to dispose along with that owner.
    referents.retain(|carrier, _| {
        carrier.opaque_identity != value.opaque_identity || !carrier.path.starts_with(&value.path)
    });
    element_views.retain(|carrier, _| {
        carrier.opaque_identity != value.opaque_identity || !carrier.path.starts_with(&value.path)
    });
    Some(value)
}

impl TerminalExecution {
    fn reference_contract(
        &self,
        reference_type: StructuralTypeId,
    ) -> Result<(StructuralTypeId, StructuralAccess), TerminalInterpretError> {
        let Some(StructuralTypeShape::Reference { referent, access }) = self
            .program
            .structural_types
            .get(&reference_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "reference execution requires a reference carrier",
            ));
        };
        if !self.program.structural_types.contains_key(referent)
            || !matches!(
                access,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            )
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok((*referent, *access))
    }

    pub(crate) fn establish_reference(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let (_, access) = self.reference_contract(result.structural_type)?;
        // Both local storage and ingress borrows already name their backing.
        // An explicit Referent projection reborrows that backing through a live
        // carrier; it never establishes ownership of the carrier's contents.
        let referent = self
            .resolve_reference_call_arguments(std::slice::from_ref(source))?
            .pop()
            .ok_or_else(invalid)?;
        if !access_covers(source.access, access)
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !terminal_semantics::reference_source_compatible(
                self.program.structural_types.values(),
                referent.structural_type,
                result.structural_type,
            )
            || !referent.qualifications.is_empty()
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

    pub(crate) fn release_reference(
        &mut self,
        operation: &Operation,
        source: PlaceId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let carrier = self.structural_values.get(&source).ok_or_else(invalid)?;
        self.reference_contract(carrier.structural_type)?;
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
            &mut self.structural_element_view_fields,
            source,
        );
        self.live_affine_frontier.remove(&frontier);
        Ok(())
    }

    pub(crate) fn resolve_reference_call_arguments(
        &self,
        arguments: &[StructuralArgument],
    ) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
        arguments
            .iter()
            .map(|argument| {
                self.resolve_live_projection(argument.place, &argument.path, argument.access)
            })
            .collect()
    }

    fn resolve_live_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let owner = self.structural_values.get(&place).ok_or_else(invalid)?;
        if path.contains(&StructuralPathSegment::Referent)
            && !self
                .live_affine_frontier
                .contains(&StructuralAffineDiscard {
                    place,
                    path: Vec::new(),
                    structural_type: owner.structural_type,
                })
        {
            return Err(invalid());
        }
        self.resolve_reference_path(owner, path, access)
    }

    /// Project storage, crossing each reference boundary through its captured
    /// backing. A suffix belongs to that backing, not to the descriptor object.
    fn resolve_reference_path(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let mut value = owner.clone();
        let mut remaining = path;
        loop {
            let boundary = remaining
                .iter()
                .position(|segment| *segment == StructuralPathSegment::Referent);
            let prefix = &remaining[..boundary.unwrap_or(remaining.len())];
            // Keep case selection and its payload field together. The ordinary
            // projection owner strips declaration-only case steps from storage.
            value = project_structural_value(&self.program.structural_types, &value, prefix)?;
            if let Some(boundary) = boundary {
                let (referent_type, authority) = self.reference_contract(value.structural_type)?;
                if !access_covers(authority, access) {
                    return Err(invalid());
                }
                let reference_type = value.structural_type;
                value = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&value))
                    .cloned()
                    .ok_or_else(invalid)?;
                if !terminal_semantics::reference_source_compatible(
                    self.program.structural_types.values(),
                    value.structural_type,
                    reference_type,
                ) {
                    return Err(invalid());
                }
                // Only the view carries the observed type. The captured backing
                // and its scalar payload retain their original declaration.
                value.structural_type = referent_type;
                remaining = &remaining[boundary + 1..];
            } else {
                return Ok(value);
            }
        }
    }

    /// Field observations address the referent of a reference-valued root.
    /// Ordinary roots already name their storage. Both preserve the original
    /// runtime identity and projection rather than copying record contents.
    pub(crate) fn observation_root(
        &self,
        source: PlaceId,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let value = self.structural_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        if !matches!(
            self.program
                .structural_types
                .get(&value.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Reference { .. })
        ) {
            return Ok(value.clone());
        }
        self.resolve_live_projection(
            source,
            &[StructuralPathSegment::Referent],
            StructuralAccess::SharedBorrow,
        )
    }

    pub(crate) fn bind_reference_return_backings(
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
                let source = origin.sole_source().ok_or(
                    TerminalInterpretError::UnsupportedSemanticVariant(
                        "selected reference returns require alternative backing transport",
                    ),
                )?;
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.place == source.place)
                    .ok_or_else(invalid)?;
                let leaf_type = resolve_structural_path_type(
                    &self.program.structural_types,
                    signature.structural_type,
                    &origin.path,
                )?;
                let (referent_type, access) = self.reference_contract(leaf_type)?;
                if !access_covers(parameter.access, source.access)
                    || !access_covers(source.access, access)
                {
                    return Err(invalid());
                }
                let expected = self.resolve_reference_path(
                    arguments.get(&parameter.place).ok_or_else(invalid)?,
                    &source.path,
                    source.access,
                )?;
                if expected.structural_type != referent_type || !expected.qualifications.is_empty()
                {
                    return Err(invalid());
                }
                // Retain only the contract's expected backing, not another
                // owner or lineage. The callee may move or repack its ingress
                // carrier before returning, deleting its original binding.
                Ok(expected)
            })
            .collect()
    }

    pub(crate) fn validate_reference_return(
        &self,
        signature: &StructuralResultDeclaration,
        carrier: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        if signature.reference_sources.is_empty() {
            return if matches!(
                self.program
                    .structural_types
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
            let source =
                origin
                    .sole_source()
                    .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                        "selected reference returns require alternative backing transport",
                    ))?;
            if signature.reference_sources[..ordinal]
                .iter()
                .any(|previous| previous.path == origin.path)
            {
                return Err(invalid());
            }
            let leaf_type = resolve_structural_path_type(
                &self.program.structural_types,
                signature.structural_type,
                &origin.path,
            )?;
            let (referent_type, access) = self.reference_contract(leaf_type)?;
            let expected = &expected_reference_backings[ordinal];
            let mut key = StructuralRuntimePlace::from(carrier);
            key.path.extend_from_slice(&origin.path);
            let actual = self.reference_referents.get(&key).ok_or_else(invalid)?;
            if !access_covers(source.access, access)
                || expected.structural_type != referent_type
                || expected != actual
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn access_covers(available: StructuralAccess, requested: StructuralAccess) -> bool {
    available == requested
        || available == StructuralAccess::Owned
        || matches!(
            (available, requested),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
            )
        )
}

#[cfg(test)]
mod tests;
