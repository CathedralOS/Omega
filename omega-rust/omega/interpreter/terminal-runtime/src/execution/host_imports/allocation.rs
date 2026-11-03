//! Exact admitted calloc/free operations over invocation-owned virtual storage.
//! Addresses identify live allocations in this invocation only. Their bytes
//! are zeroed and retained until free; no host pointer escapes. This adapter
//! grants no guest domain or content account. The selected profile resolver
//! can attest the actual allocation's backing; source authority still comes
//! only from the independently admitted guest provider contract.

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

/// Addresses never recycle within an invocation, so a retired address cannot
/// accidentally free a replacement allocation. Dropping the invocation retires
/// its private storage without creating guest completion receipts. The caller's
/// host context shares this store across receiver and component instances.
#[derive(Default)]
pub(super) struct AllocationValues {
    allocations: Vec<Allocation>,
    next_address: u64,
}

impl AllocationValues {
    fn backing_for_range(&self, base: u64, length: u64) -> Option<&Allocation> {
        let end = base.checked_add(length)?;
        // An empty range at a shared endpoint belongs to an exact-base
        // allocation when one exists, not the preceding allocation's end.
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

    fn allocate(&mut self, count: u64, width: u64, issuer: u64) -> u64 {
        let Some(length) = count.checked_mul(width) else {
            return 0;
        };
        let Ok(capacity) = usize::try_from(length) else {
            return 0;
        };
        let address = self.next_address.max(16);
        let Some(next) = address
            .checked_add(length.max(1))
            .and_then(|end| end.checked_add(15))
            .map(|end| end & !15)
        else {
            return 0;
        };
        let mut bytes = Vec::new();
        if self.allocations.try_reserve(1).is_err() || bytes.try_reserve_exact(capacity).is_err() {
            return 0;
        }
        bytes.resize(capacity, 0);
        self.allocations.push(Allocation {
            address,
            length,
            issuer,
            backing: terminal_interpreter::TerminalProviderBacking::new(bytes),
        });
        self.next_address = next;
        address
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
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(operation) = admission::select(binding)? else {
            return Ok(None);
        };
        let signature = TerminalImportSignature::from(declaration);
        // Exclusive captured storage uses the native adapter's authenticated
        // endpoint result. It is not an owned virtual allocation address.
        // Owned values remain in this model and owe its complete contract.
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
                    ))),
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

#[cfg(test)]
mod tests {
    use super::AllocationValues;

    #[test]
    fn empty_ranges_choose_the_exact_allocation_not_its_neighbor() {
        let mut values = AllocationValues::default();
        let first = values.allocate(1, 16, 7);
        let second = values.allocate(1, 16, 7);
        assert_eq!(second, first + 16);
        assert_eq!(values.backing_for_range(second, 0).unwrap().address, second);
        let empty = values.allocate(0, 1, 7);
        assert_eq!(values.backing_for_range(empty, 0).unwrap().address, empty);
        assert!(values.backing_for_range(empty, 1).is_none());
        assert!(values.backing_for_range(first + 1, 32).is_none());
        assert!(values.backing_for_range(u64::MAX, 1).is_none());
    }

    #[test]
    fn calloc_retains_exact_zeroed_backing() {
        let mut values = AllocationValues::default();
        let address = values.allocate(3, 8, 7);
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
        let first = values.allocate(1, 16, 7);
        let retained = values.allocations[0].backing.clone();
        values.release(first).unwrap();
        assert!(retained.retire().is_err());
        assert!(values.release(first).is_err());
        let second = values.allocate(1, 16, 7);
        assert!(second >= first + 16);
        assert!(values.release(second).is_ok());
    }

    #[test]
    fn overflow_and_unknown_free_do_not_discard_other_allocations() {
        let mut values = AllocationValues::default();
        let address = values.allocate(1, 16, 7);
        assert_eq!(values.allocate(u64::MAX, 2, 7), 0);
        assert!(values.release(address + 1).is_err());
        assert!(values.release(0).is_ok());
        assert_eq!(values.allocations.len(), 1);
        assert!(values.release(address).is_ok());
    }
}
