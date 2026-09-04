use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use omega_calling_conventions::MachineRegister;
use omega_machine_code::{MachineCodeFunction, X86ScalarFmaFormat};
use omega_target::{
    AdmittedX86ScalarFmaProvider, Architecture, NativeTarget, TargetProfile, X86ScalarFmaSlot,
};

use crate::ObjectError;

pub(crate) fn validate_x86_scalar_fma_function(
    target: NativeTarget,
    profile: Option<TargetProfile>,
    provider: Option<AdmittedX86ScalarFmaProvider>,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    if target.architecture != Architecture::X86_64 {
        if function.x86_scalar_fma.is_empty()
            && function.x86_scalar_fma_occurrences.is_empty()
            && function.x86_floating_control.is_none()
        {
            return Ok(());
        }
        return Err(ObjectError::X86ScalarFmaUnsupportedTarget(function.machine));
    }

    let mut decoded_offsets = Vec::new();
    let mut decoder = Decoder::with_ip(64, &function.bytes, 0, DecoderOptions::NONE);
    while decoder.can_decode() {
        let instruction = decoder.decode();
        if matches!(
            instruction.mnemonic(),
            Mnemonic::Vfmadd132ss | Mnemonic::Vfmadd132sd
        ) {
            decoded_offsets.push(instruction.ip() as usize);
        }
    }
    if function.x86_scalar_fma.is_empty() {
        if !function.x86_scalar_fma_occurrences.is_empty()
            || function.x86_floating_control.is_some()
        {
            return Err(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                function.machine,
            ));
        }
        return decoded_offsets.first().copied().map_or(Ok(()), |offset| {
            Err(ObjectError::MissingX86ScalarFmaCustody {
                machine: function.machine,
                offset,
            })
        });
    }
    let profile = profile.ok_or(ObjectError::MissingX86ScalarFmaProfile(function.machine))?;
    if provider
        .is_some_and(|provider| !provider.has_canonical_identity() || provider.profile() != profile)
    {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }

    let mut previous_end = None;
    for fragment in &function.x86_scalar_fma {
        if let Some(end) = previous_end
            && fragment.code_offset < end
        {
            return Err(ObjectError::NonCanonicalX86ScalarFmaOrder(function.machine));
        }
        if fragment.byte_count != 5 {
            return Err(ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        let end = fragment
            .code_offset
            .checked_add(fragment.byte_count)
            .ok_or(ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            })?;
        let bytes = function.bytes.get(fragment.code_offset..end).ok_or(
            ObjectError::InvalidX86ScalarFmaInterval {
                machine: function.machine,
                offset: fragment.code_offset,
            },
        )?;
        if fragment.target != target
            || fragment.requirement.profile() != profile
            || fragment.requirement.profile().native_target() != target
            || !fragment.requirement.has_canonical_identity()
            || fragment.recomputed_identity() != Some(fragment.identity)
        {
            return Err(ObjectError::InvalidX86ScalarFmaCustody {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        let decoded = omega_isa_x86_64::decode_vfmadd132_scalar(bytes).map_err(|_| {
            ObjectError::InvalidX86ScalarFmaEncoding {
                machine: function.machine,
                offset: fragment.code_offset,
            }
        })?;
        let format = match decoded.format {
            omega_isa_x86_64::DecodedScalarFmaFormat::Binary32 => X86ScalarFmaFormat::Binary32,
            omega_isa_x86_64::DecodedScalarFmaFormat::Binary64 => X86ScalarFmaFormat::Binary64,
        };
        if format != fragment.format
            || decoded.destination != fragment.destination
            || decoded.addend != fragment.addend
            || decoded.multiplicand != fragment.multiplicand
            || !matches!(fragment.destination, MachineRegister::X86Xmm(0..=15))
            || !matches!(fragment.addend, MachineRegister::X86Xmm(0..=15))
            || !matches!(fragment.multiplicand, MachineRegister::X86Xmm(0..=15))
        {
            return Err(ObjectError::InvalidX86ScalarFmaCustody {
                machine: function.machine,
                offset: fragment.code_offset,
            });
        }
        if provider.is_some_and(|provider| {
            let slot = match fragment.format {
                X86ScalarFmaFormat::Binary32 => X86ScalarFmaSlot::Binary32,
                X86ScalarFmaFormat::Binary64 => X86ScalarFmaSlot::Binary64,
            };
            !provider.admits(fragment.requirement, slot)
        }) {
            return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
        }
        previous_end = Some(end);
    }

    let retained_offsets = function
        .x86_scalar_fma
        .iter()
        .map(|fragment| fragment.code_offset)
        .collect::<Vec<_>>();
    if decoded_offsets != retained_offsets {
        let offset = decoded_offsets
            .iter()
            .find(|offset| !retained_offsets.contains(offset))
            .copied()
            .or_else(|| retained_offsets.first().copied())
            .unwrap_or(0);
        return Err(ObjectError::MissingX86ScalarFmaCustody {
            machine: function.machine,
            offset,
        });
    }
    validate_semantic_occurrences(target, provider, function)?;
    Ok(())
}

fn validate_semantic_occurrences(
    target: NativeTarget,
    provider: Option<AdmittedX86ScalarFmaProvider>,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    if function.x86_scalar_fma_occurrences.is_empty() {
        if function.x86_floating_control.is_some() {
            return Err(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                function.machine,
            ));
        }
        // The source-free mechanics/admission fixtures intentionally retain
        // only instruction custody. Ordinary compiler output always supplies
        // occurrence and floating-control records together.
        return Ok(());
    }
    if function.x86_scalar_fma_occurrences.len() != function.x86_scalar_fma.len() {
        return Err(ObjectError::InvalidX86ScalarFmaSemanticCustody(
            function.machine,
        ));
    }
    let control =
        function
            .x86_floating_control
            .ok_or(ObjectError::InvalidX86ScalarFmaFloatingControl(
                function.machine,
            ))?;
    validate_floating_control(target, function, control)?;

    let mut operations = std::collections::BTreeSet::new();
    let mut prior_end = control.install_offset + control.install_byte_count;
    for (occurrence, fragment) in function
        .x86_scalar_fma_occurrences
        .iter()
        .zip(&function.x86_scalar_fma)
    {
        let expected_slot = match occurrence.format {
            X86ScalarFmaFormat::Binary32 => X86ScalarFmaSlot::Binary32,
            X86ScalarFmaFormat::Binary64 => X86ScalarFmaSlot::Binary64,
        };
        let attribution = function
            .semantic_code_attribution
            .iter()
            .find(|attribution| {
                attribution.operation_ordinal == occurrence.operation_ordinal
                    && attribution.site
                        == omega_machine_code::SemanticCodeSite::Operation(
                            occurrence.terminal_operation,
                        )
            })
            .ok_or(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                function.machine,
            ))?;
        let attribution_end = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .ok_or(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                function.machine,
            ))?;
        if !operations.insert(occurrence.terminal_operation)
            || !function
                .provenance
                .operations
                .contains(&occurrence.terminal_operation)
            || occurrence.provider_plan_report_identity == 0
            || occurrence.provider_plan_digest == [0; 32]
            || occurrence.slot != expected_slot
            || occurrence.admitted_provider.profile().native_target() != target
            || !occurrence.admitted_provider.has_canonical_identity()
            || !occurrence
                .admitted_provider
                .admits(fragment.requirement, occurrence.slot)
            || provider.is_some_and(|provider| provider != occurrence.admitted_provider)
            || occurrence.fragment_identity != fragment.identity
            || occurrence.format != fragment.format
            || occurrence.destination != fragment.destination
            || occurrence.left.register != fragment.destination
            || occurrence.right.register != fragment.multiplicand
            || occurrence.addend.register != fragment.addend
            || occurrence.left.value.format() != float_format(occurrence.format)
            || occurrence.right.value.format() != float_format(occurrence.format)
            || occurrence.addend.value.format() != float_format(occurrence.format)
            || occurrence.left.code_offset < prior_end
            || occurrence.left.code_offset != attribution.code_offset
            || occurrence.right.code_offset
                != occurrence.left.code_offset + occurrence.left.byte_count
            || occurrence.addend.code_offset
                != occurrence.right.code_offset + occurrence.right.byte_count
            || fragment.code_offset != occurrence.addend.code_offset + occurrence.addend.byte_count
            || fragment.code_offset + fragment.byte_count != attribution_end
            || fragment.code_offset + fragment.byte_count > control.restore_offset
        {
            return Err(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                function.machine,
            ));
        }
        for operand in [occurrence.left, occurrence.right, occurrence.addend] {
            let expected = match operand.value {
                psi_core::IeeeFloatValue::Binary32(bits) => {
                    omega_isa_x86_64::encode_binary32_bits_to_xmm(bits, operand.register)
                }
                psi_core::IeeeFloatValue::Binary64(bits) => {
                    omega_isa_x86_64::encode_binary64_bits_to_xmm(bits, operand.register)
                }
            }
            .map_err(|_| ObjectError::InvalidX86ScalarFmaSemanticCustody(function.machine))?;
            let end = operand.code_offset.checked_add(operand.byte_count).ok_or(
                ObjectError::InvalidX86ScalarFmaSemanticCustody(function.machine),
            )?;
            if operand.byte_count != expected.len()
                || function.bytes.get(operand.code_offset..end) != Some(expected.as_slice())
            {
                return Err(ObjectError::InvalidX86ScalarFmaSemanticCustody(
                    function.machine,
                ));
            }
        }
        prior_end = fragment.code_offset + fragment.byte_count;
    }
    Ok(())
}

