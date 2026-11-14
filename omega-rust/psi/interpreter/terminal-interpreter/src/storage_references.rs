/*
    Placed and origin-backed structural references, aggregate reference views,
    and entry-return loan verification in the Terminal Psi interpreter.

    We resolve typed reference projections to their underlying primitive, byte,
    or aggregate storage homes and check loan ancestry on return.
*/

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::StructuralRuntimePlace;
use crate::boundary::values::TerminalStructuralValue;
use crate::engine::custody::project_structural_value;
use crate::engine::custody::resolve_structural_path_type;
use crate::engine::custody::whole_path_discard;
use crate::engine::execution::SuspendedCall;
use crate::engine::execution::SuspendedCallResult;
use crate::engine::execution::TerminalExecution;
use crate::storage::runtime_rows::RuntimeRows;
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

#[derive(Clone)]
pub(crate) struct ReferenceReferent {
    pub(crate) placed: Option<AccessReference>,
    pub(crate) view: TerminalStructuralValue,
    pub(crate) primitive_loan: Option<crate::storage::primitive_storage::PrimitiveLoan>,
    pub(crate) collection_loan: Option<crate::storage::element_views::ElementView>,
    pub(crate) byte_owner: Option<crate::storage::byte_sequences::binding::ByteSequenceBinding>,
    pub(crate) byte_region: Option<ReferenceByteRegion>,
    pub(crate) aggregate_view: Option<AggregateViewRegion>,
}

pub(crate) struct PreparedReference {
    pub(crate) referent: ReferenceReferent,
    pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

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
    ScalarRegion {
        view: terminal_semantics::ReferenceStorageProjection,
        byte_length: u64,
    },
}

#[derive(Clone)]
pub(crate) struct ResolvedStructuralProjection {
    pub(crate) array_loan: Option<crate::storage::element_views::ElementView>,
    placed: Option<AccessReference>,
    pub(crate) value: TerminalStructuralValue,
    pub(crate) view_tip: Option<AggregateViewTip>,
    endpoint_field: Option<semantic_vocabulary::StructuralFieldId>,
    pub(crate) primitive_storage: Option<crate::storage::primitive_storage::PrimitiveStorage>,
    pub(crate) collection_home: Option<crate::storage::element_views::ElementViewHome>,
}

#[derive(Clone)]
pub(crate) struct ReferenceBacking {
    array_loan: Option<crate::storage::element_views::ElementView>,
    placed: Option<AccessReference>,
    pub(crate) value: TerminalStructuralValue,
    aggregate_view: Option<AggregateViewRegion>,
    primitive_storage: Option<crate::storage::primitive_storage::PrimitiveStorage>,
    suffix: Vec<terminal_psi::ReferenceOriginProjection>,
    extent: semantic_vocabulary::ReferenceOriginExtent,
    element_view: Option<crate::storage::element_views::ElementView>,
    byte_binding: Option<crate::storage::byte_sequences::binding::ByteSequenceBinding>,
    access: StructuralAccess,
}

pub(crate) mod aggregate_views {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{StructuralFieldType, StructuralPathSegment, StructuralTypeShape};

    use crate::boundary::values::TerminalStructuralValue;
    use crate::embedding::program::ProgramDeclarations;

    use super::AggregateViewTip;
    use terminal_semantics::{
        observed_field_declaration, record_field_carrier, reference_storage_byte_width,
        reference_storage_region, reference_storage_size, static_field_projection,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct AggregateViewRegion {
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) byte_offset: u64,
        pub(crate) byte_length: u64,
    }

    #[derive(Clone)]
    pub(crate) struct AggregateBacking {
        pub(crate) value: TerminalStructuralValue,
        pub(crate) placed: Option<super::AccessReference>,
        pub(crate) region: Option<AggregateViewRegion>,
        pub(crate) byte_region: Option<super::ReferenceByteRegion>,
        pub(crate) collection_home: Option<crate::storage::element_views::ElementViewHome>,
        pub(crate) array_loan: Option<crate::storage::element_views::ElementView>,
    }

    impl AggregateViewRegion {
        pub(crate) fn whole(
            types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
            structural_type: StructuralTypeId,
        ) -> Option<Self> {
            Some(Self {
                structural_type,
                byte_offset: 0,
                byte_length: reference_storage_size(types.values(), structural_type)?,
            })
        }

        pub(crate) fn fits(
            self,
            types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
            backing: StructuralTypeId,
        ) -> bool {
            reference_storage_size(types.values(), self.structural_type) == Some(self.byte_length)
                && reference_storage_size(types.values(), backing).is_some_and(|size| {
                    self.byte_offset
                        .checked_add(self.byte_length)
                        .is_some_and(|end| end <= size)
                })
        }
    }

    pub(crate) fn translate_aggregate_view_path(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        view: AggregateViewRegion,
        backing: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<(Vec<StructuralPathSegment>, AggregateViewTip)> {
        if !view.fits(types, backing) {
            return None;
        }
        let selected = reference_storage_region(types.values(), view.structural_type, path)?;
        if selected.byte_offset.checked_add(selected.byte_length)? > view.byte_length {
            return None;
        }
        let byte_offset = view.byte_offset.checked_add(selected.byte_offset)?;
        if let Some(scalar) = selected.scalar {
            let fragments =
                scalar_storage_fragments(types, backing, byte_offset, selected.byte_length)?;
            if let [fragment] = fragments.as_slice()
                && fragment.addressable_cell
                && fragment.bit_offset == byte_offset.checked_mul(8)?
                && fragment.source_bit_offset == 0
                && fragment.bit_length == selected.byte_length.checked_mul(8)?
                && reference_storage_byte_width(fragment.scalar.stored_scalar)
                    .is_some_and(|width| u64::from(width) == selected.byte_length)
            {
                return Some((
                    fragment.path.clone(),
                    AggregateViewTip::Scalar {
                        view: scalar,
                        backing: fragment.scalar,
                    },
                ));
            }
            Some((
                Vec::new(),
                AggregateViewTip::ScalarRegion {
                    view: terminal_semantics::ReferenceStorageProjection {
                        byte_offset,
                        ..scalar
                    },
                    byte_length: selected.byte_length,
                },
            ))
        } else {
            let structural_type = crate::engine::custody::resolve_structural_path_type(
                types,
                view.structural_type,
                path,
            )
            .ok()?;
            Some((
                Vec::new(),
                AggregateViewTip::Aggregate {
                    view: AggregateViewRegion {
                        structural_type,
                        byte_offset,
                        byte_length: selected.byte_length,
                    },
                    backing,
                },
            ))
        }
    }

    pub(crate) struct ScalarStorageFragmentGeometry {
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) scalar: terminal_semantics::ReferenceStorageProjection,
        pub(crate) bit_offset: u64,
        pub(crate) source_bit_offset: u64,
        pub(crate) bit_length: u64,
        pub(crate) addressable_cell: bool,
    }

    pub(crate) fn scalar_storage_fragments(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        backing: StructuralTypeId,
        byte_offset: u64,
        byte_length: u64,
    ) -> Option<Vec<ScalarStorageFragmentGeometry>> {
        let start = byte_offset.checked_mul(8)?;
        let end = byte_offset.checked_add(byte_length)?.checked_mul(8)?;
        let mut fragments = Vec::new();
        collect_scalar_fragments(
            types,
            backing,
            backing,
            &mut Vec::new(),
            start,
            end,
            &mut fragments,
        )?;
        fragments.sort_by_key(|fragment| fragment.bit_offset);
        let mut covered = start;
        for fragment in &fragments {
            let first = fragment.bit_offset;
            let next = first.checked_add(fragment.bit_length)?;
            if first > covered || next <= covered {
                return None;
            }
            covered = next.min(end);
        }
        (covered == end).then_some(fragments)
    }

    fn collect_scalar_fragments(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        root: StructuralTypeId,
        carrier: StructuralTypeId,
        path: &mut Vec<StructuralPathSegment>,
        start: u64,
        end: u64,
        fragments: &mut Vec<ScalarStorageFragmentGeometry>,
    ) -> Option<()> {
        if path.len() >= 128 {
            return None;
        }
        if let Some(selected) =
            terminal_semantics::reference_scalar_storage(types.values(), root, path)
        {
            let mut append =
                |scalar, bit_offset: u64, source_bit_offset, bit_length: u64, addressable_cell| {
                    let last = bit_offset.checked_add(bit_length)?;
                    if start < last && bit_offset < end {
                        fragments.push(ScalarStorageFragmentGeometry {
                            path: path.clone(),
                            scalar,
                            bit_offset,
                            source_bit_offset,
                            bit_length,
                            addressable_cell,
                        });
                    }
                    Some(())
                };
            match selected {
                terminal_semantics::ReferenceScalarStorage::Contiguous(scalar) => {
                    append(
                        scalar,
                        scalar.byte_offset.checked_mul(8)?,
                        0,
                        u64::from(reference_storage_byte_width(scalar.stored_scalar)?)
                            .checked_mul(8)?,
                        true,
                    )?;
                }
                terminal_semantics::ReferenceScalarStorage::Fragmented {
                    byte_offset,
                    logical_scalar,
                    fragments: recipe,
                } => {
                    for fragment in recipe.fragments() {
                        let scalar = terminal_semantics::ReferenceStorageProjection {
                            byte_offset: byte_offset.checked_add(fragment.container_byte_offset)?,
                            logical_scalar,
                            stored_scalar: logical_scalar,
                        };
                        append(
                            scalar,
                            scalar
                                .byte_offset
                                .checked_mul(8)?
                                .checked_add(fragment.destination_lsb)?,
                            fragment.source_lsb,
                            fragment.width,
                            false,
                        )?;
                    }
                }
            }
            return Some(());
        }
        let region = reference_storage_region(types.values(), root, path)?;
        let region_start = region.byte_offset.checked_mul(8)?;
        let region_end = region
            .byte_offset
            .checked_add(region.byte_length)?
            .checked_mul(8)?;
        if end <= region_start || start >= region_end {
            return Some(());
        }
        match &types.get(&carrier)?.shape {
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    path.push(StructuralPathSegment::Field(field.identity));
                    let child = field.field_type.as_structural().unwrap_or(carrier);
                    collect_scalar_fragments(types, root, child, path, start, end, fragments)?;
                    path.pop();
                }
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if *length != 0 => {
                path.push(StructuralPathSegment::FixedIndex(0));
                let first = reference_storage_region(types.values(), root, path)?;
                let stride = if *length == 1 {
                    first.byte_length
                } else {
                    *path.last_mut()? = StructuralPathSegment::FixedIndex(1);
                    reference_storage_region(types.values(), root, path)?
                        .byte_offset
                        .checked_sub(first.byte_offset)?
                };
                if stride == 0 || stride < first.byte_length {
                    return None;
                }
                let first_index = (start / 8).saturating_sub(first.byte_offset) / stride;
                let final_index = ((end.checked_sub(1)? / 8).saturating_sub(first.byte_offset)
                    / stride)
                    .min(length - 1);
                for index in first_index..=final_index {
                    *path.last_mut()? = StructuralPathSegment::FixedIndex(index);
                    collect_scalar_fragments(types, root, *element, path, start, end, fragments)?;
                }
                path.pop();
            }
            _ => return None,
        }
        Some(())
    }

    pub(crate) fn translate_aggregate_view_field(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        view: AggregateViewRegion,
        backing: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<(Vec<CanonicalStructuralPathSegment>, StructuralFieldId)> {
        let carrier = record_field_carrier(types.values(), view.structural_type, path)?;
        let declaration =
            observed_field_declaration(types.values(), view.structural_type, path, field)?;
        let mut logical = carrier.path;
        logical.push(StructuralPathSegment::Field(declaration.identity));
        let (translated, _) = translate_aggregate_view_path(types, view, backing, &logical)?;
        let mut canonical = backing_scalar_path(types, backing, &translated)?;
        let CanonicalStructuralPathSegment::Field(field) = canonical.pop()? else {
            return None;
        };
        Some((canonical, field))
    }

    pub(crate) fn backing_scalar_path(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        backing: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<Vec<CanonicalStructuralPathSegment>> {
        if let Some((StructuralPathSegment::Field(_), prefix)) = path.split_last() {
            let field = crate::engine::custody::inline_structural_field(types, backing, path)?;
            if matches!(
                field.field_type,
                StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
            ) {
                return None;
            }
            if field.field_type.scalar_type().is_some() {
                let (mut canonical, field) =
                    static_field_projection(types.values(), backing, prefix, field.id)?;
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                return Some(canonical);
            }
        }
        terminal_semantics::canonical_primitive_path(types.values(), backing, path)
    }
}
pub(crate) mod entry_returns {
    use super::ReferenceBacking;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::engine::execution::TerminalExecution;
    use evaluation_accounting::EvaluationStorageLease;

    #[derive(Default)]
    pub(crate) struct EntryReferenceBackings {
        pub(crate) rows: Vec<Vec<ReferenceBacking>>,
        _lease: Option<EvaluationStorageLease>,
    }

    impl TerminalExecution<'_> {
        pub(crate) fn capture_entry_reference_backings(
            &mut self,
        ) -> Result<(), TerminalInterpretError> {
            let machine = self.program.machine(self.current_machine)?;
            let Some(signature) = machine
                .result
                .structural()
                .filter(|signature| !signature.reference_sources.is_empty())
            else {
                return Ok(());
            };
            if self.entry_reference_backings.is_none() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "host reference results require an explicit loan carrier interface",
                ));
            }
            let cells = signature
                .reference_sources
                .iter()
                .try_fold(0u64, |cells, source| {
                    cells
                        .checked_add(1)?
                        .checked_add(u64::try_from(source.sources.len()).ok()?)
                })
                .ok_or_else(malformed)?;
            let lease = self
                .account
                .as_ref()
                .map(|account| account.reserve_storage(cells, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            let rows = self.bind_reference_return_backings(
                &machine.structural_parameters,
                signature,
                &self.structural_values,
                &self.aggregate_argument_backings,
                &self.primitive_loan_values,
                &self.element_view_values,
                &self.byte_sequence_values,
                &self.access_view_occurrences,
            )?;
            self.entry_reference_backings = Some(EntryReferenceBackings {
                rows,
                _lease: lease,
            });
            Ok(())
        }
    }
}
pub(crate) use entry_returns::EntryReferenceBackings;
pub(crate) mod origin_backings {
    use super::*;
    use terminal_psi::ReferenceOriginProjection;
    use terminal_semantics::reference_storage_region;

