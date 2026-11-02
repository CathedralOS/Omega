//! A shared empty record still borrows its actual formed guest home.
//! No writable bytes or opaque host token are manufactured. The paused source
//! tuple retains the original owner; the private child alias preserves address
//! identity through nested shared calls. Mutable empty homes require a writable
//! carrier and remain refused by the ordinary component argument owner.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::storage_addresses::{ScalarStorageAddress, ScalarStorageAddressKey};
use crate::structural_storage::contents;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

pub(super) struct SharedEmptyArgument {
    index: usize,
    original: ScalarStorageAddress,
}

fn empty_record(
    execution: &TerminalExecution<'_>,
    value: &TerminalStructuralValue,
) -> Result<bool, TerminalInterpretError> {
    if !matches!(execution.structural_declaration(value.structural_type)?.shape,
        StructuralTypeShape::Record { ref fields, .. } if fields.is_empty())
    {
        return Ok(false);
    }
    let described = contents::describe(execution, value, StructuralAccess::SharedBorrow, None)?;
    Ok(described.leaves.is_empty()
        && !described.requires_atomic_storage
        && !described.requires_erased_establishment)
}

impl TerminalExecution<'_> {
    pub(super) fn prepare_shared_empty_argument(
        &self,
        value: &TerminalStructuralValue,
        index: usize,
    ) -> Result<Option<SharedEmptyArgument>, TerminalInterpretError> {
        if !empty_record(self, value)? {
            return Ok(None);
        }
        let home = StructuralRuntimePlace::from(value);
        let key = ScalarStorageAddressKey::Scalar(home.clone());
        let formed = self
            .local_structural_identities
            .owns_allocated(home.opaque_identity)
            && self.structural_values.values().any(|owner| {
                owner.opaque_identity == home.opaque_identity && home.path.starts_with(&owner.path)
            });
        // A nested child may reborrow the exact private alias established by its
        // parent. Public entry annotations and equal numeric IDs cannot create it.
        if !formed && !self.component_borrowed_addresses.contains_key(&key) {
            return Ok(None);
        }
        Ok(Some(SharedEmptyArgument {
            index,
            original: self.scalar_storage_address(&key)?,
        }))
    }
}

impl SharedEmptyArgument {
    pub(super) fn establish(
        &self,
        child: &mut TerminalExecution<'_>,
    ) -> Result<(), TerminalInterpretError> {
        let entry = child.program.verified_machine(child.current_machine)?;
        let parameter = entry
            .structural_parameters
            .get(self.index)
            .ok_or_else(malformed)?;
        if parameter.access != StructuralAccess::SharedBorrow {
            return Err(malformed());
        }
        let value = child.structural_value(parameter.place)?;
        if !empty_record(child, value)? {
            return Err(malformed());
        }
        let key = ScalarStorageAddressKey::Scalar(StructuralRuntimePlace::from(value));
        if let Some(existing) = child.component_borrowed_addresses.get(&key) {
            // Both formals were checked SharedBorrow above. A cohort may
            // retain the same actual formed home; a different origin cannot
            // replace an already established receiving alias.
            return if existing == &self.original {
                Ok(())
            } else {
                Err(malformed())
            };
        }
        child
            .component_borrowed_addresses
            .insert(key, self.original.clone())?;
        Ok(())
    }
}
