//! Exact admitted calloc/free operations over invocation-owned virtual storage.
//! Addresses identify live allocations in this invocation only. Their bytes
//! are zeroed and retained until free; no host pointer escapes. This adapter
//! grants no domain, content account, backing receipt or issuance authority.

mod admission;

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
}

struct Allocation {
    address: u64,
    bytes: Option<Vec<u8>>,
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
    fn allocate(&mut self, count: u64, width: u64) -> u64 {
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
            bytes: Some(bytes),
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
        allocation
            .bytes
            .take()
            .ok_or_else(|| TerminalEffectRejection::new("free address was already retired"))?;
        Ok(())
    }
}

pub(super) struct AllocationImport {
    signature: TerminalImportSignature,
    operation: AllocationOperation,
    payload: Option<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>>,
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
        let payload = admission::validate(
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
            payload,
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
    fn storage_arguments(&self, _: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        if self.payload.is_some() { &[0] } else { &[] }
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "allocation operation requires exact typed dispatch",
        ))
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new("allocation operands differ from the admitted signature")
        };
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            result,
            ..
        } = effect
        else {
            return Err(invalid());
        };
        if *result != self.signature.result || !byte_buffers.is_empty() {
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
                ] = arguments.as_slice()
                else {
                    return Err(invalid());
                };
                if *first != unsigned
                    || *second != unsigned
                    || !structural_arguments.is_empty()
                    || !storage.is_empty()
                {
                    return Err(invalid());
                }
                let count = u64::try_from(*count).map_err(|_| invalid())?;
                let width = u64::try_from(*width).map_err(|_| invalid())?;
                Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                    scalar_type: address_type,
                    value: IntegerValue::Unsigned(u128::from(values.allocate(count, width))),
                }))
            }
            AllocationOperation::Release => {
                let pointer = if let Some(path) = &self.payload {
                    let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                        return Err(invalid());
                    };
                    let [argument] = structural_arguments.as_slice() else {
                        return Err(invalid());
                    };
                    let [cell] = buffer.values() else {
                        return Err(invalid());
                    };
                    if !arguments.is_empty()
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
                    let [pointer] = arguments.as_slice() else {
                        return Err(invalid());
                    };
                    if !structural_arguments.is_empty() || !storage.is_empty() {
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
        }
    }
}
