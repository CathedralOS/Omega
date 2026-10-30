//! Typed host implementations of the exact admitted inet address operations.
//!
//! A normalized libc locator selects an operation; neither guest requirement
//! names nor carrier declaration spellings participate. Admission additionally
//! checks the complete selected opaque application and recursive public C shape.
//! Actual C return values live in this invocation's owned occurrence store.
//! Moving an opaque guest value preserves its token; consuming an owned value
//! removes that exact stored value. Checked Copy occurrences share immutable C
//! bits, while the interpreter gives each copy its own guest home.

mod admission;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use calling_conventions::BoundaryCallingApplication;
use runtime_abi::HostInternetAddress;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralTypeId};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult, StructuralMultiplicity};

use super::TerminalRuntimeError;

#[derive(Clone, Copy)]
enum AddressOperation {
    Make,
    Host,
}

struct OwnedAddress {
    // StructuralTypeId belongs to one artifact. Each adapter authenticates its
    // own local formal above; only the exact target-closed opaque application
    // can relate this owned payload across independently compiled components.
    application: [u8; 32],
    multiplicity: StructuralMultiplicity,
    value: HostInternetAddress,
}

struct AddressContract {
    structural_type: StructuralTypeId,
    application: [u8; 32],
    multiplicity: StructuralMultiplicity,
}

/// Slots are never recycled: a stale consumed token cannot name a replacement.
/// Descending host identities are reserved by the interpreter on successful
/// import, so its ascending local allocation cannot reuse a live host identity.
#[derive(Default)]
pub(super) struct AddressValues {
    values: Vec<Option<OwnedAddress>>,
}

impl AddressValues {
    fn argument(
        &mut self,
        ordinal: usize,
        application: [u8; 32],
        multiplicity: StructuralMultiplicity,
    ) -> Result<HostInternetAddress, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "inet owned occurrence differs from the admitted application or multiplicity",
            )
        };
        let slot = self.values.get_mut(ordinal).ok_or_else(invalid)?;
        let owned = slot.as_ref().ok_or_else(invalid)?;
        if owned.application != application || owned.multiplicity != multiplicity {
            return Err(invalid());
        }
        match multiplicity {
            // Retire before invoking C, including when the host call fails.
            StructuralMultiplicity::Affine | StructuralMultiplicity::Linear => {
                Ok(slot.take().ok_or_else(invalid)?.value)
            }
            // Only a checked Copy occurrence can reach this admitted path.
            // C receives bits by value; no guest authority is minted here.
            StructuralMultiplicity::Unrestricted => Ok(owned.value),
        }
    }
}

pub(super) struct NetworkAddressImport {
    signature: TerminalImportSignature,
    operation: AddressOperation,
    contract: AddressContract,
    values: Arc<Mutex<AddressValues>>,
}

impl NetworkAddressImport {
    pub(super) fn admit(
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
        values: Arc<Mutex<AddressValues>>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(operation) = admission::select(binding)? else {
            return Ok(None);
        };
        let signature = TerminalImportSignature::from(declaration);
        let contract =
            admission::validate(declaration, binding, application, &signature, operation)?;
        Ok(Some(Self {
            signature,
            operation,
            contract,
            values,
        }))
    }
}

impl TerminalImportAdapter for NetworkAddressImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for NetworkAddressImport {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "inet operation requires an authenticated typed result",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "inet operands or owned occurrence differ from the admitted contract",
            )
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
        let word = IntegerType::new(IntegerSign::Unsigned, 32).map_err(|_| invalid())?;
        let mut values = self.values.lock().map_err(|_| {
            TerminalEffectRejection::new("inet owned occurrence store was poisoned")
        })?;
        match self.operation {
            AddressOperation::Make => {
                let [
                    TerminalScalarValue::Integer {
                        scalar_type: network_type,
                        value: IntegerValue::Unsigned(network),
                    },
                    TerminalScalarValue::Integer {
                        scalar_type: host_type,
                        value: IntegerValue::Unsigned(host),
                    },
                ] = arguments.as_slice()
                else {
                    return Err(invalid());
                };
                let BoundaryMachineResult::Structural(result) = result else {
                    return Err(invalid());
                };
                if *network_type != word
                    || *host_type != word
                    || !structural_arguments.is_empty()
                    || result.structural_type != self.contract.structural_type
                    || result.multiplicity != self.contract.multiplicity
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
                let network = u32::try_from(*network).map_err(|_| invalid())?;
                let host = u32::try_from(*host).map_err(|_| invalid())?;
                let ordinal = u64::try_from(values.values.len()).map_err(|_| invalid())?;
                let identity = u64::MAX
                    .checked_sub(ordinal)
                    .filter(|value| *value != 0)
                    .ok_or_else(invalid)?;
                // Admission checks the exact native symbol, target,
                // complete by-value C signature and selected opaque geometry.
                // These are the real function's arguments, never guest oracles.
                let value = HostInternetAddress::make(network, host).ok_or_else(|| {
                    TerminalEffectRejection::new(
                        "native inet implementation is unavailable on this host",
                    )
                })?;
                values.values.push(Some(OwnedAddress {
                    application: self.contract.application,
                    multiplicity: self.contract.multiplicity,
                    value,
                }));
                Ok(TerminalEffectResult::Structural(
                    TerminalStructuralValue::minted(
                        identity,
                        self.contract.structural_type,
                        Vec::new(),
                    ),
                ))
            }
            AddressOperation::Host => {
                let [address] = structural_arguments.as_slice() else {
                    return Err(invalid());
                };
                if !arguments.is_empty()
                    || result != &self.signature.result
                    || address.structural_type != self.contract.structural_type
                    || !address.path.is_empty()
                    || !address.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
                let ordinal =
                    usize::try_from(u64::MAX - address.opaque_identity).map_err(|_| invalid())?;
                let value = values.argument(
                    ordinal,
                    self.contract.application,
                    self.contract.multiplicity,
                )?;
                // Whole semantic memberships do not change this bare C input's
                // physical representation. Psi has already bound the actual
                // input and checked any required authority; this adapter neither
                // interprets those memberships nor issues them on a result.
                // The exact owned value was produced by inet_makeaddr
                // under this same selected opaque application. The C call takes
                // the four-byte struct by value and retains no guest reference.
                let host = value.host_number().ok_or_else(|| {
                    TerminalEffectRejection::new(
                        "native inet implementation is unavailable on this host",
                    )
                })?;
                Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                    scalar_type: word,
                    value: IntegerValue::Unsigned(host.into()),
                }))
            }
        }
    }
}
