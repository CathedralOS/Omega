//! Semantic contents follow the active declaration, not native memory. Case
//! steps authenticate an overlay; runtime storage omits that step only after
//! choosing the nominal case. Common and payload fields keep distinct keys.

use crate::effects::TerminalEffectRejection;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::{PrimitiveStorage, PrimitiveStorageEndpoint};
use crate::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    TerminalStructuralValue,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, PlaceId, ScalarType, StructuralCaseId, StructuralTypeId,
};
use std::sync::Arc;
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalStorageContents {
    Scalar {
        value: TerminalScalarValue,
        stored_scalar: ScalarType,
    },
    Case(StructuralCaseId),
}

/// One semantic cell; its path is relative to the captured argument. A case
/// cell selects the sum, while its payload paths include the selected Case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryStorageValue {
    pub path: Vec<Path>,
    pub contents: TerminalStorageContents,
}

#[derive(Debug)]
pub struct TerminalBoundaryStructuralBuffer {
    pub(super) identity: Arc<()>,
    pub(super) argument_index: usize,
    pub(super) access: StructuralAccess,
    pub(super) structural_type: StructuralTypeId,
    pub(super) values: Vec<TerminalBoundaryStorageValue>,
}

impl TerminalBoundaryStructuralBuffer {
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.structural_type
    }
    pub fn values(&self) -> &[TerminalBoundaryStorageValue] {
        &self.values
    }

    /// Stage complete active contents. Completion checks nominal membership,
    /// field restrictions, original identity and retained opaque children before
    /// any interpreter storage changes. This is not a native tag constructor.
    pub fn replace(
        &mut self,
        values: Vec<TerminalBoundaryStorageValue>,
    ) -> Result<(), TerminalEffectRejection> {
        if self.access != StructuralAccess::MutableBorrow {
            return Err(TerminalEffectRejection::new(
                "owned or shared boundary storage cannot be replaced",
            ));
        }
        self.values = values;
        Ok(())
    }
}

pub(crate) struct StorageLeaf {
    pub path: Vec<Path>,
    pub kind: StorageLeafKind,
}

pub(crate) enum StorageLeafKind {
    Scalar {
        destination: PrimitiveStorageEndpoint,
        scalar: ScalarType,
        restriction: Option<StructuralFieldType>,
        stored_scalar: ScalarType,
    },
    Case {
        destination: StructuralRuntimePlace,
        structural_type: StructuralTypeId,
        case: StructuralCaseId,
    },
    Opaque {
        destination: StructuralRuntimePlace,
        declaration: StructuralTypeId,
    },
    ByteField {
        parent: StructuralRuntimePlace,
        field: semantic_vocabulary::StructuralFieldId,
        declaration: StructuralFieldType,
    },
    ByteArray {
        destination: StructuralRuntimePlace,
        length: u64,
    },
}

pub(crate) struct PinnedScalar {
    pub path: Vec<Path>,
    pub storage: PrimitiveStorage,
    pub scalar: ScalarType,
    pub shared: bool,
    pub original: TerminalScalarValue,
    pub restriction: Option<StructuralFieldType>,
}

pub(crate) fn describe(
    execution: &TerminalExecution,
    referent: &TerminalStructuralValue,
    access: StructuralAccess,
    replacement: Option<&[TerminalBoundaryStorageValue]>,
) -> Result<Vec<StorageLeaf>, TerminalInterpretError> {
    let mut leaves = Vec::new();
    collect(
        execution,
        referent.structural_type,
        &StructuralRuntimePlace::from(referent),
        access,
        replacement,
        None,
        &mut Vec::new(),
        &mut leaves,
    )?;
    Ok(leaves)
}

/// Select one semantic leaf without expanding sibling records or arrays.
/// Boundary snapshots still request the complete active roster via `describe`.
pub(crate) fn select(
    execution: &TerminalExecution,
    referent: &TerminalStructuralValue,
    selected_path: &[Path],
) -> Result<Option<StorageLeaf>, TerminalInterpretError> {
    let mut leaves = Vec::new();
    collect(
        execution,
        referent.structural_type,
        &StructuralRuntimePlace::from(referent),
        StructuralAccess::SharedBorrow,
        None,
        Some(selected_path),
        &mut Vec::new(),
        &mut leaves,
    )?;
    Ok(leaves.into_iter().find(|leaf| leaf.path == selected_path))
}

