//! Owned reference carriers preserve borrowed backing across moves.
//!
//! A descriptor's key names its current owner and record-field path; its value
//! names the original referent. Record construction relocates only the key.
//! Disposal removes descriptor subtrees, not the storage they permit access to.
//! Loan ancestry and disposal order are independently checked before execution.
//! Projection crosses both references and element windows through that captured
//! backing. A descriptor path identifies its home; only its association supplies
//! the next storage root and, for windows, the selected index offset.
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
    /// Explicit placed interpretation retains its initialized original RAM.
    pub(crate) placed_ram: Option<PlacedRamReference>,
    pub(crate) view: TerminalStructuralValue,
    pub(crate) primitive_loan: Option<crate::primitive_storage::PrimitiveLoan>,
    pub(crate) byte_owner: Option<crate::byte_sequences::binding::ByteSequenceBinding>,
    pub(crate) byte_region: Option<ReferenceByteRegion>,
    /// A typed re-view of aggregate storage with identical geometry: `view`
    /// keeps the backing under its own declaration, and this names the
    /// declaration and byte window the reference observes it as. Logical
    /// nesting does not need to match the supplying declaration.
    pub(crate) aggregate_view: Option<AggregateViewRegion>,
}

/// Where a path written against an aggregate view ends: an aggregate of both
/// declarations, or a scalar leaf with its view and backing carriers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateViewTip {
    Aggregate {
        view: AggregateViewRegion,
        backing: StructuralTypeId,
    },
    Scalar {
        view: terminal_semantics::ReferenceStorageProjection,
        backing: terminal_semantics::ReferenceStorageProjection,
    },
}

/// One resolved endpoint in its actual backing declaration. A byte snapshot
/// also needs the final field's storage identity, not its rendered name or an
/// identity looked up against the original (possibly reference-valued) root.
pub(crate) struct ResolvedStructuralProjection {
    placed_ram: Option<PlacedRamReference>,
    pub(crate) value: TerminalStructuralValue,
    view_tip: Option<AggregateViewTip>,
    endpoint_field: Option<semantic_vocabulary::StructuralFieldId>,
    primitive_storage: Option<crate::primitive_storage::PrimitiveStorage>,
}

/// A returned reference must retain the exact admitted source association,
/// including a re-view's window. Equal opaque roots alone do not authenticate
/// two distinct subregions of one aggregate.
#[derive(Clone)]
pub(crate) struct ReferenceBacking {
    placed_ram: Option<PlacedRamReference>,
    pub(crate) value: TerminalStructuralValue,
    aggregate_view: Option<AggregateViewRegion>,
    primitive_storage: Option<crate::primitive_storage::PrimitiveStorage>,
    suffix: Vec<terminal_psi::ReferenceOriginProjection>,
    extent: semantic_vocabulary::ReferenceOriginExtent,
    element_view: Option<crate::element_views::ElementView>,
    access: StructuralAccess,
}

