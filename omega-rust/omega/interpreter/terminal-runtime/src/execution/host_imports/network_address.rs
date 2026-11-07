/*
We provide typed host implementations of the exact admitted inet_makeaddr and
inet_lnaof address operations.

A normalized libc locator selects an operation without relying on guest
requirement names or carrier declaration spellings, and admission checks the
complete selected opaque application and recursive public C shape. Actual C
return values live in an invocation-scoped owned occurrence store where slots
are never recycled so stale consumed tokens cannot name replacements. Moving an
opaque guest value preserves its token, consuming an affine or linear value
removes that exact stored value before invoking C, and checked Copy occurrences
share immutable C bits by value.
*/

use std::sync::{Arc, Mutex};

use calling_conventions::{
    BoundaryCallingApplication, BoundaryValueClass, BoundaryValueGraph, CallSignature, ValueShape,
};
use runtime_abi::HostInternetAddress;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralTypeId};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralMultiplicity,
};

use super::TerminalRuntimeError;

#[derive(Clone, Copy)]
enum AddressOperation {
    Make,
    Host,
}

struct OwnedAddress {
    application: [u8; 32], // Target-closed opaque application commitment relating payloads across components.
    multiplicity: StructuralMultiplicity,
    value: HostInternetAddress,
}

struct AddressContract {
    structural_type: StructuralTypeId,
    application: [u8; 32],
    multiplicity: StructuralMultiplicity,
}

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
        let invalid = super::refusal(
            "inet owned occurrence differs from the admitted application or multiplicity",
        );
        let slot = self.values.get_mut(ordinal).ok_or_else(invalid)?;
        let owned = slot.as_ref().ok_or_else(invalid)?;
        if owned.application != application || owned.multiplicity != multiplicity {
            return Err(invalid());
        }
        match multiplicity {
            StructuralMultiplicity::Affine | StructuralMultiplicity::Linear => {
                Ok(slot.take().ok_or_else(invalid)?.value)
            }
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
        let Some(operation) = select(binding)? else {
            return Ok(None);
        };
        let signature = TerminalImportSignature::from(declaration);
        let contract = validate(declaration, binding, application, &signature, operation)?;
        Ok(Some(Self {
            signature,
            operation,
            contract,
            values,
        }))
    }
}

fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<AddressOperation>, TerminalRuntimeError> {
    for (symbol, operation) in [
        (b"inet_makeaddr".as_slice(), AddressOperation::Make),
        (b"inet_lnaof".as_slice(), AddressOperation::Host),
    ] {
        if super::matches_platform_c_symbol(
            binding,
            symbol,
            &[b"/usr/lib/libSystem.B.dylib".as_slice()],
            Some(runtime_abi::WINDOWS_INTERNET_PROVIDER_LIBRARY),
        )? {
            return Ok(Some(operation));
        }
    }
    Ok(None)
}

