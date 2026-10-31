//! Complete initialized scalar/case data uses the existing active-declaration
//! walker and constructor transaction. The supplied argument already owns its
//! identity and custody; contents cannot mint either or cross a reference.

use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::record::storage::ConstructorStorage;
use crate::structural_storage::contents::{self, StorageLeafKind};
use crate::{TerminalBoundaryStorageValue, TerminalStorageContents};

/// Complete scalar/tag leaves below one structural argument. Paths are strict
/// canonical order, with a Case selector at every active sum and exact Case
/// steps on payload fields. Inactive, extra, duplicate or missing leaves refuse.
/// Borrowed byte/reference descriptors, opaque backing and erased custody are
/// not synthesized. Fixed byte arrays use their ordinary compact backing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralContents {
    pub argument_index: u32,
    pub values: Vec<TerminalBoundaryStorageValue>,
}

impl TerminalExecution<'_> {
    pub(crate) fn bind_structural_contents(
        &mut self,
        arguments: &[TerminalStructuralContents],
    ) -> Result<(), TerminalInterpretError> {
        if arguments.is_empty() {
            return Ok(());
        }
        let mut staged = ConstructorStorage::new(self.account.as_ref());
        for argument in arguments {
            let invalid = |reason| TerminalInterpretError::StructuralContentsArgumentInvalid {
                argument_index: argument.argument_index,
                reason,
            };
            if argument
                .values
                .windows(2)
                .any(|pair| pair[0].path >= pair[1].path)
            {
                return Err(invalid(
                    "contents must have unique paths in canonical order",
                ));
            }
            let parameter = self
                .program
                .machine(self.current_machine)?
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(|| invalid("contents name no structural argument"))?;
            let root = self.structural_value(parameter.place)?.clone();
            let description = contents::describe(
                self,
                &root,
                terminal_psi::StructuralAccess::MutableBorrow,
                Some(&argument.values),
            )
            .map_err(|error| error.in_argument_preparation("describe complete entry contents"))?;
            if description.requires_erased_establishment {
                return Err(invalid(
                    "scalar/case contents cannot establish erased bindings",
                ));
            }
            let leaves = description.leaves;
            if leaves.iter().any(|leaf| {
                matches!(
                    leaf.kind,
                    StorageLeafKind::Opaque { .. } | StorageLeafKind::ByteField { .. }
                )
            }) {
                return Err(invalid(
                    "scalar/case contents cannot establish opaque or reference backing",
                ));
            }
            let mut visible = leaves
                .iter()
                .filter(|leaf| {
                    matches!(
                        leaf.kind,
                        StorageLeafKind::Scalar { .. } | StorageLeafKind::Case { .. }
                    )
                })
                .collect::<Vec<_>>();
            let byte_arrays = leaves
                .iter()
                .filter(|leaf| matches!(leaf.kind, StorageLeafKind::ByteArray { .. }))
                .collect::<Vec<_>>();
            visible.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            if visible.len() != argument.values.len() {
                return Err(invalid(
                    "contents must supply every active scalar and case exactly once",
                ));
            }
            for (leaf, value) in visible.into_iter().zip(&argument.values) {
                leaf.validate_value(value).map_err(invalid)?;
                match (&leaf.kind, value.contents) {
                    (
                        StorageLeafKind::Scalar { destination, .. },
                        TerminalStorageContents::Scalar { value, .. },
                    ) => {
                        // A byte array has one compact backing. Its individual
                        // values are still validated, but never staged twice.
                        if !byte_arrays
                            .iter()
                            .any(|array| leaf.path.starts_with(&array.path))
                        {
                            staged.stage_scalar_endpoint(destination, value)?;
                        }
                    }
                    (
                        StorageLeafKind::Case {
                            destination,
                            structural_type,
                            case,
                        },
                        TerminalStorageContents::Case(_),
                    ) => {
                        staged.stage_case(destination.clone(), *structural_type, *case)?;
                    }
                    _ => return Err(invalid("contents do not select a scalar or case")),
                }
            }
            for leaf in &leaves {
                match &leaf.kind {
                    StorageLeafKind::ByteArray {
                        destination,
                        structural_type,
                        ..
                    } => {
                        let values = argument
                            .values
                            .iter()
                            .skip(
                                argument
                                    .values
                                    .partition_point(|value| value.path < leaf.path),
                            )
                            .take_while(|value| value.path.starts_with(&leaf.path))
                            .map(|value| match value.contents {
                                TerminalStorageContents::Scalar { value, .. } => Ok(value),
                                _ => Err(invalid("byte array element is not a scalar")),
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        staged.stage_array(self, *structural_type, &values, destination)?;
                    }
                    StorageLeafKind::EmptyScalarArray {
                        destination,
                        structural_type,
                    } => {
                        staged.stage_array(self, *structural_type, &[], destination)?;
                    }
                    _ => {}
                }
            }
        }
        staged.publish(self)
    }
}
