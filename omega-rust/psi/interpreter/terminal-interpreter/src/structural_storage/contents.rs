//! Semantic contents follow the active declaration, not native memory. Case
//! steps authenticate an overlay; runtime storage omits that step only after
//! choosing the nominal case. Common and payload fields keep distinct keys.

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

pub(crate) struct StorageLeaf {
    pub path: Vec<Path>,
    pub kind: StorageLeafKind,
}

#[derive(Default)]
pub(crate) struct StorageDescription {
    pub leaves: Vec<StorageLeaf>,
    /// Erased bindings have no observable cell, but fresh contents cannot
    /// establish their proof or authority merely by supplying runtime leaves.
    pub requires_erased_establishment: bool,
}

impl StorageLeaf {
    /// Construction and replacement validate the same typed scalar/tag lane.
    /// This checks contents only, never ownership, a loan or a qualification.
    pub(crate) fn validate_value(
        &self,
        value: &TerminalBoundaryStorageValue,
    ) -> Result<(), &'static str> {
        if self.path != value.path {
            return Err("contents do not name the complete active declaration paths");
        }
        match (&self.kind, value.contents) {
            (
                StorageLeafKind::Scalar {
                    scalar,
                    restriction,
                    stored_scalar,
                    ..
                },
                TerminalStorageContents::Scalar {
                    value,
                    stored_scalar: actual,
                },
            ) => {
                if value.scalar_type() != *scalar
                    || !crate::scalar_operations::terminal_scalar_belongs_to_type(value)
                {
                    return Err("scalar contents differ from the declared logical carrier");
                }
                if actual != *stored_scalar
                    || !crate::primitive_storage::scalar_fits_storage(actual, value)
                {
                    return Err("scalar contents differ from the declared stored carrier");
                }
                if restriction
                    .as_ref()
                    .is_some_and(|field| !value.satisfies_field(field))
                {
                    return Err("scalar contents do not satisfy the declaration restriction");
                }
                Ok(())
            }
            (StorageLeafKind::Case { case, .. }, TerminalStorageContents::Case(actual))
                if actual == *case =>
            {
                Ok(())
            }
            _ => Err("contents kind differs from the active scalar or case declaration"),
        }
    }
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
        structural_type: StructuralTypeId,
        length: u64,
    },
    /// One fully validated scalar-array subtree with no scalar instances.
    /// Its type/path and initialized home remain real even without byte cells.
    EmptyScalarArray {
        destination: StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    },
}

pub(crate) struct PinnedScalar {
    pub path: Vec<Path>,
    pub storage: PrimitiveStorage,
    pub scalar: ScalarType,
    pub shared: bool,
    pub original: TerminalScalarValue,
    pub restriction: Option<StructuralFieldType>,
    pub restoration_owner: StructuralRuntimePlace,
}

pub(crate) fn describe(
    execution: &TerminalExecution,
    referent: &TerminalStructuralValue,
    access: StructuralAccess,
    replacement: Option<&[TerminalBoundaryStorageValue]>,
) -> Result<StorageDescription, TerminalInterpretError> {
    let mut leaves = StorageDescription::default();
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
    let mut leaves = StorageDescription::default();
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
    Ok(leaves
        .leaves
        .into_iter()
        .find(|leaf| leaf.path == selected_path))
}