    impl ReferenceBacking {
        pub(crate) fn retain_storage(
            &self,
            retained: &mut crate::storage::structural_storage::RetainedStorage,
        ) {
            if let Some(binding) = &self.byte_binding {
                retained.byte_binding(binding);
            }
            if let Some(storage) = &self.primitive_storage {
                retained.primitive_endpoint(&storage.endpoint);
            }
            if let Some(view) = &self.element_view {
                view.retain_projection_storage(retained, &self.suffix);
            } else {
                retained.projection(&self.value, &self.suffix);
                if let Some(view) = &self.array_loan {
                    view.retain_projection_storage(retained, &self.suffix);
                }
            }
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn resolve_return_origin(
            &self,
            backing: &ReferenceBacking,
            actual: &ReferenceReferent,
        ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
            let types = self.structural_types();
            let mut selected = ResolvedStructuralProjection {
                array_loan: backing.array_loan.clone(),
                placed: backing.placed.clone(),
                value: backing
                    .placed
                    .as_ref()
                    .map_or_else(|| backing.value.clone(), |ram| ram.presentation.clone()),
                view_tip: backing
                    .aggregate_view
                    .map(|view| AggregateViewTip::Aggregate {
                        view,
                        backing: backing.value.structural_type,
                    }),
                endpoint_field: None,
                primitive_storage: backing.primitive_storage.clone(),
                collection_home: None,
            };
            let mut remaining = backing.suffix.as_slice();
            let mut captured_view = backing.element_view.as_ref();
            let mut captured_bytes = backing.byte_binding.as_ref();
            loop {
                let family = remaining
                    .iter()
                    .position(|projection| *projection == ReferenceOriginProjection::Element);
                let prefix_length = family.unwrap_or(remaining.len());
                let prefix = remaining[..prefix_length]
                    .iter()
                    .map(|projection| match projection {
                        ReferenceOriginProjection::Exact(segment) => Ok(segment.clone()),
                        ReferenceOriginProjection::Element => Err(malformed()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !prefix.is_empty() {
                    let projected = match (captured_bytes, prefix.as_slice()) {
                        (Some(binding), [StructuralPathSegment::FixedIndex(index)]) => self
                            .resolve_captured_byte_origin(
                                &selected,
                                binding,
                                actual,
                                backing.access,
                                Some(*index),
                            )?,
                        _ => self.resolve_origin_projection(&selected, &prefix, backing.access)?,
                    };
                    let Some(projected) = projected else {
                        return Ok(None);
                    };
                    selected = projected;
                    captured_view = None;
                    captured_bytes = None;
                }
                let Some(position) = family else {
                    if backing.extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion {
                        if let Some(binding) =
                            captured_bytes.filter(|_| actual.primitive_loan.is_some())
                        {
                            return self.resolve_captured_byte_origin(
                                &selected,
                                binding,
                                actual,
                                backing.access,
                                None,
                            );
                        }
                        return self.resolve_region_return(
                            selected,
                            captured_view,
                            actual,
                            backing.access,
                        );
                    }
                    if let Some(ram) = &selected.placed {
                        self.validate_access_backing(ram)?;
                        selected.value = ram.source.clone();
                    }
                    return Ok(Some(selected));
                };
                let logical_type = match selected.view_tip {
                    Some(AggregateViewTip::Aggregate { view, .. }) => view.structural_type,
                    _ => selected.value.structural_type,
                };
                if matches!(
                    types
                        .get(&logical_type)
                        .map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::ElementView { .. })
                ) {
                    let view = match captured_view {
                        Some(view) => view,
                        None => self.element_view_at_resolved_home(&selected.value)?,
                    };
                    if !access_covers(view.access(), backing.access) {
                        return Err(malformed());
                    }
                    let Some(index) = view.captured_structural_index(&actual.view) else {
                        return Ok(None);
                    };
                    let interpretation = match selected.view_tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                        _ => None,
                    };
                    let Some(projected) = self.resolve_captured_element_origin(
                        view,
                        index,
                        backing.access,
                        interpretation,
                        selected.placed.clone(),
                    )?
                    else {
                        return Ok(None);
                    };
                    selected = projected;
                    captured_view = None;
                    captured_bytes = None;
                    remaining = &remaining[position + 1..];
                    continue;
                }
                if let Some(binding) = captured_bytes {
                    let Some(projected) = self.resolve_captured_byte_origin(
                        &selected,
                        binding,
                        actual,
                        backing.access,
                        None,
                    )?
                    else {
                        return Ok(None);
                    };
                    selected = projected;
                    captured_bytes = None;
                    captured_view = None;
                    remaining = &remaining[position + 1..];
                    continue;
                }
                if selected.value.opaque_identity != actual.view.opaque_identity {
                    return Ok(None);
                }
                let Some(suffix) = actual
                    .view
                    .path
                    .strip_prefix(selected.value.path.as_slice())
                else {
                    return Ok(None);
                };
                let Some(StructuralPathSegment::FixedIndex(index)) = suffix.first() else {
                    return Ok(None);
                };
                let projection = vec![StructuralPathSegment::FixedIndex(*index)];
                let Some(projected) =
                    self.resolve_origin_projection(&selected, &projection, backing.access)?
                else {
                    return Ok(None);
                };
                selected = projected;
                captured_view = None;
                remaining = &remaining[position + 1..];
            }
        }

        fn resolve_captured_byte_origin(
            &self,
            source: &ResolvedStructuralProjection,
            binding: &crate::storage::byte_sequences::binding::ByteSequenceBinding,
            actual: &ReferenceReferent,
            access: StructuralAccess,
            expected_index: Option<u64>,
        ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
            if !access_covers(binding.access(), access) {
                return Ok(None);
            }
            let Some(loan) = &actual.primitive_loan else {
                return Ok(None);
            };
            use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
            let index = match &loan.storage.endpoint {
                PrimitiveStorageEndpoint::ViewByte { index, .. } => {
                    u64::try_from(*index).map_err(|_| malformed())?
                }
                PrimitiveStorageEndpoint::RepresentationByte(byte) => {
                    u64::try_from(byte.index).map_err(|_| malformed())?
                }
                PrimitiveStorageEndpoint::ArrayElement { index, .. } => {
                    let crate::storage::byte_sequences::binding::ByteSequenceBinding::MutableArray {
                    offset,
                    ..
                } = binding
                else {
                    return Ok(None);
                };
                    let Some(index) = u64::try_from(*index)
                        .ok()
                        .and_then(|index| index.checked_sub(*offset))
                    else {
                        return Ok(None);
                    };
                    index
                }
                _ => return Ok(None),
            };
            if expected_index.is_some_and(|expected| expected != index) {
                return Ok(None);
            }
            let path = [StructuralPathSegment::FixedIndex(index)];
            let Some(structural_type) = terminal_semantics::primitive_projection_declaration(
                self.structural_types().values(),
                source.value.structural_type,
                &path,
            ) else {
                return Ok(None);
            };
            let mut byte =
                self.bound_byte_element_storage(&source.value, binding, index, access)?;
            if actual
                .primitive_loan
                .as_ref()
                .is_none_or(|loan| loan.storage != byte.storage)
            {
                return Ok(None);
            }
            byte.referent.structural_type = structural_type;
            Ok(Some(ResolvedStructuralProjection {
                value: byte.referent,
                primitive_storage: Some(byte.storage),
                ..source.clone()
            }))
        }

        fn resolve_origin_projection(
            &self,
            source: &ResolvedStructuralProjection,
            path: &[StructuralPathSegment],
            access: StructuralAccess,
        ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
            if path.is_empty() {
                return Ok(Some(source.clone()));
            }
            let interpretation = match source.view_tip {
                Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                _ => None,
            };
            let Some(mut projected) = self.resolve_reference_view_path_with_leaf_type(
                &source.value,
                path,
                Some(access),
                None,
                terminal_psi::StructuralFieldType::canonical_leaf_type,
                interpretation,
                source.placed.clone(),
                source.array_loan.clone(),
            )?
            else {
                return Ok(None);
            };
            if let Some(home) = &source.collection_home {
                let types = self.structural_types();
                if reference_storage_region(types.values(), source.value.structural_type, path)
                    .is_some()
                {
                    let mut selected_home = home.clone();
                    selected_home.path.extend_from_slice(path);
                    match types
                        .get(&projected.value.structural_type)
                        .map(|declaration| &declaration.shape)
                    {
                        Some(StructuralTypeShape::PrimitiveScalar(_))
                            if projected.primitive_storage.is_none() =>
                        {
                            projected.primitive_storage = Some(
                                self.primitive_home_access(
                                    selected_home.storage,
                                    access != StructuralAccess::SharedBorrow,
                                    &selected_home.path,
                                )?
                                .0,
                            );
                        }
                        Some(StructuralTypeShape::FixedArray {
                            length: terminal_psi::ArrayExtent::Literal(length),
                            ..
                        }) if projected.array_loan.is_none() => {
                            projected.array_loan = Some(
                                crate::storage::element_views::ElementView::structural(
                                    projected.value.clone(),
                                    usize::try_from(*length).map_err(|_| malformed())?,
                                    access,
                                    Some(selected_home.clone()),
                                    selected_home.storage.scalar_array_frame().1,
                                )
                                .with_descriptor_home(projected.value.clone()),
                            );
                        }
                        _ => {}
                    }
                    projected.collection_home = Some(selected_home);
                }
            }
            Ok(Some(projected))
        }

        fn resolve_captured_element_origin(
            &self,
            view: &crate::storage::element_views::ElementView,
            index: u64,
            access: StructuralAccess,
            interpretation: Option<AggregateViewRegion>,
            placed: Option<AccessReference>,
        ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
            let types = self.structural_types();
            let logical_path = [StructuralPathSegment::FixedIndex(index)];
            let (collection, physical_path) = view.structural_projection(
                &types,
                &logical_path,
                access != StructuralAccess::SharedBorrow,
            )?;
            let Some(mut projected) = self.resolve_reference_view_path_with_leaf_type(
                &collection,
                &physical_path,
                Some(access),
                None,
                terminal_psi::StructuralFieldType::canonical_leaf_type,
                interpretation,
                placed,
                None,
            )?
            else {
                return Ok(None);
            };
            match types
                .get(&projected.value.structural_type)
                .map(|declaration| &declaration.shape)
            {
                Some(StructuralTypeShape::PrimitiveScalar(_))
                    if projected.primitive_storage.is_none() =>
                {
                    projected.primitive_storage = Some(
                        view.scalar_storage(
                            self,
                            &logical_path,
                            access != StructuralAccess::SharedBorrow,
                        )?
                        .0,
                    );
                }
                Some(StructuralTypeShape::FixedArray { .. }) => {
                    projected.array_loan =
                        Some(view.project_array(&types, &logical_path, &projected.value)?);
                }
                _ => {}
            }
            projected.collection_home = view.captured_storage_home(&types, &logical_path)?;
            Ok(Some(projected))
        }

        fn resolve_region_return(
            &self,
            mut region: ResolvedStructuralProjection,
            captured_view: Option<&crate::storage::element_views::ElementView>,
            actual: &ReferenceReferent,
            access: StructuralAccess,
        ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
            if let Some(captured) = &actual.placed {
                self.retain_returned_access_interpretation(
                    &mut region,
                    actual,
                    captured.carrier(),
                    access,
                )?;
                if region.placed.as_ref() == Some(captured) && region.value == actual.view {
                    return Ok(Some(region));
                }
            }
            if let Some(view) = captured_view {
                if !access_covers(view.access(), access) {
                    return Err(malformed());
                }
                let Some(index) = view.captured_structural_index(&actual.view) else {
                    return Ok(None);
                };
                let Some(element) = self.resolve_captured_element_origin(
                    view,
                    index,
                    access,
                    None,
                    region.placed.clone(),
                )?
                else {
                    return Ok(None);
                };
                region = element;
            }
            if region.value.opaque_identity != actual.view.opaque_identity {
                return Ok(None);
            }
            let Some(path) = actual.view.path.strip_prefix(region.value.path.as_slice()) else {
                return Ok(None);
            };
            let types = self.structural_types();
            let Some(selected) =
                reference_storage_region(types.values(), region.value.structural_type, path)
            else {
                return Ok(None);
            };
            let allowed = match region.view_tip {
                Some(AggregateViewTip::Aggregate { view, .. }) => view,
                _ => match AggregateViewRegion::whole(&types, region.value.structural_type) {
                    Some(whole) => whole,
                    None => return Ok(None),
                },
            };
            let (offset, length) = match actual.aggregate_view {
                Some(view) if view.fits(&types, actual.view.structural_type) => {
                    let Some(offset) = selected.byte_offset.checked_add(view.byte_offset) else {
                        return Ok(None);
                    };
                    (offset, view.byte_length)
                }
                Some(_) => return Ok(None),
                None => (selected.byte_offset, selected.byte_length),
            };
            if offset < allowed.byte_offset
                || offset
                    .checked_add(length)
                    .zip(allowed.byte_offset.checked_add(allowed.byte_length))
                    .is_none_or(|(end, allowed_end)| end > allowed_end)
            {
                return Ok(None);
            }
            if actual.byte_region.is_some() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "returned input region requires captured byte backing",
                ));
            }
            let Some(mut projected) = self.resolve_origin_projection(&region, path, access)? else {
                return Ok(None);
            };
            if let Some(view) = actual.aggregate_view {
                projected.view_tip = Some(AggregateViewTip::Aggregate {
                    view,
                    backing: projected.value.structural_type,
                });
            }
            Ok(Some(projected))
        }
    }
}
pub(crate) mod placed {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalStructuralValue,
    };
    use crate::engine::custody::whole_path_discard;
    use crate::engine::execution::TerminalExecution;
    use semantic_vocabulary::{MachineId, PlaceId};
    use terminal_psi::{
        Operation, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralTypeShape,
    };

    pub(crate) mod view {
        use super::{AccessReference, AccessStorage};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::StructuralRuntimePlace;
        use crate::engine::execution::{TerminalExecution, operation_kind};
        use crate::storage::record::storage::ConstructorStorage;
        use terminal_psi::{
            Operation, OperationKind, StructuralAccess, StructuralAffineDiscard,
            StructuralPathSegment, TerminalAccessLifecyclePlan,
        };

        impl TerminalExecution<'_> {
            pub(crate) fn establish_access_view(
                &mut self,
                operation: &Operation,
                plan: &TerminalAccessLifecyclePlan,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => BoundaryCall {
                    structural_arguments, completion_receipts, ..
                });
                let [source, profile_argument] = structural_arguments.as_slice() else {
                    return Err(malformed());
                };
                let result = operation.result.structural().ok_or_else(malformed)?;
                if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
                    || profile_argument.access != StructuralAccess::SharedBorrow
                    || !completion_receipts.is_empty()
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || self.result_place_in_use(result.place)
                    || !self
                        .loan_completions_for_operation(operation.id)?
                        .is_empty()
                {
                    return Err(malformed());
                }
                let program = std::sync::Arc::clone(&self.program);
                terminal_semantics::validate_access_view_content(&program.catalogs, plan)
                    .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
                let terminal_semantics::AccessResultContract::View {
                    result: declared,
                    ready,
                    rejected,
                    failures,
                    carrier,
                    access,
                } = terminal_semantics::access_result_contract(&program.catalogs, plan)
                    .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                else {
                    return Err(malformed());
                };
                if source.access != access
                    || result.structural_type != declared.structural_type
                    || result.multiplicity != declared.multiplicity
                    || result.qualifications != declared.qualifications
                    || result.projected_qualifications != declared.projected_qualifications
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "borrowed placement changed its declared result contract",
                    ));
                }
                let backing = self
                    .resolve_reference_call_arguments(std::slice::from_ref(source))?
                    .pop()
                    .ok_or_else(malformed)?;
                let profile = self.resolve_live_projection(
                    profile_argument.place,
                    &profile_argument.path,
                    profile_argument.access,
                )?;
                let (binding, supplied) = self.lifecycle_profile(plan, 0, &backing, &profile)?;
                let assessment =
                    self.provider_storage
                        .prepare_view(binding, &supplied, &plan.placement)?;
                let mut identity_cursor = self.local_structural_identities.cursor();
                let value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
                let root = StructuralRuntimePlace::from(&value);
                let mut storage = ConstructorStorage::new(self.account.as_ref());
                match assessment {
                    Ok(compatibility) => {
                        let [field] = ready.fields.as_slice() else {
                            return Err(malformed());
                        };
                        storage.stage_case(root.clone(), result.structural_type, ready.id)?;
                        let mut presentation = backing.clone();
                        presentation.structural_type = carrier;
                        presentation.qualifications.clear();
                        presentation.projected_qualifications.clear();
                        let captured = AccessReference {
                            storage: AccessStorage::External {
                                carrier,
                                owner: StructuralRuntimePlace::from(&backing),
                                binding,
                                compatibility,
                            },
                            presentation,
                            access,
                            selected_field: None,
                            projection: Vec::new(),
                            source: backing.clone(),
                        };
                        self.validate_access_backing(&captured)?;
                        storage.stage_reference(
                            StructuralRuntimePlace {
                                opaque_identity: value.opaque_identity,
                                path: vec![
                                    StructuralPathSegment::Case(ready.id),
                                    StructuralPathSegment::Field(field.identity),
                                ],
                            },
                            crate::storage::reference::ReferenceReferent {
                                view: backing,
                                primitive_loan: None,
                                collection_loan: None,
                                byte_region: None,
                                byte_owner: None,
                                aggregate_view: None,
                                placed: Some(captured),
                            },
                        )?;
                    }
                    Err(rejection) => {
                        let [_, reason_field] = rejected.fields.as_slice() else {
                            return Err(malformed());
                        };
                        storage.stage_case(root.clone(), result.structural_type, rejected.id)?;
                        crate::storage::provider_storage::initialization::stage_rejection_failure(
                            &mut storage,
                            rejection,
                            failures,
                            reason_field,
                            value.opaque_identity,
                            rejected.id,
                        )?;
                    }
                }
                storage.validate_projected_memberships(
                    self,
                    &value,
                    &result.projected_qualifications,
                )?;
                let mut frontier = self.live_affine_frontier.clone();
                self.stage_result_affine_frontier(
                    result,
                    &mut frontier,
                    &|region: &StructuralAffineDiscard| {
                        storage.case_at_projection(self, &value, &region.path)
                    },
                )?;
                let storage = storage.into_detached()?;
                storage.install(self)?;
                self.structural_values.insert(result.place, value)?;
                self.live_affine_frontier = frontier;
                self.local_structural_identities
                    .commit_cursor(identity_cursor);
                Ok(())
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct AccessReference {
        storage: AccessStorage,
        pub(crate) presentation: TerminalStructuralValue,
        pub(crate) access: StructuralAccess,
        pub(crate) selected_field: Option<u32>,
        projection: Vec<terminal_psi::StructuralPathSegment>,
        pub(crate) source: TerminalStructuralValue,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum AccessStorage {
        Ram {
            correspondence: terminal_semantics::AccessRamCorrespondence,
            home: AccessRamHome,
            byte_offset: u64,
        },
        Resident {
            carrier: semantic_vocabulary::StructuralTypeId,
            owner: StructuralRuntimePlace,
            binding: crate::storage::provider_storage::ProviderStorageBinding,
        },
        External {
            carrier: semantic_vocabulary::StructuralTypeId,
            owner: StructuralRuntimePlace,
            binding: crate::storage::provider_storage::ProviderStorageBinding,
            compatibility: access_plans::AccessResourceCompatibility,
        },
    }

    impl AccessReference {
        pub(crate) fn carrier(&self) -> semantic_vocabulary::StructuralTypeId {
            match &self.storage {
                AccessStorage::Ram { correspondence, .. } => correspondence.carrier,
                AccessStorage::Resident { carrier, .. }
                | AccessStorage::External { carrier, .. } => *carrier,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum AccessRamHome {
        Frame {
            depth: usize,
            machine: MachineId,
            place: PlaceId,
            allocation: TerminalStructuralValue,
        },
        PrivateEntry {
            recipe: terminal_installation::CheckedPrivateEntryRamRecipe,
            allocation: TerminalStructuralValue,
        },
    }

    pub(crate) struct PrivateEntryRamAllocation {
        pub(crate) recipe: terminal_installation::CheckedPrivateEntryRamRecipe,
        pub(crate) allocation: TerminalStructuralValue,
    }

    impl TerminalExecution<'_> {
        pub(crate) fn retain_returned_access_interpretation(
            &self,
            projected: &mut super::ResolvedStructuralProjection,
            actual: &super::ReferenceReferent,
            referent: semantic_vocabulary::StructuralTypeId,
            access: StructuralAccess,
        ) -> Result<(), TerminalInterpretError> {
            let Some(captured) = &actual.placed else {
                return Ok(());
            };
            self.validate_access_backing(captured)?;
            if projected.placed.is_some() {
                return Ok(());
            }
            if captured.selected_field.is_none()
                && captured.presentation.structural_type == referent
                && captured.carrier() == referent
                && captured.source == actual.view
                && projected.value == captured.source
                && projected.view_tip.is_none()
                && actual.aggregate_view.is_none()
                && super::access_covers(captured.access, access)
            {
                projected.placed = Some(captured.clone());
            }
            Ok(())
        }

        pub(crate) fn validate_access_backing(
            &self,
            captured: &AccessReference,
        ) -> Result<(), TerminalInterpretError> {
            let home = match &captured.storage {
                AccessStorage::Ram { home, .. } => home,
                AccessStorage::Resident { owner, binding, .. }
                | AccessStorage::External { owner, binding, .. } => {
                    return if self.provider_storage.descriptors.get(owner) == Some(binding)
                        && self.is_live_provider_descriptor(owner, binding)
                    {
                        Ok(())
                    } else {
                        Err(TerminalInterpretError::ContentTransformationMismatch(
                            "resident reference lost its original live owner or backing",
                        ))
                    };
                }
            };
            match home {
                AccessRamHome::Frame {
                    depth,
                    machine,
                    place,
                    allocation,
                } => {
                    let (actual_machine, values) = if *depth == self.call_stack.len() {
                        (self.current_machine, &self.structural_values)
                    } else {
                        let frame = self.call_stack.get(*depth).ok_or_else(malformed)?;
                        (frame.current_machine, &frame.structural_values)
                    };
                    if actual_machine != *machine || values.get(place) != Some(allocation) {
                        return Err(malformed());
                    }
                }
                AccessRamHome::PrivateEntry { recipe, allocation } => {
                    if self.result.is_some()
                        || !self.private_entry_ram.iter().any(|original| {
                            original.recipe == *recipe && original.allocation == *allocation
                        })
                    {
                        return Err(malformed());
                    }
                }
            }
            Ok(())
        }

        pub(crate) fn establish_private_entry_ram(
            &mut self,
            recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
            establishments: &[terminal_installation::TerminalAccessViewEstablishment],
        ) -> Result<(), TerminalInterpretError> {
            for recipe in recipes {
                if recipe.terminal() != self.program.identity.semantic()
                    || recipe.input().machine != self.current_machine
                {
                    return Err(malformed());
                }
                let establishment = establishments
                    .iter()
                    .find(|loan| loan.input == *recipe.input())
                    .ok_or_else(malformed)?;
                if establishment.referent.structural_type != recipe.schema()
                    || !establishment.referent.path.is_empty()
                    || !establishment.referent.qualifications.is_empty()
                    || self.private_entry_ram.iter().any(|original| {
                        original.allocation.opaque_identity
                            == establishment.referent.opaque_identity
                    })
                {
                    return Err(malformed());
                }
                let allocation = establishment.referent.clone();
                for field in recipe.fields() {
                    let value = match field.value {
                        semantic_vocabulary::ScalarTerm::Boolean(value) => {
                            crate::TerminalScalarValue::Boolean(value)
                        }
                        semantic_vocabulary::ScalarTerm::Integer { scalar_type, value } => {
                            crate::TerminalScalarValue::Integer { scalar_type, value }
                        }
                        _ => return Err(malformed()),
                    };
                    if self
                        .structural_scalar_fields
                        .insert(
                            StructuralScalarRuntimeField::member(
                                StructuralRuntimePlace::from(&allocation),
                                field.field,
                            ),
                            value,
                        )?
                        .is_some()
                    {
                        return Err(malformed());
                    }
                }
                let occurrence = self
                    .access_view_occurrences
                    .get_mut(recipe.input())
                    .ok_or_else(malformed)?;
                let parameter = terminal_semantics::access_view_input_parameter(
                    &self.program.catalogs,
                    recipe.input(),
                )
                .map_err(|_| malformed())?;
                let presentation = self.structural_values.require(&parameter.place)?.clone();
                occurrence.ram = Some(AccessReference {
                    storage: AccessStorage::Ram {
                        correspondence: recipe.correspondence().clone(),
                        home: AccessRamHome::PrivateEntry {
                            recipe: recipe.clone(),
                            allocation: allocation.clone(),
                        },
                        byte_offset: 0,
                    },
                    presentation,
                    access: recipe.input().access,
                    selected_field: None,
                    projection: Vec::new(),
                    source: allocation.clone(),
                });
                self.private_entry_ram.push(PrivateEntryRamAllocation {
                    recipe: recipe.clone(),
                    allocation,
                });
            }
            Ok(())
        }

        pub(crate) fn observe_private_entry_ram(
            &self,
        ) -> Result<Vec<(u32, Vec<u8>)>, TerminalInterpretError> {
            if self.private_entry_ram.is_empty() {
                return Ok(Vec::new());
            }
            if self.result != Some(crate::TerminalExecutionResult::Unit)
                || !self.call_stack.is_empty()
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "private entry RAM observation requires normal Unit return and retired loans",
                ));
            }
            self.private_entry_ram
                .iter()
                .map(|original| {
                    let mut bytes = vec![0; original.recipe.contents().len()];
                    for field in original.recipe.fields() {
                        let value = self
                            .structural_scalar_fields
                            .get(&StructuralScalarRuntimeField::member(
                                StructuralRuntimePlace::from(&original.allocation),
                                field.field,
                            ))
                            .ok_or_else(malformed)?
                            .read()?;
                        let bits = match value {
                            crate::TerminalScalarValue::Boolean(value) => u128::from(value),
                            crate::TerminalScalarValue::Integer {
                                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                                ..
                            } => value,
                            crate::TerminalScalarValue::Integer {
                                value: semantic_vocabulary::IntegerValue::Signed(value),
                                ..
                            } => value as u128,
                            _ => return Err(malformed()),
                        };
                        let end = field
                            .byte_offset
                            .checked_add(field.byte_width)
                            .ok_or_else(malformed)?;
                        bytes
                            .get_mut(field.byte_offset..end)
                            .ok_or_else(malformed)?
                            .copy_from_slice(&bits.to_le_bytes()[..field.byte_width]);
                    }
                    Ok((original.recipe.input().position, bytes))
                })
                .collect()
        }

        fn capture_access_ram_home(
            &self,
            source: TerminalStructuralValue,
            access: StructuralAccess,
            correspondence: terminal_semantics::AccessRamCorrespondence,
        ) -> Result<AccessReference, TerminalInterpretError> {
            let original = self.original_owned_allocation(&source)?;
            let depth = original.depth;
            let machine = original.machine;
            let place = original.place;
            let allocation = original.value;
            if terminal_semantics::contains_access_carrier(
                &self.program.catalogs,
                allocation.structural_type,
            ) {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "placed RAM original allocation contains placed backing",
                ));
            }
            let path = &source.path[allocation.path.len()..];
            let types = self.structural_types();
            let (size, alignment) = terminal_semantics::reference_storage_layout(
                types.values(),
                allocation.structural_type,
            )
            .ok_or_else(malformed)?;
            let region = terminal_semantics::reference_storage_region(
                types.values(),
                allocation.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            let tip = terminal_semantics::runtime_structural_path_tip(
                types.values(),
                allocation.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            if tip.id != correspondence.schema
                || region.scalar.is_some()
                || region.byte_length != correspondence.source_byte_length
                || region
                    .byte_offset
                    .checked_add(region.byte_length)
                    .is_none_or(|end| end > size)
                || alignment % correspondence.required_base.modulus() != 0
                || !correspondence.required_base.admits(region.byte_offset)
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "placed RAM source does not meet its captured allocation geometry",
                ));
            }
            let mut presentation = source.clone();
            presentation.structural_type = correspondence.carrier;
            presentation.qualifications.clear();
            presentation.projected_qualifications.clear();
            let captured = AccessReference {
                storage: AccessStorage::Ram {
                    correspondence,
                    home: AccessRamHome::Frame {
                        depth,
                        machine,
                        place,
                        allocation,
                    },
                    byte_offset: region.byte_offset,
                },
                presentation,
                access,
                selected_field: None,
                projection: Vec::new(),
                source: source.clone(),
            };
            let StructuralTypeShape::Record { fields, .. } =
                &self.structural_declaration(source.structural_type)?.shape
            else {
                return Err(malformed());
            };
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let Some(scalar) = field.field_type.scalar_type() else {
                    let root = field.field_type.as_structural().ok_or_else(malformed)?;
                    let (scalar, count) = terminal_semantics::scalar_array_leaf_shape(
                        self.structural_types().values(),
                        root,
                    )
                    .ok_or(
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "placed RAM initialization requires a supported scalar storage tree",
                        ),
                    )?;
                    let AccessStorage::Ram {
                        home:
                            AccessRamHome::Frame {
                                depth,
                                machine,
                                place,
                                allocation,
                            },
                        ..
                    } = &captured.storage
                    else {
                        return Err(malformed());
                    };
                    let home =
                        crate::storage::primitive_storage::PrimitiveStorageHome::captured_frame(
                            *depth,
                            *machine,
                            *place,
                            allocation.structural_type,
                        );
                    let mut path = source.path[allocation.path.len()..].to_vec();
                    path.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                    let prefix_length = path.len();
                    for ordinal in 0..count {
                        path.truncate(prefix_length);
                        path.extend(
                            terminal_semantics::scalar_array_leaf_path(
                                self.structural_types().values(),
                                root,
                                ordinal,
                            )
                            .ok_or_else(malformed)?,
                        );
                        let value = self.read_primitive_home(home, &path, scalar)?;
                        if !crate::engine::scalar_operations::terminal_scalar_belongs_to_type(value)
                        {
                            return Err(malformed());
                        }
                    }
                    continue;
                };
                let value = self
                    .structural_scalar_fields
                    .get(&StructuralScalarRuntimeField::member(
                        StructuralRuntimePlace::from(&source),
                        field.id,
                    ))
                    .ok_or_else(malformed)?
                    .read()?;
                if value.scalar_type() != scalar || !value.satisfies_field(&field.field_type) {
                    return Err(malformed());
                }
            }
            self.validate_access_backing(&captured)?;
            Ok(captured)
        }

        fn capture_resident_reference(
            &self,
            source: &TerminalStructuralValue,
            access: StructuralAccess,
            result_type: semantic_vocabulary::StructuralTypeId,
            roles: terminal_psi::ResidentReferenceSource,
        ) -> Result<AccessReference, TerminalInterpretError> {
            let plan = terminal_semantics::resident_reference_plan(
                &self.program.catalogs,
                source.structural_type,
                result_type,
                access,
                roles,
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            let owner = StructuralRuntimePlace::from(source);
            let binding = self
                .provider_storage
                .descriptors
                .get(&owner)
                .copied()
                .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                    "resident reference has no actual provider backing",
                ))?;
            if binding.resident == arena::Handle::default()
                || !self.is_live_provider_descriptor(&owner, &binding)
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "resident reference has no live original resident claim",
                ));
            }
            let (initialized, custody, compatibility) =
                self.live_resident_custody(binding.resident)?;
            if source.structural_type != initialized.storage.carrier
                || !source.qualifications.contains(&roles.granted)
                || !source.qualifications.contains(&roles.resident)
                || initialized
                    .storage
                    .vacant
                    .is_some_and(|vacant| source.qualifications.contains(&vacant))
                || initialized.storage.granted != roles.granted
                || initialized.storage.resident != Some(roles.resident)
                || initialized.storage.base != roles.base
                || initialized.storage.length != roles.length
                || initialized.schema != plan.schema
                || custody.value.structural_type != plan.schema
                || initialized.policy_identity != plan.policy_identity
                || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
                || initialized.schema_identity != plan.schema_identity
                || initialized.placement != plan.placement
                || plan.placement.layout().size != Some(compatibility.byte_length())
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "resident reference changed its original owner, policy or schema",
                ));
            }
            let (_, _, base) = self.provider_storage.live_provider_allocation(&binding)?;
            if !self.provider_storage_geometry_matches(
                &owner,
                &initialized.storage,
                base,
                binding.length,
            )? {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "resident reference changed its original allocation geometry",
                ));
            }
            let mut presentation = source.clone();
            presentation.structural_type = plan.carrier;
            presentation.qualifications.clear();
            presentation.projected_qualifications.clear();
            let (_, result_access) = self.reference_contract(result_type)?;
            Ok(AccessReference {
                storage: AccessStorage::Resident {
                    carrier: plan.carrier,
                    owner,
                    binding,
                },
                presentation,
                access: result_access,
                selected_field: None,
                projection: Vec::new(),
                source: source.clone(),
            })
        }

        pub(crate) fn establish_access_reference(
            &mut self,
            operation: &Operation,
            source: &StructuralArgument,
        ) -> Result<(), TerminalInterpretError> {
            let result = operation.result.structural().ok_or_else(malformed)?;
            let backing = self
                .resolve_reference_call_arguments(std::slice::from_ref(source))?
                .pop()
                .ok_or_else(malformed)?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || self.structural_values.contains_key(&result.place)
                || self.live_affine_discard_at(result.place).is_some()
            {
                return Err(malformed());
            }
            let (_, access) = self.reference_contract(result.structural_type)?;
            let terminal_psi::OperationKind::EstablishAccessReference { resident, .. } =
                operation.kind
            else {
                return Err(malformed());
            };
            let captured = if let Some(resident) = resident {
                self.capture_resident_reference(
                    &backing,
                    source.access,
                    result.structural_type,
                    resident,
                )?
            } else {
                let correspondence = terminal_semantics::access_reference_correspondence(
                    &self.program.catalogs,
                    backing.structural_type,
                    result.structural_type,
                    source.access,
                )
                .map_err(TerminalInterpretError::AccessReference)?;
                self.capture_access_ram_home(backing.clone(), access, correspondence)?
            };
            let carrier = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                result.structural_type,
                Vec::new(),
            );
            self.reference_referents.insert(
                StructuralRuntimePlace::from(&carrier),
                super::ReferenceReferent {
                    view: backing,
                    primitive_loan: None,
                    collection_loan: None,
                    byte_region: None,
                    byte_owner: None,
                    aggregate_view: None,
                    placed: Some(captured),
                },
            )?;
            self.structural_values.insert(result.place, carrier)?;
            self.live_affine_frontier
                .insert(whole_path_discard(result.place, result.structural_type));
            Ok(())
        }
        pub(crate) fn project_access_presentation(
            &self,
            captured: &mut AccessReference,
            prefix: &[terminal_psi::StructuralPathSegment],
            presentation: &TerminalStructuralValue,
        ) -> Result<(), TerminalInterpretError> {
            self.validate_access_backing(captured)?;
            if !prefix.is_empty() {
                let plan = self
                    .program
                    .catalogs
                    .access_view_plans
                    .iter()
                    .find(|plan| plan.carrier == captured.carrier())
                    .ok_or_else(malformed)?;
                let (field, relative) = if let Some(slot) = captured.selected_field {
                    (
                        plan.fields
                            .iter()
                            .find(|field| field.access_slot == slot)
                            .ok_or_else(malformed)?,
                        prefix,
                    )
                } else {
                    let Some((terminal_psi::StructuralPathSegment::Field(identity), relative)) =
                        prefix.split_first()
                    else {
                        return Err(malformed());
                    };
                    (
                        plan.fields
                            .iter()
                            .find(|field| field.view_field == *identity)
                            .ok_or_else(malformed)?,
                        relative,
                    )
                };
                let mut projection = captured.projection.clone();
                projection.extend_from_slice(relative);
                let selected = terminal_semantics::runtime_structural_path_tip(
                    self.structural_types().values(),
                    field.accessor,
                    &projection,
                )
                .ok_or_else(malformed)?;
                if presentation.structural_type != selected.id {
                    return Err(malformed());
                }
                captured.selected_field = Some(field.access_slot);
                captured.projection = projection;
            }
            captured.presentation = presentation.clone();
            Ok(())
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn execute_access_boundary(
            &mut self,
            operation: &Operation,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            arguments: &[crate::boundary::values::TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
        ) -> Result<bool, TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let mut operations = program.catalogs.access_view_plans.iter().flat_map(|plan| {
                plan.fields.iter().flat_map(move |field| {
                    field
                        .operations
                        .iter()
                        .filter(move |accessor| accessor.boundary == declaration.id)
                        .map(move |accessor| (plan, field, accessor.kind))
                })
            });
            let Some((plan, field, kind)) = operations.next() else {
                return Ok(false);
            };
            if operations.next().is_some() {
                return Err(malformed());
            }
            let [argument] = structural_arguments else {
                return Err(malformed());
            };
            let (presentation, Some(ram)) = self.resolve_live_access_projection(argument)? else {
                return Ok(false);
            };
            self.validate_access_backing(&ram)?;
            let terminal_psi::OperationKind::BoundaryCall {
                completion_receipts,
                ..
            } = &operation.kind
            else {
                return Err(malformed());
            };
            if !completion_receipts.is_empty() {
                return Err(malformed());
            }
            let required = match kind {
                terminal_psi::TerminalAccessorKind::Read => StructuralAccess::SharedBorrow,
                terminal_psi::TerminalAccessorKind::Write => StructuralAccess::MutableBorrow,
                terminal_psi::TerminalAccessorKind::Take => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "ordinary RAM does not establish destructive External supply",
                    ));
                }
            };
            if argument.access != required
                || !super::access_covers(ram.access, required)
                || ram.carrier() != plan.carrier
                || ram.selected_field != Some(field.access_slot)
                || declaration
                    .structural_parameters
                    .first()
                    .is_none_or(|parameter| {
                        presentation.structural_type != parameter.structural_type
                    })
            {
                return Err(malformed());
            }
            crate::engine::custody::bind_structural_arguments(
                &declaration.structural_parameters,
                std::slice::from_ref(&presentation),
                self.account.as_ref(),
            )?;
            crate::engine::custody::validate_boundary_requirements(declaration, |index| {
                (index == 0).then_some(&presentation)
            })?;
            self.preflight_boundary_result(declaration, &operation.result)?;
            if kind == terminal_psi::TerminalAccessorKind::Write
                && (declaration.result != terminal_psi::BoundaryMachineResult::Unit
                    || operation.result != terminal_psi::OperationResult::Unit)
            {
                return Err(malformed());
            }
            let returned = match &ram.storage {
                AccessStorage::Resident { binding, .. } => self.execute_provider_field(
                    *binding,
                    crate::storage::provider_storage::field_access::ProviderFieldContent::Resident,
                    plan,
                    field,
                    &ram.projection,
                    kind,
                    ram.access,
                    arguments,
                )?,
                AccessStorage::External {
                    binding,
                    compatibility,
                    ..
                } => self.execute_provider_field(
                    *binding,
                    crate::storage::provider_storage::field_access::ProviderFieldContent::Borrowed(
                        compatibility,
                    ),
                    plan,
                    field,
                    &ram.projection,
                    kind,
                    ram.access,
                    arguments,
                )?,
                AccessStorage::Ram {
                    correspondence,
                    byte_offset,
                    home,
                } => {
                    let source = correspondence
                        .fields
                        .iter()
                        .find(|source| source.access_slot == field.access_slot)
                        .ok_or_else(malformed)?;
                    let selected_projection = if ram.projection.is_empty() {
                        terminal_semantics::ReferenceStorageProjection {
                            byte_offset: source.byte_offset,
                            logical_scalar: source.logical_scalar,
                            stored_scalar: source.stored_scalar,
                        }
                    } else {
                        terminal_semantics::access_region_projection(
                            &program.catalogs.structural_types,
                            plan,
                            field,
                            &ram.projection,
                        )
                        .map_err(TerminalInterpretError::AccessReference)?
                    };
                    let width = u64::from(source.transfer_width_bits)
                        .checked_div(8)
                        .ok_or_else(malformed)?;
                    if width == 0
                        || source.transfer_width_bits % 8 != 0
                        || selected_projection
                            .byte_offset
                            .checked_add(width)
                            .is_none_or(|end| end > correspondence.source_byte_length)
                        || byte_offset
                            .checked_add(selected_projection.byte_offset)
                            .is_none()
                    {
                        return Err(malformed());
                    }
                    let StructuralTypeShape::Record { fields, .. } = &program
                        .structural_declaration(ram.source.structural_type)?
                        .shape
                    else {
                        return Err(malformed());
                    };
                    let source_field = fields
                        .iter()
                        .find(|field| field.id == source.source_field)
                        .ok_or_else(malformed)?;
                    let storage = if !ram.projection.is_empty() {
                        let AccessRamHome::Frame {
                            depth,
                            machine,
                            place,
                            allocation,
                        } = home
                        else {
                            return Err(malformed());
                        };
                        let mut path = ram.source.path[allocation.path.len()..].to_vec();
                        path.push(terminal_psi::StructuralPathSegment::Field(
                            source_field.identity,
                        ));
                        path.extend_from_slice(&ram.projection);
                        let (storage, scalar, _) = self.primitive_home_access(
                            crate::storage::primitive_storage::PrimitiveStorageHome::captured_frame(
                                *depth,
                                *machine,
                                *place,
                                allocation.structural_type,
                            ),
                            kind == terminal_psi::TerminalAccessorKind::Write,
                            &path,
                        )?;
                        if scalar != selected_projection.logical_scalar {
                            return Err(malformed());
                        }
                        storage
                    } else {
                        let cell = StructuralScalarRuntimeField::member(
                            StructuralRuntimePlace::from(&ram.source),
                            source.source_field,
                        );
                        crate::storage::primitive_storage::PrimitiveStorage {
                        endpoint:
                            crate::storage::primitive_storage::PrimitiveStorageEndpoint::ScalarField(
                                cell,
                            ),
                        encoding: Some(
                            crate::storage::primitive_storage::PrimitiveStorageEncoding {
                                source: source.stored_scalar,
                                view: source.stored_scalar,
                            },
                        ),
                    }
                    };
                    let value_type = if ram.projection.is_empty() {
                        source_field.field_type.clone()
                    } else {
                        terminal_psi::StructuralFieldType::Scalar(
                            selected_projection.logical_scalar,
                        )
                    };
                    let write = match (kind, arguments) {
                        (terminal_psi::TerminalAccessorKind::Read, []) => None,
                        (terminal_psi::TerminalAccessorKind::Write, [value])
                            if value.scalar_type() == source.logical_scalar
                                && value.satisfies_field(&value_type) =>
                        {
                            Some(*value)
                        }
                        _ => return Err(malformed()),
                    };
                    if write.is_some()
                        && (!ram.source.qualifications.is_empty()
                            || !ram.source.projected_qualifications.is_empty())
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "qualified RAM writes require source frame correspondence",
                        ));
                    }
                    if let Some(value) = write {
                        self.store_primitive_storage(
                            argument.place,
                            storage,
                            source.logical_scalar,
                            value,
                        )?;
                        crate::boundary::effects::results::TerminalEffectResult::Unit
                    } else {
                        let value = self.read_primitive_storage(
                            argument.place,
                            storage,
                            source.logical_scalar,
                            source.logical_scalar,
                            false,
                        )?;
                        if !value.satisfies_field(&value_type) {
                            return Err(malformed());
                        }
                        crate::boundary::effects::results::TerminalEffectResult::Scalar(value)
                    }
                }
            };
            self.commit_boundary_result(&operation.result, &declaration.result, returned)?;
            self.effects
                .push(crate::boundary::effects::TerminalEffect::BoundaryCall {
                    site: crate::TerminalEffectWait {
                        program: self.program.identity.clone(),
                        machine: self.current_machine,
                        block: self.current,
                        operation: operation.id,
                        boundary: declaration.id,
                    },
                    arguments: arguments.to_vec(),
                    structural_arguments: vec![presentation],
                    byte_sequence_arguments: vec![None],
                    completion_receipts: Vec::new(),
                    result: declaration.result.clone(),
                });
            Ok(true)
        }
    }
}
pub(crate) use aggregate_views::{
    AggregateBacking, AggregateViewRegion, backing_scalar_path, scalar_storage_fragments,
    translate_aggregate_view_field, translate_aggregate_view_path,
};
pub(crate) use placed::AccessReference;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ReferenceByteRegion {
    pub(crate) endpoint: crate::storage::primitive_storage::PrimitiveStorageEndpoint,
    pub(crate) byte_length: usize,
}

