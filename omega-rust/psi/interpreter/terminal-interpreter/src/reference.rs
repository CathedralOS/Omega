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
    /// A typed re-view of aggregate storage with identical geometry: `view`
    /// keeps the backing under its own declaration, and this names the
    /// declaration the reference observes it as. Paths written against the
    /// view rejoin the backing position by position.
    pub(crate) aggregate_view: Option<StructuralTypeId>,
}

/// Where a path written against an aggregate view ends: an aggregate of both
/// declarations, or a scalar leaf with its view and backing carriers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateViewTip {
    Aggregate {
        view: StructuralTypeId,
        backing: StructuralTypeId,
    },
    Scalar {
        view: semantic_vocabulary::ScalarType,
        backing: semantic_vocabulary::ScalarType,
    },
}

/// The backing field at the same relevant position as the view field
/// `selects` names. Identical geometry pairs relevant fields one to one.
fn paired_field<'fields>(
    view: &'fields [terminal_psi::StructuralFieldDeclaration],
    backing: &'fields [terminal_psi::StructuralFieldDeclaration],
    selects: impl Fn(&terminal_psi::StructuralFieldDeclaration) -> bool,
) -> Option<(
    &'fields terminal_psi::StructuralFieldDeclaration,
    &'fields terminal_psi::StructuralFieldDeclaration,
)> {
    let relevant = |field: &&terminal_psi::StructuralFieldDeclaration| !field.relevance.is_erased();
    let position = view.iter().filter(relevant).position(selects)?;
    Some((
        view.iter().filter(relevant).nth(position)?,
        backing.iter().filter(relevant).nth(position)?,
    ))
}

