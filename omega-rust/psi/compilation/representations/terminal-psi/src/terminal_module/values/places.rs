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

use crate::terminal_module::TerminalIdentityIndex;
use crate::{OperationKind, StructuralAccess};
use semantic_vocabulary::{
    ObligationId, PlaceId, ScalarType, StructuralCaseId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    /// Select one declared sum payload namespace. The next segment must name
    /// its field. Geometry alone grants no authority: the consuming operation
    /// independently proves this exact case active at the original root.
    Case(StructuralCaseId),
    Field(TerminalIdentityIndex),
    FixedIndex(u64),
    /// A call-scoped borrowed window into fixed byte-array backing. It must
    /// end the path; consumers independently check `start <= end <= extent`.
    /// The window is not an owned subtree or an escaping reference result.
    FixedByteRange {
        start: u64,
        end: u64,
    },
    /// A runtime-selected element of a fixed array, or of an element view
    /// at the root (see the module documentation). `index` is an integer
    /// scalar the carrying operation may read — defined before it in the same
    /// machine. `obligation` is owned by that operation; the verifier
    /// reconstructs `index < extent` (and `0 <= index` for a signed carrier)
    /// against the array the preceding segments resolve to, or against a root
    /// view's dominating `ElementViewLength`, and a certificate must discharge
    /// it there.
    RuntimeIndex {
        index: ValueId,
        obligation: ObligationId,
    },
    /// Interpret the same selected storage in an admitted declaration. This
    /// changes the namespace for following projections, not its original home,
    /// carrier residence, access or lifetime. It is neither an executable
    /// conversion nor a descriptor crossing. Receiving custody must reconstruct
    /// it from the actual reference formation; this path alone grants nothing.
    RepresentationView {
        structural_type: StructuralTypeId,
        stored_scalar: Option<ScalarType>,
    },
    /// Cross a reference carrier's borrowed boundary. This never grants owned
    /// access to the referent or includes it in the carrier's cleanup.
    Referent,
}

impl Default for StructuralPathSegment {
    fn default() -> Self {
        // Arena zero is absent, not a projection. An empty field is invalid
        // under the structural path contract, so its dummy grants no access.
        Self::Field(TerminalIdentityIndex::ABSENT)
    }
}

/// The projection inventories' exhaustiveness fallbacks share two runs of
/// scalar-only catalog operations — each restated per inventory so a new
/// operation edited into one arm silently escaped the other. One spec each;
/// the path-bearing `*StructuralField` variants sit between them in the
/// by-path inventory only.
macro_rules! call_and_scalar_constant_operations {
    () => {
        Self::Call { .. }
            | Self::SelectImplementation { .. }
            | Self::PortWrite { .. }
            | Self::IntegerConstant { .. }
            | Self::BooleanConstant { .. }
            | Self::IeeeFloatConstant { .. }
            | Self::IeeeFloatCompare { .. }
            | Self::ScalarRepresentationView { .. }
            | Self::NearestIeeeFloatFusedMultiplyAdd { .. }
    };
}

macro_rules! scalar_predicate_operations {
    () => {
        Self::BooleanNot { .. }
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
            | Self::SaturatingIntegerMultiply { .. }
            | Self::TrappingInteger { .. }
            | Self::FloatResultPolicy { .. }
    };
}

impl StructuralPathSegment {
    /// The runtime selector and its obligation when this segment is a
    /// `RuntimeIndex`.
    pub const fn runtime_index(&self) -> Option<(ValueId, ObligationId)> {
        match self {
            Self::RuntimeIndex { index, obligation } => Some((*index, *obligation)),
            Self::Case(_)
            | Self::Field(_)
            | Self::FixedIndex(_)
            | Self::FixedByteRange { .. }
            | Self::RepresentationView { .. }
            | Self::Referent => None,
        }
    }
}

/// Whether a path is a static projection: no segment depends on a runtime
/// value. Consumers that key facts or layouts by path use this before
/// treating the path as one exact place.
pub fn is_static_structural_path(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| segment.runtime_index().is_none())
}