fn collect(
    execution: &TerminalExecution,
    root: StructuralTypeId,
    destination: &StructuralRuntimePlace,
    access: StructuralAccess,
    replacement: Option<&[TerminalBoundaryStorageValue]>,
    selected_path: Option<&[Path]>,
    path: &mut Vec<Path>,
    leaves: &mut Vec<StorageLeaf>,
) -> Result<(), TerminalInterpretError> {
    let declaration = execution
        .program
        .structural_types()
        .get(&root)
        .ok_or_else(malformed)?;
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => leaves.push(StorageLeaf {
            path: path.clone(),
            kind: StorageLeafKind::Scalar {
                destination: PrimitiveStorageEndpoint::Scalar(destination.clone()),
                scalar: *scalar,
                restriction: None,
                stored_scalar: *scalar,
            },
        }),
        StructuralTypeShape::Record { fields, .. } => collect_fields(
            execution,
            fields,
            destination,
            access,
            replacement,
            selected_path,
            path,
            leaves,
            false,
        )?,
        StructuralTypeShape::FixedArray { element, length } => {
            if execution.program.structural_types().get(element).is_some_and(|row| matches!(row.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(carrier)) if carrier.bits() == 8 && !carrier.is_address() && carrier.sign() == semantic_vocabulary::IntegerSign::Unsigned)) {
                leaves.push(StorageLeaf { path: path.clone(), kind: StorageLeafKind::ByteArray { destination: destination.clone(), length: *length } });
            }
            let elements = match selected_path {
                None => 0..*length,
                Some(selected) => match selected.get(path.len()) {
                    Some(Path::FixedIndex(index)) if index < length => *index..index + 1,
                    _ => 0..0,
                },
            };
            for element_index in elements {
                path.push(Path::FixedIndex(element_index));
                let mut child = destination.clone();
                child
                    .path
                    .push(StructuralPathSegment::FixedIndex(element_index));
                collect(
                    execution,
                    *element,
                    &child,
                    access,
                    replacement,
                    selected_path,
                    path,
                    leaves,
                )?;
                path.pop();
            }
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let case = if let Some(values) = replacement {
                // Completion first admits strict canonical order. Find one
                // nominal selector without rescanning a large array per case.
                let position = values
                    .binary_search_by(|value| value.path.cmp(path))
                    .map_err(|_| malformed())?;
                let value = &values[position];
                let TerminalStorageContents::Case(case) = value.contents else {
                    return Err(malformed());
                };
                case
            } else {
                let contents = execution
                    .structural_cases
                    .get(destination)
                    .ok_or_else(malformed)?;
                if contents.structural_type != root {
                    return Err(malformed());
                }
                contents.case
            };
            let selected = cases
                .iter()
                .find(|selected| selected.id == case)
                .ok_or_else(malformed)?;
            leaves.push(StorageLeaf {
                path: path.clone(),
                kind: StorageLeafKind::Case {
                    destination: destination.clone(),
                    structural_type: root,
                    case,
                },
            });
            if let StructuralTypeShape::Mixed { fields, .. } = &declaration.shape {
                collect_fields(
                    execution,
                    fields,
                    destination,
                    access,
                    replacement,
                    selected_path,
                    path,
                    leaves,
                    false,
                )?;
            }
            if selected_path
                .is_some_and(|selected| selected.get(path.len()) != Some(&Path::Case(case)))
            {
                return Ok(());
            }
            path.push(Path::Case(case));
            collect_fields(
                execution,
                &selected.fields,
                destination,
                access,
                replacement,
                selected_path,
                path,
                leaves,
                true,
            )?;
            path.pop();
        }
        _ => leaves.push(StorageLeaf {
            path: path.clone(),
            kind: StorageLeafKind::Opaque {
                destination: destination.clone(),
                declaration: root,
            },
        }),
    }
    Ok(())
}

