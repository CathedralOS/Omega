/*
We execute admitted `calloc`, `free`, and `memset` operations
(`AllocationImport`, `AllocationValues`) over invocation-owned virtual storage.

Addresses identify live allocations within this invocation only and never
recycle, so a retired address cannot accidentally free a replacement allocation.
Their bytes are zeroed and retained until `free`, and no host pointer escapes.
When resolving a range, an empty range at a shared endpoint belongs to an
exact-base allocation when one exists rather than the preceding allocation's end.

The same ledger records the opaque host addresses that native imports return
(`admit_host_address`), so the two kinds never share a value. A host address
may be stored, copied, compared and passed back to a native import, but
never resolved as interpreter storage: one inside the emulated span is
refused, and no emulated allocation is placed over a recorded host address.
*/

mod admission;
mod backing;
pub(super) use backing::AllocationStorage;

use super::TerminalRuntimeError;
use calling_conventions::BoundaryCallingApplication;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use std::sync::{Arc, Mutex};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::BoundaryMachineDeclaration;

#[derive(Clone, Copy)]
enum AllocationOperation {
    Allocate,
    Release,
    Initialize,
}

struct Allocation {
    address: u64,
    length: u64,
    issuer: u64,
    backing: terminal_interpreter::TerminalProviderBacking,
}

#[derive(Default)]
pub(super) struct AllocationValues {
    allocations: Vec<Allocation>,
    next_address: u64,
    host_addresses: Vec<u64>,
}

impl AllocationValues {
    fn backing_for_range(&self, base: u64, length: u64) -> Option<&Allocation> {
        let end = base.checked_add(length)?;
        self.allocations
            .iter()
            .find(|allocation| allocation.address == base && length <= allocation.length)
            .or_else(|| {
                self.allocations.iter().find(|allocation| {
                    base >= allocation.address
                        && base < allocation.address + allocation.length
                        && end <= allocation.address + allocation.length
                })
            })
    }

    fn allocate(
        &mut self,
        count: u64,
        width: u64,
        issuer: u64,
    ) -> Result<u64, TerminalEffectRejection> {
        let Some(length) = count.checked_mul(width) else {
            return Ok(0);
        };
        let Ok(capacity) = usize::try_from(length) else {
            return Ok(0);
        };
        let address = self.next_address.max(16);
        let Some(next) = address
            .checked_add(length.max(1))
            .and_then(|end| end.checked_add(15))
            .map(|end| end & !15)
        else {
            return Ok(0);
        };
        if self
            .host_addresses
            .iter()
            .any(|host| (address..next).contains(host))
        {
            return Err(TerminalEffectRejection::new(
                "emulated allocation would cover an opaque host address",
            ));
        }
        let mut bytes = Vec::new();
        if self.allocations.try_reserve(1).is_err() || bytes.try_reserve_exact(capacity).is_err() {
            return Ok(0);
        }
        bytes.resize(capacity, 0);
        self.allocations.push(Allocation {
            address,
            length,
            issuer,
            backing: terminal_interpreter::TerminalProviderBacking::new(bytes),
        });
        self.next_address = next;
        Ok(address)
    }

    // Null names no storage, so it carries no provenance either way.
    pub(super) fn admit_host_address(
        &mut self,
        address: u64,
    ) -> Result<(), TerminalEffectRejection> {
        if address == 0 || self.host_addresses.contains(&address) {
            return Ok(());
        }
        if (16..self.next_address).contains(&address) {
            return Err(TerminalEffectRejection::new(
                "native import returned an address inside the emulated allocation span",
            ));
        }
        self.host_addresses
            .try_reserve(1)
            .map_err(|_| TerminalEffectRejection::new("host address ledger is exhausted"))?;
        self.host_addresses.push(address);
        Ok(())
    }

    pub(super) fn is_host_address(&self, address: u64) -> bool {
        address == 0 || self.host_addresses.contains(&address)
    }

    fn release(&mut self, address: u64) -> Result<(), TerminalEffectRejection> {
        if address == 0 {
            return Ok(());
        }
        let allocation = self
            .allocations
            .iter_mut()
            .find(|value| value.address == address)
            .ok_or_else(|| {
                TerminalEffectRejection::new(
                    "free address belongs to no allocation in this instance",
                )
            })?;
        allocation.backing.retire()?;
        Ok(())
    }
}

pub(super) struct AllocationImport {
    signature: TerminalImportSignature,
    operation: AllocationOperation,
    location: admission::AddressLocation,
    values: Arc<Mutex<AllocationValues>>,
}

