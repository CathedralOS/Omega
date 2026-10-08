//! Enumerate semantic scalar leaves, without native offsets or byte geometry.
//! These paths pin original backing through the access-specific storage resolver;
//! the host receives closed field identities, never source spellings or keys.
//! Non-scalar members remain original opaque storage: traversal stops at a
//! descriptor rather than following its referent. A native byte adapter must
//! independently cover its accessed interval without pointer or padding bytes.

use crate::errors::{TerminalInterpretError, malformed};
use crate::program::AdmittedTerminalProgram;
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralFieldType, StructuralTypeShape};

type ScalarLeaf = (Vec<CanonicalStructuralPathSegment>, ScalarType);

pub(super) fn scalar_leaves(
    program: &AdmittedTerminalProgram,
    root: StructuralTypeId,
    access: StructuralAccess,
) -> Result<Vec<ScalarLeaf>, TerminalInterpretError> {
    let mut leaves = Vec::new();
    collect(program, root, access, &mut Vec::new(), &mut leaves)?;
    Ok(leaves)
}

fn collect(
    program: &AdmittedTerminalProgram,
    root: StructuralTypeId,
    access: StructuralAccess,
    closed: &mut Vec<CanonicalStructuralPathSegment>,
    leaves: &mut Vec<ScalarLeaf>,
) -> Result<(), TerminalInterpretError> {
    match &program
        .structural_types()
        .get(&root)
        .ok_or_else(malformed)?
        .shape
    {
        StructuralTypeShape::PrimitiveScalar(scalar) => leaves.push((closed.clone(), *scalar)),
        StructuralTypeShape::FusedServiceBinding
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::Sum { .. }
        | StructuralTypeShape::Mixed { .. } => {}
        StructuralTypeShape::Record { fields, .. } => {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                // Shared host access cannot replace an atomic event with an
                // ordinary snapshot. Keep that interval uncovered, while an
                // unrelated ordinary prefix remains observable.
                if matches!(field.field_type, StructuralFieldType::Atomic(_))
                    && access != StructuralAccess::MutableBorrow
                {
                    continue;
                }
                closed.push(CanonicalStructuralPathSegment::Field(field.id));
                if let Some(scalar) = field.field_type.scalar_type() {
                    leaves.push((closed.clone(), scalar));
                } else if let StructuralFieldType::Structural(child) = field.field_type {
                    collect(program, child, access, closed, leaves)?;
                }
                closed.pop();
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            for index in 0..*length {
                closed.push(CanonicalStructuralPathSegment::FixedIndex(index));
                collect(program, *element, access, closed, leaves)?;
                closed.pop();
            }
        }
    }
    Ok(())
}