mod aggregate_views;
mod origin_backings;
pub(crate) mod placed_ram;
pub(crate) use aggregate_views::{
    AggregateBacking, AggregateViewRegion, backing_scalar_path, translate_aggregate_view_field,
    translate_aggregate_view_path,
};
pub(crate) use placed_ram::PlacedRamReference;

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
    /// Borrowed entry and state arrivals retain descriptor storage without an
    /// affine owner entry in this frame. Its declared authority bounds every
    /// reference crossing. Owned carrier availability follows its declared
    /// multiplicity: affine storage has a disposal frontier; linear storage
    /// remains resident under its exact live claim.
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
            .structural_parameter(place)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
        {
            if parameter.structural_type != owner.structural_type
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
            {
                return Err(malformed());
            }
            return Ok(parameter.access);
        }
        // Unrestricted owned storage has no affine-disposal frontier. Its
        // live value and exact producing/receiving declaration authorize the
        // carrier observation; each crossed descriptor still bounds access
        // to its captured referent below. Do not invent an affine obligation
        // just because this value contains a shared reference.
        let unrestricted = machine
            .structural_parameter(place)
            .map(|parameter| (parameter.structural_type, parameter.multiplicity))
            .or_else(|| {
                let (operation, structural_type) = machine.producing_operation(place)?;
                let result = operation.result.structural()?;
                (result.place == place && result.structural_type == structural_type)
                    .then_some((structural_type, result.multiplicity))
            })
            .is_some_and(|(structural_type, multiplicity)| {
                structural_type == owner.structural_type
                    && multiplicity == StructuralMultiplicity::Unrestricted
            });
        if unrestricted {
            return Ok(StructuralAccess::Owned);
        }
        // A partial owner retains authority over each live residual subtree.
        // Only the prefix before the first pointer crossing is carrier storage;
        // the suffix is governed by that descriptor's captured loan.
        // Linear regions are excluded from the affine disposal frontier, not
        // from observation. Their live claim authenticates the same prefix;
        // neither roster permits reading a detached or consumed sibling.
        let prefix = &path[..path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)
            .unwrap_or(path.len())];
        let resident = self.live_affine_frontier.iter().any(|entry| {
            entry.place == place
                && prefix.starts_with(&entry.path)
                && (!entry.path.is_empty() || entry.structural_type == owner.structural_type)
        }) || self.live_claims.values().any(|claim| {
            claim.place == Some(place)
                && claim.multiplicity == Some(StructuralMultiplicity::Linear)
                && prefix.starts_with(&claim.path)
        });
        resident
            .then_some(StructuralAccess::Owned)
            .ok_or_else(malformed)
    }

    /// A descriptor observation borrows its exact carrier, without consuming
    /// its home. Borrowed ingress bounds access; owned carrier storage must
    /// retain the selected residual subtree. Unrestricted completed descriptors
    /// may be read but never acquire exclusive authority from their contents.
    pub(crate) fn validate_descriptor_borrow(
        &self,
        source: &StructuralArgument,
        requested: StructuralAccess,
    ) -> Result<(), TerminalInterpretError> {
        if !matches!(
            requested,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || !access_covers(source.access, requested)
        {
            return Err(malformed());
        }
        let root = self
            .structural_values
            .get(&source.place)
            .ok_or_else(malformed)?;
        let machine = self.program.verified_machine(self.current_machine)?;
        if let Some(parameter) = self.owned_block_source(source.place) {
            if parameter.structural_type != root.structural_type {
                return Err(malformed());
            }
            if parameter.access != StructuralAccess::Owned {
                return (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && access_covers(parameter.access, requested))
                .then_some(())
                .ok_or_else(malformed);
            }
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && requested == StructuralAccess::SharedBorrow
            {
                return Ok(());
            }
        } else if requested == StructuralAccess::SharedBorrow {
            let unrestricted =
                machine
                    .structural_place(source.place)
                    .is_some_and(|place| match place.kind {
                        semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                            ..
                        } => true,
                        semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            ..
                        } => machine
                            .operation(producer)
                            .and_then(|operation| operation.result.structural())
                            .is_some_and(|result| {
                                result.multiplicity == StructuralMultiplicity::Unrestricted
                            }),
                        _ => false,
                    });
            if unrestricted {
                return Ok(());
            }
        }
        self.reference_carrier_access(source.place, root, &source.path)?;
        Ok(())
    }

    /// A byte selected inside a byte view or bounded owner has no catalog
    /// child of its own; it names the unique declared `u8` carrier, the same
    /// endpoint judgment primitive loans of inline bytes use.
    fn inline_byte_source_type(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let byte = StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .ok()?,
        ));
        let types = self.program.structural_types();
        if !matches!(
            types.get(&root)?.shape,
            StructuralTypeShape::ByteSequence(_)
        ) || !matches!(path, [StructuralPathSegment::FixedIndex(_)])
        {
            return None;
        }
        let mut matching = types
            .values()
            .filter(|declaration| declaration.shape == byte)
            .map(|declaration| declaration.id);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
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
        // A descriptor retains the loan's access; it need not permit reads.
        // Individual observations and stores still check that authority.
        if !self.program.structural_types().contains_key(referent)
            || *access == StructuralAccess::Owned
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
        let mut normalized_source = source.clone();
        normalized_source.path = terminal_semantics::reference_source_storage_path(
            self.program.structural_types().values(),
            self.live_structural_type(source.place)?,
            &source.path,
        )
        .to_vec();
        let source = &normalized_source;
        // Both local storage and ingress borrows already name their backing.
        // An explicit Referent projection reborrows that backing through a live
        // carrier; it never establishes ownership of the carrier's contents.
        let mut identity_cursor = self.local_structural_identities.cursor();
        let mut byte_region = None;
        let mut region_referent = None;
        let mut aggregate_view = None;
        let field_compatible = storage.is_none()
            && terminal_semantics::reference_byte_field_compatible(
                self.program.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                self.live_structural_type(source.place)?,
                &source.path,
                result.structural_type,
                &result.projected_qualifications,
            );
        // Inline owners keep the ordinary call binding's parent, case namespace
        // and field identity. Capturing a reference must not mint an owned copy
        // or pretend that the field has an independent nominal storage cell.
        let byte_owner_loan = if field_compatible {
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &[],
                    &result.projected_qualifications,
                    &[StructuralPathSegment::Referent],
                );
            let parameter = StructuralParameterDeclaration {
                place: source.place,
                position: 0,
                structural_type: referent_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access,
                qualifications,
                projected_qualifications,
            };
            Some(
                self.prepare_byte_sequence_loan(
                    &parameter,
                    source,
                    &mut identity_cursor,
                    &mut BTreeMap::new(),
                )?
                .ok_or_else(malformed)?,
            )
        } else {
            None
        };
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
            let observed = (source.access == StructuralAccess::SharedBorrow)
                .then(|| {
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        self.program.structural_types().values(),
                        root_type,
                        &source.path,
                        None,
                    )
                })
                .flatten();
            let original_type = if let Some(observed) = observed {
                observed
            } else if let Some(tip) = terminal_semantics::runtime_structural_path_tip(
                self.program.structural_types().values(),
                root_type,
                &source.path,
            ) {
                tip.id
            } else if let Some(byte) = self.inline_byte_source_type(root_type, &source.path) {
                byte
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
            if storage.is_none() && byte_owner_loan.is_none() {
                let (backing, tip) =
                    self.resolve_live_view_projection(source.place, &source.path, source.access)?;
                if let Some(AggregateViewTip::Aggregate {
                    view,
                    backing: actual,
                }) = tip
                {
                    if view.structural_type != referent_type || actual != backing.structural_type {
                        return Err(malformed());
                    }
                    // Reborrowing an ingress re-view keeps the supplying
                    // declaration, just as crossing a stored descriptor does.
                    aggregate_view = Some(view);
                    region_referent = Some(backing);
                }
            }
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
                let root_type = self.live_structural_type(source.place)?;
                let tip = terminal_semantics::runtime_structural_path_tip(
                    self.program.structural_types().values(),
                    root_type,
                    &source.path,
                )
                .map(|tip| tip.id);
                // A byte inside a byte view has no structural projection; the
                // loan below names the view's own backing byte.
                let inline_byte = match tip {
                    Some(_) => None,
                    None => self.inline_byte_source_type(root_type, &source.path),
                };
                let original_type = tip.or(inline_byte).ok_or_else(malformed)?;
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
                let byte_endpoint = matches!(
                    self.program
                        .structural_types()
                        .get(&original_type)
                        .ok_or_else(malformed)?
                        .shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                );
                // Primitive capture owns the actual byte endpoint across
                // scalar arrays, stored arrays and borrowed views. Its retained
                // referent is also the region lookup coordinate; requiring an
                // opaque structural row first loses initialized array storage.
                let byte_loan = if byte_endpoint {
                    let parameter = StructuralParameterDeclaration {
                        place: source.place,
                        position: 0,
                        structural_type: original_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: source.access,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    };
                    Some(self.capture_primitive_loan(&parameter, source, &mut identity_cursor)?)
                } else {
                    None
                };
                let (projected, projected_view) = match &byte_loan {
                    Some(loan) => (loan.referent.clone(), None),
                    None => self.resolve_live_view_projection(
                        source.place,
                        &source.path,
                        source.access,
                    )?,
                };
                let byte_offset = usize::try_from(byte_offset).map_err(|_| malformed())?;
                let captured = self.reference_byte_region(&projected)?;
                if captured.is_none() && !byte_endpoint {
                    // Aggregate storage with identical geometry keeps its own
                    // typed cells: the view names that whole backing under
                    // its declaration, and crossings retain the selected window.
                    let source_length = terminal_semantics::reference_storage_size(
                        self.program.structural_types().values(),
                        original_type,
                    )
                    .and_then(|length| usize::try_from(length).ok());
                    if byte_offset != 0 || source_length != Some(byte_length) {
                        return Err(malformed());
                    }
                    let mut region =
                        AggregateViewRegion::whole(&self.program.structural_types(), referent_type)
                            .ok_or_else(malformed)?;
                    if let Some(AggregateViewTip::Aggregate { view, backing }) = projected_view {
                        if backing != projected.structural_type
                            || view.byte_length != region.byte_length
                        {
                            return Err(malformed());
                        }
                        region.byte_offset = view.byte_offset;
                    }
                    aggregate_view = Some(region);
                    region_referent = Some(projected);
                } else {
                    let (mut view, region) = if let Some(region) = captured {
                        if byte_offset
                            .checked_add(byte_length)
                            .is_none_or(|end| end > region.byte_length)
                        {
                            return Err(malformed());
                        }
                        (
                            projected,
                            ReferenceByteRegion {
                                endpoint: crate::primitive_storage::shifted_byte_endpoint(
                                    &region.endpoint,
                                    byte_offset,
                                )?,
                                byte_length,
                            },
                        )
                    } else {
                        let mut loan = byte_loan.ok_or_else(malformed)?;
                        self.position_reference_storage(&mut loan, byte_offset, byte_length)?;
                        (
                            loan.referent,
                            ReferenceByteRegion {
                                endpoint: loan.storage.endpoint,
                                byte_length,
                            },
                        )
                    };
                    view.structural_type = referent_type;
                    byte_region = Some(region);
                    region_referent = Some(view);
                }
            }
            None
        };
        let mut placed_ram = None;
        let referent = match &primitive_loan {
            Some(loan) => loan.referent.clone(),
            None if byte_owner_loan.is_some() => {
                byte_owner_loan.as_ref().ok_or_else(malformed)?.0.clone()
            }
            None if region_referent.is_some() => region_referent.ok_or_else(malformed)?,
            None => {
                let (view, captured) = self.resolve_live_placed_projection(source)?;
                placed_ram = captured;
                view
            }
        };
        if let Some(ram) = &mut placed_ram {
            self.validate_placed_ram_home(ram)?;
            if !access_covers(ram.access, access) {
                return Err(malformed());
            }
            ram.access = access;
        }
        let mut captured_memberships = Vec::new();
        terminal_psi::StructuralPathQualification::append_under(
            &referent.qualifications,
            &referent.projected_qualifications,
            &[StructuralPathSegment::Referent],
            &mut captured_memberships,
        );
        if !access_covers(source.access, access)
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || result.projected_qualifications.iter().any(|row| !captured_memberships.contains(row))
            || (access != StructuralAccess::SharedBorrow
                && captured_memberships.iter().any(|row| !result.projected_qualifications.contains(row)))
            || !result.claims.is_empty()
            // The storage judgment above admitted an aggregate re-view.
            || (aggregate_view.is_none() && !field_compatible
                && !terminal_semantics::reference_source_compatible(
                    self.program.structural_types().values(),
                    referent.structural_type,
                    result.structural_type,
                ))
            || self.structural_values.contains_key(&result.place)
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        let mut carrier = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        carrier.projected_qualifications = result.projected_qualifications.clone();
        self.reference_referents.insert(
            StructuralRuntimePlace::from(&carrier),
            ReferenceReferent {
                view: placed_ram
                    .as_ref()
                    .map_or(referent, |ram| ram.source.clone()),
                placed_ram,
                primitive_loan,
                byte_owner: byte_owner_loan.map(|(_, binding)| binding),
                byte_region,
                aggregate_view,
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
        if let Some(ram) = self
            .reference_referents
            .get(&key)
            .and_then(|reference| reference.placed_ram.as_ref())
        {
            self.validate_placed_ram_home(ram)?;
        }
        // Release consumes the descriptor's ordinary owned projection, leaving
        // exactly the same affine complement as transfer. It never removes the
        // referent or the scalar storage of surviving siblings.
        let mut frontier = self.live_affine_frontier.clone();
        self.consume_affine_projection(
            &mut frontier,
            &StructuralArgument {
                place: source,
                path: path.to_vec(),
                access: StructuralAccess::Owned,
            },
        )?;
        self.reference_referents.remove(&key);
        self.live_affine_frontier = frontier;
        if path.is_empty() {
            self.structural_values.remove(&source);
        }
        Ok(())
    }

    /// Rejoin the storage association retained by an inline-owner capture.
    /// The projected value supplies identity; equal carrier types alone never
    /// select a field, and moving the descriptor does not move its backing.
    pub(crate) fn captured_byte_owner_binding(
        &self,
        selected: &TerminalStructuralValue,
    ) -> Option<&crate::byte_sequences::binding::ByteSequenceBinding> {
        self.reference_referents.values().find_map(|capture| {
            (StructuralRuntimePlace::from(&capture.view) == StructuralRuntimePlace::from(selected)
                && capture.view.structural_type == selected.structural_type)
                .then_some(capture.byte_owner.as_ref())
                .flatten()
        })
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

    pub(crate) fn resolve_live_placed_projection(
        &self,
        argument: &StructuralArgument,
    ) -> Result<(TerminalStructuralValue, Option<PlacedRamReference>), TerminalInterpretError> {
        let projected = self.resolve_live_projection_with_leaf_type(
            argument.place,
            &argument.path,
            argument.access,
            terminal_psi::StructuralFieldType::canonical_leaf_type,
        )?;
        Ok((projected.value, projected.placed_ram))
    }

    pub(crate) fn resolve_live_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        self.resolve_live_view_projection(place, path, access)
            .map(|(value, _)| value)
    }

    /// A scalar projection presents its logical loan type while preserving the
    /// already selected backing endpoint and any composed storage codec.
    pub(crate) fn resolve_live_primitive_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        expected: StructuralTypeId,
    ) -> Result<
        (
            TerminalStructuralValue,
            Option<crate::primitive_storage::PrimitiveStorage>,
        ),
        TerminalInterpretError,
    > {
        let mut projected = self.resolve_live_projection_with_leaf_type(
            place,
            path,
            access,
            self.borrowed_projection_leaf(expected, access),
        )?;
        // A fixed-array formal's loan has no descriptor crossing in its typed
        // path. Retain that frame home for primitive leaves, just as a dynamic
        // window retains it below. Record fields already have their own cell
        // namespace and must not become canonical primitive-array storage.
        if projected.primitive_storage.is_none()
            && projected.endpoint_field.is_none()
            && let Some(loan) = self.element_view_values.get(&place)
        {
            projected.primitive_storage = Some(
                loan.scalar_storage(self, path, access != StructuralAccess::SharedBorrow)?
                    .0,
            );
        }
        Ok((projected.value, projected.primitive_storage))
    }

    /// Present the exact selected declaration at borrowed ingress while
    /// retaining its supplying storage namespace. Calls and state edges use
    /// the same projection; neither may turn a re-view into an owned copy.
    pub(crate) fn present_structural_argument(
        &self,
        argument: &StructuralArgument,
        expected: StructuralTypeId,
    ) -> Result<(TerminalStructuralValue, Option<AggregateBacking>), TerminalInterpretError> {
        let projected = self.resolve_live_projection_with_leaf_type(
            argument.place,
            &argument.path,
            argument.access,
            self.borrowed_projection_leaf(expected, argument.access),
        )?;
        let (mut value, tip) = (projected.value, projected.view_tip);
        let backing = if let Some(AggregateViewTip::Aggregate { view, backing }) = tip {
            if argument.access == StructuralAccess::Owned
                || expected != view.structural_type
                || value.structural_type != backing
            {
                return Err(malformed());
            }
            let backing = AggregateBacking {
                value: value.clone(),
                region: view,
            };
            value.structural_type = view.structural_type;
            Some(backing)
        } else {
            None
        };
        Ok((value, backing))
    }

    /// The receiving declaration selects a loan presentation, not its backing.
    /// Keep an exact restricted referee exact; a bare scalar formal presents
    /// only the carrier while Psi owns the original field's handback predicate.
    fn borrowed_projection_leaf(
        &self,
        expected: StructuralTypeId,
        access: StructuralAccess,
    ) -> fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType> {
        let preserve_restriction = self
            .program
            .structural_types()
            .get(&expected)
            .is_some_and(|declaration| declaration.scalar_restriction.is_some());
        if preserve_restriction || access == StructuralAccess::Owned {
            terminal_psi::StructuralFieldType::canonical_leaf_type
        } else if access == StructuralAccess::SharedBorrow {
            |field| field.borrowed_presentation_type(StructuralAccess::SharedBorrow, None)
        } else {
            |field| field.borrowed_presentation_type(StructuralAccess::MutableBorrow, None)
        }
    }

    /// Resolve a live projection and, when its final run crosses into an
    /// aggregate re-view, where that run ends in both declarations.
    pub(crate) fn resolve_live_view_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<(TerminalStructuralValue, Option<AggregateViewTip>), TerminalInterpretError> {
        self.resolve_live_projection_with_leaf_type(
            place,
            path,
            access,
            if access == StructuralAccess::SharedBorrow {
                terminal_psi::StructuralFieldType::shared_observation_type
            } else {
                terminal_psi::StructuralFieldType::canonical_leaf_type
            },
        )
        .map(|projection| (projection.value, projection.view_tip))
    }

    /// A snapshot observes the same live backing and reference authority as
    /// other reads. Only its final inline field uses the copy presentation;
    /// intermediate descriptors retain their original storage contracts.
    pub(crate) fn resolve_live_leaf_copy(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<
        (
            TerminalStructuralValue,
            Option<semantic_vocabulary::StructuralFieldId>,
        ),
        TerminalInterpretError,
    > {
        self.resolve_live_projection_with_leaf_type(
            place,
            path,
            StructuralAccess::SharedBorrow,
            terminal_psi::StructuralFieldType::leaf_copy_type,
        )
        .map(|projection| (projection.value, projection.endpoint_field))
    }

    fn resolve_live_projection_with_leaf_type(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        leaf_type: fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>,
    ) -> Result<ResolvedStructuralProjection, TerminalInterpretError> {
        let owner = self.structural_values.get(&place).ok_or_else(malformed)?;
        self.resolve_storage_projection_with_leaf_type(place, owner, path, access, leaf_type)
    }

    /// Project an already selected storage home. A packed array transaction
    /// stages its identity before publishing it, so resolving its cells must
    /// not rediscover that identity through the placed-value map. Descriptor
    /// crossings and aggregate re-views still use the same authority checks.
    pub(crate) fn resolve_storage_projection_with_leaf_type(
        &self,
        place: PlaceId,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        leaf_type: fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>,
    ) -> Result<ResolvedStructuralProjection, TerminalInterpretError> {
        if path.contains(&StructuralPathSegment::Referent) {
            let authority = self.reference_carrier_access(place, owner, path)?;
            if !access_covers(authority, access) {
                return Err(malformed());
            }
        }
        let (backing, view) = match self.aggregate_argument_backings.get(&place) {
            Some(backing) => (&backing.value, Some(backing.region)),
            None => (owner, None),
        };
        self.resolve_reference_view_path_with_leaf_type(
            backing,
            path,
            Some(access),
            leaf_type,
            view,
            None,
        )?
        .ok_or_else(malformed)
    }

    /// Project like `resolve_reference_path`, and also name where a final
    /// run written against an aggregate re-view ends in both declarations.
    /// The returned value then carries the backing's declaration. A contrary
    /// stored case guard is an absent candidate, not a descriptor to read.
    pub(crate) fn resolve_reference_view_path(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Option<(TerminalStructuralValue, Option<AggregateViewTip>)>, TerminalInterpretError>
    {
        self.resolve_reference_view_path_with_leaf_type(
            owner,
            path,
            Some(access),
            if access == StructuralAccess::SharedBorrow {
                terminal_psi::StructuralFieldType::shared_observation_type
            } else {
                terminal_psi::StructuralFieldType::canonical_leaf_type
            },
            None,
            None,
        )
        .map(|selected| selected.map(|projection| (projection.value, projection.view_tip)))
    }

    /// Discriminator validation inspects runtime custody, not a source read.
    /// Follow the same retained reference/view backing as operand resolution,
    /// without requiring readable payload authority from a write-only operation.
    /// The operation's consumer still checks its actual access independently.
    pub(crate) fn resolve_case_guard_carrier(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let owner = self.structural_values.get(&place).ok_or_else(malformed)?;
        let (backing, view) = match self.aggregate_argument_backings.get(&place) {
            Some(backing) => (&backing.value, Some(backing.region)),
            None => (owner, None),
        };
        self.resolve_reference_view_path_with_leaf_type(
            backing,
            path,
            None,
            terminal_psi::StructuralFieldType::canonical_leaf_type,
            view,
            None,
        )?
        .map(|projection| projection.value)
        .ok_or_else(malformed)
    }

    fn resolve_reference_view_path_with_leaf_type(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: Option<StructuralAccess>,
        leaf_type: fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>,
        initial_view: Option<AggregateViewRegion>,
        initial_placed: Option<PlacedRamReference>,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        let types = self.program.structural_types();
        let mut value = owner.clone();
        let mut remaining = path;
        let mut aggregate_view = initial_view;
        let mut primitive_storage = None;
        let mut collection_home = None;
        let mut placed_ram = initial_placed.or_else(|| {
            self.placed_view_occurrences
                .values()
                .find(|occurrence| occurrence.view == StructuralRuntimePlace::from(owner))
                .and_then(|occurrence| occurrence.ram.clone())
        });
        loop {
            // Inline geometry ends at each descriptor. Resolving the whole
            // suffix statically would either invent an extent or address the
            // descriptor itself instead of its captured backing.
            let mut boundary = None;
            for (position, segment) in remaining.iter().enumerate() {
                if *segment == StructuralPathSegment::Referent {
                    boundary = Some(position);
                    break;
                }
                if matches!(segment, StructuralPathSegment::FixedIndex(_)) {
                    let prefix_type = resolve_structural_path_type(
                        &types,
                        aggregate_view.map_or(value.structural_type, |view| view.structural_type),
                        &remaining[..position],
                    )?;
                    if matches!(
                        types.get(&prefix_type).ok_or_else(malformed)?.shape,
                        StructuralTypeShape::ElementView { .. }
                    ) {
                        boundary = Some(position);
                        break;
                    }
                }
            }
            let prefix = &remaining[..boundary.unwrap_or(remaining.len())];
            let logical_prefix = prefix;
            // Origin recipes may introduce a new presentation before any
            // descriptor crossing. Start its geometry in the retained backing,
            // then let the existing window translator compose the whole run.
            // Executable operands cannot carry these origin-only steps.
            let active_view = match aggregate_view.take() {
                Some(view) => Some(view),
                None if prefix.iter().any(|segment| {
                    matches!(segment, StructuralPathSegment::RepresentationView { .. })
                }) =>
                {
                    Some(
                        AggregateViewRegion::whole(&types, value.structural_type)
                            .ok_or_else(malformed)?,
                    )
                }
                None => None,
            };
            let translated;
            let mut tip = None;
            let prefix = match active_view {
                Some(view) => {
                    let (path, view_tip) =
                        translate_aggregate_view_path(&types, view, value.structural_type, prefix)
                            .ok_or_else(malformed)?;
                    translated = path;
                    tip = Some(view_tip);
                    translated.as_slice()
                }
                None => prefix,
            };
            // Keep case selection and its payload field together. The ordinary
            // projection owner strips declaration-only case steps from storage.
            if !crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                &types,
                &value,
                &self.structural_cases,
                prefix,
            )? {
                return Ok(None);
            }
            let endpoint_field = boundary
                .is_none()
                .then(|| {
                    crate::custody::inline_structural_field(&types, value.structural_type, prefix)
                        .map(|field| field.id)
                })
                .flatten();
            // An aggregate re-view is a window in this backing root, not an
            // eagerly paired child. Keep its coordinate for subsequent reads,
            // reborrows and ingress; scalar endpoints have a real backing path.
            value = if let Some(AggregateViewTip::Scalar { .. }) = tip {
                let view = active_view.ok_or_else(malformed)?;
                let (storage, _, _) = self.aggregate_view_primitive_access(
                    &value,
                    AggregateViewTip::Aggregate {
                        view,
                        backing: value.structural_type,
                    },
                    logical_prefix,
                )?;
                primitive_storage = Some(storage);
                // This is a logical loan descriptor, not a relabelled cell.
                // The actual endpoint and codec above still name the source.
                let structural_type = if let Some(field) = crate::custody::inline_structural_field(
                    &types,
                    view.structural_type,
                    logical_prefix,
                ) {
                    leaf_type(&field.field_type)
                        .and_then(|leaf| leaf.leaf_declaration(types.values()))
                        .map(|declaration| declaration.id)
                } else {
                    terminal_semantics::runtime_structural_path_tip(
                        types.values(),
                        view.structural_type,
                        logical_prefix,
                    )
                    .map(|declaration| declaration.id)
                }
                .ok_or_else(malformed)?;
                (value.qualifications, value.projected_qualifications) =
                    terminal_psi::StructuralPathQualification::select(
                        &value.qualifications,
                        &value.projected_qualifications,
                        prefix,
                    );
                value.structural_type = structural_type;
                value.path.extend_from_slice(prefix);
                value
            } else if matches!(tip, Some(AggregateViewTip::Aggregate { .. })) {
                value
            } else if boundary.is_none() {
                crate::custody::project_structural_value_with_leaf(
                    &types, &value, prefix, leaf_type,
                )?
            } else if access == Some(StructuralAccess::SharedBorrow) {
                crate::custody::project_shared_structural_observation(&types, &value, prefix)?
            } else {
                project_structural_value(&types, &value, prefix)?
            };
            if let Some(captured) = &mut placed_ram {
                self.project_placed_ram_presentation(captured, prefix, &value)?;
            }
            if let Some(boundary) = boundary {
                if matches!(remaining[boundary], StructuralPathSegment::FixedIndex(_)) {
                    let view = self.element_view_at_resolved_home(&value)?;
                    if access.is_some_and(|required| !access_covers(view.access(), required)) {
                        return Err(malformed());
                    }
                    let (collection, selected) = view.structural_projection(
                        &types,
                        &remaining[boundary..boundary + 1],
                        access.is_some_and(|required| required != StructuralAccess::SharedBorrow),
                    )?;
                    // Consume this selector once in the actual collection.
                    // The suffix remains in its observed element declaration;
                    // a re-view must not become an untyped backing path.
                    aggregate_view = match tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => {
                            let StructuralTypeShape::ElementView { element } = types
                                .get(&view.structural_type)
                                .ok_or_else(malformed)?
                                .shape
                            else {
                                return Err(malformed());
                            };
                            Some(
                                AggregateViewRegion::whole(&types, element)
                                    .ok_or_else(malformed)?,
                            )
                        }
                        None => None,
                        Some(AggregateViewTip::Scalar { .. }) => return Err(malformed()),
                    };
                    value = if access == Some(StructuralAccess::SharedBorrow) {
                        crate::custody::project_shared_structural_observation(
                            &types,
                            &collection,
                            &selected,
                        )?
                    } else {
                        project_structural_value(&types, &collection, &selected)?
                    };
                    // Keep the supplying home through the remaining inline
                    // projection, including nested arrays. The presentation's
                    // synthetic identity is not primitive payload storage.
                    collection_home = Some((view, remaining[boundary].clone()));
                    remaining = &remaining[boundary + 1..];
                    continue;
                }
                let (referent_type, authority) = self.reference_contract(value.structural_type)?;
                if access.is_some_and(|required| !access_covers(authority, required)) {
                    return Err(malformed());
                }
                let reference_type = value.structural_type;
                let captured = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&value))
                    .ok_or_else(malformed)?;
                primitive_storage = captured
                    .primitive_loan
                    .as_ref()
                    .map(|loan| loan.storage.clone());
                collection_home = None;
                placed_ram = captured.placed_ram.clone();
                if let Some(ram) = &placed_ram {
                    self.validate_placed_ram_home(ram)?;
                    if ram.presentation.structural_type != referent_type
                        || !access_covers(ram.access, authority)
                    {
                        return Err(malformed());
                    }
                    value = ram.presentation.clone();
                    remaining = &remaining[boundary + 1..];
                    continue;
                }
                value = captured.view.clone();
                if let Some(view) = captured.aggregate_view {
                    // The backing keeps its declaration; the next run is
                    // written against the captured logical window. Formation
                    // already admitted its plain storage and validity relation;
                    // a child need not have the full backing root's size.
                    if view.structural_type != referent_type
                        || !view.fits(&types, value.structural_type)
                    {
                        return Err(malformed());
                    }
                    aggregate_view = Some(view);
                } else {
                    if !terminal_semantics::reference_source_compatible(
                        types.values(),
                        value.structural_type,
                        reference_type,
                    ) {
                        return Err(malformed());
                    }
                    // Only the view carries the observed type. The captured
                    // backing and its scalar payload retain their declaration.
                    value.structural_type = referent_type;
                }
                remaining = &remaining[boundary + 1..];
            } else {
                if primitive_storage.is_none()
                    && endpoint_field.is_none()
                    && matches!(
                        types
                            .get(&value.structural_type)
                            .ok_or_else(malformed)?
                            .shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    )
                    && let Some((view, selector)) = collection_home
                {
                    let mut selected = Vec::with_capacity(1 + prefix.len());
                    selected.push(selector);
                    selected.extend_from_slice(prefix);
                    primitive_storage = Some(
                        view.scalar_storage(
                            self,
                            &selected,
                            access
                                .is_some_and(|required| required != StructuralAccess::SharedBorrow),
                        )?
                        .0,
                    );
                }
                // A crossing that ends the path names the whole re-viewed
                // backing; its view tip is the view declaration itself.
                let tip = tip.or_else(|| {
                    aggregate_view.map(|view| AggregateViewTip::Aggregate {
                        view,
                        backing: value.structural_type,
                    })
                });
                return Ok(Some(ResolvedStructuralProjection {
                    placed_ram,
                    value,
                    view_tip: tip,
                    endpoint_field,
                    primitive_storage,
                }));
            }
        }
    }

    /// Field observations address the referent of a reference-valued root.
    /// Ordinary roots already name their storage. Both preserve the original
    /// runtime identity and projection rather than copying record contents.
    /// Behind an aggregate re-view, the root carries the backing declaration
    /// and the tip names the view declaration observations are written in.
    pub(crate) fn observation_view_root(
        &self,
        source: PlaceId,
    ) -> Result<(TerminalStructuralValue, Option<AggregateViewTip>), TerminalInterpretError> {
        let value = crate::custody::verified_structural_place(&self.structural_values, source)?;
        if !matches!(
            self.program
                .structural_types()
                .get(&value.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Reference { .. })
        ) {
            return self.resolve_live_view_projection(source, &[], StructuralAccess::SharedBorrow);
        }
        self.resolve_live_view_projection(
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
        aggregate_backings: &BTreeMap<PlaceId, AggregateBacking>,
        primitive_loans: &BTreeMap<PlaceId, crate::primitive_storage::PrimitiveLoan>,
        placed_occurrences: &BTreeMap<
            terminal_psi::TerminalPlacedViewInput,
            crate::structural_inputs::placed_views::PlacedViewOccurrence,
        >,
    ) -> Result<Vec<Vec<ReferenceBacking>>, TerminalInterpretError> {
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
                let mut backings = Vec::with_capacity(origin.sources.len());
                for source in &origin.sources {
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.place == source.place)
                        .ok_or_else(malformed)?;
                    if !access_covers(parameter.access, source.access)
                        || !access_covers(source.access, access)
                    {
                        return Err(malformed());
                    }
                    let owner = arguments.get(&parameter.place).ok_or_else(malformed)?;
                    // Normalize provenance in the formal's logical namespace,
                    // then resolve its captured original window. Applying the
                    // marker rule to the backing's different nesting would
                    // mistake a view field for an absent descriptor crossing.
                    let binding = terminal_semantics::bind_reference_origin(
                        self.program.structural_types().values(),
                        owner.structural_type,
                        source,
                    )
                    .map_err(|reason| TerminalInterpretError::UnsupportedSemanticVariant(reason))?;
                    if binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                        && binding.endpoint != referent_type
                    {
                        return Err(malformed());
                    }
                    let path = binding.source.path.as_slice();
                    let (backing, view) = match aggregate_backings.get(&parameter.place) {
                        Some(backing) => (&backing.value, Some(backing.region)),
                        None => (owner, None),
                    };
                    let Some(projected) = self.resolve_reference_view_path_with_leaf_type(
                        backing,
                        path,
                        Some(source.access),
                        terminal_psi::StructuralFieldType::canonical_leaf_type,
                        view,
                        placed_occurrences
                            .values()
                            .find(|occurrence| {
                                occurrence.view == StructuralRuntimePlace::from(owner)
                            })
                            .and_then(|occurrence| occurrence.ram.clone()),
                    )?
                    else {
                        continue;
                    };
                    let view = match projected.view_tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                        _ => None,
                    };
                    let mut placed_ram = projected.placed_ram;
                    if let Some(ram) = &mut placed_ram {
                        self.validate_placed_ram_home(ram)?;
                        if !access_covers(ram.access, access) {
                            return Err(malformed());
                        }
                        ram.access = access;
                    }
                    let logical = projected.value;
                    let endpoint = terminal_semantics::reference_origin_type(
                        self.program.structural_types().values(),
                        view.map_or(logical.structural_type, |view| view.structural_type),
                        binding.suffix,
                    );
                    if endpoint.is_none()
                        || (binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                            && endpoint != Some(referent_type))
                        || !logical.qualifications.is_empty()
                    {
                        return Err(malformed());
                    }
                    if placed_ram.is_some()
                        && (!binding.suffix.is_empty()
                            || binding.extent
                                != semantic_vocabulary::ReferenceOriginExtent::Projected
                            || endpoint != Some(referent_type))
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "placed return requires exact whole-view origin",
                        ));
                    }
                    // Capture every resident alternative before the callee
                    // moves or repacks ingress storage. These are backing
                    // identities, not another owner or a runtime address choice.
                    let primitive_storage = projected.primitive_storage.or_else(|| {
                        path.is_empty()
                            .then(|| {
                                primitive_loans
                                    .get(&parameter.place)
                                    .map(|loan| loan.storage.clone())
                            })
                            .flatten()
                    });
                    let expected = placed_ram
                        .as_ref()
                        .map_or(logical, |ram| ram.source.clone());
                    backings.push(ReferenceBacking {
                        element_view: if (!binding.suffix.is_empty()
                            || binding.extent
                                == semantic_vocabulary::ReferenceOriginExtent::WithinRegion)
                            && matches!(
                                self.program
                                    .structural_types()
                                    .get(&expected.structural_type)
                                    .map(|declaration| &declaration.shape),
                                Some(StructuralTypeShape::ElementView { .. })
                            ) {
                            Some(self.element_view_at_resolved_home(&expected)?.clone())
                        } else {
                            None
                        },
                        placed_ram,
                        value: expected,
                        aggregate_view: view,
                        primitive_storage,
                        suffix: binding.suffix.to_vec(),
                        extent: binding.extent,
                        access: source.access,
                    });
                }
                Ok(backings)
            })
            .collect()
    }

    pub(crate) fn validate_reference_return(
        &self,
        signature: &StructuralResultDeclaration,
        carrier: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        // A projected return keeps its original backing prefix. Count only
        // that subtree; the stored tags below select which declared slots owe
        // descriptors. A possible inactive payload is not resident storage.
        let resident_count = self
            .reference_referents
            .keys()
            .filter(|key| {
                key.opaque_identity == carrier.opaque_identity
                    && key.path.starts_with(&carrier.path)
            })
            .count();
        if signature.reference_sources.is_empty() {
            return if resident_count != 0
                || matches!(
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
        // Authenticate case guards before resolving the physical descriptor
        // path (which omits Case). Every active slot must have its captured
        // backing, and every resident descriptor must be accounted for. The
        // ordinary return moves this owner identity without copying referents.
        let mut active_count = 0usize;
        for (ordinal, origin) in signature.reference_sources.iter().enumerate() {
            if signature.reference_sources[..ordinal]
                .iter()
                .any(|previous| previous.path == origin.path)
            {
                return Err(malformed());
            }
            if !crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                &self.program.structural_types(),
                carrier,
                &self.structural_cases,
                &origin.path,
            )? {
                continue;
            }
            active_count += 1;
            let leaf_type = resolve_structural_path_type(
                &self.program.structural_types(),
                signature.structural_type,
                &origin.path,
            )?;
            let (referent_type, access) = self.reference_contract(leaf_type)?;
            let expected = &expected_reference_backings[ordinal];
            let descriptor =
                project_structural_value(&self.program.structural_types(), carrier, &origin.path)?;
            let key = StructuralRuntimePlace::from(&descriptor);
            let actual = self.reference_referents.get(&key).ok_or_else(malformed)?;
            if let Some(ram) = &actual.placed_ram {
                self.validate_placed_ram_home(ram)?;
            }
            if origin.sources.is_empty()
                || origin
                    .sources
                    .iter()
                    .any(|source| !access_covers(source.access, access))
            {
                return Err(malformed());
            }
            let mut matched = false;
            for backing in expected {
                if let Some(projected) = self.resolve_return_origin(backing, actual)? {
                    let view = match projected.view_tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                        _ => None,
                    };
                    matched |= view.map_or_else(
                        || {
                            projected
                                .placed_ram
                                .as_ref()
                                .map_or(projected.value.structural_type, |ram| {
                                    ram.presentation.structural_type
                                })
                        },
                        |view| view.structural_type,
                    ) == referent_type
                        && projected.value == actual.view
                        && projected.placed_ram == actual.placed_ram
                        && view == actual.aggregate_view
                        && projected.primitive_storage.as_ref().is_none_or(|storage| {
                            actual
                                .primitive_loan
                                .as_ref()
                                .is_some_and(|loan| &loan.storage == storage)
                        });
                }
            }
            if !matched {
                return Err(malformed());
            }
        }
        if active_count != resident_count {
            return Err(malformed());
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
