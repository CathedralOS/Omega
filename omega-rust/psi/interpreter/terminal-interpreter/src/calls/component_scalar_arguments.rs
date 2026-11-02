//! Bind component parameters to the lender's actual scalar endpoints.
//! Paths cross artifact namespaces through declarations; cells retain their
//! original backing and call-scoped access. No result-copyback path exists.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::PrimitiveStorageEndpoint;
use crate::program::TerminalProgram;
use crate::scalar_storage::{ScalarCell, ScalarLoanScope};
use crate::storage_addresses::{ScalarStorageAddress, ScalarStorageAddressKey};
use crate::structural_storage::contents::{self, StorageLeafKind};
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, StructuralFieldType};

pub(super) struct PreparedScalarArgument {
    index: usize,
    root: StructuralRuntimePlace,
    access: StructuralAccess,
    cells: Vec<PreparedScalarCell>,
}

struct PreparedScalarCell {
    source: ScalarStorageAddressKey,
    path: Vec<CanonicalStructuralPathSegment>,
    scalar: ScalarType,
    stored_scalar: ScalarType,
    restriction: Option<StructuralFieldType>,
    address: ScalarStorageAddress,
    parent_addresses: Vec<ScalarStorageAddress>,
}

pub(super) struct LentScalarArgument {
    index: usize,
    access: StructuralAccess,
    cells: Vec<(PreparedScalarCell, ScalarCell)>,
}

fn reject() -> TerminalInterpretError {
    TerminalInterpretError::UnsupportedSemanticVariant(
        "component borrowed input needs its original materialized storage carrier",
    )
}

fn scalar_key(
    endpoint: &PrimitiveStorageEndpoint,
) -> Result<ScalarStorageAddressKey, TerminalInterpretError> {
    match endpoint {
        PrimitiveStorageEndpoint::Scalar(home) => Ok(ScalarStorageAddressKey::Scalar(home.clone())),
        PrimitiveStorageEndpoint::ScalarField(field) => {
            Ok(ScalarStorageAddressKey::Field(field.clone()))
        }
        // Packed arrays, descriptor projections and provider memory retain
        // their own physical owners; an ordinary scalar row cannot replace them.
        _ => Err(reject()),
    }
}

fn parent(key: &ScalarStorageAddressKey) -> &StructuralRuntimePlace {
    match key {
        ScalarStorageAddressKey::Scalar(home) => home,
        ScalarStorageAddressKey::Field(field) => &field.parent,
    }
}

fn cell<'source>(
    source: &'source TerminalExecution<'_>,
    key: &ScalarStorageAddressKey,
) -> Option<&'source ScalarCell> {
    match key {
        ScalarStorageAddressKey::Scalar(home) => source.structural_primitive_storage.get(home),
        ScalarStorageAddressKey::Field(field) => source.structural_scalar_fields.get(field),
    }
}

fn cell_mut<'source>(
    source: &'source mut TerminalExecution<'_>,
    key: &ScalarStorageAddressKey,
) -> Option<&'source mut ScalarCell> {
    match key {
        ScalarStorageAddressKey::Scalar(home) => source.structural_primitive_storage.get_mut(home),
        ScalarStorageAddressKey::Field(field) => source.structural_scalar_fields.get_mut(field),
    }
}

impl TerminalExecution<'_> {
    pub(super) fn prepare_component_scalar_argument(
        &self,
        value: &TerminalStructuralValue,
        index: usize,
        access: StructuralAccess,
        provider: &TerminalProgram<'_>,
        destination_type: StructuralTypeId,
    ) -> Result<PreparedScalarArgument, TerminalInterpretError> {
        let description = contents::describe(self, value, access, None)?;
        if description.requires_erased_establishment
            || description.requires_atomic_storage
            || description.leaves.is_empty()
        {
            return Err(reject());
        }
        let root = StructuralRuntimePlace::from(value);
        let mut cells = Vec::with_capacity(description.leaves.len());
        for leaf in description.leaves {
            let StorageLeafKind::Scalar {
                destination,
                scalar,
                restriction,
                stored_scalar,
            } = leaf.kind
            else {
                return Err(reject());
            };
            let source = scalar_key(&destination)?;
            let actual = cell(self, &source).ok_or_else(reject)?;
            if actual.scalar_type() != stored_scalar {
                return Err(malformed());
            }
            let source_parent = parent(&source);
            if source_parent.opaque_identity != root.opaque_identity
                || !source_parent.path.starts_with(&root.path)
            {
                return Err(reject());
            }
            let parent_addresses = (root.path.len()..=source_parent.path.len())
                .map(|length| {
                    self.scalar_storage_address(&ScalarStorageAddressKey::Scalar(
                        StructuralRuntimePlace {
                            opaque_identity: root.opaque_identity,
                            path: source_parent.path[..length].to_vec(),
                        },
                    ))
                })
                .collect::<Result<_, _>>()?;
            cells.push(PreparedScalarCell {
                path: self.component_storage_path(
                    value.structural_type,
                    destination_type,
                    provider,
                    &leaf.path,
                )?,
                scalar,
                stored_scalar,
                restriction,
                address: self.scalar_storage_address(&source)?,
                source,
                parent_addresses,
            });
        }
        Ok(PreparedScalarArgument {
            index,
            root,
            access,
            cells,
        })
    }
}