fn collect_fields(
    execution: &TerminalExecution,
    fields: &[StructuralFieldDeclaration],
    parent: &StructuralRuntimePlace,
    access: StructuralAccess,
    replacement: Option<&[TerminalBoundaryStorageValue]>,
    selected_path: Option<&[Path]>,
    path: &mut Vec<Path>,
    leaves: &mut Vec<StorageLeaf>,
    payload: bool,
) -> Result<(), TerminalInterpretError> {
    for field in fields.iter().filter(|field| {
        !field.relevance.is_erased()
            && !matches!(field.field_type, StructuralFieldType::Erased { .. })
    }) {
        if selected_path
            .is_some_and(|selected| selected.get(path.len()) != Some(&Path::Field(field.id)))
        {
            continue;
        }
        if matches!(field.field_type, (StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)))
            && access != StructuralAccess::MutableBorrow
        {
            continue;
        }
        path.push(Path::Field(field.id));
        if let Some(scalar) = field.field_type.scalar_type() {
            if matches!(
                field.storage,
                Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
            ) {
                return Err(malformed());
            }
            let destination = if payload {
                StructuralScalarRuntimeField::case_payload(parent.clone(), field.id)
            } else {
                StructuralScalarRuntimeField::member(parent.clone(), field.id)
            };
            leaves.push(StorageLeaf {
                path: path.clone(),
                kind: StorageLeafKind::Scalar {
                    destination: PrimitiveStorageEndpoint::ScalarField(destination),
                    scalar,
                    restriction: Some(field.field_type.clone()),
                    stored_scalar: field
                        .storage
                        .as_ref()
                        .and_then(|storage| storage.stored_scalar())
                        .unwrap_or(scalar),
                },
            });
        } else if let Some(child_type) = field.field_type.as_structural() {
            let mut child = parent.clone();
            child
                .path
                .push(StructuralPathSegment::Field(field.identity.clone()));
            collect(
                execution,
                child_type,
                &child,
                access,
                replacement,
                selected_path,
                path,
                leaves,
            )?;
        } else {
            // Inline byte fields retain their parent-owned descriptor, not an
            // invented standalone structural type or native pointer word.
            leaves.push(StorageLeaf {
                path: path.clone(),
                kind: StorageLeafKind::ByteField {
                    parent: parent.clone(),
                    field: field.id,
                    declaration: field.field_type.clone(),
                },
            });
        }
        path.pop();
    }
    Ok(())
}

pub(super) fn scalar_storage(
    execution: &TerminalExecution,
    referent: &TerminalStructuralValue,
    array_loan: Option<&crate::element_views::ElementView>,
    access: StructuralAccess,
    leaf: &StorageLeaf,
) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
    let StorageLeafKind::Scalar { scalar, .. } = leaf.kind else {
        return Err(malformed());
    };
    if let Some(loan) = array_loan {
        let path = leaf
            .path
            .iter()
            .map(|step| match step {
                Path::FixedIndex(index) => Ok(StructuralPathSegment::FixedIndex(*index)),
                _ => Err(malformed()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        loan.scalar_storage(execution, &path, access == StructuralAccess::MutableBorrow)
    } else {
        execution.boundary_scalar_storage(referent, &leaf.path, scalar, access)
    }
}

pub(crate) fn capture(
    execution: &TerminalExecution,
    source: PlaceId,
    referent: &TerminalStructuralValue,
    array_loan: Option<&crate::element_views::ElementView>,
    access: StructuralAccess,
    leaves: &[StorageLeaf],
) -> Result<(Vec<TerminalBoundaryStorageValue>, Vec<PinnedScalar>), TerminalInterpretError> {
    let mut values = Vec::new();
    let mut endpoints = Vec::new();
    for leaf in leaves {
        let contents = match &leaf.kind {
            StorageLeafKind::Case { case, .. } => TerminalStorageContents::Case(*case),
            StorageLeafKind::Scalar {
                scalar,
                restriction,
                stored_scalar,
                ..
            } => {
                let (storage, logical, shared) =
                    scalar_storage(execution, referent, array_loan, access, leaf)?;
                if logical != *scalar {
                    return Err(malformed());
                }
                // Owned payload snapshots publish no replacement or loan. Their
                // qualifications remain on the consumed argument; a read cannot
                // weaken that owner or manufacture another qualified value.
                if access != StructuralAccess::Owned {
                    execution.validate_scalar_loan_qualifications(&storage)?;
                }
                let value = execution.read_primitive_storage(
                    source,
                    storage.clone(),
                    logical,
                    logical,
                    shared,
                )?;
                if restriction
                    .as_ref()
                    .is_some_and(|field| !value.satisfies_field(field))
                {
                    return Err(malformed());
                }
                let original_restriction =
                    execution.captured_scalar_restriction(&storage, access)?;
                if original_restriction.as_ref().is_some_and(|field| {
                    !crate::effects::scalar_loans::satisfies_source_restriction(
                        value,
                        field,
                        storage.encoding,
                    )
                }) {
                    return Err(malformed());
                }
                endpoints.push(PinnedScalar {
                    path: leaf.path.clone(),
                    storage: storage.clone(),
                    scalar: logical,
                    shared,
                    original: value,
                    restriction: original_restriction,
                });
                TerminalStorageContents::Scalar {
                    value,
                    stored_scalar: storage.storage_scalar().unwrap_or(*stored_scalar),
                }
            }
            StorageLeafKind::Opaque { .. }
            | StorageLeafKind::ByteField { .. }
            | StorageLeafKind::ByteArray { .. } => continue,
        };
        values.push(TerminalBoundaryStorageValue {
            path: leaf.path.clone(),
            contents,
        });
    }
    Ok((values, endpoints))
}
