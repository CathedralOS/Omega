//! Address exposure observes existing backing, never a reference descriptor.
//!
//! Primitive access already follows captured loans into suspended owners and
//! canonicalizes runtime indices. Reuse those endpoints and the ordinary local
//! referent allocator. Array payloads retain their own occurrence identity;
//! stack depth alone would alias a later call with an earlier live address.
//! The checked encoding below is injective, not a hash or simulated byte layout.
//! Every component must fit, including the execution namespace. Exhaustion
//! refuses before publishing an address. Odd encodings reserve the even
//! namespace for future code entries and exclude zero from live storage.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::{PrimitiveStorageEndpoint, scalar_field_projection};
use crate::structural_inputs::byte_arrays::storage::ArrayStorageHome;
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use semantic_vocabulary::{IntegerValue, ScalarType};
use terminal_psi::{Operation, StructuralArgument, StructuralPathSegment};

fn exhausted() -> TerminalInterpretError {
    TerminalInterpretError::StructuralIdentityExhausted
}

/// A finite canonical location path, tagged independently of source types.
type Coordinate = (u64, u64);

/// Elias gamma codes have an unambiguous end, including for zero (encode n+1).
/// A leading sentinel retains every leading zero. Complete components append
/// only while they fit; the final odd tag reserves a separate code namespace.
fn encode(namespace: u64, root: u64, path: &[Coordinate]) -> Result<u64, TerminalInterpretError> {
    fn append(bits: &mut u64, value: u64) -> Result<(), TerminalInterpretError> {
        let value = value.checked_add(1).ok_or_else(exhausted)?;
        let width = u64::BITS - value.leading_zeros();
        let shift = width
            .checked_mul(2)
            .and_then(|width| width.checked_sub(1))
            .ok_or_else(exhausted)?;
        let scale = 1_u64.checked_shl(shift).ok_or_else(exhausted)?;
        *bits = bits
            .checked_mul(scale)
            .and_then(|bits| bits.checked_add(value))
            .ok_or_else(exhausted)?;
        Ok(())
    }
    let mut bits = 1;
    append(&mut bits, namespace)?;
    append(&mut bits, root)?;
    append(
        &mut bits,
        u64::try_from(path.len()).map_err(|_| exhausted())?,
    )?;
    for (kind, coordinate) in path {
        append(&mut bits, *kind)?;
        append(&mut bits, *coordinate)?;
    }
    bits.checked_mul(2)
        .and_then(|bits| bits.checked_add(1))
        .ok_or_else(exhausted)
}

fn path_coordinates(
    path: &[StructuralPathSegment],
) -> Result<Vec<Coordinate>, TerminalInterpretError> {
    path.iter()
        .map(|segment| match segment {
            StructuralPathSegment::FixedIndex(index) => Ok((0, *index)),
            StructuralPathSegment::Field(field) => Ok((1, u64::from(field.0))),
            StructuralPathSegment::Case(case) => Ok((2, case.get())),
            // These must have been resolved to original backing first. A source
            // type or unevaluated selector is never a storage coordinate.
            _ => Err(malformed()),
        })
        .collect()
}