impl AllocationImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
        values: Arc<Mutex<AllocationValues>>,
        emulates_allocation: bool,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(operation) = admission::select(binding)? else {
            return Ok(None);
        };
        // Without an emulated `calloc`, every address `free` or `memset`
        // receives came from the host, so the native import must take it.
        if !emulates_allocation && !matches!(operation, AllocationOperation::Allocate) {
            return Ok(None);
        }
        let signature = TerminalImportSignature::from(declaration);
        if matches!(operation, AllocationOperation::Initialize)
            && declaration.parameter_order.first()
                == Some(&terminal_psi::BoundaryParameterKind::Structural)
            && signature
                .structural_parameters
                .first()
                .is_some_and(|parameter| {
                    parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                })
        {
            return Ok(None);
        }
        let location = admission::validate(
            program,
            declaration,
            binding,
            application,
            &signature,
            operation,
        )?;
        Ok(Some(Self {
            signature,
            operation,
            location,
            values,
        }))
    }
}

impl TerminalImportAdapter for AllocationImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for AllocationImport {
    fn storage_requirements(
        &self,
        _: semantic_vocabulary::BoundaryMachineId,
        _arguments: &[terminal_interpreter::TerminalScalarValue],
    ) -> terminal_interpreter::TerminalCallStorageRequirements<'_> {
        terminal_interpreter::TerminalCallStorageRequirements {
            original: false,
            arguments: if matches!(self.location, admission::AddressLocation::OwnedArgument(_)) {
                &[0]
            } else {
                &[]
            },
        }
    }

    handle_effect_requires_dispatch!("allocation operation requires exact typed dispatch");

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = super::refusal("allocation operands differ from the admitted signature");
        let call = super::call_operands(effect, invalid)?;
        if *call.result != self.signature.result || !byte_buffers.is_empty() {
            return Err(invalid());
        }
        let mut values = self
            .values
            .lock()
            .map_err(|_| TerminalEffectRejection::new("allocation lifecycle store was poisoned"))?;
        let address_type = IntegerType::address(64).map_err(|_| invalid())?;
        match self.operation {
            AllocationOperation::Allocate => {
                let unsigned =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
                let [
                    TerminalScalarValue::Integer {
                        scalar_type: first,
                        value: IntegerValue::Unsigned(count),
                    },
                    TerminalScalarValue::Integer {
                        scalar_type: second,
                        value: IntegerValue::Unsigned(width),
                    },
                ] = call.arguments
                else {
                    return Err(invalid());
                };
                if *first != unsigned
                    || *second != unsigned
                    || !call.structural_arguments.is_empty()
                    || !storage.is_empty()
                {
                    return Err(invalid());
                }
                let count = u64::try_from(*count).map_err(|_| invalid())?;
                let width = u64::try_from(*width).map_err(|_| invalid())?;
                let minted = TerminalScalarValue::Integer {
                    scalar_type: address_type,
                    value: IntegerValue::Unsigned(u128::from(values.allocate(
                        count,
                        width,
                        call.boundary.get(),
                    )?)),
                };
                Ok(match &self.location {
                    admission::AddressLocation::RecordResult(structural_type, path) => {
                        TerminalEffectResult::ScalarAggregate(
                            terminal_interpreter::TerminalScalarAggregateValue {
                                structural_type: *structural_type,
                                result_case: None,
                                fields: vec![(path.clone(), minted)],
                            },
                        )
                    }
                    _ => TerminalEffectResult::Scalar(minted),
                })
            }
            AllocationOperation::Release => {
                let pointer =
                    if let admission::AddressLocation::OwnedArgument(path) = &self.location {
                        let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                            return Err(invalid());
                        };
                        let [argument] = call.structural_arguments else {
                            return Err(invalid());
                        };
                        let [cell] = buffer.values() else {
                            return Err(invalid());
                        };
                        if !call.arguments.is_empty()
                            || buffer.argument_index() != 0
                            || buffer.access() != terminal_psi::StructuralAccess::Owned
                            || buffer.structural_type() != argument.structural_type
                            || cell.path != *path
                        {
                            return Err(invalid());
                        }
                        let TerminalStorageContents::Scalar {
                            value,
                            stored_scalar,
                        } = cell.contents
                        else {
                            return Err(invalid());
                        };
                        if stored_scalar != semantic_vocabulary::ScalarType::Integer(address_type) {
                            return Err(invalid());
                        }
                        value
                    } else {
                        let [pointer] = call.arguments else {
                            return Err(invalid());
                        };
                        if !call.structural_arguments.is_empty() || !storage.is_empty() {
                            return Err(invalid());
                        }
                        *pointer
                    };
                let TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(address),
                } = pointer
                else {
                    return Err(invalid());
                };
                if scalar_type != address_type {
                    return Err(invalid());
                }
                values.release(u64::try_from(address).map_err(|_| invalid())?)?;
                Ok(TerminalEffectResult::Unit)
            }
            AllocationOperation::Initialize => {
                let [
                    TerminalScalarValue::Integer {
                        scalar_type: fill_type,
                        value: IntegerValue::Signed(fill),
                    },
                    TerminalScalarValue::Integer {
                        scalar_type: length_type,
                        value: IntegerValue::Unsigned(length),
                    },
                ] = call.arguments
                else {
                    return Err(invalid());
                };
                let signed = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
                let unsigned =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
                if *fill_type != signed || *length_type != unsigned {
                    return Err(invalid());
                }
                let length = u64::try_from(*length).map_err(|_| invalid())?;
                let admission::AddressLocation::OwnedArgument(path) = &self.location else {
                    return Err(invalid());
                };
                let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                    return Err(invalid());
                };
                let [argument] = call.structural_arguments else {
                    return Err(invalid());
                };
                let [cell] = buffer.values() else {
                    return Err(invalid());
                };
                if buffer.argument_index() != 0
                    || buffer.access() != terminal_psi::StructuralAccess::Owned
                    || buffer.structural_type() != argument.structural_type
                    || cell.path != *path
                {
                    return Err(invalid());
                }
                let TerminalStorageContents::Scalar {
                    value,
                    stored_scalar,
                } = cell.contents
                else {
                    return Err(invalid());
                };
                if stored_scalar != semantic_vocabulary::ScalarType::Integer(address_type) {
                    return Err(invalid());
                }
                let TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(address),
                } = value
                else {
                    return Err(invalid());
                };
                if scalar_type != address_type || *fill != 0 {
                    return Err(invalid());
                }
                let address = u64::try_from(address).map_err(|_| invalid())?;
                if values.backing_for_range(address, length).is_none() {
                    return Err(invalid());
                }
                Ok(TerminalEffectResult::Reference(
                    terminal_interpreter::TerminalReferenceResult {
                        referent: terminal_interpreter::TerminalReferencePayload::Zeroed,
                        references: Vec::new(),
                    },
                ))
            }
        }
    }
}

