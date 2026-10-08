//! Owned reference carriers preserve borrowed backing across moves.
//!
//! A descriptor's key names its current owner and record-field path; its value
//! names the original referent. Record construction relocates only the key.
//! Disposal removes descriptor subtrees, not the storage they permit access to.
//! Loan ancestry and disposal order are independently checked before execution.
use crate::custody::project_structural_value;
use crate::custody::resolve_structural_path_type;
use crate::errors::{TerminalInterpretError, malformed};
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

/// The reference descriptor owns this complete captured association. Moves
/// relocate its carrier key and retain both view and original primitive home;
/// ordinary reference disposition removes them together, never the payload.
#[derive(Clone)]
pub(crate) struct ReferenceReferent {
    pub(crate) view: TerminalStructuralValue,
    pub(crate) primitive_loan: Option<crate::primitive_storage::PrimitiveLoan>,
    pub(crate) byte_region: Option<ReferenceByteRegion>,
}

/// An aggregate view retains its captured byte endpoint and complete extent.
/// Projecting a child applies catalog geometry, never the formation expression.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ReferenceByteRegion {
    pub(crate) endpoint: crate::primitive_storage::PrimitiveStorageEndpoint,
    pub(crate) byte_length: usize,
}

/// Disposition ends carrier custody, never the captured primitive backing.
/// Ownership transfers deliberately keep using ordinary map removal instead.
pub(crate) fn discard_structural_value(
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    referents: &mut BTreeMap<StructuralRuntimePlace, ReferenceReferent>,
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

impl TerminalExecution<'_> {
    /// Borrowed ingress retains the caller's descriptor storage without an
    /// affine owner entry in this frame. Its declared authority bounds every
    /// reference crossing; owned carriers still require live affine custody.
    fn reference_carrier_access(
        &self,
        place: PlaceId,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
    ) -> Result<StructuralAccess, TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        if let Some(parameter) = machine
            .signature_structural_parameter(place)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
        {
            if parameter.structural_type != owner.structural_type
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
            {
                return Err(malformed());
            }
            return Ok(parameter.access);
        }
        // A partial owner retains authority over each live residual subtree.
        // Only the prefix before the first pointer crossing is carrier storage;
        // the suffix is governed by that descriptor's captured loan.
        let prefix = &path[..path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)
            .unwrap_or(path.len())];
        self.live_affine_frontier
            .iter()
            .any(|entry| {
                entry.place == place
                    && prefix.starts_with(&entry.path)
                    && (!entry.path.is_empty() || entry.structural_type == owner.structural_type)
            })
            .then_some(StructuralAccess::Owned)
            .ok_or_else(malformed)
    }

    fn reference_contract(
        &self,
        reference_type: StructuralTypeId,
    ) -> Result<(StructuralTypeId, StructuralAccess), TerminalInterpretError> {
        let Some(StructuralTypeShape::Reference { referent, access }) = self
            .program
            .structural_types()
            .get(&reference_type)
            .map(|declaration| &declaration.shape)
        else {
            // A reference operation naming no reference carrier is malformed,
            // not a valid operation awaiting an interpreter implementation.
            return Err(malformed());
        };
        if !self.program.structural_types().contains_key(referent)
            || !matches!(
                access,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            )
        {
            return Err(malformed());
        }
        Ok((*referent, *access))
    }

    pub(crate) fn establish_reference(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let (referent_type, access) = self.reference_contract(result.structural_type)?;
        // Reference-source provenance ends with a borrowed-leaf marker even
        // for non-reference storage. Only a real descriptor has pointer bytes
        // to cross; formation at ordinary storage captures that exact place.
        let mut normalized_source = source.clone();
        if let Some((StructuralPathSegment::Referent, prefix)) = source.path.split_last()
            && terminal_semantics::runtime_structural_path_tip(
                self.program.structural_types().values(),
                self.live_structural_type(source.place)?,
                prefix,
            )
            .is_some_and(|tip| !matches!(tip.shape, StructuralTypeShape::Reference { .. }))
        {
            normalized_source.path.pop();
        }
        let source = &normalized_source;
        // Both local storage and ingress borrows already name their backing.
        // An explicit Referent projection reborrows that backing through a live
        // carrier; it never establishes ownership of the carrier's contents.
        let mut identity_cursor = self.local_structural_identities.cursor();
        let mut byte_region = None;
        let mut region_referent = None;
        let primitive_loan = if matches!(
            self.program
                .structural_types()
                .get(&referent_type)
                .ok_or_else(malformed)?
                .shape,
            StructuralTypeShape::PrimitiveScalar(_)
        ) {
            // Retain the actual projected carrier, including shared total views
            // into differently represented primitives. The reference contract
            // below independently decides whether its declared view is valid.
            let root_type = self.live_structural_type(source.place)?;
            let original_type = if let Some(tip) = terminal_semantics::runtime_structural_path_tip(
                self.program.structural_types().values(),
                root_type,
                &source.path,
            ) {
                tip.id
            } else {
                self.resolve_live_projection(source.place, &source.path, source.access)?
                    .structural_type
            };
            let parameter = StructuralParameterDeclaration {
                place: source.place,
                position: 0,
                structural_type: original_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: source.access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            let mut loan = self.capture_primitive_loan(&parameter, source, &mut identity_cursor)?;
            if let Some(storage) = storage {
                let stored_scalar = storage.stored_scalar.ok_or_else(malformed)?;
                let crate::values::TerminalScalarValue::Integer {
                    scalar_type: offset_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte_offset),
                } = self.scalar_value(storage.byte_offset)?
                else {
                    return Err(malformed());
                };
                let StructuralTypeShape::PrimitiveScalar(logical_scalar) = self
                    .program
                    .structural_types()
                    .get(&referent_type)
                    .ok_or_else(malformed)?
                    .shape
                else {
                    return Err(malformed());
                };
                if offset_type.is_address()
                    || offset_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
                    || !terminal_semantics::reference_storage_source_compatible(
                        self.program.structural_types().values(),
                        original_type,
                        result.structural_type,
                        Some(stored_scalar),
                    )
                {
                    return Err(malformed());
                }
                self.apply_reference_storage_view(
                    &mut loan,
                    usize::try_from(byte_offset).map_err(|_| malformed())?,
                    stored_scalar,
                    logical_scalar,
                )?;
                loan.referent.structural_type = referent_type;
            }
            loan.access = access;
            Some(loan)
        } else {
            if let Some(storage) = storage {
                if storage.stored_scalar.is_some() {
                    return Err(malformed());
                }
                let crate::values::TerminalScalarValue::Integer {
                    scalar_type: offset_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte_offset),
                } = self.scalar_value(storage.byte_offset)?
                else {
                    return Err(malformed());
                };
                let original_type = terminal_semantics::runtime_structural_path_tip(
                    self.program.structural_types().values(),
                    self.live_structural_type(source.place)?,
                    &source.path,
                )
                .ok_or_else(malformed)?
                .id;
                if offset_type.is_address()
                    || offset_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
                    || !terminal_semantics::reference_storage_source_compatible(
                        self.program.structural_types().values(),
                        original_type,
                        result.structural_type,
                        None,
                    )
                {
                    return Err(malformed());
                }
                let byte_length = usize::try_from(
                    terminal_semantics::reference_storage_size(
                        self.program.structural_types().values(),
                        referent_type,
                    )
                    .ok_or(
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "reference aggregate has no closed byte geometry",
                        ),
                    )?,
                )
                .map_err(|_| malformed())?;
                let mut view =
                    self.resolve_live_projection(source.place, &source.path, source.access)?;
                let byte_offset = usize::try_from(byte_offset).map_err(|_| malformed())?;
                let region = if let Some(region) = self.reference_byte_region(&view)? {
                    if byte_offset
                        .checked_add(byte_length)
                        .is_none_or(|end| end > region.byte_length)
                    {
                        return Err(malformed());
                    }
                    ReferenceByteRegion {
                        endpoint: crate::primitive_storage::shifted_byte_endpoint(
                            &region.endpoint,
                            byte_offset,
                        )?,
                        byte_length,
                    }
                } else {
                    let parameter = StructuralParameterDeclaration {
                        place: source.place,
                        position: 0,
                        structural_type: original_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: source.access,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    };
                    let mut loan =
                        self.capture_primitive_loan(&parameter, source, &mut identity_cursor)?;
                    self.position_reference_storage(&mut loan, byte_offset, byte_length)?;
                    view = loan.referent;
                    ReferenceByteRegion {
                        endpoint: loan.storage.endpoint,
                        byte_length,
                    }
                };
                view.structural_type = referent_type;
                byte_region = Some(region);
                region_referent = Some(view);
            }
            None
        };
        let referent = match &primitive_loan {
            Some(loan) => loan.referent.clone(),
            None if region_referent.is_some() => region_referent.ok_or_else(malformed)?,
            None => self
                .resolve_reference_call_arguments(std::slice::from_ref(source))?
                .pop()
                .ok_or_else(malformed)?,
        };
        if !access_covers(source.access, access)
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !terminal_semantics::reference_source_compatible(
                self.program.structural_types().values(),
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
            return Err(malformed());
        }
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        let carrier = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        self.reference_referents.insert(
            StructuralRuntimePlace::from(&carrier),
            ReferenceReferent {
                view: referent,
                primitive_loan,
                byte_region,
            },
        );
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
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let owner = self.structural_values.get(&source).ok_or_else(malformed)?;
        let carrier = project_structural_value(&self.program.structural_types(), owner, path)?;
        self.reference_contract(carrier.structural_type)?;
        let key = StructuralRuntimePlace::from(&carrier);
        if operation.result != OperationResult::Unit || !self.reference_referents.contains_key(&key)
        {
            return Err(malformed());
        }
        // Release consumes the descriptor's ordinary owned projection, leaving
        // exactly the same affine complement as transfer. It never removes the
        // referent or the scalar storage of surviving siblings.
        let mut frontier = self.live_affine_frontier.clone();
        self.consume_affine_projection(&mut frontier, &StructuralArgument {
                place: source,
                path: path.to_vec(),
                access: StructuralAccess::Owned,
            })?;
        self.reference_referents.remove(&key);
        self.live_affine_frontier = frontier;
        if path.is_empty() {
            self.structural_values.remove(&source);
        }
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

    pub(crate) fn resolve_live_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let owner = self.structural_values.get(&place).ok_or_else(malformed)?;
        // An element selected through a live element view (`&mut cells[2]`
        // for `cells: &mut [Cell]`) lives in the view's original collection:
        // the window offsets the index, and the view's own access bounds the
        // requested authority. The descriptor never owns its elements.
        if let Some(view) = self.element_view_values.get(&place)
            && matches!(path.first(), Some(StructuralPathSegment::FixedIndex(_)))
        {
            if !view.matches_descriptor_home(owner) || !access_covers(view.access(), access) {
                return Err(malformed());
            }
            let (collection, selected) =
                view.structural_projection(path, access != StructuralAccess::SharedBorrow)?;
            return self.resolve_reference_path(&collection, &selected, access);
        }
        if path.contains(&StructuralPathSegment::Referent) {
            let authority = self.reference_carrier_access(place, owner, path)?;
            if !access_covers(authority, access) {
                return Err(malformed());
            }
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
        let mut value = owner.clone();
        let mut remaining = path;
        loop {
            let boundary = remaining
                .iter()
                .position(|segment| *segment == StructuralPathSegment::Referent);
            let prefix = &remaining[..boundary.unwrap_or(remaining.len())];
            // Keep case selection and its payload field together. The ordinary
            // projection owner strips declaration-only case steps from storage.
            value = project_structural_value(&self.program.structural_types(), &value, prefix)?;
            if let Some(boundary) = boundary {
                let (referent_type, authority) = self.reference_contract(value.structural_type)?;
                if !access_covers(authority, access) {
                    return Err(malformed());
                }
                let reference_type = value.structural_type;
                value = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&value))
                    .map(|referent| referent.view.clone())
                    .ok_or_else(malformed)?;
                if !terminal_semantics::reference_source_compatible(
                    self.program.structural_types().values(),
                    value.structural_type,
                    reference_type,
                ) {
                    return Err(malformed());
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
                .structural_types()
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
    ) -> Result<Vec<Vec<TerminalStructuralValue>>, TerminalInterpretError> {
        signature
            .reference_sources
            .iter()
            .map(|origin| {
                if origin.sources.is_empty() {
                    return Err(malformed());
                }
                let leaf_type = resolve_structural_path_type(
                    &self.program.structural_types(),
                    signature.structural_type,
                    &origin.path,
                )?;
                let (referent_type, access) = self.reference_contract(leaf_type)?;
                origin
                    .sources
                    .iter()
                    .map(|source| {
                        let parameter = parameters
                            .iter()
                            .find(|parameter| parameter.place == source.place)
                            .ok_or_else(malformed)?;
                        if !access_covers(parameter.access, source.access)
                            || !access_covers(source.access, access)
                        {
                            return Err(malformed());
                        }
                        let expected = self.resolve_reference_path(
                            arguments.get(&parameter.place).ok_or_else(malformed)?,
                            &source.path,
                            source.access,
                        )?;
                        if expected.structural_type != referent_type
                            || !expected.qualifications.is_empty()
                        {
                            return Err(malformed());
                        }
                        // Capture every admitted alternative before the callee moves
                        // or repacks ingress storage. These are backing identities,
                        // not another owner and not a choice of the runtime address.
                        Ok(expected)
                    })
                    .collect()
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
                    .structural_types()
                    .get(&signature.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Reference { .. })
            ) {
                Err(malformed())
            } else {
                Ok(())
            };
        }
        if self.call_stack.is_empty() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference results require an explicit loan carrier interface",
            ));
        }
        // The returned place owns its whole logical carrier. A projected
        // transfer may retain that carrier at a nonempty backing path under
        // its original runtime identity; moving it does not require copying
        // descriptor storage. Count only this exact stored subtree, then
        // compare every captured referent with the admitted ingress below.
        if self
            .reference_referents
            .keys()
            .filter(|key| {
                key.opaque_identity == carrier.opaque_identity
                    && key.path.starts_with(&carrier.path)
            })
            .count()
            != signature.reference_sources.len()
        {
            return Err(malformed());
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
            return Err(malformed());
        };
        if expected_reference_backings.len() != signature.reference_sources.len() {
            return Err(malformed());
        }
        // The verifier reconstructs the complete typed result roster. Runtime
        // preflight compares each captured backing against the callee's actual
        // bound ingress before the return charge. The ordinary return then moves
        // this same owner identity; its descriptor subtree is never discarded.
        for (ordinal, origin) in signature.reference_sources.iter().enumerate() {
            if signature.reference_sources[..ordinal]
                .iter()
                .any(|previous| previous.path == origin.path)
            {
                return Err(malformed());
            }
            let leaf_type = resolve_structural_path_type(
                &self.program.structural_types(),
                signature.structural_type,
                &origin.path,
            )?;
            let (referent_type, access) = self.reference_contract(leaf_type)?;
            let expected = &expected_reference_backings[ordinal];
            let mut key = StructuralRuntimePlace::from(carrier);
            key.path.extend_from_slice(&origin.path);
            let actual = self.reference_referents.get(&key).ok_or_else(malformed)?;
            if origin.sources.is_empty()
                || origin
                    .sources
                    .iter()
                    .any(|source| !access_covers(source.access, access))
                || expected.len() != origin.sources.len()
                || expected
                    .iter()
                    .any(|backing| backing.structural_type != referent_type)
                || !expected.iter().any(|backing| backing == &actual.view)
            {
                return Err(malformed());
            }
        }
        Ok(())
    }
}

/// Attenuation of existing storage authority, not proof of a loan's formation
/// or lifetime. Write-only authority never supplies readable access.
pub(crate) fn access_covers(available: StructuralAccess, requested: StructuralAccess) -> bool {
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
