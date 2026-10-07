//! The canonical referent declarations projected arguments replay against:
//! operation-established places, then non-entry block parameters, then caller
//! parameters — the same precedence
//! `structural_argument_sources::expected_sources` binds homes.

use std::collections::BTreeMap;

use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, AbstractStructuralCallResult,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralParameterDeclaration};

/// structural type and the access that declaration grants. Operation
/// establishments own their storage; caller and block parameters keep their
/// declared access. `local_loan` marks a non-linear, unqualified,
/// claim-free operation home whose collection projection may be lent through
/// the common exact window relation. `operation_home`
/// marks addressable aggregate storage established in this body, independently
/// of the other operands or the result of a call that borrows it.
pub(super) struct RootDeclaration {
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: StructuralAccess,
    pub local_loan: bool,
    pub operation_home: bool,
}

impl RootDeclaration {
    /// Present a local home through the argument's own borrow for
    /// fixed-array window geometry, exactly as Terminal verification presents
    /// its array-result loan. The home itself stays owned; the window relation
    /// separately checks the projection. Parameters present nothing here.
    pub(super) fn loan_presentation(
        &self,
        semantic: &StructuralArgument,
        declared: &StructuralParameterDeclaration,
    ) -> Option<StructuralParameterDeclaration> {
        (self.local_loan
            && matches!(
                semantic.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ))
        .then(|| StructuralParameterDeclaration {
            place: semantic.place,
            position: declared.position,
            structural_type: self.structural_type,
            multiplicity: self.multiplicity,
            access: self.access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        })
    }
}

/// Reconstruct each argument place's referent declaration in the same
/// precedence `structural_argument_sources::expected_sources` binds homes:
/// operation-established places, then non-entry block parameters, then
/// caller parameters. Every projected argument must replay its root type and
/// byte offset against the referent's own declaration, not only when that
/// referent happens to arrive as a machine parameter.
pub(super) fn canonical_roots(source: &AbstractFunction) -> BTreeMap<PlaceId, RootDeclaration> {
    let mut roots = BTreeMap::new();
    for operation in &source.operations {
        let (place, structural_type, multiplicity, operation_home) = match operation {
            AbstractOperation::EstablishScalarArray { result, .. } | AbstractOperation::EstablishStructuralArray { result, .. }
            | AbstractOperation::EstablishZeroedStructural { result, .. }
            | AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | AbstractOperation::BoundaryCall {
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => (result.place, result.structural_type, result.multiplicity, true),
            AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishReference { result, .. }
            | AbstractOperation::EstablishCase { result, .. }
            | AbstractOperation::EstablishStructuralLocal { result, .. }
            | AbstractOperation::StructuralLeafCopy { result, .. }
            // Extraction transfers the field into a distinct owned root;
            // its later call operand is not the borrowed source root.
            | AbstractOperation::MoveStructuralField { result, .. } => {
                (result.place, result.structural_type, result.multiplicity, true)
            }
            // Primitive storage and byte views are not aggregate backing homes.
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::ByteSequenceSubslice { result, .. } => {
                (result.place, result.structural_type, result.multiplicity, false)
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type,
                ..
            } => (place.id, structural_type.id, terminal_psi::StructuralMultiplicity::Unrestricted, false),
            AbstractOperation::EstablishTrivialAffineLocal {
                place,
                structural_type,
                ..
            } => (place.id, structural_type.id, terminal_psi::StructuralMultiplicity::Affine, true),
            _ => continue,
        };
        let local_loan = operation_home
            && operation.structural_result().is_some_and(|(_, result)| {
                result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            });
        roots.entry(place).or_insert(RootDeclaration {
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            local_loan,
            operation_home,
        });
    }
    for entry in &source.block_entries {
        if entry.block == source.entry {
            continue;
        }
        for parameter in &entry.structural_parameters {
            roots.entry(parameter.place).or_insert(RootDeclaration {
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                local_loan: false,
                operation_home: false,
            });
        }
    }
    for parameter in &source.structural_parameters {
        roots.entry(parameter.place).or_insert(RootDeclaration {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            local_loan: false,
            operation_home: false,
        });
    }
    roots
}
