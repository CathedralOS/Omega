//! Structural places and the name-spelled projections beneath them.
//!
//! # Runtime-selected elements
//!
//! A path may select an array element with a value computed at run time
//! (`self.items[self.cursor].hp`, `grid[row][column]`). That selection is one
//! path segment, `RuntimeIndex { index, obligation }`, so it composes at any
//! depth: fields and further indexes may follow it, and every operation that
//! takes a name-spelled path — a call argument, a leaf copy, a store or read
//! of a primitive leaf — accepts it without an operation of its own.
//!
//! The segment names a runtime scalar (`index`) and the bounds evidence for it
//! (`obligation`); it never names a byte offset and grants no authority. The
//! operation whose path contains the segment owns the obligation. The
//! verifier resolves the prefix to its fixed array and reconstructs the
//! proposition itself — `index < extent`, plus `0 <= index` for a signed
//! carrier — against the facts that hold at that operation, so a producer
//! never states a bound the verifier would have to trust. A machine
//! parameter used as a selector is the same case: its value is the index and
//! the caller's published `requires` discharge the obligation.
//!
//! Two alternatives were rejected. The previous segment named a direct
//! parameter position and restated its published entry interval; that could
//! only be replayed against entry evidence, so a field, local, or computed
//! selector had no spelling. Dedicated indexed store/read operations took the
//! index as an operand beside a static path; the index then had to be the last
//! step, so `items[i].hp` and `grid[i][j]` had no spelling either.
//!
//! Runtime indexes stay out of `CanonicalStructuralPathSegment`: canonical
//! paths key facts inside propositions, and a runtime-selected element is not
//! a fact key without alias reasoning. Consumers treat a runtime segment as
//! "some element of this array": it may overlap any sibling index, and a write
//! through it forgets the facts observed beneath the array's root.

use crate::OperationKind;
use semantic_vocabulary::{ObligationId, PlaceId, StructuralPlaceKind, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    Field(String),
    FixedIndex(u64),
    /// A call-scoped borrowed window into fixed byte-array backing. It must
    /// end the path; consumers independently check `start <= end <= extent`.
    /// The window is not an owned subtree or an escaping reference result.
    FixedByteRange {
        start: u64,
        end: u64,
    },
    /// A runtime-selected element of a fixed array (see the module
    /// documentation). `index` is an integer scalar the carrying operation
    /// may read — defined before it in the same machine. `obligation` is owned
    /// by that operation; the verifier reconstructs `index < extent` (and
    /// `0 <= index` for a signed carrier) against the array the preceding
    /// segments resolve to, and a certificate must discharge it there.
    RuntimeIndex {
        index: ValueId,
        obligation: ObligationId,
    },
    /// Cross a reference carrier's borrowed boundary. This never grants owned
    /// access to the referent or includes it in the carrier's cleanup.
    Referent,
}

impl StructuralPathSegment {
    /// The runtime selector and its obligation when this segment is a
    /// `RuntimeIndex`.
    pub const fn runtime_index(&self) -> Option<(ValueId, ObligationId)> {
        match self {
            Self::RuntimeIndex { index, obligation } => Some((*index, *obligation)),
            Self::Field(_) | Self::FixedIndex(_) | Self::FixedByteRange { .. } | Self::Referent => {
                None
            }
        }
    }
}

/// Whether a path is a static projection: no segment depends on a runtime
/// value. Consumers that key facts or layouts by path use this before
/// treating the path as one exact place.
pub fn is_static_structural_path(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| segment.runtime_index().is_none())
}

/// Whether a scalar-store carrier path is within the currently executable
/// bounded projection grammar: record fields, optionally followed by one
/// literal fixed-array index. A bare fixed-array root has no record-field
/// owner, so its carrier path is the literal element index alone. Anything
/// after the first index — a second index or a further field — is excluded
/// by the grammar itself, not by path resolution, and `Referent` crossings
/// are outside the store contract: borrowing through another borrow's
/// boundary is different custody, not a projection. A runtime index is the
/// primitive leaf store's projection, not this field store's; resolution
/// still requires each literal index below its declared extent.
pub fn is_bounded_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
    let first_index = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        .unwrap_or(path.len());
    let index_count = path.len() - first_index;
    path[..first_index].iter().all(
        |segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()),
    ) && path[first_index..]
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        && index_count <= 1
}

impl From<String> for StructuralPathSegment {
    fn from(identity: String) -> Self {
        Self::Field(identity)
    }
}

