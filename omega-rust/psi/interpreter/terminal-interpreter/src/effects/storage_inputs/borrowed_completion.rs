//! Validate every replacement before publishing either input or result. A
//! changed overlay is rebuilt at the original referent identity; inactive old
//! payload storage is removed, not left addressable through a stale view.

use super::contents::{self, StorageLeafKind, TerminalStorageContents};
use super::{StructuralLoanBinding, TerminalBoundaryStructuralBuffer};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::ConstructorStorage;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralRuntimePlace;
use std::sync::Arc;
use terminal_psi::StructuralAccess;

pub(super) fn stage(
    execution: &TerminalExecution,
    buffer: &TerminalBoundaryStructuralBuffer,
    binding: &mut StructuralLoanBinding,
) -> Result<(), TerminalInterpretError> {
    if !Arc::ptr_eq(&buffer.identity, &binding.identity)
        || buffer.argument_index != binding.argument_index
        || buffer.access != binding.access
        || buffer.structural_type != binding.referent.structural_type
        || (binding.access == StructuralAccess::SharedBorrow && buffer.values != binding.original)
        || buffer
            .values
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(malformed());
    }
    for scalar in &binding.scalars {
        execution.validate_scalar_loan_qualifications(&scalar.storage)?;
        if execution.read_primitive_storage(
            binding.source,
            scalar.storage.clone(),
            scalar.scalar,
            scalar.scalar,
            scalar.shared,
        )? != scalar.original
        {
            return Err(malformed());
        }
    }
    for leaf in &binding.leaves {
        if let StorageLeafKind::Case {
            destination,
            structural_type,
            case,
        } = &leaf.kind
        {
            let original = execution
                .structural_cases
                .get(destination)
                .ok_or_else(malformed)?;
            if original.structural_type != *structural_type || original.case != *case {
                return Err(malformed());
            }
        }
    }
    let leaves = contents::describe(
        execution,
        &binding.referent,
        binding.access,
        Some(&buffer.values),
    )?;
    let visible: Vec<_> = leaves
        .iter()
        .filter(|leaf| {
            matches!(
                leaf.kind,
                StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. }
            )
        })
        .collect();
    if visible.len() != buffer.values.len() {
        return Err(malformed());
    }
    let changed_case = visible.iter().any(|leaf| matches!(leaf.kind, StorageLeafKind::Case { case, .. }
        if binding.original.binary_search_by(|value| value.path.cmp(&leaf.path)).ok()
            .is_none_or(|position| binding.original[position].contents != TerminalStorageContents::Case(case))));
    let mut staged = ConstructorStorage::default();
    for (leaf, value) in visible.into_iter().zip(&buffer.values) {
        if leaf.path != value.path {
            return Err(malformed());
        }
        match (&leaf.kind, value.contents) {
            (
                StorageLeafKind::Scalar {
                    destination,
                    scalar,
                    restriction,
                    stored_scalar,
                },
                TerminalStorageContents::Scalar {
                    value,
                    stored_scalar: actual,
                },
            ) => {
                if value.scalar_type() != *scalar
                    || !terminal_scalar_belongs_to_type(value)
                    || actual != *stored_scalar
                    || !crate::primitive_storage::scalar_fits_storage(actual, value)
                    || restriction
                        .as_ref()
                        .is_some_and(|field| !value.satisfies_field(field))
                {
                    return Err(malformed());
                }
                let original = binding
                    .scalars
                    .binary_search_by(|original| original.path.cmp(&leaf.path))
                    .ok()
                    .map(|position| &binding.scalars[position]);
                if let Some(original) = original
                    && original.restriction.as_ref().is_some_and(|field| {
                        !crate::effects::scalar_loans::satisfies_source_restriction(
                            value,
                            field,
                            original.storage.encoding,
                        )
                    })
                {
                    return Err(malformed());
                }
                if changed_case {
                    staged.stage_scalar_endpoint(destination, value)?;
                } else {
                    let original = original.ok_or_else(malformed)?;
                    binding
                        .stores
                        .push((original.storage.clone(), original.scalar, value));
                }
            }
            (
                StorageLeafKind::Case {
                    destination,
                    structural_type,
                    case,
                },
                TerminalStorageContents::Case(actual),
            ) if actual == *case => {
                if changed_case {
                    staged.stage_case(destination.clone(), *structural_type, *case);
                }
            }
            _ => return Err(malformed()),
        }
    }
    if changed_case {
        // Replacing an overlay cannot silently lose contained permission or
        // resource custody. Opaque contents require identity-preserving retention
        // in both directions; native bytes cannot establish or dispose them.
        // Unrestricted byte owners are also retained because this semantic
        // buffer has no codec for their contents yet, not because retagging
        // away from an owned byte value is a language prohibition.
        for old in &binding.leaves {
            let retained = match &old.kind {
                StorageLeafKind::Opaque { declaration, .. } => leaves.iter().any(|leaf| leaf.path == old.path && matches!(&leaf.kind, StorageLeafKind::Opaque { declaration: current, .. } if current == declaration)),
                StorageLeafKind::ByteField { declaration, .. } => leaves.iter().any(|leaf| leaf.path == old.path && matches!(&leaf.kind, StorageLeafKind::ByteField { declaration: current, .. } if current == declaration)),
                _ => true,
            };
            if !retained {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "case replacement cannot yet transfer or remove opaque child storage",
                ));
            }
        }
        for leaf in &leaves {
            match &leaf.kind {
                StorageLeafKind::Opaque {
                    destination,
                    declaration,
                } => {
                    if !binding.leaves.iter().any(|old| old.path == leaf.path && matches!(&old.kind, StorageLeafKind::Opaque { declaration: original, .. } if original == declaration)) { return Err(TerminalInterpretError::UnsupportedSemanticVariant("case replacement cannot establish new opaque child storage")); }
                    staged.retain_replacement_subtree(execution, destination)?;
                }
                StorageLeafKind::ByteField {
                    parent,
                    field,
                    declaration,
                } => {
                    if !binding.leaves.iter().any(|old| old.path == leaf.path && matches!(&old.kind, StorageLeafKind::ByteField { declaration: original, .. } if original == declaration)) { return Err(TerminalInterpretError::UnsupportedSemanticVariant("case replacement lacks a semantic codec for new byte fields")); }
                    let namespace = if matches!(
                        leaf.path.iter().rev().nth(1),
                        Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(_))
                    ) {
                        crate::values::StructuralFieldNamespace::CasePayload
                    } else {
                        crate::values::StructuralFieldNamespace::Member
                    };
                    staged.retain_byte_field(execution, parent, *field, namespace)?;
                }
                StorageLeafKind::ByteArray {
                    destination,
                    length,
                } => {
                    let bytes = (0..*length)
                        .map(|element_index| {
                            let mut path = leaf.path.clone();
                            path.push(
                                semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(
                                    element_index,
                                ),
                            );
                            let position = buffer
                                .values
                                .binary_search_by(|value| value.path.cmp(&path))
                                .map_err(|_| malformed())?;
                            let value = &buffer.values[position];
                            let TerminalStorageContents::Scalar {
                                value:
                                    crate::values::TerminalScalarValue::Integer {
                                        value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                                        ..
                                    },
                                ..
                            } = value.contents
                            else {
                                return Err(malformed());
                            };
                            u8::try_from(byte).map_err(|_| malformed())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    staged.stage_byte_array(destination.clone(), bytes);
                }
                _ => {}
            }
        }
        binding.replacement = Some(staged);
    }
    Ok(())
}

pub(super) fn commit(
    execution: &mut TerminalExecution,
    binding: StructuralLoanBinding,
) -> Result<(), TerminalInterpretError> {
    if binding.access == StructuralAccess::SharedBorrow {
        return Ok(());
    }
    if let Some(replacement) = binding.replacement {
        execution.prune_subtree(&StructuralRuntimePlace::from(&binding.referent));
        replacement.publish(execution)?;
        return Ok(());
    }
    // The case roster is unchanged. Preserve scalar-array homes and narrowed
    // byte codecs by writing through the pre-call backing, not a rebuilt root.
    for (storage, scalar, value) in binding.stores {
        execution.store_primitive_storage(binding.source, storage, scalar, value)?;
    }
    Ok(())
}