/// Whether a `StructuralScalarFieldStore` carrier path is within the store
/// grammar: common or selected-case fields and fixed-array elements, literal or
/// runtime-selected, in any order (`ents[i].pos` for `ents[i].pos.x`,
/// `values[0][1]` for `values[0][1].first`). `Referent` crossings and byte
/// windows are outside the store contract: borrowing through another
/// borrow's boundary is different custody, not a projection. Resolution
/// still requires each literal index below its declared extent, and a
/// runtime element's bound is the obligation the store owns.
pub fn is_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
    path.iter().all(|segment| match segment {
        StructuralPathSegment::Field(identity) => !identity.is_absent(),
        StructuralPathSegment::Case(_) => true,
        StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => true,
        StructuralPathSegment::FixedByteRange { .. }
        | StructuralPathSegment::RepresentationView { .. }
        | StructuralPathSegment::Referent => false,
    })
}

/// Static storage uses the same field/array projection grammar as scalar
/// stores. Successive indices and fields after indices are ordinary storage,
/// not different custody. Operations that do not carry runtime selectors
/// additionally require static paths; type, extent and access resolution
/// remain independent. Reference crossings and byte windows stay excluded.
pub fn is_static_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
    is_static_structural_path(path) && is_structural_scalar_store_path(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPlaceDeclaration {
    pub id: PlaceId,
    pub kind: StructuralPlaceKind,
}

/// One name-spelled projection an operation carries: the place it starts
/// from and the ordered path beneath it.
/// An `ElementViewRead` carries a descriptor root but starts its path below
/// the separately selected element; its index/length operands retain the outer
/// live bound. Path consumers resolve descendant types from that element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationProjection<'a> {
    pub root: PlaceId,
    pub path: &'a [StructuralPathSegment],
}

impl OperationKind {
    /// Rewrite every direct runtime structural root operand through `map`, in
    /// operand order. Stores visit their destination before their source.
    ///
    /// This is a mechanical use inventory, not a custody transformation.
    /// Result declarations and establishment destinations are definitions,
    /// not uses. Scalar selectors, paths, access, and identities inside proof
    /// terms, crash continuations, claims or other evidence stay unchanged.
    /// Consumers must separately preserve that evidence and all module-level
    /// catalogs when replacing a place; this method grants no new authority.
    pub fn map_structural_uses(&mut self, map: &mut impl FnMut(PlaceId) -> PlaceId) {
        self.map_structural_uses_with_access(&mut |place, _| map(place));
    }