impl From<&str> for StructuralPathSegment {
    fn from(identity: &str) -> Self {
        Self::Field(identity.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPlaceDeclaration {
    pub id: PlaceId,
    pub kind: StructuralPlaceKind,
}

/// One name-spelled projection an operation carries: the place it starts
/// from and the ordered path beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationProjection<'a> {
    pub root: PlaceId,
    pub path: &'a [StructuralPathSegment],
}

impl OperationKind {
    /// Every name-spelled structural projection this operation carries, in
    /// operand order. This is the inventory runtime-index consumers walk:
    /// each `RuntimeIndex` found here is a scalar use of the operation and an
    /// obligation the operation owns. Canonical (proposition-safe) paths and
    /// result claim paths are not projections of an operand and never carry a
    /// runtime index.
    pub fn structural_projections(&self) -> Vec<OperationProjection<'_>> {
        let mut projections = Vec::new();
        match self {
            Self::EstablishReference { source } | Self::EstablishElementView { source, .. } => {
                projections.push(argument_projection(source));
            }
            Self::PrimitiveScalarRead { source, path }
            | Self::StructuralByteSequenceFieldLength { source, path, .. }
            | Self::StructuralByteSequenceFieldRead { source, path, .. }
            | Self::StructuralCaseMembership { source, path, .. }
            | Self::StructuralLeafCopy { source, path }
            | Self::MoveStructuralField { source, path, .. } => {
                projections.push(OperationProjection {
                    root: *source,
                    path,
                });
            }
            Self::WriteOnlyPrimitiveStore {
                destination, path, ..
            }
            | Self::StructuralByteSequenceFieldByteStore {
                destination, path, ..
            }
            | Self::StructuralByteSequenceFieldStore {
                destination, path, ..
            }
            | Self::StructuralScalarFieldStore {
                destination, path, ..
            } => projections.push(OperationProjection {
                root: *destination,
                path,
            }),
            Self::StoreStructuralField {
                destination,
                path,
                value,
                ..
            } => {
                projections.push(OperationProjection {
                    root: *destination,
                    path,
                });
                projections.push(argument_projection(value));
            }
            Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                projections.extend(fields.iter().filter_map(|field| match &field.value {
                    crate::RecordFieldValue::Structural(value) => Some(argument_projection(value)),
                    crate::RecordFieldValue::Scalar { .. } => None,
                }));
            }
            Self::CallUnit {
                structural_arguments,
                ..
            }
            | Self::CallStructuralScalar {
                structural_arguments,
                ..
            }
            | Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::CallStructuralWithScalarArguments {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            } => projections.extend(structural_arguments.iter().map(argument_projection)),
            // No name-spelled projection. Listed exhaustively so a new
            // operation must declare here whether its paths may carry a
            // runtime element (and so an obligation it owns).
            Self::ReleaseReference { .. }
            | Self::EstablishScalarArray { .. }
            | Self::EstablishPrimitiveLocal { .. }
            | Self::EstablishScalarCase { .. }
            | Self::StructuralCaseLeafCopy { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceLength { .. }
            | Self::ByteSequenceRead { .. }
            | Self::ByteSequenceWrite { .. }
            | Self::ByteSequenceSubslice { .. }
            | Self::ElementViewLength { .. }
            | Self::ElementViewRead { .. }
            | Self::ElementViewSubslice { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::StoreDynamicDescriptor { .. }
            | Self::Call { .. }
            | Self::CallDynamicScalar { .. }
            | Self::CallDynamicParameterScalar { .. }
            | Self::CallDynamicUnit { .. }
            | Self::CallDynamicParameterUnit { .. }
            | Self::PortWrite { .. }
            | Self::IntegerConstant { .. }
            | Self::BooleanConstant { .. }
            | Self::IeeeFloatConstant { .. }
            | Self::IeeeFloatCompare { .. }
            | Self::NearestIeeeFloatFusedMultiplyAdd { .. }
            | Self::BooleanStructuralField { .. }
            | Self::IntegerStructuralField { .. }
            | Self::BooleanNot { .. }
            | Self::BooleanEqual { .. }
            | Self::IntegerEqual { .. }
            | Self::IntegerLessThan { .. }
            | Self::IntegerLessOrEqual { .. }
            | Self::IntegerBitwiseNot { .. }
            | Self::IntegerWiden { .. }
            | Self::IntegerExactCast { .. }
            | Self::IntegerBitwiseAnd { .. }
            | Self::IntegerBitwiseOr { .. }
            | Self::IntegerBitwiseXor { .. }
            | Self::WrappingIntegerShiftLeft { .. }
            | Self::WrappingIntegerShiftRight { .. }
            | Self::ExactIntegerShiftLeft { .. }
            | Self::ExactIntegerShiftRight { .. }
            | Self::ExactIntegerAdd { .. }
            | Self::ExactIntegerSubtract { .. }
            | Self::ExactIntegerMultiply { .. }
            | Self::ExactIntegerDivide { .. }
            | Self::ExactIntegerRemainder { .. }
            | Self::WrappingIntegerDivide { .. }
            | Self::WrappingIntegerRemainder { .. }
            | Self::SaturatingIntegerDivide { .. }
            | Self::SaturatingIntegerRemainder { .. }
            | Self::WrappingIntegerAdd { .. }
            | Self::SaturatingIntegerAdd { .. }
            | Self::WrappingIntegerSubtract { .. }
            | Self::SaturatingIntegerSubtract { .. }
            | Self::WrappingIntegerMultiply { .. }
            | Self::SaturatingIntegerMultiply { .. } => {}
        }
        projections
    }

    /// Mutable access to the same inventory's paths, in the same order, for
    /// rewrites that must keep runtime-index operands in step with the
    /// operation's other uses. It mirrors `structural_projections` arm for arm.
    pub fn structural_projection_paths_mut(&mut self) -> Vec<&mut Vec<StructuralPathSegment>> {
        let mut paths = Vec::new();
        match self {
            Self::EstablishReference { source } | Self::EstablishElementView { source, .. } => {
                paths.push(&mut source.path);
            }
            Self::PrimitiveScalarRead { path, .. }
            | Self::WriteOnlyPrimitiveStore { path, .. }
            | Self::StructuralByteSequenceFieldLength { path, .. }
            | Self::StructuralByteSequenceFieldRead { path, .. }
            | Self::StructuralCaseMembership { path, .. }
            | Self::StructuralLeafCopy { path, .. }
            | Self::MoveStructuralField { path, .. }
            | Self::StructuralByteSequenceFieldByteStore { path, .. }
            | Self::StructuralByteSequenceFieldStore { path, .. }
            | Self::StructuralScalarFieldStore { path, .. } => paths.push(path),
            Self::StoreStructuralField { path, value, .. } => {
                paths.push(path);
                paths.push(&mut value.path);
            }
            Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                paths.extend(
                    fields
                        .iter_mut()
                        .filter_map(|field| match &mut field.value {
                            crate::RecordFieldValue::Structural(value) => Some(&mut value.path),
                            crate::RecordFieldValue::Scalar { .. } => None,
                        }),
                );
            }
            Self::CallUnit {
                structural_arguments,
                ..
            }
            | Self::CallStructuralScalar {
                structural_arguments,
                ..
            }
            | Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::CallStructuralWithScalarArguments {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            } => paths.extend(
                structural_arguments
                    .iter_mut()
                    .map(|argument| &mut argument.path),
            ),
            _ => {}
        }
        paths
    }

    /// Every runtime index this operation's projections carry, in operand
    /// order, with the obligation the operation owns for it.
    pub fn runtime_indexes(&self) -> Vec<(ValueId, ObligationId)> {
        self.structural_projections()
            .iter()
            .flat_map(|projection| projection.path)
            .filter_map(StructuralPathSegment::runtime_index)
            .collect()
    }
}

fn argument_projection(argument: &crate::StructuralArgument) -> OperationProjection<'_> {
    OperationProjection {
        root: argument.place,
        path: &argument.path,
    }
}