fn validate_floating_control(
    target: NativeTarget,
    function: &MachineCodeFunction,
    control: omega_machine_code::X86FloatingControlRecord,
) -> Result<(), ObjectError> {
    let expected_save =
        omega_isa_x86_64::encode_stmxcsr_rsp_displacement(control.saved_slot_byte_offset)
            .map_err(|_| ObjectError::InvalidX86ScalarFmaFloatingControl(function.machine))?;
    let expected_store = omega_isa_x86_64::encode_store_mxcsr_constant_rsp_displacement(
        control.canonical_slot_byte_offset,
        omega_isa_x86_64::OMEGA_CANONICAL_MXCSR,
    )
    .map_err(|_| ObjectError::InvalidX86ScalarFmaFloatingControl(function.machine))?;
    let expected_install =
        omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(control.canonical_slot_byte_offset)
            .map_err(|_| ObjectError::InvalidX86ScalarFmaFloatingControl(function.machine))?;
    let expected_restore =
        omega_isa_x86_64::encode_ldmxcsr_rsp_displacement(control.saved_slot_byte_offset)
            .map_err(|_| ObjectError::InvalidX86ScalarFmaFloatingControl(function.machine))?;
    let exact = |offset: usize, count: usize, expected: &[u8]| {
        count == expected.len()
            && function.bytes.get(offset..offset.saturating_add(count)) == Some(expected)
    };
    let frame = function.unit_stack.and_then(|stack| stack.frame).ok_or(
        ObjectError::InvalidX86ScalarFmaFloatingControl(function.machine),
    )?;
    let body_start = control
        .install_offset
        .checked_add(control.install_byte_count)
        .ok_or(ObjectError::InvalidX86ScalarFmaFloatingControl(
            function.machine,
        ))?;
    let internal_calls_stay_inside_control = function.internal_unit_calls.iter().all(|call| {
        call.code_offset >= body_start
            && call
                .code_offset
                .checked_add(call.byte_count)
                .is_some_and(|end| end <= control.restore_offset)
    });
    let foreign_calls_stay_inside_control = function.foreign_calls.iter().all(|call| {
        call.x86_floating_control.is_some_and(|nested| {
            nested.save_offset >= body_start
                && nested
                    .restore_offset
                    .checked_add(nested.restore_byte_count)
                    .is_some_and(|end| end <= control.restore_offset)
        })
    });
    if control.target != target
        || control.canonical_mxcsr != omega_isa_x86_64::OMEGA_CANONICAL_MXCSR
        || control.saved_slot_byte_offset == control.canonical_slot_byte_offset
        || control.saved_slot_byte_offset + 4 > frame.byte_size
        || control.canonical_slot_byte_offset + 4 > frame.byte_size
        || control.canonical_store_offset != control.save_offset + control.save_byte_count
        || control.install_offset
            != control.canonical_store_offset + control.canonical_store_byte_count
        || control.restore_offset <= body_start
        || !internal_calls_stay_inside_control
        || !foreign_calls_stay_inside_control
        || !exact(control.save_offset, control.save_byte_count, &expected_save)
        || !exact(
            control.canonical_store_offset,
            control.canonical_store_byte_count,
            &expected_store,
        )
        || !exact(
            control.install_offset,
            control.install_byte_count,
            &expected_install,
        )
        || !exact(
            control.restore_offset,
            control.restore_byte_count,
            &expected_restore,
        )
    {
        return Err(ObjectError::InvalidX86ScalarFmaFloatingControl(
            function.machine,
        ));
    }
    Ok(())
}

const fn float_format(format: X86ScalarFmaFormat) -> psi_core::IeeeFloatFormat {
    match format {
        X86ScalarFmaFormat::Binary32 => psi_core::IeeeFloatFormat::Binary32,
        X86ScalarFmaFormat::Binary64 => psi_core::IeeeFloatFormat::Binary64,
    }
}