    /// Visit the same runtime operands with the access each operation needs.
    /// Explicit argument access stays exact; intrinsic observations, writes and
    /// ownership consumption remain distinct. This inventory describes demand,
    /// not permission: independent validation still authenticates the producer,
    /// availability, path, and resource custody before granting that access.
    pub fn map_structural_uses_with_access(
        &mut self,
        map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
    ) {
        match self {
            Self::EstablishPlacedReference { source }
            | Self::EstablishReference { source, .. }
            | Self::EstablishElementView { source, .. }
            | Self::EstablishMutableElementView { source, .. } => {
                source.place = map(source.place, source.access);
            }
            Self::ElementViewLength { source } | Self::ByteSequenceLength { source } => {
                source.place = map(source.place, StructuralAccess::SharedBorrow);
            }
            Self::ReleaseReference { source, .. } => {
                *source = map(*source, StructuralAccess::Owned);
            }
            Self::MoveStructuralField { source, .. } => {
                *source = map(*source, StructuralAccess::MutableBorrow);
            }
            Self::ByteSequenceSubslice { source, .. }
            | Self::ElementViewSubslice { source, .. } => {
                source.place = map(source.place, source.access);
            }
            Self::PrimitiveScalarRead { source, .. }
            | Self::StructuralByteSequenceFieldLength { source, .. }
            | Self::StructuralByteSequenceFieldRead { source, .. }
            | Self::StructuralCaseMembership { source, .. }
            | Self::StructuralLeafCopy { source, .. }
            | Self::StructuralCaseLeafCopy { source, .. }
            | Self::ByteSequenceRead { source, .. }
            | Self::ElementViewRead { source, .. }
            | Self::BooleanStructuralField { source, .. }
            | Self::ScalarStructuralField { source, .. } => {
                *source = map(*source, StructuralAccess::SharedBorrow)
            }
            Self::StructuralByteSequenceFieldByteStore { destination, .. }
            | Self::WriteOnlyPrimitiveStore { destination, .. }
            | Self::StructuralScalarFieldStore { destination, .. }
            | Self::ByteSequenceWrite { destination, .. }
            | Self::ElementViewWrite { destination, .. } => {
                *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
            }
            Self::ByteSequenceStore {
                destination,
                source,
                ..
            }
            | Self::StructuralByteSequenceFieldStore {
                destination,
                source,
                ..
            }
            | Self::StructuralByteViewFieldStore {
                destination,
                source,
                ..
            } => {
                *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                *source = map(*source, StructuralAccess::SharedBorrow);
            }
            Self::StoreStructuralField {
                destination, value, ..
            } => {
                *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                value.place = map(value.place, value.access);
            }
            Self::EstablishByteSequenceConcatenation { left, right, .. }
            | Self::ByteSequenceEqual { left, right } => {
                for operand in [left, right] {
                    map_byte_sequence_operand(operand, map);
                }
            }
            Self::ByteSequencePredicate { operand, .. } => {
                map_byte_sequence_operand(operand, map);
            }
            Self::EstablishStructuralArray { elements } => {
                for argument in elements {
                    argument.place = map(argument.place, argument.access);
                }
            }
            Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                for field in fields {
                    match &mut field.value {
                        crate::RecordFieldValue::Structural(value) => {
                            value.place = map(value.place, value.access);
                        }
                        crate::RecordFieldValue::Scalar { .. } => {}
                    }
                }
            }
            crate::call_structural_argument_variants!(structural_arguments) => {
                for argument in structural_arguments {
                    argument.place = map(argument.place, argument.access);
                }
            }
            Self::AtomicAccess { place, event, .. } => {
                let access = match (event.observes_resident(), event.modifies_resident()) {
                    (true, true) => StructuralAccess::MutableBorrow,
                    (false, true) => StructuralAccess::WriteOnlyBorrow,
                    (_, false) => StructuralAccess::SharedBorrow,
                };
                *place = map(*place, access);
            }
            // Definitions, scalar-only operations and catalog coordinates
            // have no direct structural root operands. Exhaustiveness keeps
            // future operations from silently escaping this inventory.
            Self::EstablishScalarArray { .. }
            | Self::EstablishZeroedStructural { .. }
            | Self::EstablishStructuralLocal { .. }
            | Self::EstablishPrimitiveLocal { .. }
            | Self::EstablishScalarCase { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::AtomicFence { .. }
            | call_and_scalar_constant_operations!()
            | scalar_predicate_operations!() => {}
        }
    }