fn collect(
    execution: &TerminalExecution,
    root: StructuralTypeId,
    destination: &StructuralRuntimePlace,
    access: StructuralAccess,
    replacement: Option<&[TerminalBoundaryStorageValue]>,
    selected_path: Option<&[Path]>,
    path: &mut Vec<Path>,
    leaves: &mut StorageDescription,
) -> Result<(), TerminalInterpretError> {
    let declaration = execution.structural_declaration(root)?;
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => leaves.leaves.push(StorageLeaf {
            path: path.clone(),
            kind: StorageLeafKind::Scalar {
                destination: PrimitiveStorageEndpoint::Scalar(destination.clone()),
                scalar: *scalar,
                restriction: declaration.scalar_restriction.clone(),
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
        StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Application(_),
            ..
        } => return Err(TerminalInterpretError::UnclosedArrayExtent(root)),
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            // Validate every dimension and the terminal primitive declaration
            // before pruning instance work. An empty payload is not a shortcut
            // for opaque/reference/cyclic element schemas. Compact a selected
            // subtree only after its complete path has been consumed; selecting
            // inside it still checks every declared index on the route.
            if selected_path.is_none_or(|selected| selected == path.as_slice())
                && terminal_semantics::scalar_array_leaf_shape(
                    execution.structural_types().values(),
                    root,
                )
                .is_some_and(|(_, count)| count == 0)
            {
                leaves.leaves.push(StorageLeaf {
                    path: path.clone(),
                    kind: StorageLeafKind::EmptyScalarArray {
                        destination: destination.clone(),
                        structural_type: root,
                    },
                });
                return Ok(());
            }
            if execution.structural_types().get(element).is_some_and(|row| matches!(row.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(carrier)) if carrier.bits() == 8 && !carrier.is_address() && carrier.sign() == semantic_vocabulary::IntegerSign::Unsigned)) {
                leaves.leaves.push(StorageLeaf { path: path.clone(), kind: StorageLeafKind::ByteArray { destination: destination.clone(), structural_type: root, length: *length } });
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
                let contents = execution.structural_cases.require(destination)?;
                if contents.structural_type != root {
                    return Err(malformed());
                }
                contents.case
            };
            let selected = cases
                .iter()
                .find(|selected| selected.id == case)
                .ok_or_else(malformed)?;
            leaves.leaves.push(StorageLeaf {
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
        _ => leaves.leaves.push(StorageLeaf {
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
    leaves: &mut StorageDescription,
    payload: bool,
) -> Result<(), TerminalInterpretError> {
    for field in fields {
        if selected_path
            .is_some_and(|selected| selected.get(path.len()) != Some(&Path::Field(field.id)))
        {
            continue;
        }
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
        {
            leaves.requires_erased_establishment = true;
            continue;
        }
        if matches!(
            field.field_type,
            StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
        ) && access != StructuralAccess::MutableBorrow
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
            leaves.leaves.push(StorageLeaf {
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
            leaves.leaves.push(StorageLeaf {
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
    aggregate: Option<&crate::reference::AggregateBacking>,
    access: StructuralAccess,
    leaf: &StorageLeaf,
) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
    let StorageLeafKind::Scalar { scalar, .. } = leaf.kind else {
        return Err(malformed());
    };
    if let Some(backing) = aggregate {
        // Logical paths select bytes in the retained region. Only the core
        // projector may join those bytes to original scalar endpoints.
        if backing.placed.is_some() || array_loan.is_some() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host aggregate storage has no resident codec for placed cells",
            ));
        }
        let path = terminal_semantics::canonical_structural_storage_path(
            execution.structural_types().values(),
            referent.structural_type,
            &leaf.path,
        )
        .ok_or_else(malformed)?;
        if let Some(region) = backing.region {
            execution.aggregate_view_primitive_access(
                &backing.value,
                crate::reference::AggregateViewTip::Aggregate {
                    view: region,
                    backing: backing.value.structural_type,
                },
                &path,
                backing.collection_home.as_ref(),
                matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()),
            )
        } else {
            let region = backing.byte_region.as_ref().ok_or_else(malformed)?;
            let (storage, scalar) =
                execution.captured_byte_region_access(&backing.value, region, &path)?;
            Ok((storage, scalar, access == StructuralAccess::SharedBorrow))
        }
    } else if let Some(loan) = array_loan {
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
    aggregate: Option<&crate::reference::AggregateBacking>,
    access: StructuralAccess,
    leaves: &[StorageLeaf],
    normal_qualifications: &[crate::effects::normal_qualifications::CapturedNormalQualification],
) -> Result<(Vec<TerminalBoundaryStorageValue>, Vec<PinnedScalar>), TerminalInterpretError> {
    let mut values = Vec::new();
    let mut endpoints = Vec::new();
    for leaf in leaves {
        if aggregate.is_some()
            && !matches!(
                leaf.kind,
                StorageLeafKind::Scalar { .. } | StorageLeafKind::ByteArray { .. }
            )
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host aggregate re-view requires independently represented scalar leaves",
            ));
        }
        let contents = match &leaf.kind {
            StorageLeafKind::Case { case, .. } => TerminalStorageContents::Case(*case),
            StorageLeafKind::Scalar {
                scalar,
                restriction,
                stored_scalar,
                ..
            } => {
                let (storage, logical, shared) =
                    scalar_storage(execution, referent, array_loan, aggregate, access, leaf)
                        .map_err(|error| {
                            error.in_argument_preparation("project captured host scalar")
                        })?;
                if logical != *scalar {
                    return Err(malformed());
                }
                // A re-view's authenticated projection can use a different
                // descriptor namespace from its physical cells. Preserve the
                // actual source owner's memberships, not that presentation's
                // incidental path. The captured endpoint/region still bounds
                // access; this ancestor scope grants no additional cells.
                let restoration_owner = aggregate
                    .and_then(|_| {
                        crate::effects::scalar_loans::scalar_qualification_owner(&storage)
                    })
                    .cloned()
                    .unwrap_or_else(|| StructuralRuntimePlace::from(referent));
                // Owned payload snapshots publish no replacement or loan. Their
                // qualifications remain on the consumed argument; a read cannot
                // weaken that owner or manufacture another qualified value.
                if access == StructuralAccess::MutableBorrow {
                    execution
                        .validate_scalar_loan_restoration(
                            &storage,
                            &restoration_owner,
                            normal_qualifications,
                        )
                        .map_err(|error| {
                            error.in_argument_preparation("preserve captured scalar source owner")
                        })?;
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
                    restoration_owner,
                });
                TerminalStorageContents::Scalar {
                    value,
                    stored_scalar: storage.storage_scalar().unwrap_or(*stored_scalar),
                }
            }
            StorageLeafKind::EmptyScalarArray {
                destination,
                structural_type,
            } => {
                let initialized = if let Some(loan) = array_loan {
                    let selected = terminal_semantics::canonical_structural_storage_path(
                        execution.structural_types().values(),
                        referent.structural_type,
                        &leaf.path,
                    )
                    .ok_or_else(malformed)?;
                    loan.initialized_empty_scalar_array(execution, &selected, *structural_type)?
                } else {
                    execution.initialized_empty_scalar_array(destination, *structural_type)?
                };
                if !initialized {
                    return Err(malformed());
                }
                // This buffer contains scalar/case values only. The typed
                // initialization receipt was checked above; it is not a scalar
                // value, native byte, reference or fabricated replacement.
                continue;
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
