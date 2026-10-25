//! Typed host implementations of the exact admitted inet address operations.
//!
//! A normalized libc locator selects an operation; neither guest requirement
//! names nor carrier declaration spellings participate. Admission additionally
//! checks the complete selected opaque application and recursive public C shape.
//! Actual C return values live in this invocation's owned occurrence store.
//! Moving an affine guest value preserves its token; consuming that one
//! occurrence removes the stored value. A checked semantic copy of an
//! unrestricted inert carrier is another occurrence of the same stored value,
//! so reading one does not retire the other. A physical copy that is not a
//! checked semantic copy does not mint a token.

mod admission;

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

#[derive(Clone)]
struct OwnedAddress {
    // StructuralTypeId belongs to one artifact. Each adapter authenticates its
    // own local formal above; only the exact target-closed opaque application
    // can relate this owned payload across independently compiled components.
    application: [u8; 32],
    value: HostInternetAddress,
}

/// Slots are never recycled: a stale consumed token cannot name a replacement.
/// Descending host identities are reserved by the interpreter on successful
/// import, so its ascending local allocation cannot reuse a live host identity.
#[derive(Default)]
pub(super) struct AddressValues {
    values: Vec<Option<OwnedAddress>>,
}

pub(super) struct NetworkAddressImport {
    signature: TerminalImportSignature,
    operation: AddressOperation,
    application: [u8; 32],
    structural_type: StructuralTypeId,
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
        let (structural_type, application) =
            admission::validate(declaration, binding, application, &signature, operation)?;
        Ok(Some(Self {
            signature,
            operation,
            application,
            structural_type,
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
                    || result.structural_type != self.structural_type
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
                    application: self.application,
                    value,
                }));
                Ok(TerminalEffectResult::Structural(
                    TerminalStructuralValue::minted(identity, self.structural_type, Vec::new()),
                ))
            }
            AddressOperation::Host => {
                let [address] = structural_arguments.as_slice() else {
                    return Err(invalid());
                };
                if !arguments.is_empty()
                    || result != &self.signature.result
                    || address.structural_type != self.structural_type
                    || !address.path.is_empty()
                    || !address.qualifications.is_empty()
                    || !address.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
                let ordinal =
                    usize::try_from(u64::MAX - address.opaque_identity).map_err(|_| invalid())?;
                let slot = values.values.get_mut(ordinal).ok_or_else(invalid)?;
                let owned = slot.as_ref().ok_or_else(invalid)?;
                if owned.application != self.application {
                    return Err(invalid());
                }
                let copyable = matches!(
                    self.signature.structural_parameters.as_slice(),
                    [parameter]
                        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
                );
                let owned = if copyable {
                    slot.as_ref().cloned().ok_or_else(invalid)?
                } else {
                    slot.take().ok_or_else(invalid)?
                };
                // The exact owned value was produced by inet_makeaddr
                // under this same selected opaque application. The C call takes
                // the four-byte struct by value and retains no guest reference.
                let host = owned.value.host_number().ok_or_else(|| {
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