    /// Every name-spelled structural projection this operation carries, in
    /// operand order. This is the inventory runtime-index consumers walk:
    /// each `RuntimeIndex` found here is a scalar use of the operation and an
    /// obligation the operation owns. Canonical (proposition-safe) paths and
    /// result claim paths are not projections of an operand and never carry a
    /// runtime index.
    pub fn structural_projections(&self) -> Vec<OperationProjection<'_>> {
        let mut projections = Vec::new();
        match self {
            Self::EstablishByteSequenceConcatenation { left, right, .. }
            | Self::ByteSequenceEqual { left, right } => {
                for operand in [left, right] {
                    projections.push(argument_projection(operand.argument()));
                }
            }
            Self::ByteSequencePredicate { operand, .. } => {
                projections.push(argument_projection(operand.argument()));
            }
            Self::EstablishPlacedReference { source }
            | Self::EstablishReference { source, .. }
            | Self::EstablishElementView { source, .. }
            | Self::EstablishMutableElementView { source, .. }
            | Self::ElementViewLength { source }
            | Self::ByteSequenceLength { source }
            | Self::ByteSequenceSubslice { source, .. }
            | Self::ElementViewSubslice { source, .. } => {
                projections.push(argument_projection(source));
            }
            Self::PrimitiveScalarRead { source, path }
            | Self::ElementViewRead { source, path, .. }
            | Self::AtomicAccess {
                place: source,
                path,
                ..
            }
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
            | Self::StructuralByteViewFieldStore {
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
            Self::EstablishStructuralArray { elements } => {
                projections.extend(elements.iter().map(argument_projection));
            }
            Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                projections.extend(fields.iter().filter_map(|field| match &field.value {
                    crate::RecordFieldValue::Structural(value) => Some(argument_projection(value)),
                    crate::RecordFieldValue::Scalar { .. } => None,
                }));
            }
            crate::call_structural_argument_variants!(structural_arguments) => {
                projections.extend(structural_arguments.iter().map(argument_projection))
            }
            // No name-spelled projection. Listed exhaustively so a new
            // operation must declare here whether its paths may carry a
            // runtime element (and so an obligation it owns).
            Self::ReleaseReference { .. }
            | Self::EstablishScalarArray { .. }
            | Self::EstablishZeroedStructural { .. }
            | Self::EstablishStructuralLocal { .. }
            | Self::EstablishPrimitiveLocal { .. }
            | Self::EstablishScalarCase { .. }
            | Self::StructuralCaseLeafCopy { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceRead { .. }
            | Self::ByteSequenceWrite { .. }
            | Self::ByteSequenceStore { .. }
            | Self::ElementViewWrite { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::AtomicFence { .. }
            | call_and_scalar_constant_operations!()
            | Self::BooleanStructuralField { .. }
            | Self::ScalarStructuralField { .. }
            | scalar_predicate_operations!() => {}
        }
        projections
    }

    /// Mutable access to the same inventory's paths, in the same order, for
    /// rewrites that must keep runtime-index operands in step with the
    /// operation's other uses. It mirrors `structural_projections` arm for arm.
    pub fn structural_projection_paths_mut(&mut self) -> Vec<&mut Vec<StructuralPathSegment>> {
        let mut paths = Vec::new();
        match self {
            Self::EstablishByteSequenceConcatenation { left, right, .. }
            | Self::ByteSequenceEqual { left, right } => {
                for operand in [left, right] {
                    paths.push(&mut operand.argument_mut().path);
                }
            }
            Self::ByteSequencePredicate { operand, .. } => {
                paths.push(&mut operand.argument_mut().path);
            }
            Self::EstablishPlacedReference { source }
            | Self::EstablishReference { source, .. }
            | Self::EstablishElementView { source, .. }
            | Self::EstablishMutableElementView { source, .. }
            | Self::ElementViewLength { source }
            | Self::ByteSequenceLength { source }
            | Self::ByteSequenceSubslice { source, .. }
            | Self::ElementViewSubslice { source, .. } => {
                paths.push(&mut source.path);
            }
            Self::ReleaseReference { path, .. }
            | Self::ElementViewWrite { path, .. }
            | Self::PrimitiveScalarRead { path, .. }
            | Self::ElementViewRead { path, .. }
            | Self::AtomicAccess { path, .. }
            | Self::WriteOnlyPrimitiveStore { path, .. }
            | Self::StructuralByteSequenceFieldLength { path, .. }
            | Self::StructuralByteSequenceFieldRead { path, .. }
            | Self::StructuralCaseMembership { path, .. }
            | Self::StructuralLeafCopy { path, .. }
            | Self::MoveStructuralField { path, .. }
            | Self::StructuralByteSequenceFieldByteStore { path, .. }
            | Self::StructuralByteSequenceFieldStore { path, .. }
            | Self::StructuralByteViewFieldStore { path, .. }
            | Self::StructuralScalarFieldStore { path, .. } => paths.push(path),
            Self::StoreStructuralField { path, value, .. } => {
                paths.push(path);
                paths.push(&mut value.path);
            }
            Self::EstablishStructuralArray { elements } => {
                paths.extend(elements.iter_mut().map(|argument| &mut argument.path));
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
            crate::call_structural_argument_variants!(structural_arguments) => paths.extend(
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

/// Both byte operand forms retain their exact root, path and spelled access.
fn map_byte_sequence_operand(
    operand: &mut crate::control_flow::ByteSequenceOperand,
    map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
) {
    let source = operand.argument_mut();
    source.place = map(source.place, source.access);
}

#[cfg(test)]
mod tests {
    use super::{
        StructuralPathSegment, is_static_structural_scalar_store_path,
        is_structural_scalar_store_path,
    };
    use crate::{
        OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralAccess,
        StructuralArgument,
    };
    use semantic_vocabulary::{
        MachineId, ObligationId, PlaceId, ScalarTerm, StructuralFieldId, StructuralTypeId, ValueId,
    };

    fn place(identity: u64) -> PlaceId {
        PlaceId::new(identity).unwrap()
    }

    fn argument(identity: u64) -> StructuralArgument {
        StructuralArgument {
            place: place(identity),
            path: vec![runtime()],
            access: StructuralAccess::SharedBorrow,
        }
    }

    #[test]
    fn structural_use_mapping_preserves_scalar_and_proof_call_operands() {
        let scalar = ValueId::new(1).unwrap();
        let evidence = ScalarTerm::boolean_field(place(1), StructuralFieldId::new(1).unwrap());
        let mut operation = OperationKind::CallStructuralScalar {
            callee: MachineId::new(1).unwrap(),
            arguments: vec![scalar],
            erased_arguments: vec![evidence.clone()],
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![argument(1), argument(2)],
            claim_transfers: Vec::new(),
            requirement_obligations: vec![ObligationId::new(1).unwrap()],
            crash_continuations: Vec::new(),
        };
        let mut expected = operation.clone();
        if let OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        } = &mut expected
        {
            structural_arguments[0].place = place(11);
            structural_arguments[1].place = place(12);
        }
        let mut visited = Vec::new();
        operation.map_structural_uses(&mut |source| {
            visited.push(source);
            place(source.get() + 10)
        });
        assert_eq!(visited, vec![place(1), place(2)]);
        assert_eq!(operation, expected);
    }

    #[test]
    fn structural_use_mapping_visits_nested_initializers_and_byte_operands() {
        use crate::control_flow::ByteSequenceOperand;

        let scalar = ValueId::new(1).unwrap();
        let mut record = OperationKind::EstablishRecord {
            fields: vec![
                RecordFieldInitializer {
                    field: StructuralFieldId::new(1).unwrap(),
                    value: RecordFieldValue::Scalar {
                        value: scalar,
                        range_obligation: None,
                    },
                },
                RecordFieldInitializer {
                    field: StructuralFieldId::new(2).unwrap(),
                    value: RecordFieldValue::Structural(argument(2)),
                },
            ],
        };
        let mut expected = record.clone();
        if let OperationKind::EstablishRecord { fields } = &mut expected
            && let RecordFieldValue::Structural(source) = &mut fields[1].value
        {
            source.place = place(12);
        }
        record.map_structural_uses(&mut |source| place(source.get() + 10));
        assert_eq!(record, expected);

        let mut concatenation = OperationKind::EstablishByteSequenceConcatenation {
            left: ByteSequenceOperand::Whole {
                source: argument(1),
                length: scalar,
            },
            right: ByteSequenceOperand::Field {
                source: argument(2),
                field: StructuralFieldId::new(1).unwrap(),
                length: scalar,
            },
            capacity_obligation: ObligationId::new(1).unwrap(),
        };
        let mut expected = concatenation.clone();
        if let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &mut expected
        {
            *left = ByteSequenceOperand::Whole {
                source: argument(11),
                length: scalar,
            };
            if let ByteSequenceOperand::Field { source, .. } = right {
                source.place = place(12);
            }
        }
        concatenation.map_structural_uses(&mut |source| place(source.get() + 10));
        assert_eq!(concatenation, expected);
    }

    #[test]
    fn structural_use_mapping_distinguishes_stores_from_establishments() {
        let mut store = OperationKind::StoreStructuralField {
            disposition: crate::StructuralStoreDisposition::Vacant,
            destination: place(1),
            path: vec![runtime()],
            field: StructuralFieldId::new(1),
            value: argument(2),
        };
        let mut expected = store.clone();
        if let OperationKind::StoreStructuralField {
            destination, value, ..
        } = &mut expected
        {
            *destination = place(11);
            value.place = place(12);
        }
        let mut visited = Vec::new();
        store.map_structural_uses(&mut |source| {
            visited.push(source);
            place(source.get() + 10)
        });
        assert_eq!(visited, vec![place(1), place(2)]);
        assert_eq!(store, expected);

        let mut view = OperationKind::EstablishMutableElementView {
            region: crate::ElementViewRegion::WholeCollection,
            destination: place(1),
            source: argument(2),
            element: StructuralTypeId::new(1).unwrap(),
        };
        let mut expected = view.clone();
        if let OperationKind::EstablishMutableElementView { source, .. } = &mut expected {
            source.place = place(12);
        }
        view.map_structural_uses(&mut |source| place(source.get() + 10));
        assert_eq!(view, expected);
        for mut declaration in [
            OperationKind::EstablishTrivialAffineLocal {
                destination: place(1),
            },
            OperationKind::EstablishByteSequenceLiteral {
                destination: place(1),
                bytes: vec![7],
                qualifications: Vec::new(),
            },
        ] {
            declaration
                .map_structural_uses(&mut |_| panic!("a destination declaration is not a use"));
        }
    }

    fn field(identity: u32) -> StructuralPathSegment {
        StructuralPathSegment::Field(crate::TerminalIdentityIndex(identity))
    }

    fn runtime() -> StructuralPathSegment {
        StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(1).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }
    }

    #[test]
    fn field_store_carriers_compose_fields_and_elements_in_any_order() {
        for path in [
            vec![],
            vec![field(1), StructuralPathSegment::FixedIndex(2)],
            vec![field(1), runtime(), field(2)],
            vec![field(3), runtime(), StructuralPathSegment::FixedIndex(1)],
            vec![StructuralPathSegment::FixedIndex(0), field(4)],
        ] {
            assert!(is_structural_scalar_store_path(&path), "{path:?}");
        }
        for path in [
            vec![StructuralPathSegment::Referent],
            vec![field(1), StructuralPathSegment::Referent],
            vec![StructuralPathSegment::FixedByteRange { start: 0, end: 1 }],
            vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                0,
            ))],
        ] {
            assert!(!is_structural_scalar_store_path(&path), "{path:?}");
        }
    }

    #[test]
    fn static_store_paths_compose_fields_and_literal_indices() {
        for path in [
            vec![],
            vec![field(1)],
            vec![field(2), field(3)],
            vec![StructuralPathSegment::FixedIndex(0)],
            vec![field(4), StructuralPathSegment::FixedIndex(2)],
            vec![
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::FixedIndex(1),
            ],
            vec![StructuralPathSegment::FixedIndex(0), field(5)],
            vec![field(4), StructuralPathSegment::FixedIndex(0), field(5)],
        ] {
            assert!(is_static_structural_scalar_store_path(&path), "{path:?}");
        }
        for path in [
            // A runtime element is outside the bounded grammar.
            vec![field(4), runtime()],
            // Borrowing through another borrow's boundary is different
            // custody, not a projection.
            vec![StructuralPathSegment::Referent],
            vec![field(1), StructuralPathSegment::Referent],
            vec![StructuralPathSegment::Referent, field(1)],
            // An empty identity names no record field.
            vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                0,
            ))],
        ] {
            assert!(!is_static_structural_scalar_store_path(&path), "{path:?}");
        }
    }
}