fn validate(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: AddressOperation,
) -> Result<AddressContract, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let application = application.ok_or(TerminalRuntimeError::InvalidProvision(
        "inet adapter requires the complete admitted opaque calling application",
    ))?;
    let word = IntegerType::new(IntegerSign::Unsigned, 32)
        .map_err(|_| invalid("inet native u32 type is not admitted"))?;
    let scalar = ScalarType::Integer(word);
    let (structural_type, root, multiplicity) = match operation {
        AddressOperation::Make => {
            let BoundaryMachineResult::Structural(result) = &signature.result else {
                return Err(invalid("inet_makeaddr requires one structural result"));
            };
            if signature.scalar_parameters != [scalar, scalar]
                || !signature.structural_parameters.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
                || application.signature.parameters.len() != 2
            {
                return Err(invalid(
                    "inet_makeaddr requires two u32 scalars and an unqualified result",
                ));
            }
            for root in &application.signature.parameters {
                let shape = application
                    .signature
                    .shapes
                    .get(usize::from(*root))
                    .ok_or_else(|| invalid("inet native signature references an absent shape"))?;
                if shape.class != BoundaryValueClass::Integer
                    || shape.byte_size != 4
                    || shape.alignment != 4
                {
                    return Err(invalid(
                        "inet_makeaddr native inputs must be aligned four-byte integers",
                    ));
                }
            }
            (
                result.structural_type,
                application
                    .signature
                    .result
                    .ok_or_else(|| invalid("inet_makeaddr native result shape is absent"))?,
                result.multiplicity,
            )
        }
        AddressOperation::Host => {
            let [parameter] = signature.structural_parameters.as_slice() else {
                return Err(invalid(
                    "inet_lnaof requires exactly one structural argument",
                ));
            };
            if !signature.scalar_parameters.is_empty()
                || signature.result != BoundaryMachineResult::Scalar(scalar.into())
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Structural]
            {
                return Err(invalid(
                    "inet_lnaof requires one unqualified owned argument and a u32 result",
                ));
            }
            let [root] = application.signature.parameters.as_slice() else {
                return Err(invalid(
                    "inet_lnaof native signature requires exactly one argument",
                ));
            };
            let result = application
                .signature
                .result
                .and_then(|root| application.signature.shapes.get(usize::from(root)))
                .ok_or_else(|| invalid("inet native signature references an absent shape"))?;
            if result.class != BoundaryValueClass::Integer
                || result.byte_size != 4
                || result.alignment != 4
            {
                return Err(invalid(
                    "inet_lnaof native result must be an aligned four-byte integer",
                ));
            }
            (parameter.structural_type, *root, parameter.multiplicity)
        }
    };
    let envelope =
        super::native_word::native_word_envelope(&application.signature, root).map_err(invalid)?;
    if envelope.byte_size != 4 || envelope.alignment != 4 || !envelope.contains_record {
        return Err(invalid(
            "inet carrier must be a four-byte aligned C record ending in u32",
        ));
    }
    if application.opaque_applications.len() != 1
        || application.opaque_applications[0].shape_root != root
        || application.opaque_applications[0].selected_application_commitment == [0; 32]
        || application.opaque_applications[0].conformance_application_commitment == [0; 32]
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "inet selected opaque application differs from its exact signature root",
        ));
    }
    let graph = BoundaryValueGraph {
        shapes: &application.signature.shapes,
        fields: &application.signature.fields,
    };
    let policy = application.plan.plan().call.policy;
    let native_signature = CallSignature {
        parameters: application
            .signature
            .parameters
            .iter()
            .map(|root| graph.classify(*root, policy))
            .collect::<Result<Vec<ValueShape>, _>>()
            .map_err(|_| {
                invalid("inet native shape cannot be classified under the selected calling policy")
            })?,
        result: application
            .signature
            .result
            .map(|root| graph.classify(root, policy))
            .transpose()
            .map_err(|_| {
                invalid("inet native shape cannot be classified under the selected calling policy")
            })?,
    };
    super::admit_exact_foreign_call(signature, declaration, binding, &native_signature)?;
    Ok(AddressContract {
        structural_type,
        application: application.opaque_applications[0].selected_application_commitment,
        multiplicity,
    })
}

impl TerminalImportAdapter for NetworkAddressImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for NetworkAddressImport {
    handle_effect_requires_dispatch!("inet operation requires an authenticated typed result");

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid =
            super::refusal("inet operands or owned occurrence differ from the admitted contract");
        let call = super::call_operands(effect, invalid)?;
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
                ] = call.arguments
                else {
                    return Err(invalid());
                };
                let BoundaryMachineResult::Structural(result) = call.result else {
                    return Err(invalid());
                };
                if *network_type != word
                    || *host_type != word
                    || !call.structural_arguments.is_empty()
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
                let [address] = call.structural_arguments else {
                    return Err(invalid());
                };
                if !call.arguments.is_empty()
                    || call.result != &self.signature.result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_application_and_multiplicity_are_checked_before_consumption() {
        let application = [7; 32];
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Linear,
            StructuralMultiplicity::Unrestricted,
        ] {
            let mut values = AddressValues {
                values: vec![Some(OwnedAddress {
                    application,
                    multiplicity,
                    value: HostInternetAddress::make(10, 66051).unwrap(),
                })],
            };
            assert!(values.argument(0, [8; 32], multiplicity).is_err());
            for other in [
                StructuralMultiplicity::Affine,
                StructuralMultiplicity::Linear,
                StructuralMultiplicity::Unrestricted,
            ] {
                if other != multiplicity {
                    assert!(values.argument(0, application, other).is_err());
                }
            }
            assert_eq!(
                values
                    .argument(0, application, multiplicity)
                    .unwrap()
                    .host_number(),
                Some(66051)
            );
            let again = values.argument(0, application, multiplicity);
            if multiplicity != StructuralMultiplicity::Unrestricted {
                assert!(again.is_err());
                values.values.push(Some(OwnedAddress {
                    application,
                    multiplicity,
                    value: HostInternetAddress::make(10, 1027).unwrap(),
                }));
                assert!(values.argument(0, application, multiplicity).is_err());
                assert_eq!(
                    values
                        .argument(1, application, multiplicity)
                        .unwrap()
                        .host_number(),
                    Some(1027)
                );
            } else {
                assert_eq!(again.unwrap().host_number(), Some(66051));
            }
        }
    }
}