impl TerminalExecution<'_> {
    /// Read-only scalar fields can retain declaration predicates and therefore
    /// need not have a canonical primitive-store path. Resolve their original
    /// cell through the same authority and re-view owners as scalar reads,
    /// without reading contents or inventing a separate field allocation.
    fn storage_address_endpoint(
        &self,
        source: &StructuralArgument,
        scalar_type: Option<ScalarType>,
    ) -> Result<PrimitiveStorageEndpoint, TerminalInterpretError> {
        let Some(scalar_type) = scalar_type else {
            // Aggregate references retain the original runtime occurrence and
            // subplace, including when the descriptor crossed a call frame.
            // Resolving that custody observes no fields and allocates no copy.
            let (carrier, view) = self.resolve_live_view_projection(
                source.place,
                &source.path,
                terminal_psi::StructuralAccess::SharedBorrow,
            )?;
            if let Some(crate::reference::AggregateViewTip::Aggregate { view, backing }) = view
                && (view.byte_offset != 0
                    || terminal_semantics::reference_storage_size(
                        self.program.structural_types().values(),
                        backing,
                    ) != Some(view.byte_length))
            {
                return Err(TerminalInterpretError::StructuralArgumentPreparation {
                    stage: "storage address",
                    detail: "an aggregate re-view window has no canonical storage address endpoint"
                        .into(),
                });
            }
            return Ok(PrimitiveStorageEndpoint::Scalar(
                StructuralRuntimePlace::from(&carrier),
            ));
        };
        if let Some((prefix, field_path)) = scalar_field_projection(&source.path) {
            let (carrier, view) = self.resolve_live_view_projection(
                source.place,
                prefix,
                terminal_psi::StructuralAccess::SharedBorrow,
            )?;
            if let Some(view) = view {
                return self
                    .aggregate_view_primitive_access(&carrier, view, field_path)
                    .map(|(storage, _, _)| storage.endpoint);
            }
            if let Some(observed) =
                self.scalar_field_at_carrier(carrier, view, field_path, scalar_type)?
            {
                return Ok(self
                    .reference_byte_region_access(&observed.carrier, &observed.leaf)?
                    .map(|(storage, _)| storage.endpoint)
                    .unwrap_or(PrimitiveStorageEndpoint::ScalarField(observed.cell)));
            }
        }
        self.primitive_access(source.place, false, &source.path)
            .map(|(storage, _, _)| storage.endpoint)
    }

    fn array_storage_identity(
        &mut self,
        depth: usize,
        place: semantic_vocabulary::PlaceId,
    ) -> Result<u64, TerminalInterpretError> {
        if let Some(identity) = self
            .frame_array(depth, place)
            .ok_or_else(malformed)?
            .storage_identity
        {
            return Ok(identity);
        }
        let identity = self.local_structural_identities.allocate()?;
        let arrays = if depth == self.call_stack.len() {
            &mut self.scalar_array_values
        } else {
            &mut self
                .call_stack
                .get_mut(depth)
                .ok_or_else(malformed)?
                .scalar_array_values
        };
        arrays
            .get_mut(&place)
            .ok_or_else(malformed)?
            .storage_identity = Some(identity);
        Ok(identity)
    }

    fn endpoint_identity(
        &mut self,
        endpoint: PrimitiveStorageEndpoint,
    ) -> Result<(Option<u64>, u64, Vec<Coordinate>), TerminalInterpretError> {
        let selected = match endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => place,
            PrimitiveStorageEndpoint::ScalarField(field) => {
                // Inline scalar fields share the same terminated path grammar
                // as other subplaces. Their selected namespace is a segment
                // tag, never an untagged integer confused with a path suffix.
                let (kind, value) = match field.namespace {
                    crate::values::StructuralFieldNamespace::Member => (3, field.field.get()),
                    crate::values::StructuralFieldNamespace::CasePayload => (4, field.field.get()),
                };
                let mut path = path_coordinates(&field.parent.path)?;
                path.push((kind, value));
                return Ok((None, field.parent.opaque_identity, path));
            }
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                let (depth, place, _) = home.scalar_array_frame();
                StructuralRuntimePlace {
                    opaque_identity: self.array_storage_identity(depth, place)?,
                    path: vec![StructuralPathSegment::FixedIndex(
                        u64::try_from(index).map_err(|_| exhausted())?,
                    )],
                }
            }
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                let mut place = match array.home {
                    ArrayStorageHome::Runtime => array.referent,
                    ArrayStorageHome::Frame { depth, place, .. } => StructuralRuntimePlace {
                        opaque_identity: self.array_storage_identity(depth, place)?,
                        path: Vec::new(),
                    },
                };
                place.path.push(StructuralPathSegment::FixedIndex(
                    u64::try_from(index).map_err(|_| exhausted())?,
                ));
                place
            }
            PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                let (namespace, index) = bytes
                    .storage_address_coordinate(index)
                    .ok_or_else(exhausted)?;
                return Ok((
                    Some(namespace),
                    0,
                    path_coordinates(&[StructuralPathSegment::FixedIndex(index)])?,
                ));
            }
        };
        Ok((
            None,
            selected.opaque_identity,
            path_coordinates(&selected.path)?,
        ))
    }

    pub(crate) fn execute_storage_address(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.scalar().ok_or_else(malformed)?;
        let ScalarType::Integer(integer) = result.scalar_type else {
            return Err(malformed());
        };
        if !integer.is_address() || source.access != terminal_psi::StructuralAccess::SharedBorrow {
            return Err(malformed());
        }
        let scalar_type = terminal_semantics::primitive_read_projection_type(
            self.program.structural_types().values(),
            self.live_structural_type(source.place)?,
            &source.path,
        );
        let endpoint = self.storage_address_endpoint(source, scalar_type)?;
        let (namespace, root, path) = self.endpoint_identity(endpoint)?;
        let namespace = namespace
            .or(self.local_structural_identities.exposure_namespace)
            .ok_or_else(exhausted)?;
        let encoded = encode(namespace, root, &path)?;
        let value = IntegerValue::Unsigned(u128::from(encoded));
        if !integer.admits(value) {
            return Err(exhausted());
        }
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_and_array_coordinates_do_not_collide() {
        let field = encode(0, 7, &[(3, 1)]).unwrap();
        let nested = encode(0, 7, &[(0, 0), (1, 1)]).unwrap();
        assert_ne!(field, nested);
        assert_ne!(field, encode(1, 7, &[(3, 1)]).unwrap());
        assert_eq!(field, encode(0, 7, &[(3, 1)]).unwrap());
    }

    #[test]
    fn complete_identity_overflow_rejects() {
        assert!(encode(u64::MAX, 0, &[]).is_err());
        assert!(encode(0, u64::MAX, &[]).is_err());
        assert_ne!(encode(0, 0, &[]).unwrap(), 0);
        assert_eq!(encode(0, 0, &[]).unwrap() & 1, 1);
    }
}