pub(super) fn emulates_allocation(
    binding: &NormalizedForeignCallBinding,
) -> Result<bool, TerminalRuntimeError> {
    Ok(matches!(
        admission::select(binding)?,
        Some(AllocationOperation::Allocate)
    ))
}

#[cfg(test)]
mod tests {
    use super::AllocationValues;

    #[test]
    fn empty_ranges_choose_the_exact_allocation_not_its_neighbor() {
        let mut values = AllocationValues::default();
        let first = values.allocate(1, 16, 7).unwrap();
        let second = values.allocate(1, 16, 7).unwrap();
        assert_eq!(second, first + 16);
        assert_eq!(values.backing_for_range(second, 0).unwrap().address, second);
        let empty = values.allocate(0, 1, 7).unwrap();
        assert_eq!(values.backing_for_range(empty, 0).unwrap().address, empty);
        assert!(values.backing_for_range(empty, 1).is_none());
        assert!(values.backing_for_range(first + 1, 32).is_none());
        assert!(values.backing_for_range(u64::MAX, 1).is_none());
    }

    #[test]
    fn calloc_retains_exact_zeroed_backing() {
        let mut values = AllocationValues::default();
        let address = values.allocate(3, 8, 7).unwrap();
        assert_ne!(address, 0);
        assert_eq!(address % 16, 0);
        assert_eq!(values.allocations[0].length, 24);
        assert_eq!(values.allocations[0].issuer, 7);
        assert_eq!(values.allocations[0].backing.retire().unwrap(), vec![0; 24]);
        assert!(values.release(address).is_err());
    }

    #[test]
    fn free_invalidates_retained_handles_without_recycling_addresses() {
        let mut values = AllocationValues::default();
        let first = values.allocate(1, 16, 7).unwrap();
        let retained = values.allocations[0].backing.clone();
        values.release(first).unwrap();
        assert!(retained.retire().is_err());
        assert!(values.release(first).is_err());
        let second = values.allocate(1, 16, 7).unwrap();
        assert!(second >= first + 16);
        assert!(values.release(second).is_ok());
    }

    #[test]
    fn host_and_emulated_addresses_never_share_a_value() {
        let mut values = AllocationValues::default();
        let emulated = values.allocate(1, 16, 7).unwrap();
        assert!(values.admit_host_address(emulated).is_err());
        assert!(values.admit_host_address(emulated + 8).is_err());
        assert!(!values.is_host_address(emulated));
        let host = emulated + 64;
        values.admit_host_address(host).unwrap();
        assert!(values.is_host_address(host));
        assert!(values.allocate(4, 16, 7).is_err());
        assert!(values.release(host).is_err());
    }

    #[test]
    fn overflow_and_unknown_free_do_not_discard_other_allocations() {
        let mut values = AllocationValues::default();
        let address = values.allocate(1, 16, 7).unwrap();
        assert_eq!(values.allocate(u64::MAX, 2, 7).unwrap(), 0);
        assert!(values.release(address + 1).is_err());
        assert!(values.release(0).is_ok());
        assert_eq!(values.allocations.len(), 1);
        assert!(values.release(address).is_ok());
    }
}