pub(crate) fn discard_structural_value(
    values: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    referents: &mut RuntimeRows<StructuralRuntimePlace, ReferenceReferent>,
    element_views: &mut RuntimeRows<
        StructuralRuntimePlace,
        crate::storage::element_views::ElementView,
    >,
    retired: &mut Vec<(PlaceId, TerminalStructuralValue, Option<ReferenceReferent>)>,
    place: PlaceId,
    machine: &terminal_psi::TerminalMachine,
) -> Option<TerminalStructuralValue> {
    let value = values.remove(&place)?;
    let origin = referents
        .get(&StructuralRuntimePlace::from(&value))
        .cloned();
    retired.push((place, value.clone(), origin));
    if machine
        .structural_parameter(place)
        .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
    {
        return Some(value);
    }
    referents.retain(|carrier, _| {
        carrier.opaque_identity != value.opaque_identity || !carrier.path.starts_with(&value.path)
    });
    element_views.retain(|carrier, _| {
        carrier.opaque_identity != value.opaque_identity || !carrier.path.starts_with(&value.path)
    });
    Some(value)
}

impl TerminalExecution<'_> {
    pub(crate) fn reference_carrier_access(
        &self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<StructuralAccess, TerminalInterpretError> {
        let authority = self.storage_carrier_access(place, structural_type, path)?;
        (authority != StructuralAccess::WriteOnlyBorrow)
            .then_some(authority)
            .ok_or_else(malformed)
    }

    pub(crate) fn storage_carrier_access(
        &self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<StructuralAccess, TerminalInterpretError> {
        let machine = self.program.machine(self.current_machine)?;
        let mut authority = StructuralAccess::Owned;
        if let Some(parameter) = machine
            .structural_parameter(place)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
        {
            if parameter.structural_type != structural_type
                || parameter.multiplicity == StructuralMultiplicity::Linear
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
            {
                return Err(malformed());
            }
            authority = parameter.access;
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                return Ok(authority);
            }
        }
        let unrestricted = machine
            .structural_parameter(place)
            .map(|parameter| (parameter.structural_type, parameter.multiplicity))
            .or_else(|| {
                let (operation, structural_type) = machine.producing_operation(place)?;
                let result = operation.result.structural()?;
                (result.place == place && result.structural_type == structural_type)
                    .then_some((structural_type, result.multiplicity))
            })
            .is_some_and(|(declared_type, multiplicity)| {
                declared_type == structural_type
                    && multiplicity == StructuralMultiplicity::Unrestricted
            });
        if unrestricted {
            return Ok(StructuralAccess::Owned);
        }
        let prefix = &path[..path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)
            .unwrap_or(path.len())];
        let resident = self.live_affine_frontier.iter().any(|entry| {
            entry.place == place
                && prefix.starts_with(&entry.path)
                && (!entry.path.is_empty() || entry.structural_type == structural_type)
        }) || self.live_claims.values().any(|claim| {
            claim.place == Some(place)
                && claim.multiplicity == Some(StructuralMultiplicity::Linear)
                && prefix.starts_with(&claim.path)
        });
        if resident || self.resident_storage_region(place, structural_type, prefix)? {
            Ok(authority)
        } else {
            Err(malformed())
        }
    }

    fn resident_storage_region(
        &self,
        place: PlaceId,
        root_type: StructuralTypeId,
        prefix: &[StructuralPathSegment],
    ) -> Result<bool, TerminalInterpretError> {
        let types = self.structural_types();
        crate::boundary::structural_inputs::case_membership::validate_case_projection(
            &types,
            &self.structural_values,
            self.structural_cases.as_map(),
            place,
            prefix,
        )?;
        let selected_type = resolve_structural_path_type(&types, root_type, prefix)?;
        let root =
            crate::engine::custody::verified_structural_place(&self.structural_values, place)?;
        let mut remaining = std::collections::BTreeSet::from([StructuralAffineDiscard {
            place,
            path: prefix.to_vec(),
            structural_type: selected_type,
        }]);
        let claims = self
            .live_claims
            .values()
            .filter(|claim| {
                claim.place == Some(place)
                    && claim.multiplicity == Some(StructuralMultiplicity::Linear)
                    && claim.path.starts_with(prefix)
            })
            .map(|claim| claim.path.as_slice());
        let affine = self
            .live_affine_frontier
            .iter()
            .filter(|entry| entry.place == place && entry.path.starts_with(prefix))
            .map(|entry| entry.path.as_slice());
        crate::engine::custody::exclude_linear_regions(
            &types,
            &mut remaining,
            claims.chain(affine).map(|path| (place, path)),
            &|entry| {
                crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                    &types,
                    root,
                    self.structural_cases.as_map(),
                    &entry.path,
                )
            },
        )?;
        Ok(remaining.is_empty())
    }

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
        let root_type = self.live_structural_type(source.place)?;
        let machine = self.program.verified_machine(self.current_machine)?;
        if let Some(parameter) = self.owned_block_source(source.place) {
            if parameter.structural_type != root_type {
                return Err(malformed());
            }
            if parameter.access != StructuralAccess::Owned {
                return (parameter.multiplicity != StructuralMultiplicity::Linear
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
        let root = self.structural_value(source.place)?;
        self.reference_carrier_access(source.place, root.structural_type, &source.path)?;
        Ok(())
    }

    pub(crate) fn reference_contract(
        &self,
        reference_type: StructuralTypeId,
    ) -> Result<(StructuralTypeId, StructuralAccess), TerminalInterpretError> {
        let Some(StructuralTypeShape::Reference { referent, access }) = self
            .structural_types()
            .get(&reference_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(malformed());
        };
        if !self.structural_types().contains_key(referent) || *access == StructuralAccess::Owned {
            return Err(malformed());
        }
        Ok((*referent, *access))
    }

    pub(crate) fn capture_reference(
        &self,
        source: &StructuralArgument,
        reference_type: StructuralTypeId,
        projected_qualifications: &[terminal_psi::StructuralPathQualification],
        storage: Option<terminal_psi::ReferenceStorageView>,
        identity_cursor: &mut Option<u64>,
    ) -> Result<PreparedReference, TerminalInterpretError> {
        let (referent_type, access) = self.reference_contract(reference_type)?;
        let mut normalized_source = source.clone();
        normalized_source.path = terminal_semantics::reference_source_storage_path(
            self.structural_types().values(),
            self.live_structural_type(source.place)?,
            &source.path,
        )
        .to_vec();
        let source = &normalized_source;
        let mut byte_region = None;
        let mut region_referent = None;
        let mut aggregate_view = None;
        let mut collection_loan = if storage.is_none() && access_covers(source.access, access) {
            let argument = StructuralArgument {
                access,
                ..source.clone()
            };
            self.prepare_scalar_array_reference(
                referent_type,
                &argument,
                identity_cursor,
                &mut RuntimeRows::new(self.account.as_ref()),
            )
            .map_err(|error| error.in_reference_formation("capture collection backing"))?
        } else {
            None
        };
        let field_compatible = storage.is_none()
            && (terminal_semantics::reference_source_projection_compatible(
                self.structural_types().values(),
                self.live_structural_type(source.place)?,
                &source.path,
                reference_type,
            ) || terminal_semantics::reference_byte_field_compatible(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                self.live_structural_type(source.place)?,
                &source.path,
                reference_type,
                projected_qualifications,
            ));
        let byte_owner_loan = if storage.is_none()
            && self
                .structural_types()
                .get(&referent_type)
                .is_some_and(|declaration| {
                    declaration.shape.is_bounded_owned_byte_sequence()
                        || declaration.shape.is_borrowed_byte_carrier()
                }) {
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &[],
                    projected_qualifications,
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
                additional_input_qualifications: Vec::new(),
            };
            Some(
                self.prepare_byte_sequence_loan(
                    &parameter,
                    source,
                    identity_cursor,
                    &mut RuntimeRows::new(self.account.as_ref()),
                )?
                .ok_or_else(malformed)?,
            )
        } else {
            None
        };
        let primitive_loan = if matches!(
            self.structural_declaration(referent_type)?.shape,
            StructuralTypeShape::PrimitiveScalar(_)
        ) {
            let root_type = self.live_structural_type(source.place)?;
            let observed = (source.access == StructuralAccess::SharedBorrow)
                .then(|| {
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        self.structural_types().values(),
                        root_type,
                        &source.path,
                        None,
                    )
                })
                .flatten();
            let original_type = if let Some(observed) = observed {
                observed
            } else if let Some(tip) = terminal_semantics::runtime_structural_path_tip(
                self.structural_types().values(),
                root_type,
                &source.path,
            ) {
                tip.id
            } else if let Some(byte) = terminal_semantics::primitive_projection_declaration(
                self.structural_types().values(),
                root_type,
                &source.path,
            ) {
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
                additional_input_qualifications: Vec::new(),
            };
            let mut loan = self
                .capture_primitive_loan(&parameter, source, identity_cursor)
                .map_err(|error| error.in_reference_formation("capture scalar backing"))?;
            if let Some(storage) = storage {
                let stored_scalar = storage.stored_scalar.ok_or_else(malformed)?;
                let crate::boundary::values::TerminalScalarValue::Integer {
                    scalar_type: offset_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte_offset),
                } = self.scalar_value(storage.byte_offset)?
                else {
                    return Err(malformed());
                };
                let StructuralTypeShape::PrimitiveScalar(logical_scalar) =
                    self.structural_declaration(referent_type)?.shape
                else {
                    return Err(malformed());
                };
                if offset_type.is_address()
                    || offset_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
                    || !terminal_semantics::reference_storage_source_compatible(
                        self.structural_types().values(),
                        original_type,
                        reference_type,
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
                )
                .map_err(|error| error.in_reference_formation("position scalar storage view"))?;
                loan.referent.structural_type = referent_type;
            }
            loan.access = access;
            Some(loan)
        } else {
            if storage.is_none() && byte_owner_loan.is_none() && collection_loan.is_none() {
                let projected = self
                    .resolve_live_storage_projection(source.place, &source.path, source.access)
                    .map_err(|error| error.in_reference_formation("resolve source view"))?;
                let (backing, tip) = (projected.value, projected.view_tip);
                if let Some(AggregateViewTip::Aggregate {
                    view,
                    backing: actual,
                }) = tip
                {
                    if view.structural_type != referent_type || actual != backing.structural_type {
                        return Err(malformed());
                    }
                    aggregate_view = Some(view);
                    collection_loan = projected
                        .array_loan
                        .map(|loan| -> Result<_, TerminalInterpretError> {
                            let attenuated =
                                loan.subslice(0, loan.len(), access).ok_or_else(malformed)?;
                            Ok((backing.clone(), attenuated))
                        })
                        .transpose()?;
                    region_referent = Some(backing);
                }
            }
            if let Some(storage) = storage {
                if storage.stored_scalar.is_some() {
                    return Err(malformed());
                }
                let crate::boundary::values::TerminalScalarValue::Integer {
                    scalar_type: offset_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte_offset),
                } = self.scalar_value(storage.byte_offset)?
                else {
                    return Err(malformed());
                };
                let root_type = self.live_structural_type(source.place)?;
                let tip = terminal_semantics::runtime_structural_path_tip(
                    self.structural_types().values(),
                    root_type,
                    &source.path,
                )
                .map(|tip| tip.id);
                let inline_byte = match tip {
                    Some(_) => None,
                    None => terminal_semantics::primitive_projection_declaration(
                        self.structural_types().values(),
                        root_type,
                        &source.path,
                    ),
                };
                let array_window = terminal_semantics::reference_storage_region(
                    self.structural_types().values(),
                    root_type,
                    &source.path,
                )
                .and_then(|region| region.array);
                let mut backing_source = source.clone();
                if array_window.is_some() {
                    backing_source.path.pop().ok_or_else(malformed)?;
                }
                let original_type = tip
                    .or(inline_byte)
                    .or_else(|| array_window.map(|window| window.declaration))
                    .ok_or_else(malformed)?;
                let compatible = if array_window.is_some() {
                    terminal_semantics::reference_storage_region_source_compatible(
                        self.structural_types().values(),
                        root_type,
                        &source.path,
                        reference_type,
                        None,
                    )
                } else {
                    terminal_semantics::reference_storage_source_compatible(
                        self.structural_types().values(),
                        original_type,
                        reference_type,
                        None,
                    )
                };
                if offset_type.is_address()
                    || offset_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
                    || !compatible
                {
                    return Err(TerminalInterpretError::ReferenceFormation {
                        stage: "validate aggregate storage view",
                        detail: format!(
                            "source_type={original_type:?}, reference={reference_type:?}, offset_type={offset_type:?}, compatible={compatible}: {:?}",
                            malformed()
                        ),
                    });
                }
                let byte_length = usize::try_from(
                    terminal_semantics::reference_storage_size(
                        self.structural_types().values(),
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
                    self.structural_declaration(original_type)?.shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                );
                let byte_loan = if byte_endpoint {
                    let parameter = StructuralParameterDeclaration {
                        place: source.place,
                        position: 0,
                        structural_type: original_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: source.access,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    };
                    Some(
                        self.capture_primitive_loan(&parameter, source, identity_cursor)
                            .map_err(|error| {
                                error.in_reference_formation("capture byte endpoint")
                            })?,
                    )
                } else {
                    None
                };
                if collection_loan.is_none() && !byte_endpoint {
                    let argument = StructuralArgument {
                        access,
                        ..backing_source.clone()
                    };
                    collection_loan = self
                        .prepare_scalar_array_reference(
                            original_type,
                            &argument,
                            identity_cursor,
                            &mut RuntimeRows::new(self.account.as_ref()),
                        )
                        .map_err(|error| {
                            error.in_reference_formation("capture aggregate collection backing")
                        })?;
                }
                let (projected, projected_view) = match &byte_loan {
                    Some(loan) => (loan.referent.clone(), None),
                    None => {
                        if let Some((backing, _)) = &collection_loan {
                            (backing.clone(), None)
                        } else {
                            let projected = self.resolve_live_storage_projection(
                                backing_source.place,
                                &backing_source.path,
                                backing_source.access,
                            )?;
                            collection_loan = projected
                                .array_loan
                                .clone()
                                .map(|loan| (projected.value.clone(), loan));
                            (projected.value, projected.view_tip)
                        }
                    }
                };
                let relative_offset = usize::try_from(byte_offset).map_err(|_| malformed())?;
                let array_offset = array_window.map_or(0, |window| window.byte_offset);
                let byte_offset = relative_offset
                    .checked_add(usize::try_from(array_offset).map_err(|_| malformed())?)
                    .ok_or_else(malformed)?;
                let captured = self.reference_byte_region(&projected)?;
                let resident_array = if captured.is_none()
                    && let Some(loan) = byte_loan.as_ref().filter(|loan| {
                        loan.storage.encoding.is_none()
                            && matches!(
                                loan.storage.endpoint,
                                crate::storage::primitive_storage::PrimitiveStorageEndpoint::Scalar(
                                    _
                                )
                            )
                    }) {
                    self.capture_resident_array_region(
                        source,
                        loan,
                        referent_type,
                        byte_offset,
                        byte_length,
                        access,
                        identity_cursor,
                    )?
                } else {
                    None
                };
                if let Some((backing, loan, region)) = resident_array {
                    collection_loan = Some((backing.clone(), loan));
                    aggregate_view = Some(region);
                    region_referent = Some(backing);
                } else if captured.is_none() && !byte_endpoint {
                    let source_length = array_window
                        .map(|window| window.byte_length)
                        .or_else(|| {
                            terminal_semantics::reference_storage_size(
                                self.structural_types().values(),
                                original_type,
                            )
                        })
                        .and_then(|length| usize::try_from(length).ok());
                    if relative_offset != 0 || source_length != Some(byte_length) {
                        return Err(malformed());
                    }
                    let mut region =
                        AggregateViewRegion::whole(&self.structural_types(), referent_type)
                            .ok_or_else(malformed)?;
                    region.byte_offset = u64::try_from(byte_offset).map_err(|_| malformed())?;
                    if let Some(AggregateViewTip::Aggregate { view, backing }) = projected_view {
                        if backing != projected.structural_type
                            || terminal_semantics::reference_storage_size(
                                self.structural_types().values(),
                                original_type,
                            ) != Some(view.byte_length)
                            || region
                                .byte_offset
                                .checked_add(region.byte_length)
                                .is_none_or(|end| end > view.byte_length)
                        {
                            return Err(malformed());
                        }
                        region.byte_offset = view
                            .byte_offset
                            .checked_add(region.byte_offset)
                            .ok_or_else(malformed)?;
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
                                endpoint: crate::storage::primitive_storage::shifted_byte_endpoint(
                                    &region.endpoint,
                                    byte_offset,
                                )?,
                                byte_length,
                            },
                        )
                    } else {
                        let mut loan = byte_loan.ok_or_else(malformed)?;
                        self.position_reference_storage(&mut loan, byte_offset, byte_length)
                            .map_err(|error| {
                                error.in_reference_formation("position aggregate byte region")
                            })?;
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
        let mut placed = None;
        let mut referent = match &primitive_loan {
            Some(loan) => loan.referent.clone(),
            None if collection_loan.is_some() => {
                collection_loan.as_ref().ok_or_else(malformed)?.0.clone()
            }
            None if byte_owner_loan.is_some() => {
                byte_owner_loan.as_ref().ok_or_else(malformed)?.0.clone()
            }
            None if region_referent.is_some() => region_referent.ok_or_else(malformed)?,
            None => {
                let (view, captured) = self
                    .resolve_live_access_projection(source)
                    .map_err(|error| error.in_reference_formation("resolve source storage"))?;
                placed = captured;
                view
            }
        };
        if let Some(ram) = &mut placed {
            self.validate_access_backing(ram)?;
            if !access_covers(ram.access, access) {
                return Err(malformed());
            }
            ram.access = access;
        }
        if access == StructuralAccess::SharedBorrow {
            let (whole, projected) = terminal_psi::StructuralPathQualification::select(
                &[],
                projected_qualifications,
                &[StructuralPathSegment::Referent],
            );
            let observable = |identity| {
                self.program
                    .catalogs
                    .structural_domains
                    .iter()
                    .find(|domain| domain.id == identity)
                    .is_some_and(|domain| {
                        domain.content_projection.is_none()
                            && domain.establishment_routes.is_none()
                            && domain.has_no_required_memberships()
                            && match &domain.predicate {
                                Some(predicate) => terminal_semantics::domain_predicate_is_closed(
                                    self.structural_types().values(),
                                    domain.carrier,
                                    predicate,
                                ),
                                None => {
                                    terminal_semantics::byte_predicate_carrier(
                                        self.structural_types().values(),
                                        domain.carrier,
                                    ) && terminal_semantics::byte_qualification_predicates_in(
                                        self.program.catalogs.structural_domains.iter(),
                                        domain.carrier,
                                        &[identity],
                                    )
                                    .is_some()
                                }
                            }
                    })
            };
            for domain in whole {
                if !referent.qualifications.contains(&domain) && observable(domain) {
                    referent.qualifications.push(domain);
                }
            }
            for row in projected {
                if !referent.projected_qualifications.contains(&row) && observable(row.domain) {
                    referent.projected_qualifications.push(row);
                }
            }
            referent.qualifications.sort();
            referent.projected_qualifications.sort();
            if let Some(ram) = &mut placed {
                ram.presentation = referent.clone();
            }
        }
        let (required_whole, required_paths) = terminal_psi::StructuralPathQualification::select(
            &[],
            projected_qualifications,
            &[StructuralPathSegment::Referent],
        );
        let required_paths =
            crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                &self.structural_types(),
                &referent,
                self.structural_cases.as_map(),
                &required_paths,
            )?;
        for domain in &required_whole {
            if !referent.qualifications.contains(domain)
                && terminal_semantics::scalar_projection_entails_membership(
                    self.structural_types().values(),
                    self.program.catalogs.structural_domains.iter(),
                    referent.structural_type,
                    &[],
                    *domain,
                )
            {
                referent.qualifications.push(*domain);
            }
        }
        for row in &required_paths {
            if !referent.projected_qualifications.contains(row)
                && terminal_semantics::scalar_projection_entails_membership(
                    self.structural_types().values(),
                    self.program.catalogs.structural_domains.iter(),
                    referent.structural_type,
                    &row.path,
                    row.domain,
                )
            {
                referent.projected_qualifications.push(row.clone());
            }
        }
        referent.qualifications.sort();
        referent.qualifications.dedup();
        referent.projected_qualifications.sort();
        referent.projected_qualifications.dedup();
        let mut required_projected = Vec::new();
        terminal_psi::StructuralPathQualification::append_under(
            &required_whole,
            &required_paths,
            &[StructuralPathSegment::Referent],
            &mut required_projected,
        );
        referent.projected_qualifications =
            crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                &self.structural_types(),
                &referent,
                self.structural_cases.as_map(),
                &referent.projected_qualifications,
            )?;
        let mut projected = Vec::new();
        terminal_psi::StructuralPathQualification::append_under(
            &referent.qualifications,
            &referent.projected_qualifications,
            &[StructuralPathSegment::Referent],
            &mut projected,
        );
        projected.sort();
        projected.dedup();
        if !access_covers(source.access, access)
            || projected_qualifications
                .iter()
                .any(|row| !row.path.starts_with(&[StructuralPathSegment::Referent]))
            || required_projected
                .iter()
                .any(|row| !projected.contains(row))
            || (access != StructuralAccess::SharedBorrow
                && projected.iter().any(|row| {
                    !required_projected.contains(row)
                        && !self
                            .program
                            .catalogs
                            .structural_domains
                            .iter()
                            .any(|domain| {
                                domain.id == row.domain
                                    && domain.is_pure_predicate()
                                    && terminal_semantics::qualification_path_carrier(
                                        self.structural_types().values(),
                                        reference_type,
                                        &row.path,
                                    ) == Some(domain.carrier)
                            })
                }))
            || (aggregate_view.is_none()
                && !field_compatible
                && !terminal_semantics::reference_source_compatible(
                    self.structural_types().values(),
                    referent.structural_type,
                    reference_type,
                ))
        {
            let summarize_memberships =
                |whole: &[semantic_vocabulary::StructuralDomainId],
                 paths: &[terminal_psi::StructuralPathQualification]| {
                    (
                        whole.len(),
                        whole.iter().take(4).copied().collect::<Vec<_>>(),
                        paths.len(),
                        paths
                            .iter()
                            .take(4)
                            .map(|row| {
                                (
                                    row.domain,
                                    row.path.len(),
                                    row.path.iter().take(4).cloned().collect::<Vec<_>>(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                };
            let describe_declaration = |machine: &terminal_psi::TerminalMachine, place| {
                machine
                    .structural_parameter(place)
                    .map(|parameter| {
                        (
                            "parameter",
                            parameter.structural_type,
                            parameter.multiplicity,
                            None,
                            summarize_memberships(
                                &parameter.qualifications,
                                &parameter.projected_qualifications,
                            ),
                        )
                    })
                    .or_else(|| {
                        let (operation, _) = machine.producing_operation(place)?;
                        let result = operation.result.structural()?;
                        Some((
                            terminal_semantics::OperationSemanticTag::for_operation(
                                &operation.kind,
                            )
                            .name(),
                            result.structural_type,
                            result.multiplicity,
                            Some(operation.id),
                            summarize_memberships(
                                &result.qualifications,
                                &result.projected_qualifications,
                            ),
                        ))
                    })
            };
            let machine = self.program.machine(self.current_machine).ok();
            let declared_source =
                machine.and_then(|machine| describe_declaration(machine, source.place));
            let mut whole_store_count = 0usize;
            let mut whole_stores = Vec::new();
            if let Some(machine) = machine {
                for block in &machine.blocks {
                    for (ordinal, operation) in block.operations.iter().enumerate() {
                        if let terminal_psi::OperationKind::StoreStructuralField {
                            destination,
                            path,
                            field: None,
                            value,
                            ..
                        } = &operation.kind
                            && *destination == source.place
                            && path.is_empty()
                        {
                            whole_store_count += 1;
                            if whole_stores.len() < 4 {
                                whole_stores.push((
                                    block.id,
                                    ordinal,
                                    operation.id,
                                    value.place,
                                    value.access,
                                    value.path.len(),
                                    value.path.iter().take(4).cloned().collect::<Vec<_>>(),
                                    describe_declaration(machine, value.place),
                                ));
                            }
                        }
                    }
                }
            }
            let runtime_source = self.structural_values.get(&source.place).map(|value| {
                (
                    value.opaque_identity,
                    value.structural_type,
                    value.path.len(),
                    summarize_memberships(&value.qualifications, &value.projected_qualifications),
                )
            });
            let packed_source = self.scalar_array_values.get(&source.place).map(|array| {
                (
                    array.structural_type,
                    array.len(),
                    array.qualifications.len(),
                    array
                        .qualifications
                        .iter()
                        .take(4)
                        .copied()
                        .collect::<Vec<_>>(),
                )
            });
            let captured_array_home = collection_loan.as_ref().map(|(_, loan)| {
                (
                    loan.len(),
                    loan.access(),
                    loan.descriptor_home().map(|home| {
                        (
                            home.opaque_identity,
                            home.structural_type,
                            home.path.len(),
                            summarize_memberships(
                                &home.qualifications,
                                &home.projected_qualifications,
                            ),
                        )
                    }),
                )
            });
            let source_array_home = self
                .element_view_values
                .get(&source.place)
                .and_then(|loan| loan.descriptor_home())
                .map(|home| {
                    (
                        home.opaque_identity,
                        home.structural_type,
                        home.path.len(),
                        summarize_memberships(&home.qualifications, &home.projected_qualifications),
                    )
                });
            return Err(TerminalInterpretError::ReferenceFormation {
                stage: "validate captured reference contract",
                detail: format!(
                    "source={source:?}, referent={:?}, reference={reference_type:?}, required={required_projected:?}, observed={projected:?}; declared_source={declared_source:?}, runtime_source={runtime_source:?}, packed_source={packed_source:?}, source_has_array_companion={}, source_has_aggregate_backing={}, requested_roster={:?}, captured_array_home={captured_array_home:?}, source_array_home={source_array_home:?}, static_whole_store_count={whole_store_count}, first_static_whole_stores={whole_stores:?}",
                    referent.structural_type,
                    self.element_view_values.contains_key(&source.place),
                    self.aggregate_argument_backings.contains_key(&source.place),
                    summarize_memberships(&[], projected_qualifications),
                ),
            });
        }
        let collection_loan =
            collection_loan.map(|(_, loan)| loan.with_descriptor_home(referent.clone()));
        Ok(PreparedReference {
            projected_qualifications: projected,
            referent: ReferenceReferent {
                view: placed.as_ref().map_or(referent, |ram| ram.source.clone()),
                placed,
                primitive_loan,
                collection_loan,
                byte_owner: byte_owner_loan.map(|(_, binding)| binding),
                byte_region,
                aggregate_view,
            },
        })
    }

    fn capture_resident_array_region(
        &self,
        source: &StructuralArgument,
        byte_loan: &crate::storage::primitive_storage::PrimitiveLoan,
        referent_type: StructuralTypeId,
        byte_offset: usize,
        byte_length: usize,
        access: StructuralAccess,
        identity_cursor: &mut Option<u64>,
    ) -> Result<
        Option<(
            TerminalStructuralValue,
            crate::storage::element_views::ElementView,
            AggregateViewRegion,
        )>,
        TerminalInterpretError,
    > {
        let Some((StructuralPathSegment::FixedIndex(index), prefix)) = source.path.split_last()
        else {
            return Ok(None);
        };
        let types = self.structural_types();
        let Some(array) = terminal_semantics::runtime_structural_path_tip(
            types.values(),
            self.live_structural_type(source.place)?,
            prefix,
        ) else {
            return Ok(None);
        };
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = array.shape
        else {
            return Ok(None);
        };
        if element != byte_loan.referent.structural_type
            || !matches!(
                self.structural_declaration(element)?.shape,
                StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                    if integer.bits() == Some(8)
                        && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                        && !integer.is_address()
            )
        {
            return Ok(None);
        }
        let parent = StructuralArgument {
            place: source.place,
            path: prefix.to_vec(),
            access,
        };
        let Some((backing, loan)) = self.prepare_scalar_array_reference(
            array.id,
            &parent,
            identity_cursor,
            &mut RuntimeRows::new(self.account.as_ref()),
        )?
        else {
            return Ok(None);
        };
        let offset = usize::try_from(*index)
            .ok()
            .and_then(|index| index.checked_add(byte_offset))
            .ok_or_else(malformed)?;
        let mut region = AggregateViewRegion::whole(&types, referent_type).ok_or_else(malformed)?;
        region.byte_offset = u64::try_from(offset).map_err(|_| malformed())?;
        let mut selected = StructuralRuntimePlace::from(&backing);
        selected
            .path
            .push(StructuralPathSegment::FixedIndex(*index));
        if !matches!(
            &byte_loan.storage.endpoint,
            crate::storage::primitive_storage::PrimitiveStorageEndpoint::Scalar(cell)
                if *cell == selected && selected == StructuralRuntimePlace::from(&byte_loan.referent)
        ) {
            return Err(TerminalInterpretError::ReferenceFormation {
                stage: "capture resident array region",
                detail: "the selected byte does not belong to the captured array".into(),
            });
        }
        if *index >= length
            || u64::try_from(loan.len()).ok() != Some(length)
            || region.byte_length != u64::try_from(byte_length).map_err(|_| malformed())?
            || !region.fits(&types, backing.structural_type)
            || self.reference_byte_region(&backing)?.is_some()
        {
            return Err(TerminalInterpretError::ReferenceFormation {
                stage: "capture resident array region",
                detail: "the selected region exceeds its original array or retains a narrower byte window"
                    .into(),
            });
        }
        Ok(Some((backing, loan, region)))
    }

    pub(crate) fn establish_reference(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        let mut identity_cursor = self.local_structural_identities.cursor();
        let prepared = self
            .capture_reference(
                source,
                result.structural_type,
                &result.projected_qualifications,
                storage,
                &mut identity_cursor,
            )
            .map_err(|error| TerminalInterpretError::ReferenceFormation {
                stage: "establish reference",
                detail: format!(
                    "source={source:?} result={result:?} storage={storage:?}: {error:?}"
                ),
            })?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        let mut carrier = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        carrier.projected_qualifications = prepared.projected_qualifications;
        self.reference_referents
            .insert(StructuralRuntimePlace::from(&carrier), prepared.referent)?;
        self.structural_values.insert(result.place, carrier)?;
        self.live_affine_frontier
            .insert(whole_path_discard(result.place, result.structural_type));
        Ok(())
    }

    pub(crate) fn release_reference(
        &mut self,
        operation: &Operation,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let owner = self.structural_value(source)?;
        let carrier = project_structural_value(&self.structural_types(), owner, path)?;
        self.reference_contract(carrier.structural_type)?;
        let key = StructuralRuntimePlace::from(&carrier);
        if operation.result != OperationResult::Unit || !self.reference_referents.contains_key(&key)
        {
            return Err(malformed());
        }
        if let Some(ram) = self
            .reference_referents
            .get(&key)
            .and_then(|reference| reference.placed.as_ref())
        {
            self.validate_access_backing(ram)?;
        }
        let mut frontier = self.live_affine_frontier.clone();
        self.consume_affine_projection(
            &mut frontier,
            &StructuralArgument {
                place: source,
                path: path.to_vec(),
                access: StructuralAccess::Owned,
            },
        )?;
        let origin = self.reference_referents.remove(&key);
        self.live_affine_frontier = frontier;
        if path.is_empty()
            && let Some(retired) = self.structural_values.remove(&source)
        {
            self.retired_structural_homes
                .push((source, retired, origin));
        }
        Ok(())
    }

    pub(crate) fn captured_byte_owner_binding(
        &self,
        selected: &TerminalStructuralValue,
    ) -> Option<&crate::storage::byte_sequences::binding::ByteSequenceBinding> {
        self.captured_byte_owner_binding_at(
            &StructuralRuntimePlace::from(selected),
            selected.structural_type,
        )
    }

    pub(crate) fn captured_byte_owner_binding_at(
        &self,
        selected: &StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    ) -> Option<&crate::storage::byte_sequences::binding::ByteSequenceBinding> {
        self.reference_referents.values().find_map(|capture| {
            (StructuralRuntimePlace::from(&capture.view) == *selected
                && capture.view.structural_type == structural_type)
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

    pub(crate) fn resolve_live_access_projection(
        &self,
        argument: &StructuralArgument,
    ) -> Result<(TerminalStructuralValue, Option<AccessReference>), TerminalInterpretError> {
        let projected = self.resolve_live_projection_with_leaf_type(
            argument.place,
            &argument.path,
            argument.access,
            None,
            terminal_psi::StructuralFieldType::canonical_leaf_type,
        )?;
        Ok((projected.value, projected.placed))
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

    pub(crate) fn resolve_live_primitive_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        expected: StructuralTypeId,
    ) -> Result<
        (
            TerminalStructuralValue,
            Option<crate::storage::primitive_storage::PrimitiveStorage>,
        ),
        TerminalInterpretError,
    > {
        let mut projected = self.resolve_live_projection_with_leaf_type(
            place,
            path,
            access,
            Some(expected),
            self.borrowed_projection_leaf(expected, access)?,
        )?;
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

    pub(crate) fn present_structural_argument(
        &self,
        argument: &StructuralArgument,
        expected: StructuralTypeId,
    ) -> Result<(TerminalStructuralValue, Option<AggregateBacking>), TerminalInterpretError> {
        let owned_prefix = &argument.path[..argument
            .path
            .iter()
            .position(|step| {
                !matches!(
                    step,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::Case(_)
                )
            })
            .unwrap_or(argument.path.len())];
        let machine = self.program.verified_machine(self.current_machine)?;
        let caller_owned_borrow = argument.access != StructuralAccess::Owned
            && terminal_semantics::arrival_parameter(machine, argument.place)
                .is_some_and(|parameter| parameter.access != StructuralAccess::Owned);
        if !caller_owned_borrow {
            self.validate_complete_owned_value_claims(
                argument.place,
                self.live_structural_type(argument.place)?,
                owned_prefix,
            )?;
        }
        let projected = self.resolve_live_projection_with_leaf_type(
            argument.place,
            &argument.path,
            argument.access,
            Some(expected),
            self.borrowed_projection_leaf(expected, argument.access)?,
        )?;
        let (mut value, tip) = (projected.value, projected.view_tip);
        let region = if let Some(AggregateViewTip::Aggregate { view, backing }) = tip {
            if argument.access == StructuralAccess::Owned
                || expected != view.structural_type
                || value.structural_type != backing
            {
                return Err(malformed());
            }
            Some(view)
        } else {
            None
        };
        let byte_region = self.reference_byte_region(&value)?;
        if byte_region.is_some() && argument.access == StructuralAccess::Owned {
            return Err(malformed());
        }
        let mut placed = projected.placed;
        if let Some(ram) = &mut placed {
            self.validate_access_backing(ram)?;
            if argument.access == StructuralAccess::Owned
                || !access_covers(ram.access, argument.access)
                || ram.presentation != value
            {
                return Err(malformed());
            }
            ram.access = argument.access;
        }
        let backing = (region.is_some() || byte_region.is_some() || placed.is_some()).then(|| {
            AggregateBacking {
                value: value.clone(),
                placed,
                region,
                byte_region,
                collection_home: projected.collection_home,
                array_loan: projected.array_loan,
            }
        });
        if let Some(region) = region {
            value.structural_type = region.structural_type;
        }
        if argument.access != StructuralAccess::Owned
            && self
                .structural_types()
                .get(&expected)
                .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
            && terminal_semantics::borrowed_referent_matches(
                self.structural_types().values(),
                value.structural_type,
                expected,
            )
        {
            value.structural_type = expected;
        }
        Ok((value, backing))
    }

    pub(crate) fn present_owned_descriptor(
        &self,
        mut value: TerminalStructuralValue,
        expected: StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        if value.structural_type == expected {
            return Ok(value);
        }
        if !terminal_semantics::descriptor_value_transfer_matches(
            self.structural_types().values(),
            value.structural_type,
            expected,
            multiplicity,
        ) {
            return Err(malformed());
        }
        terminal_semantics::present_structural_memberships(
            self.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            expected,
            &mut value.qualifications,
            &mut value.projected_qualifications,
        )
        .map_err(|reason| malformed().in_argument_preparation(reason))?;
        value.structural_type = expected;
        Ok(value)
    }

    fn borrowed_projection_leaf(
        &self,
        expected: StructuralTypeId,
        access: StructuralAccess,
    ) -> Result<
        impl Fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>
        + Copy
        + use<'_>,
        TerminalInterpretError,
    > {
        let expected = self.structural_declaration(expected)?;
        Ok(move |field: &terminal_psi::StructuralFieldType| {
            if access == StructuralAccess::Owned {
                field.canonical_leaf_type()
            } else {
                terminal_semantics::borrowed_field_presentation(field, access, Some(expected))
            }
        })
    }

    pub(crate) fn resolve_live_view_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<(TerminalStructuralValue, Option<AggregateViewTip>), TerminalInterpretError> {
        self.resolve_live_storage_projection(place, path, access)
            .map(|projection| (projection.value, projection.view_tip))
    }

    pub(crate) fn resolve_live_storage_projection(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<ResolvedStructuralProjection, TerminalInterpretError> {
        self.resolve_live_projection_with_leaf_type(
            place,
            path,
            access,
            None,
            if access == StructuralAccess::SharedBorrow {
                terminal_psi::StructuralFieldType::shared_observation_type
            } else {
                terminal_psi::StructuralFieldType::canonical_leaf_type
            },
        )
    }

    pub(crate) fn resolve_live_leaf_copy(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        expected: StructuralTypeId,
    ) -> Result<
        (
            TerminalStructuralValue,
            Option<semantic_vocabulary::StructuralFieldId>,
        ),
        TerminalInterpretError,
    > {
        let expected = self.structural_declaration(expected)?;
        self.resolve_live_projection_with_leaf_type(
            place,
            path,
            StructuralAccess::SharedBorrow,
            Some(expected.id),
            |field| field.leaf_copy_type(Some(expected)),
        )
        .map(|projection| (projection.value, projection.endpoint_field))
    }

    pub(crate) fn resolve_live_projection_with_leaf_type(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        expected: Option<StructuralTypeId>,
        leaf_type: impl Fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>
        + Copy,
    ) -> Result<ResolvedStructuralProjection, TerminalInterpretError> {
        let owner = self.structural_value(place)?;
        self.resolve_storage_projection_with_leaf_type(
            place, owner, path, access, expected, leaf_type,
        )
    }

    pub(crate) fn resolve_storage_projection_with_leaf_type(
        &self,
        place: PlaceId,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        expected: Option<StructuralTypeId>,
        leaf_type: impl Fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>
        + Copy,
    ) -> Result<ResolvedStructuralProjection, TerminalInterpretError> {
        if path.contains(&StructuralPathSegment::Referent) {
            let authority = self.reference_carrier_access(place, owner.structural_type, path)?;
            if !access_covers(authority, access) {
                return Err(malformed());
            }
        }
        let (backing, view, placed) = match self.aggregate_argument_backings.get(&place) {
            Some(backing) => (&backing.value, backing.region, backing.placed.clone()),
            None => (owner, None, None),
        };
        self.resolve_reference_view_path_with_leaf_type(
            backing,
            path,
            Some(access),
            expected,
            leaf_type,
            view,
            placed,
            self.aggregate_argument_backings
                .get(&place)
                .and_then(|backing| backing.array_loan.clone())
                .or_else(|| self.element_view_values.get(&place).cloned()),
        )?
        .ok_or_else(malformed)
    }

    pub(crate) fn resolve_reference_view_path(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Option<(TerminalStructuralValue, Option<AggregateViewTip>)>, TerminalInterpretError>
    {
        self.resolve_reference_storage_projection(owner, path, access)
            .map(|selected| selected.map(|projection| (projection.value, projection.view_tip)))
    }

    pub(crate) fn resolve_reference_storage_projection(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        self.resolve_reference_view_path_with_leaf_type(
            owner,
            path,
            Some(access),
            None,
            if access == StructuralAccess::SharedBorrow {
                terminal_psi::StructuralFieldType::shared_observation_type
            } else {
                terminal_psi::StructuralFieldType::canonical_leaf_type
            },
            None,
            None,
            None,
        )
    }

    pub(crate) fn resolve_case_guard_carrier(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        let owner = self.structural_value(place)?;
        let (backing, view, placed) = match self.aggregate_argument_backings.get(&place) {
            Some(backing) => (&backing.value, backing.region, backing.placed.clone()),
            None => (owner, None, None),
        };
        self.resolve_reference_view_path_with_leaf_type(
            backing,
            path,
            None,
            None,
            terminal_psi::StructuralFieldType::canonical_leaf_type,
            view,
            placed,
            self.aggregate_argument_backings
                .get(&place)
                .and_then(|backing| backing.array_loan.clone())
                .or_else(|| self.element_view_values.get(&place).cloned()),
        )?
        .map(|projection| projection.value)
        .ok_or_else(malformed)
    }

    fn resolve_reference_view_path_with_leaf_type(
        &self,
        owner: &TerminalStructuralValue,
        path: &[StructuralPathSegment],
        access: Option<StructuralAccess>,
        expected: Option<StructuralTypeId>,
        leaf_type: impl Fn(
            &terminal_psi::StructuralFieldType,
        ) -> Option<terminal_psi::StructuralFieldType>
        + Copy,
        initial_view: Option<AggregateViewRegion>,
        initial_access: Option<AccessReference>,
        initial_array: Option<crate::storage::element_views::ElementView>,
    ) -> Result<Option<ResolvedStructuralProjection>, TerminalInterpretError> {
        let types = self.structural_types();
        let mut value = owner.clone();
        let mut remaining = path;
        let mut aggregate_view = initial_view;
        let mut primitive_storage = None;
        let mut collection_home: Option<(
            &crate::storage::element_views::ElementView,
            &[StructuralPathSegment],
        )> = None;
        let mut array_loan = initial_array;
        let mut placed = initial_access.or_else(|| {
            self.access_view_occurrences
                .values()
                .find(|occurrence| occurrence.view == StructuralRuntimePlace::from(owner))
                .and_then(|occurrence| occurrence.ram.clone())
        });
        loop {
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
                        types.declaration(&prefix_type)?.shape,
                        StructuralTypeShape::ElementView { .. }
                    ) {
                        boundary = Some(position);
                        break;
                    }
                }
            }
            let prefix = &remaining[..boundary.unwrap_or(remaining.len())];
            let logical_prefix = prefix;
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
            let supplying_array = array_loan.take();
            if !crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                &types,
                &value,
                self.structural_cases.as_map(),
                prefix,
            )? {
                return Ok(None);
            }
            let endpoint_field = boundary
                .is_none()
                .then(|| {
                    crate::engine::custody::inline_structural_field(
                        &types,
                        value.structural_type,
                        prefix,
                    )
                    .map(|field| field.id)
                })
                .flatten();
            value = if let Some(
                AggregateViewTip::Scalar { .. } | AggregateViewTip::ScalarRegion { .. },
            ) = tip
            {
                let view = active_view.ok_or_else(malformed)?;
                let projection_home = match collection_home {
                    Some((loan, selected)) => loan.captured_storage_home(&types, selected)?,
                    None => match supplying_array.as_ref() {
                        Some(loan) => loan.captured_storage_home(&types, &[])?,
                        None => None,
                    },
                };
                let (storage, _, _) = self.aggregate_view_primitive_access(
                    &value,
                    AggregateViewTip::Aggregate {
                        view,
                        backing: value.structural_type,
                    },
                    logical_prefix,
                    projection_home.as_ref(),
                    access.is_some_and(|access| access != StructuralAccess::SharedBorrow),
                )?;
                primitive_storage = Some(storage);
                let structural_type =
                    crate::engine::custody::resolve_structural_path_type_with_leaf(
                        &types,
                        view.structural_type,
                        logical_prefix,
                        if boundary.is_none() { expected } else { None },
                        leaf_type,
                    )?;
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
                crate::engine::custody::project_structural_value_with_leaf(
                    &types, &value, prefix, expected, leaf_type,
                )?
            } else if access == Some(StructuralAccess::SharedBorrow) {
                crate::engine::custody::project_shared_structural_observation(
                    &types, &value, prefix,
                )?
            } else {
                project_structural_value(&types, &value, prefix)?
            };
            if let Some(loan) = supplying_array {
                if matches!(
                    types.get(&value.structural_type).map(|row| &row.shape),
                    Some(StructuralTypeShape::FixedArray { .. })
                ) {
                    array_loan = Some(loan.project_array(&types, prefix, &value)?);
                } else if !prefix.is_empty()
                    && matches!(
                        types.get(&value.structural_type).map(|row| &row.shape),
                        Some(StructuralTypeShape::PrimitiveScalar(_))
                    )
                {
                    primitive_storage = Some(
                        loan.scalar_storage(
                            self,
                            prefix,
                            access
                                .is_some_and(|required| required != StructuralAccess::SharedBorrow),
                        )?
                        .0,
                    );
                }
            }
            if let Some(captured) = &mut placed {
                self.project_access_presentation(captured, prefix, &value)?;
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
                    aggregate_view = match tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => {
                            let StructuralTypeShape::ElementView { element, .. } =
                                types.declaration(&view.structural_type)?.shape
                            else {
                                return Err(malformed());
                            };
                            Some(
                                AggregateViewRegion::whole(&types, element)
                                    .ok_or_else(malformed)?,
                            )
                        }
                        None => None,
                        Some(
                            AggregateViewTip::Scalar { .. } | AggregateViewTip::ScalarRegion { .. },
                        ) => return Err(malformed()),
                    };
                    value = if access == Some(StructuralAccess::SharedBorrow) {
                        crate::engine::custody::project_shared_structural_observation(
                            &types,
                            &collection,
                            &selected,
                        )?
                    } else {
                        project_structural_value(&types, &collection, &selected)?
                    };
                    collection_home = Some((view, &remaining[boundary..boundary + 1]));
                    remaining = &remaining[boundary + 1..];
                    continue;
                }
                let (referent_type, authority) = self.reference_contract(value.structural_type)?;
                if access.is_some_and(|required| !access_covers(authority, required)) {
                    return Err(malformed());
                }
                let reference_type = value.structural_type;
                let qualifications = terminal_psi::StructuralPathQualification::select(
                    &value.qualifications,
                    &value.projected_qualifications,
                    &[StructuralPathSegment::Referent],
                );
                let captured = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&value))
                    .ok_or_else(malformed)?;
                primitive_storage = captured
                    .primitive_loan
                    .as_ref()
                    .map(|loan| loan.storage.clone());
                collection_home = captured
                    .collection_loan
                    .as_ref()
                    .map(|loan| (loan, &[][..]));
                array_loan = captured.collection_loan.clone();
                let suffix = &remaining[boundary + 1..];
                // The suffix is spelled in the referent's coordinates. A collection loan
                // reads it directly only when the referent presents the collection as
                // itself; a recast view first translates it through its aggregate view.
                if let Some(loan) = &captured.collection_loan
                    && captured.aggregate_view.is_none()
                    && !suffix.is_empty()
                    && terminal_semantics::primitive_projection_type(
                        types.values(),
                        referent_type,
                        suffix,
                    )
                    .is_some()
                {
                    primitive_storage = Some(
                        loan.scalar_storage(
                            self,
                            suffix,
                            access
                                .is_some_and(|required| required != StructuralAccess::SharedBorrow),
                        )?
                        .0,
                    );
                }
                placed = captured.placed.clone();
                if let Some(ram) = &placed {
                    self.validate_access_backing(ram)?;
                    if ram.presentation.structural_type != referent_type
                        || !access_covers(ram.access, authority)
                    {
                        return Err(malformed());
                    }
                    value = ram.presentation.clone();
                    (value.qualifications, value.projected_qualifications) = qualifications;
                    remaining = &remaining[boundary + 1..];
                    continue;
                }
                value = captured.view.clone();
                if let Some(view) = captured.aggregate_view {
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
                    value.structural_type = referent_type;
                }
                (value.qualifications, value.projected_qualifications) = qualifications;
                remaining = &remaining[boundary + 1..];
            } else {
                if primitive_storage.is_none()
                    && endpoint_field.is_none()
                    && matches!(
                        types.declaration(&value.structural_type)?.shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    )
                    && let Some((view, selected_prefix)) = collection_home
                {
                    let mut selected = Vec::with_capacity(selected_prefix.len() + prefix.len());
                    selected.extend_from_slice(selected_prefix);
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
                let tip = tip.or_else(|| {
                    aggregate_view.map(|view| AggregateViewTip::Aggregate {
                        view,
                        backing: value.structural_type,
                    })
                });
                let collection_home = if matches!(
                    types
                        .get(&value.structural_type)
                        .map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::FixedArray { .. })
                ) {
                    let selected = collection_home
                        .map(|(view, selected_prefix)| {
                            let mut selected = selected_prefix.to_vec();
                            selected.extend_from_slice(prefix);
                            view.captured_storage_home(&types, &selected)
                        })
                        .transpose()?
                        .flatten();
                    match selected {
                        Some(home) => Some(home),
                        None => array_loan
                            .as_ref()
                            .map(|loan| loan.captured_storage_home(&types, &[]))
                            .transpose()?
                            .flatten(),
                    }
                } else {
                    None
                };
                return Ok(Some(ResolvedStructuralProjection {
                    array_loan,
                    placed,
                    view_tip: tip,
                    endpoint_field,
                    primitive_storage,
                    collection_home,
                    value,
                }));
            }
        }
    }

    pub(crate) fn observation_view_root(
        &self,
        source: PlaceId,
    ) -> Result<(TerminalStructuralValue, Option<AggregateViewTip>), TerminalInterpretError> {
        let value =
            crate::engine::custody::verified_structural_place(&self.structural_values, source)?;
        if !matches!(
            self.structural_types()
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
        arguments: &RuntimeRows<PlaceId, TerminalStructuralValue>,
        aggregate_backings: &RuntimeRows<PlaceId, AggregateBacking>,
        primitive_loans: &RuntimeRows<PlaceId, crate::storage::primitive_storage::PrimitiveLoan>,
        array_loans: &RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
        byte_bindings: &RuntimeRows<
            PlaceId,
            crate::storage::byte_sequences::binding::ByteSequenceBinding,
        >,
        access_occurrences: &BTreeMap<
            terminal_psi::TerminalAccessViewInput,
            crate::boundary::structural_inputs::access_views::AccessViewOccurrence,
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
                    &self.structural_types(),
                    signature.structural_type,
                    &origin.path,
                )?;
                let (referent_type, access) = self.reference_contract(leaf_type)?;
                let mut backings = Vec::with_capacity(origin.sources.len());
                for source in &origin.sources {
                    let terminal_psi::StructuralReferenceOrigin::Parameter(source) = source else {
                        continue;
                    };
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
                    let binding = terminal_semantics::bind_reference_origin(
                        self.structural_types().values(),
                        owner.structural_type,
                        source,
                    )
                    .map_err(TerminalInterpretError::UnsupportedSemanticVariant)?;
                    if binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                        && binding.endpoint != referent_type
                    {
                        return Err(malformed());
                    }
                    let path = binding.source.path.as_slice();
                    let (backing, view, placed) = match aggregate_backings.get(&parameter.place) {
                        Some(backing) => (&backing.value, backing.region, backing.placed.clone()),
                        None => (owner, None, None),
                    };
                    let Some(projected) = self.resolve_reference_view_path_with_leaf_type(
                        backing,
                        path,
                        Some(source.access),
                        None,
                        terminal_psi::StructuralFieldType::canonical_leaf_type,
                        view,
                        placed.or_else(|| {
                            access_occurrences
                                .values()
                                .find(|occurrence| {
                                    occurrence.view == StructuralRuntimePlace::from(owner)
                                })
                                .and_then(|occurrence| occurrence.ram.clone())
                        }),
                        array_loans.get(&parameter.place).cloned(),
                    )?
                    else {
                        continue;
                    };
                    let view = match projected.view_tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                        _ => None,
                    };
                    let mut placed = projected.placed;
                    if let Some(ram) = &mut placed {
                        self.validate_access_backing(ram)?;
                        if !access_covers(ram.access, access) {
                            return Err(malformed());
                        }
                        ram.access = access;
                    }
                    let logical = projected.value;
                    let endpoint = terminal_semantics::reference_origin_type(
                        self.structural_types().values(),
                        view.map_or(logical.structural_type, |view| view.structural_type),
                        binding.suffix,
                    );
                    if endpoint.is_none()
                        || (binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                            && endpoint != Some(referent_type))
                    {
                        return Err(malformed());
                    }
                    if placed.is_some()
                        && (!binding.suffix.is_empty()
                            || binding.extent
                                != semantic_vocabulary::ReferenceOriginExtent::Projected
                            || endpoint != Some(referent_type))
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "placed return requires exact whole-view origin",
                        ));
                    }
                    let primitive_storage = projected.primitive_storage.or_else(|| {
                        path.is_empty()
                            .then(|| {
                                primitive_loans
                                    .get(&parameter.place)
                                    .map(|loan| loan.storage.clone())
                            })
                            .flatten()
                    });
                    let expected = placed.as_ref().map_or(logical, |ram| ram.source.clone());
                    let array_loan = projected.array_loan.or_else(|| {
                        path.is_empty()
                            .then(|| array_loans.get(&parameter.place).cloned())
                            .flatten()
                    });
                    let byte_binding = if self
                        .structural_types()
                        .get(&expected.structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_borrowed_byte_carrier()
                                || declaration.shape.is_bounded_owned_byte_sequence()
                        }) {
                        let binding = match byte_bindings
                            .get(&parameter.place)
                            .filter(|_| path.is_empty())
                        {
                            Some(binding) => binding.clone(),
                            None => self.retained_byte_descriptor_binding(
                                &StructuralArgument {
                                    place: parameter.place,
                                    path: path.to_vec(),
                                    access: source.access,
                                },
                                &expected,
                            )?.ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                                "returned byte reference source has no retained descriptor binding",
                            ))?,
                        };
                        if !access_covers(binding.access(), source.access) {
                            return Err(malformed());
                        }
                        self.byte_descriptor_length(&binding, &expected)?;
                        Some(binding)
                    } else {
                        None
                    };
                    backings.push(ReferenceBacking {
                        byte_binding,
                        element_view: if (!binding.suffix.is_empty()
                            || binding.extent
                                == semantic_vocabulary::ReferenceOriginExtent::WithinRegion)
                            && matches!(
                                self.structural_types()
                                    .get(&expected.structural_type)
                                    .map(|declaration| &declaration.shape),
                                Some(StructuralTypeShape::ElementView { .. })
                            ) {
                            let view = match array_loan.as_ref() {
                                Some(view) => view,
                                None => self.element_view_at_resolved_home(&expected)?,
                            };
                            if !view.matches_descriptor_home(&expected)
                                || !access_covers(view.access(), source.access)
                                || !access_covers(view.access(), access)
                            {
                                return Err(malformed().in_argument_preparation(
                                    "capture exact element-view return backing",
                                ));
                            }
                            Some(view.clone())
                        } else {
                            None
                        },
                        array_loan,
                        placed,
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
                    self.structural_types()
                        .get(&signature.structural_type)
                        .map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Reference { .. })
                ) {
                Err(TerminalInterpretError::ContentTransformationMismatch(
                    "returned reference has no declared source association",
                ))
            } else {
                Ok(())
            };
        }
        let expected_reference_backings = match self.call_stack.last() {
            Some(SuspendedCall {
                result:
                    SuspendedCallResult::Structural {
                        expected_reference_backings,
                        ..
                    },
                ..
            }) => expected_reference_backings,
            Some(_) => return Err(malformed()),
            None => {
                &self
                    .entry_reference_backings
                    .as_ref()
                    .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                        "host reference results require an explicit loan carrier interface",
                    ))?
                    .rows
            }
        };
        if expected_reference_backings.len() != signature.reference_sources.len() {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "returned reference source roster differs from the captured call",
            ));
        }
        let mut active_count = 0usize;
        for (ordinal, origin) in signature.reference_sources.iter().enumerate() {
            if signature.reference_sources[..ordinal]
                .iter()
                .any(|previous| previous.path == origin.path)
            {
                return Err(malformed());
            }
            if !crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                &self.structural_types(),
                carrier,
                self.structural_cases.as_map(),
                &origin.path,
            )? {
                continue;
            }
            active_count += 1;
            let leaf_type = resolve_structural_path_type(
                &self.structural_types(),
                signature.structural_type,
                &origin.path,
            )?;
            let (referent_type, access) = self.reference_contract(leaf_type)?;
            let expected = &expected_reference_backings[ordinal];
            let descriptor =
                project_structural_value(&self.structural_types(), carrier, &origin.path)?;
            let key = StructuralRuntimePlace::from(&descriptor);
            let actual = self.reference_referents.get(&key).ok_or(
                TerminalInterpretError::ContentTransformationMismatch(
                    "returned reference leaf has no captured backing",
                ),
            )?;
            if let Some(ram) = &actual.placed {
                self.validate_access_backing(ram)?;
            }
            if origin.sources.is_empty()
                || origin
                    .sources
                    .iter()
                    .any(|source| !access_covers(source.access(), access))
            {
                return Err(malformed());
            }
            let mut invocation_backings = Vec::new();
            for source in &origin.sources {
                let terminal_psi::StructuralReferenceOrigin::Invocation {
                    carrier_type,
                    path,
                    extent,
                    access: source_access,
                } = source
                else {
                    continue;
                };
                let endpoint = terminal_semantics::reference_origin_type(
                    self.structural_types().values(),
                    *carrier_type,
                    path,
                )
                .ok_or_else(malformed)?;
                if endpoint != referent_type {
                    return Err(malformed());
                }
                if path.iter().any(|projection| {
                    !matches!(
                        projection,
                        terminal_psi::ReferenceOriginProjection::Exact(_)
                    )
                }) {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "invocation reference origin element window",
                    ));
                }
                let machine = self.program.verified_machine(self.current_machine)?;
                let produced_entries = self
                    .structural_values
                    .iter()
                    .map(|(place, value)| (place, value, None))
                    .chain(
                        self.retired_structural_homes
                            .iter()
                            .map(|(place, value, origin)| (place, value, origin.as_ref())),
                    );
                for (place, value, retired_origin) in produced_entries {
                    if value.structural_type != *carrier_type
                        || machine.producing_operation(*place).is_none()
                    {
                        continue;
                    }
                    let (value, suffix, placed, aggregate_view, primitive_storage) =
                        match retired_origin {
                            Some(origin)
                                if path.first()
                                    == Some(&terminal_psi::ReferenceOriginProjection::Exact(
                                        StructuralPathSegment::Referent,
                                    )) =>
                            {
                                (
                                    &origin.view,
                                    &path[1..],
                                    origin.placed.clone(),
                                    origin.aggregate_view,
                                    origin
                                        .primitive_loan
                                        .as_ref()
                                        .map(|loan| loan.storage.clone()),
                                )
                            }
                            _ => (value, &path[..], None, None, None),
                        };
                    invocation_backings.push(ReferenceBacking {
                        array_loan: None,
                        placed,
                        value: value.clone(),
                        aggregate_view,
                        primitive_storage,
                        suffix: suffix.to_vec(),
                        extent: *extent,
                        element_view: None,
                        byte_binding: None,
                        access: *source_access,
                    });
                }
            }
            let mut matched = false;
            for backing in expected.iter().chain(&invocation_backings) {
                if let Some(mut projected) =
                    self.resolve_return_origin(backing, actual)
                        .map_err(|error| TerminalInterpretError::ReferenceFormation {
                            stage: "resolve returned reference origin",
                            detail: format!(
                                "source={:?} path={:?} returned={:?}: {error:?}",
                                backing.value, backing.suffix, actual.view
                            ),
                        })?
                {
                    self.retain_returned_access_interpretation(
                        &mut projected,
                        actual,
                        referent_type,
                        access,
                    )?;
                    let view = match projected.view_tip {
                        Some(AggregateViewTip::Aggregate { view, .. }) => Some(view),
                        _ => None,
                    };
                    matched |= view.map_or_else(
                        || {
                            projected
                                .placed
                                .as_ref()
                                .map_or(projected.value.structural_type, |ram| {
                                    ram.presentation.structural_type
                                })
                        },
                        |view| view.structural_type,
                    ) == referent_type
                        && projected.value == actual.view
                        && projected.placed == actual.placed
                        && view == actual.aggregate_view
                        && projected.array_loan.as_ref().is_none_or(|expected| {
                            actual
                                .collection_loan
                                .as_ref()
                                .is_some_and(|actual| expected.lends_same_array_storage(actual))
                        })
                        && projected.primitive_storage.as_ref().is_none_or(|storage| {
                            actual
                                .primitive_loan
                                .as_ref()
                                .is_some_and(|loan| &loan.storage == storage)
                        });
                }
            }
            if !matched {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "returned reference does not match any captured source backing",
                ));
            }
        }
        if active_count != resident_count {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "returned reference carrier has missing or undeclared descriptor leaves",
            ));
        }
        Ok(())
    }
}

pub(crate) use terminal_psi::structural_access_covers as access_covers;