impl PreparedScalarArgument {
    pub(super) fn mutated_home(&self) -> Option<&StructuralRuntimePlace> {
        (self.access == StructuralAccess::MutableBorrow).then_some(&self.root)
    }

    pub(super) fn lend(
        self,
        source: &mut TerminalExecution<'_>,
        lifetime: &ScalarLoanScope,
    ) -> Result<LentScalarArgument, TerminalInterpretError> {
        let mut cells = Vec::with_capacity(self.cells.len());
        for prepared in self.cells {
            let original = cell_mut(source, &prepared.source).ok_or_else(malformed)?;
            let borrowed = lifetime.lend(original, self.access)?;
            cells.push((prepared, borrowed));
        }
        Ok(LentScalarArgument {
            index: self.index,
            access: self.access,
            cells,
        })
    }
}

impl LentScalarArgument {
    pub(super) fn establish(
        &self,
        child: &mut TerminalExecution<'_>,
    ) -> Result<(), TerminalInterpretError> {
        let entry = child.program.verified_machine(child.current_machine)?;
        let parameter = entry
            .structural_parameters
            .get(self.index)
            .ok_or_else(malformed)?;
        if parameter.access != self.access {
            return Err(malformed());
        }
        let value = child.structural_value(parameter.place)?.clone();
        let root = StructuralRuntimePlace::from(&value);
        let description = contents::describe(child, &value, self.access, None)?;
        if description.requires_erased_establishment
            || description.requires_atomic_storage
            || description.leaves.len() != self.cells.len()
        {
            return Err(reject());
        }
        let mut destinations = BTreeMap::new();
        for leaf in description.leaves {
            if destinations.insert(leaf.path, leaf.kind).is_some() {
                return Err(malformed());
            }
        }
        let mut bindings = Vec::with_capacity(self.cells.len());
        let mut addresses = BTreeMap::new();
        for (prepared, original) in &self.cells {
            let Some(StorageLeafKind::Scalar {
                destination,
                scalar,
                restriction,
                stored_scalar,
            }) = destinations.remove(&prepared.path)
            else {
                return Err(reject());
            };
            if scalar != prepared.scalar
                || stored_scalar != prepared.stored_scalar
                || restriction != prepared.restriction
            {
                return Err(reject());
            }
            let key = scalar_key(&destination)?;
            if cell(child, &key).is_some() {
                return Err(malformed());
            }
            let destination_parent = parent(&key);
            if destination_parent.opaque_identity != root.opaque_identity
                || !destination_parent.path.starts_with(&root.path)
                || destination_parent.path.len() - root.path.len() + 1
                    != prepared.parent_addresses.len()
            {
                return Err(malformed());
            }
            for (suffix, original_address) in prepared.parent_addresses.iter().enumerate() {
                let home = StructuralRuntimePlace {
                    opaque_identity: root.opaque_identity,
                    path: destination_parent.path[..root.path.len() + suffix].to_vec(),
                };
                insert_address(
                    &mut addresses,
                    ScalarStorageAddressKey::Scalar(home),
                    original_address,
                )?;
            }
            insert_address(&mut addresses, key.clone(), &prepared.address)?;
            bindings.push((key, original.clone()));
        }
        if !destinations.is_empty() {
            return Err(malformed());
        }
        for key in addresses.keys() {
            if child.component_scalar_addresses.contains_key(key) {
                return Err(malformed());
            }
        }
        // These are aliases of the existing endpoints. Fund receiving rows
        // before any child operation; failed preparation drops its private views.
        for (key, original) in bindings {
            match key {
                ScalarStorageAddressKey::Scalar(home) => {
                    child
                        .structural_primitive_storage
                        .insert_cell(home, original)?;
                }
                ScalarStorageAddressKey::Field(field) => {
                    child
                        .structural_scalar_fields
                        .insert_cell(field, original)?;
                }
            }
        }
        for (key, address) in addresses {
            child.component_scalar_addresses.insert(key, address)?;
        }
        Ok(())
    }
}

fn insert_address(
    addresses: &mut BTreeMap<ScalarStorageAddressKey, ScalarStorageAddress>,
    key: ScalarStorageAddressKey,
    original: &ScalarStorageAddress,
) -> Result<(), TerminalInterpretError> {
    if addresses
        .insert(key, original.clone())
        .is_some_and(|previous| previous != *original)
    {
        return Err(malformed());
    }
    Ok(())
}