/// Rewrite a canonical observation (`path` to a record, then `field`) written
/// against an aggregate view into its backing's declaration, pairing fields
/// by position. Case selections have no translation and refuse.
pub(crate) fn translate_aggregate_view_field(
    types: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    mut view: StructuralTypeId,
    mut backing: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<(
    Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    semantic_vocabulary::StructuralFieldId,
)> {
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    let mut translated = Vec::with_capacity(path.len());
    for segment in path {
        match (
            segment,
            &types.get(&view)?.shape,
            &types.get(&backing)?.shape,
        ) {
            (
                CanonicalStructuralPathSegment::Field(selected),
                StructuralTypeShape::Record {
                    fields: view_fields,
                    ..
                },
                StructuralTypeShape::Record {
                    fields: backing_fields,
                    ..
                },
            ) => {
                let (view_field, backing_field) =
                    paired_field(view_fields, backing_fields, |field| field.id == *selected)?;
                let (
                    terminal_psi::StructuralFieldType::Structural(view_child),
                    terminal_psi::StructuralFieldType::Structural(backing_child),
                ) = (&view_field.field_type, &backing_field.field_type)
                else {
                    return None;
                };
                translated.push(CanonicalStructuralPathSegment::Field(backing_field.id));
                (view, backing) = (*view_child, *backing_child);
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray {
                    element: view_element,
                    length: view_length,
                },
                StructuralTypeShape::FixedArray {
                    element: backing_element,
                    length: backing_length,
                },
            ) if view_length == backing_length && index < view_length => {
                translated.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                (view, backing) = (*view_element, *backing_element);
            }
            _ => return None,
        }
    }
    let (
        StructuralTypeShape::Record {
            fields: view_fields,
            ..
        },
        StructuralTypeShape::Record {
            fields: backing_fields,
            ..
        },
    ) = (&types.get(&view)?.shape, &types.get(&backing)?.shape)
    else {
        return None;
    };
    let (_, backing_field) = paired_field(view_fields, backing_fields, |candidate| {
        candidate.id == field
    })?;
    Some((translated, backing_field.id))
}

/// Rewrite a path written against an aggregate view into its backing's
/// declaration. Identical geometry (checked when the view formed) pairs the
/// relevant fields of each record by position and each array by element;
/// any other segment has no translation and refuses.
pub(crate) fn translate_aggregate_view_path(
    types: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    view: StructuralTypeId,
    backing: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(Vec<StructuralPathSegment>, AggregateViewTip)> {
    let mut tip = AggregateViewTip::Aggregate { view, backing };
    let mut translated = Vec::with_capacity(path.len());
    for segment in path {
        let AggregateViewTip::Aggregate { view, backing } = tip else {
            return None;
        };
        tip = match (
            segment,
            &types.get(&view)?.shape,
            &types.get(&backing)?.shape,
        ) {
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record {
                    fields: view_fields,
                    ..
                },
                StructuralTypeShape::Record {
                    fields: backing_fields,
                    ..
                },
            ) => {
                let (view_field, backing_field) =
                    paired_field(view_fields, backing_fields, |field| {
                        field.identity == *identity
                    })?;
                translated.push(StructuralPathSegment::Field(backing_field.identity.clone()));
                match (&view_field.field_type, &backing_field.field_type) {
                    (
                        terminal_psi::StructuralFieldType::Structural(view),
                        terminal_psi::StructuralFieldType::Structural(backing),
                    ) => AggregateViewTip::Aggregate {
                        view: *view,
                        backing: *backing,
                    },
                    (view, backing) => AggregateViewTip::Scalar {
                        view: view.scalar_type()?,
                        backing: backing.scalar_type()?,
                    },
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray {
                    element: view,
                    length: view_length,
                },
                StructuralTypeShape::FixedArray {
                    element: backing,
                    length: backing_length,
                },
            ) if view_length == backing_length && index < view_length => {
                translated.push(StructuralPathSegment::FixedIndex(*index));
                AggregateViewTip::Aggregate {
                    view: *view,
                    backing: *backing,
                }
            }
            _ => return None,
        };
    }
    Some((translated, tip))
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
    /// Borrowed entry and state arrivals retain descriptor storage without an
    /// affine owner entry in this frame. Its declared authority bounds every
    /// reference crossing. Owned carrier availability follows its declared
    /// multiplicity: only affine storage has a disposal frontier.
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
        let mut aggregate_view = None;
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
                let projected = match inline_byte {
                    Some(_) => None,
                    None => Some(self.resolve_live_projection(
                        source.place,
                        &source.path,
                        source.access,
                    )?),
                };
                let byte_offset = usize::try_from(byte_offset).map_err(|_| malformed())?;
                let captured = match &projected {
                    Some(view) => self.reference_byte_region(view)?,
                    None => None,
                };
                let byte_endpoint = matches!(
                    self.program
                        .structural_types()
                        .get(&original_type)
                        .ok_or_else(malformed)?
                        .shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                );
                if captured.is_none() && !byte_endpoint {
                    // Aggregate storage with identical geometry keeps its own
                    // typed cells: the view names that whole backing under
                    // its declaration, and crossings rejoin it by position.
                    let source_length = terminal_semantics::reference_storage_size(
                        self.program.structural_types().values(),
                        original_type,
                    )
                    .and_then(|length| usize::try_from(length).ok());
                    if byte_offset != 0 || source_length != Some(byte_length) {
                        return Err(malformed());
                    }
                    aggregate_view = Some(referent_type);
                    region_referent = Some(projected.ok_or_else(malformed)?);
                } else {
                    let (mut view, region) = if let Some(region) = captured {
                        if byte_offset
                            .checked_add(byte_length)
                            .is_none_or(|end| end > region.byte_length)
                        {
                            return Err(malformed());
                        }
                        (
                            projected.ok_or_else(malformed)?,
                            ReferenceByteRegion {
                                endpoint: crate::primitive_storage::shifted_byte_endpoint(
                                    &region.endpoint,
                                    byte_offset,
                                )?,
                                byte_length,
                            },
                        )
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
            // The storage judgment above admitted an aggregate re-view.
            || (aggregate_view.is_none()
                && !terminal_semantics::reference_source_compatible(
                    self.program.structural_types().values(),
                    referent.structural_type,
                    result.structural_type,
                ))
            || !referent.qualifications.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.live_affine_discard_at(result.place).is_some()
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
        self.resolve_live_view_projection(place, path, access)
            .map(|(value, _)| value)
    }

    /// Resolve a live projection and, when its final run crosses into an
    /// aggregate re-view, where that run ends in both declarations.
    pub(crate) fn resolve_live_view_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<(TerminalStructuralValue, Option<AggregateViewTip>), TerminalInterpretError> {
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
            return self
                .resolve_reference_view_path(&collection, &selected, access)?
                .ok_or_else(malformed);
        }
        if path.contains(&StructuralPathSegment::Referent) {
            let authority = self.reference_carrier_access(place, owner, path)?;
            if !access_covers(authority, access) {
                return Err(malformed());
            }
        }
        self.resolve_reference_view_path(owner, path, access)?
            .ok_or_else(malformed)
    }

    /// Project storage, crossing each reference boundary through its captured
    /// backing. A suffix belongs to that backing, not to the descriptor object.
    fn resolve_reference_path(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Option<TerminalStructuralValue>, TerminalInterpretError> {
        self.resolve_reference_view_path(owner, path, access)
            .map(|selected| selected.map(|(value, _)| value))
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
        let types = self.program.structural_types();
        let mut value = owner.clone();
        let mut remaining = path;
        let mut aggregate_view = None;
        loop {
            let boundary = remaining
                .iter()
                .position(|segment| *segment == StructuralPathSegment::Referent);
            let prefix = &remaining[..boundary.unwrap_or(remaining.len())];
            let translated;
            let mut tip = None;
            let prefix = match aggregate_view.take() {
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
            value = if access == StructuralAccess::SharedBorrow {
                crate::custody::project_shared_structural_observation(&types, &value, prefix)?
            } else {
                project_structural_value(&types, &value, prefix)?
            };
            if let Some(boundary) = boundary {
                let (referent_type, authority) = self.reference_contract(value.structural_type)?;
                if !access_covers(authority, access) {
                    return Err(malformed());
                }
                let reference_type = value.structural_type;
                let captured = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&value))
                    .ok_or_else(malformed)?;
                value = captured.view.clone();
                if let Some(view) = captured.aggregate_view {
                    // The backing keeps its declaration; the next run is
                    // written against the view and rejoins it by position.
                    if view != referent_type
                        || !terminal_semantics::reference_storage_source_compatible(
                            types.values(),
                            value.structural_type,
                            reference_type,
                            None,
                        )
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
                // A crossing that ends the path names the whole re-viewed
                // backing; its view tip is the view declaration itself.
                let tip = tip.or_else(|| {
                    aggregate_view.map(|view| AggregateViewTip::Aggregate {
                        view,
                        backing: value.structural_type,
                    })
                });
                return Ok(Some((value, tip)));
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
            return Ok((value.clone(), None));
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
                    let Some(expected) = self.resolve_reference_path(
                        arguments.get(&parameter.place).ok_or_else(malformed)?,
                        &source.path,
                        source.access,
                    )?
                    else {
                        continue;
                    };
                    if expected.structural_type != referent_type
                        || !expected.qualifications.is_empty()
                    {
                        return Err(malformed());
                    }
                    // Capture every resident alternative before the callee
                    // moves or repacks ingress storage. These are backing
                    // identities, not another owner or a runtime address choice.
                    backings.push(expected);
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
            if origin.sources.is_empty()
                || origin
                    .sources
                    .iter()
                    .any(|source| !access_covers(source.access, access))
                || expected
                    .iter()
                    .any(|backing| backing.structural_type != referent_type)
                || !expected.iter().any(|backing| backing == &actual.view)
            {
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