#[cfg(test)]
mod tests {
    use super::{StructuralPathSegment, is_bounded_structural_scalar_store_path};
    use semantic_vocabulary::{ObligationId, ValueId};

    fn field(identity: &str) -> StructuralPathSegment {
        StructuralPathSegment::Field(identity.to_owned())
    }

    #[test]
    fn scalar_store_carrier_paths_are_fields_then_one_literal_index() {
        for path in [
            vec![],
            vec![field("record")],
            vec![field("outer"), field("inner")],
            vec![StructuralPathSegment::FixedIndex(0)],
            vec![field("items"), StructuralPathSegment::FixedIndex(2)],
        ] {
            assert!(is_bounded_structural_scalar_store_path(&path), "{path:?}");
        }
        for path in [
            // A second index and a field after the index are outside the
            // grammar even where a resolver could still walk them.
            vec![
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::FixedIndex(1),
            ],
            vec![StructuralPathSegment::FixedIndex(0), field("nested")],
            vec![
                field("items"),
                StructuralPathSegment::FixedIndex(0),
                field("nested"),
            ],
            // A runtime element is the primitive leaf store's projection.
            vec![
                field("items"),
                StructuralPathSegment::RuntimeIndex {
                    index: ValueId::new(1).unwrap(),
                    obligation: ObligationId::new(1).unwrap(),
                },
            ],
            // Borrowing through another borrow's boundary is different
            // custody, not a projection.
            vec![StructuralPathSegment::Referent],
            vec![field("record"), StructuralPathSegment::Referent],
            vec![StructuralPathSegment::Referent, field("record")],
            // An empty identity names no record field.
            vec![StructuralPathSegment::Field(String::new())],
        ] {
            assert!(!is_bounded_structural_scalar_store_path(&path), "{path:?}");
        }
    }
}
