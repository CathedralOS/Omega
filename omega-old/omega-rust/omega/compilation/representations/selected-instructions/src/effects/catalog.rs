/*
  Target machine-effect catalog declarations (`MachineEffectCatalog`,
  `PreparedMachineEffectCatalog`), semantic kind inventory
  (`MachineSemanticKind`), alternative families (`MachineAlternativeFamily`),
  and constraint-key ordering.
*/

mod constraint_keys {
    use crate::{SaturatingCarrier, TrappingOperation};
    use register_model::RegisterConstraintKey;

    use super::MachineSemanticKind;
    use crate::IeeeFloatArithmeticOperation::FromInteger;
    use crate::SelectedConstraintKeys;

    impl SelectedConstraintKeys {
        pub fn ordinary_call_constraints(&self) -> Vec<RegisterConstraintKey> {
            let mut rows = Vec::new();
            for key in self
                .call_unit
                .iter()
                .chain(&self.call_unit_mixed)
                .chain(&self.call_scalar)
                .chain(&self.call_aggregate)
            {
                if !rows.contains(key) {
                    rows.push(*key);
                }
            }
            rows
        }

        pub fn in_identity_order(&self) -> Vec<RegisterConstraintKey> {
            [
                self.access_read,
                self.access_write,
                self.port_write,
                self.system_register_read,
                self.system_register_write,
                self.control_register_read,
                self.control_register_write,
                self.global_descriptor_table,
                self.task_register_activate,
                self.task_register_read,
                self.installed_interrupt_cpu_check,
                self.installed_interrupt_task_commit,
                self.installed_interrupt_table_publication,
                self.code_segment_reload,
                self.data_segment_reload,
                self.segment_selector_read,
                self.port_register_read,
                self.port_register_write,
                self.model_specific_read,
                self.model_specific_write,
                self.processor_halt,
                self.privileged_crash,
                self.interrupt_control,
                self.memory_fence,
                self.atomic_fence,
                self.atomic_load,
                self.atomic_store,
                self.atomic_update,
                self.atomic_compare_exchange,
                self.atomic_compare_exchange_once,
                if self.atomic_bitwise_update == self.atomic_update {
                    None
                } else {
                    self.atomic_bitwise_update
                },
                self.copy_bytes,
                self.load_encoded_scalar,
                self.store_encoded_scalar,
                self.zero_bytes,
                self.hosted_read_byte,
                self.hosted_write_byte_i32,
                self.save_floating_control,
                self.restore_floating_control,
                self.hosted_exit_process_i32,
                self.store,
                self.address_offset,
                self.load64,
                self.load8,
                self.load16,
                self.load32,
                self.load8_indexed,
                self.store64,
                self.frame_address,
                self.byte_literal_image_address,
                self.float32_to_bits,
                self.float64_to_bits,
                self.bits_to_float32,
                self.bits_to_float64,
                self.ieee_float_arithmetic,
                if self.ieee_float_directed_binary == self.ieee_float_arithmetic {
                    None
                } else {
                    self.ieee_float_directed_binary
                },
                if self.ieee_float_directed_unary == self.ieee_float_unary {
                    None
                } else {
                    self.ieee_float_directed_unary
                },
                self.ieee_float_unary,
                if self.ieee_float_conversion_control == self.ieee_float_unary {
                    None
                } else {
                    self.ieee_float_conversion_control
                },
                self.ieee_float_ternary,
            ]
            .into_iter()
            .flatten()
            .chain(self.call_unit.iter().copied())
            .chain(self.call_unit_mixed.iter().copied())
            .chain(self.call_scalar.iter().copied())
            .chain(self.call_aggregate.iter().copied())
            .chain(self.call_normalized_foreign.iter().copied())
            .chain(self.return_aggregate.iter().copied())
            .chain(self.return_float.iter().copied())
            .chain([
                self.crash,
                self.materialize_i64,
                self.materialize_boolean,
                self.copy_i64,
                self.add_i64,
                self.add_i64_immediate,
                self.subtract_i64,
                self.subtract_i64_immediate,
                self.compare_i64_zero,
                self.conditional_branch,
                self.return_i64,
                self.return_unit,
                self.compare_i64,
                self.jump,
                self.compare_i64_immediate,
                self.saturating_subtract_unsigned,
                self.saturating_add_u64,
                self.divide_u64,
                self.remainder_u64,
                self.remainder_i64,
                self.divide_i64,
                self.shift_i64,
                self.saturating_add_clamped,
                self.saturating_subtract_clamped,
                self.saturating_divide_signed,
                self.multiply_i64,
                self.saturating_multiply_clamped,
                self.saturating_multiply_u64,
                self.trapping_binary,
                self.trapping_fixed_pair,
                self.trapping_shift,
                self.trapping_convert,
            ])
            .chain(
                [self.float_result_trapping, self.float_result_saturating]
                    .into_iter()
                    .flatten(),
            )
            .collect()
        }

        pub const fn for_semantic(
            &self,
            semantic: MachineSemanticKind,
        ) -> Option<RegisterConstraintKey> {
            Some(match semantic {
                MachineSemanticKind::AccessorScalarAccess(form) => {
                    return if form.write {
                        self.rows.access_write
                    } else {
                        self.rows.access_read
                    };
                }
                MachineSemanticKind::PortWrite => return self.rows.port_write,
                MachineSemanticKind::SystemRegisterRead => return self.rows.system_register_read,
                MachineSemanticKind::SystemRegisterWrite => return self.rows.system_register_write,
                MachineSemanticKind::TaskRegisterActivate => {
                    return self.rows.task_register_activate;
                }
                MachineSemanticKind::TaskRegisterRead => return self.rows.task_register_read,
                MachineSemanticKind::InstalledInterruptCpuCheck => {
                    return self.rows.installed_interrupt_cpu_check;
                }
                MachineSemanticKind::InstalledInterruptTaskCommit => {
                    return self.rows.installed_interrupt_task_commit;
                }
                MachineSemanticKind::InstalledInterruptTablePublication => {
                    return self.rows.installed_interrupt_table_publication;
                }
                MachineSemanticKind::CodeSegmentReload => return self.rows.code_segment_reload,
                MachineSemanticKind::DataSegmentReload => return self.rows.data_segment_reload,
                MachineSemanticKind::SegmentSelectorRead => return self.rows.segment_selector_read,
                MachineSemanticKind::GlobalDescriptorTableLoad
                | MachineSemanticKind::GlobalDescriptorTableStore => {
                    return self.rows.global_descriptor_table;
                }
                MachineSemanticKind::ControlRegisterRead => return self.rows.control_register_read,
                MachineSemanticKind::ControlRegisterWrite => {
                    return self.rows.control_register_write;
                }
                MachineSemanticKind::PortRegisterRead => return self.rows.port_register_read,
                MachineSemanticKind::PortRegisterWrite => return self.rows.port_register_write,
                MachineSemanticKind::ModelSpecificRead => return self.rows.model_specific_read,
                MachineSemanticKind::ModelSpecificWrite => return self.rows.model_specific_write,
                MachineSemanticKind::ProcessorHalt => return self.rows.processor_halt,
                MachineSemanticKind::InterruptControl => return self.rows.interrupt_control,
                MachineSemanticKind::MemoryFence => return self.rows.memory_fence,
                MachineSemanticKind::AtomicFence => return self.rows.atomic_fence,
                MachineSemanticKind::AtomicAccess(form) => {
                    return match form.operation {
                        crate::AtomicOperation::Load => self.rows.atomic_load,
                        crate::AtomicOperation::Store => self.rows.atomic_store,
                        crate::AtomicOperation::CompareExchange => {
                            self.rows.atomic_compare_exchange
                        }
                        crate::AtomicOperation::CompareExchangeOnce => {
                            self.rows.atomic_compare_exchange_once
                        }
                        crate::AtomicOperation::FetchAnd
                        | crate::AtomicOperation::FetchOr
                        | crate::AtomicOperation::FetchXor => self.rows.atomic_bitwise_update,
                        _ => self.rows.atomic_update,
                    };
                }
                MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                    if form.operation.rounding_direction().is_some() {
                        return match form.operand_count() {
                            1 => self.rows.ieee_float_directed_unary,
                            2 => self.rows.ieee_float_directed_binary,
                            _ => None,
                        };
                    }
                    return match form.operand_count() {
                        1 => match form.operation {
                            crate::IeeeFloatArithmeticOperation::IsNan
                            | crate::IeeeFloatArithmeticOperation::IsInfinite
                            | crate::IeeeFloatArithmeticOperation::IsFinite
                            | crate::IeeeFloatArithmeticOperation::IsNormal
                            | crate::IeeeFloatArithmeticOperation::IsSubnormal
                            | crate::IeeeFloatArithmeticOperation::Classify
                            | FromInteger(crate::IeeeFloatIntegerCarrier::U64)
                            | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                                target: crate::IeeeFloatIntegerCarrier::U64,
                                ..
                            }
                            | crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                                domain: crate::IeeeFloatToIntegerDomain::Trapping,
                                ..
                            } => self.rows.ieee_float_conversion_control,
                            _ => self.rows.ieee_float_unary,
                        },
                        2 => self.rows.ieee_float_arithmetic,
                        3 => self.rows.ieee_float_ternary,
                        _ => None,
                    };
                }
                MachineSemanticKind::FloatResultPolicy(form) => {
                    return match form.policy {
                        crate::FloatResultPolicyKind::Trapping => self.rows.float_result_trapping,
                        crate::FloatResultPolicyKind::Saturating => {
                            self.rows.float_result_saturating
                        }
                    };
                }
                MachineSemanticKind::CopyBytes => return self.rows.copy_bytes,
                MachineSemanticKind::ZeroBytes => return self.rows.zero_bytes,
                MachineSemanticKind::ScanByteSequence => return self.rows.scan_byte_sequence,
                MachineSemanticKind::LoadEncodedScalar => return self.rows.load_encoded_scalar,
                MachineSemanticKind::StoreEncodedScalar => return self.rows.store_encoded_scalar,
                MachineSemanticKind::CompareByteSequences => {
                    return self.rows.compare_byte_sequences;
                }
                MachineSemanticKind::Float32ToBits => return self.rows.float32_to_bits,
                MachineSemanticKind::Float64ToBits => return self.rows.float64_to_bits,
                MachineSemanticKind::BitsToFloat32 => return self.rows.bits_to_float32,
                MachineSemanticKind::BitsToFloat64 => return self.rows.bits_to_float64,
                MachineSemanticKind::HostedReadByte => return self.rows.hosted_read_byte,
                MachineSemanticKind::HostedWriteByteI32 => return self.rows.hosted_write_byte_i32,
                MachineSemanticKind::SaveFloatingControl => return self.rows.save_floating_control,
                MachineSemanticKind::RestoreFloatingControl => {
                    return self.rows.restore_floating_control;
                }
                MachineSemanticKind::Crash => self.rows.crash,
                MachineSemanticKind::PrivilegedCrash => return self.rows.privileged_crash,
                MachineSemanticKind::HostedExitProcessI32 => {
                    return self.rows.hosted_exit_process_i32;
                }
                MachineSemanticKind::Store => return self.rows.store,
                MachineSemanticKind::AddressOffset => return self.rows.address_offset,
                MachineSemanticKind::Load8Indexed => return self.rows.load8_indexed,
                MachineSemanticKind::Load64 => return self.rows.load64,
                MachineSemanticKind::LoadPacked3
                | MachineSemanticKind::LoadPacked5
                | MachineSemanticKind::LoadPacked6
                | MachineSemanticKind::LoadPacked7 => return self.rows.load_packed,
                MachineSemanticKind::StorePacked => return self.rows.store_packed,
                MachineSemanticKind::Load8 => return self.rows.load8,
                MachineSemanticKind::Load16 => return self.rows.load16,
                MachineSemanticKind::Load32 => return self.rows.load32,
                MachineSemanticKind::Store64 => return self.rows.store64,
                MachineSemanticKind::FrameAddress => return self.rows.frame_address,
                MachineSemanticKind::ImageAddress => {
                    return self.rows.byte_literal_image_address;
                }
                MachineSemanticKind::CallUnit | MachineSemanticKind::InvokeNoNormal => return None,
                MachineSemanticKind::CompareI64Zero => self.rows.compare_i64_zero,
                MachineSemanticKind::MaterializeI64 => self.rows.materialize_i64,
                MachineSemanticKind::MaterializeBooleanEqual => self.rows.materialize_boolean,
                MachineSemanticKind::MaterializeBooleanU64LessThan => self.rows.materialize_boolean,
                MachineSemanticKind::MaterializeBooleanI64LessThan => self.rows.materialize_boolean,
                MachineSemanticKind::MaterializeBooleanU64LessOrEqual => {
                    self.rows.materialize_boolean
                }
                MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
                    self.rows.materialize_boolean
                }

                MachineSemanticKind::CopyI64
                | MachineSemanticKind::ZeroExtendU8
                | MachineSemanticKind::ZeroExtendU16
                | MachineSemanticKind::SignExtendI8
                | MachineSemanticKind::SignExtendI16
                | MachineSemanticKind::SignExtendI32
                | MachineSemanticKind::ZeroExtendU32 => self.rows.copy_i64,
                MachineSemanticKind::ByteViewAddress
                | MachineSemanticKind::ExactAddI64
                | MachineSemanticKind::WrappingAddI64 => self.rows.add_i64,
                MachineSemanticKind::BitwiseAndI64 => self.rows.subtract_i64,
                MachineSemanticKind::BitwiseOrI64 => self.rows.subtract_i64,
                MachineSemanticKind::BitwiseXorI64 => self.rows.subtract_i64,
                MachineSemanticKind::BitwiseNotI64 => self.rows.copy_i64,
                MachineSemanticKind::ExactAddI64Immediate => self.rows.add_i64_immediate,
                MachineSemanticKind::ExactSubtractI64 => self.rows.subtract_i64,
                MachineSemanticKind::WrappingSubtractI64 => self.rows.subtract_i64,
                MachineSemanticKind::ExactMultiplyI64 => self.rows.multiply_i64,
                MachineSemanticKind::WrappingMultiplyI64 => self.rows.multiply_i64,
                MachineSemanticKind::ExactDivideU64 => self.rows.divide_u64,
                MachineSemanticKind::ExactRemainderU64 => self.rows.remainder_u64,
                MachineSemanticKind::WrappingRemainderI64 => self.rows.remainder_i64,
                MachineSemanticKind::WrappingDivideI64 => self.rows.divide_i64,
                MachineSemanticKind::ExactDivideI64 => self.rows.divide_i64,
                MachineSemanticKind::ExactRemainderI64 => self.rows.remainder_i64,
                MachineSemanticKind::WrappingShiftLeftI64
                | MachineSemanticKind::WrappingShiftRightI64
                | MachineSemanticKind::WrappingShiftRightU64
                | MachineSemanticKind::ExactShiftLeftI64
                | MachineSemanticKind::ExactShiftRightI64
                | MachineSemanticKind::ExactShiftRightU64 => self.rows.shift_i64,
                MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64) => {
                    self.rows.saturating_add_u64
                }
                MachineSemanticKind::SaturatingAdd(_) => self.rows.saturating_add_clamped,
                MachineSemanticKind::SaturatingSubtract(carrier) if !carrier.is_signed() => {
                    self.rows.saturating_subtract_unsigned
                }
                MachineSemanticKind::SaturatingSubtract(_) => self.rows.saturating_subtract_clamped,
                MachineSemanticKind::SaturatingDivide(carrier) if !carrier.is_signed() => {
                    self.rows.divide_u64
                }
                MachineSemanticKind::SaturatingDivide(_) => self.rows.saturating_divide_signed,
                MachineSemanticKind::SaturatingRemainder(carrier) if !carrier.is_signed() => {
                    self.rows.remainder_u64
                }
                MachineSemanticKind::SaturatingRemainder(_) => self.rows.remainder_i64,
                MachineSemanticKind::SaturatingMultiply(SaturatingCarrier::U64) => {
                    self.rows.saturating_multiply_u64
                }
                MachineSemanticKind::SaturatingMultiply(_) => self.rows.saturating_multiply_clamped,
                MachineSemanticKind::TrappingInteger(form) => match form.operation {
                    TrappingOperation::Divide | TrappingOperation::Remainder => {
                        self.rows.trapping_fixed_pair
                    }
                    TrappingOperation::Multiply
                        if matches!(form.carrier, SaturatingCarrier::U64) =>
                    {
                        self.rows.trapping_fixed_pair
                    }
                    TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                        self.rows.trapping_shift
                    }
                    TrappingOperation::Convert { .. } => self.rows.trapping_convert,
                    TrappingOperation::Add
                    | TrappingOperation::Subtract
                    | TrappingOperation::Multiply => self.rows.trapping_binary,
                },
                MachineSemanticKind::ExactSubtractI64Immediate => self.rows.subtract_i64_immediate,
                MachineSemanticKind::ConditionalBranchNonZero => self.rows.conditional_branch,
                MachineSemanticKind::ReturnScalar => self.rows.return_i64,
                MachineSemanticKind::ReturnUnit => self.rows.return_unit,
                MachineSemanticKind::CompareI64 => self.rows.compare_i64,
                MachineSemanticKind::CompareI64Immediate => self.rows.compare_i64_immediate,
                MachineSemanticKind::ConditionalBranchU64LessThan => self.rows.conditional_branch,
                MachineSemanticKind::ConditionalBranchI64LessThan => self.rows.conditional_branch,
                MachineSemanticKind::CallScalar
                | MachineSemanticKind::CallAggregate
                | MachineSemanticKind::ReturnAggregate => return None,
                MachineSemanticKind::NormalizedForeignCall => return None,
                MachineSemanticKind::Jump => self.rows.jump,
            })
        }

        pub fn declaration_keys(&self) -> Vec<(MachineSemanticKind, RegisterConstraintKey)> {
            MachineSemanticKind::ALL
                .into_iter()
                .flat_map(|semantic| {
                    if semantic == MachineSemanticKind::ReturnScalar {
                        return std::iter::once(self.return_i64)
                            .chain(self.return_float.iter().copied())
                            .map(|key| (semantic, key))
                            .collect();
                    }
                    if matches!(
                        semantic,
                        MachineSemanticKind::CallScalar
                            | MachineSemanticKind::InvokeNoNormal
                            | MachineSemanticKind::CallUnit
                            | MachineSemanticKind::CallAggregate
                            | MachineSemanticKind::ReturnAggregate
                            | MachineSemanticKind::NormalizedForeignCall
                    ) {
                        (if semantic == MachineSemanticKind::InvokeNoNormal {
                            &self.invoke_no_normal
                        } else if semantic == MachineSemanticKind::CallAggregate {
                            &self.call_aggregate
                        } else if semantic == MachineSemanticKind::ReturnAggregate {
                            &self.return_aggregate
                        } else if semantic == MachineSemanticKind::CallUnit {
                            &self.call_unit
                        } else if semantic == MachineSemanticKind::NormalizedForeignCall {
                            &self.call_normalized_foreign
                        } else {
                            &self.call_scalar
                        })
                        .iter()
                        .chain(if semantic == MachineSemanticKind::CallUnit {
                            self.call_unit_mixed.iter()
                        } else {
                            [].iter()
                        })
                        .copied()
                        .map(|key| (semantic, key))
                        .collect()
                    } else {
                        self.for_semantic(semantic)
                            .map(|key| (semantic, key))
                            .into_iter()
                            .collect::<Vec<_>>()
                    }
                })
                .collect()
        }
    }
}
mod identity;
mod validation;

use std::collections::BTreeSet;

use register_model::PreparedRegisterConstraintCatalog;

use validation::validate_declaration;

use crate::{
    AccessorScalarForm, AtomicForm, FloatResultPolicyForm, IeeeFloatArithmeticForm,
    SaturatingCarrier, SelectedConstraintKeys, TrappingForm,
};
pub use identity::{
    MachineIdentityBytes, alternative_family_tag, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity,
    machine_effect_catalog_identity, saturating_family_tag, trapping_family_tag,
};
use register_model::RegisterConstraintFamily::SystemCall;
use register_model::find_register_constraint;
use register_model::{RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterViewId};
use target::NativeTarget;

pub fn validate_machine_effect_catalog(
    constraints: &PreparedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<PreparedMachineEffectCatalog, MachineEffectCatalogValidationError> {
    if catalog.target.architecture != constraints.architecture() {
        return Err(MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    if catalog.register_constraints != constraints.identity() {
        return Err(MachineEffectCatalogValidationError::RegisterConstraintRootMismatch);
    }
    let selected = catalog.selected_keys.in_identity_order();
    if catalog.selected_keys.invoke_no_normal != catalog.selected_keys.ordinary_call_constraints() {
        return Err(MachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(MachineEffectCatalogValidationError::DuplicateSelectedConstraintKey);
    }
    if catalog
        .declarations
        .windows(2)
        .any(|pair| pair[0].semantic > pair[1].semantic)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalDeclarations);
    }
    let expected = catalog.selected_keys.declaration_keys();
    if catalog.declarations.len() != expected.len()
        || catalog
            .declarations
            .iter()
            .zip(&expected)
            .any(|(actual, (semantic, constraint))| {
                actual.semantic != *semantic || actual.constraint != *constraint
            })
    {
        return Err(MachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    for declaration in &catalog.declarations {
        let row =
            find_register_constraint(&constraints.catalog().constraints, declaration.constraint)
                .ok_or(MachineEffectCatalogValidationError::UnknownConstraint(
                    declaration.semantic,
                ))?;
        validate_declaration(catalog.target.architecture, row, declaration)?;
    }
    let identity = machine_effect_catalog_identity(&catalog);
    Ok(PreparedMachineEffectCatalog { catalog, identity })
}

crate::identity::sha256_identity!(MachineEffectCatalogIdentity);

macro_rules! machine_semantic_kinds {
    (@count fixed $variant:ident) => { 1usize };
    (@count family $variant:ident($carrier:ident)) => { $carrier::ALL.len() };
    (@count dynamic $variant:ident($form:ident)) => { 0usize };
    (@entry ($fixed:ident, $index:ident) fixed $variant:ident) => {{
        $fixed[$index] = Self::$variant;
        $index += 1;
    }};
    (@entry ($fixed:ident, $index:ident) family $variant:ident($carrier:ident)) => {{
        let mut carrier = 0usize;
        while carrier < $carrier::ALL.len() {
            $fixed[$index] = Self::$variant($carrier::ALL[carrier]);
            $index += 1;
            carrier += 1;
        }
    }};
    (@entry ($fixed:ident, $index:ident) dynamic $variant:ident($form:ident)) => {};
    ($( { $(#[$doc:meta])* $kind:ident $variant:ident $(($payload:ident))? } ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum MachineSemanticKind {
            $( $(#[$doc])* $variant $(($payload))?, )*
        }

        impl MachineSemanticKind {
            #[allow(unused_assignments)]
            const FIXED: [Self; 0 $(+ machine_semantic_kinds!(@count $kind $variant $(($payload))?))*] = {
                let mut fixed = [Self::Crash; 0 $(+ machine_semantic_kinds!(@count $kind $variant $(($payload))?))*];
                let mut index = 0usize;
                $( machine_semantic_kinds!(@entry (fixed, index) $kind $variant $(($payload))?); )*
                fixed
            };
        }
    };
}

machine_semantic_kinds! {
    { fixed Crash },
    { fixed PrivilegedCrash },
    { fixed CopyBytes },
    { fixed BitwiseAndI64 },
    { fixed BitwiseXorI64 },
    { fixed CallAggregate },
    { fixed ReturnAggregate },
    { fixed HostedExitProcessI32 },
    { fixed LoadPacked3 },
    { fixed LoadPacked5 },
    { fixed LoadPacked6 },
    { fixed LoadPacked7 },
    { fixed StorePacked },
    { fixed Load8 },
    { fixed Load16 },
    { fixed Load32 },
    { fixed Float32ToBits },
    { fixed Float64ToBits },
    { fixed BitsToFloat32 },
    { fixed BitsToFloat64 },
    { fixed Load8Indexed },
    { fixed CompareI64Zero },
    { fixed MaterializeI64 },
    { fixed CopyI64 },
    { fixed ExactAddI64 },
    { fixed ExactAddI64Immediate },
    { fixed ExactSubtractI64 },
    { fixed ExactDivideU64 },
    { fixed ExactSubtractI64Immediate },
    { fixed ConditionalBranchNonZero },
    { fixed ReturnScalar },
    { fixed ReturnUnit },
    { fixed CompareI64 },
    { fixed CompareI64Immediate },
    { fixed ConditionalBranchU64LessThan },
    { fixed ConditionalBranchI64LessThan },
    { fixed CallScalar },
    { fixed Jump },
    { fixed ZeroExtendU8 },
    { fixed ZeroExtendU32 },
    { fixed ZeroExtendU16 },
    { fixed SignExtendI8 },
    { fixed SignExtendI16 },
    { fixed SignExtendI32 },
    { fixed Load64 },
    { fixed Store64 },
    { fixed FrameAddress },
    { fixed CallUnit },
    { fixed ByteViewAddress },
    { fixed HostedReadByte },
    { fixed HostedWriteByteI32 },
    { fixed Store },
    { fixed AddressOffset },
    { fixed MaterializeBooleanEqual },
    { fixed MaterializeBooleanU64LessThan },
    { fixed MaterializeBooleanI64LessThan },
    { fixed MaterializeBooleanU64LessOrEqual },
    { fixed MaterializeBooleanI64LessOrEqual },
    { fixed WrappingRemainderI64 },
    { fixed WrappingAddI64 },
    { family SaturatingAdd(SaturatingCarrier) },
    { family SaturatingSubtract(SaturatingCarrier) },
    { family SaturatingDivide(SaturatingCarrier) },
    { family SaturatingRemainder(SaturatingCarrier) },
    { fixed NormalizedForeignCall },
    { fixed ExactMultiplyI64 },
    { fixed ExactRemainderU64 },
    { fixed WrappingSubtractI64 },
    { fixed WrappingMultiplyI64 },
    { fixed WrappingDivideI64 },
    { fixed BitwiseOrI64 },
    { fixed BitwiseNotI64 },
    { fixed SaveFloatingControl },
    { fixed RestoreFloatingControl },
    { fixed WrappingShiftLeftI64 },
    { fixed WrappingShiftRightI64 },
    { fixed WrappingShiftRightU64 },
    { fixed ExactShiftLeftI64 },
    { fixed ExactShiftRightI64 },
    { fixed ExactShiftRightU64 },
    { fixed ExactDivideI64 },
    { fixed ExactRemainderI64 },
    { family SaturatingMultiply(SaturatingCarrier) },
    {
        fixed ImageAddress
    },
    {
        fixed ScanByteSequence
    },
    { fixed LoadEncodedScalar },
    { fixed StoreEncodedScalar },
    {
        fixed CompareByteSequences
    },
    {
        fixed ZeroBytes
    },
    { fixed PortWrite },
    { fixed SystemRegisterRead },
    { fixed SystemRegisterWrite },
    { fixed PortRegisterRead },
    { fixed PortRegisterWrite },
    { fixed ModelSpecificRead },
    { fixed ModelSpecificWrite },
    { fixed ProcessorHalt },
    { fixed InterruptControl },
    { fixed MemoryFence },
    { fixed ControlRegisterRead },
    { fixed ControlRegisterWrite },
    { fixed GlobalDescriptorTableLoad },
    { fixed GlobalDescriptorTableStore },

    { fixed AtomicFence },
    { fixed InvokeNoNormal },
    { fixed TaskRegisterActivate },
    { fixed TaskRegisterRead },
    { fixed InstalledInterruptCpuCheck },
    { fixed InstalledInterruptTaskCommit },
    { fixed InstalledInterruptTablePublication },

    { fixed CodeSegmentReload },
    { fixed DataSegmentReload },
    { fixed SegmentSelectorRead },
    {
        dynamic TrappingInteger(TrappingForm)
    },
    { dynamic AtomicAccess(AtomicForm) },
    { dynamic NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm) },
    { dynamic FloatResultPolicy(FloatResultPolicyForm) },
    { dynamic AccessorScalarAccess(AccessorScalarForm) },
}

impl MachineSemanticKind {
    pub const ALL: [Self;
        Self::FIXED.len()
            + TrappingForm::COUNT
            + AtomicForm::COUNT
            + IeeeFloatArithmeticForm::COUNT
            + FloatResultPolicyForm::COUNT
            + AccessorScalarForm::COUNT] = {
        let mut all = [Self::Crash;
            Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + FloatResultPolicyForm::COUNT
                + AccessorScalarForm::COUNT];
        let mut index = 0;
        while index < Self::FIXED.len() {
            all[index] = Self::FIXED[index];
            index += 1;
        }
        let mut form = 0;
        while form < TrappingForm::COUNT {
            all[Self::FIXED.len() + form] = Self::TrappingInteger(TrappingForm::ALL[form]);
            form += 1;
        }
        let mut atomic = 0;
        while atomic < AtomicForm::COUNT {
            all[Self::FIXED.len() + TrappingForm::COUNT + atomic] =
                Self::AtomicAccess(AtomicForm::ALL[atomic]);
            atomic += 1;
        }
        let mut float = 0;
        while float < IeeeFloatArithmeticForm::COUNT {
            all[Self::FIXED.len() + TrappingForm::COUNT + AtomicForm::COUNT + float] =
                Self::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm::ALL[float]);
            float += 1;
        }
        let mut policy = 0;
        while policy < FloatResultPolicyForm::COUNT {
            all[Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + policy] = Self::FloatResultPolicy(FloatResultPolicyForm::ALL[policy]);
            policy += 1;
        }
        let mut placed = 0;
        while placed < AccessorScalarForm::COUNT {
            all[Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + FloatResultPolicyForm::COUNT
                + placed] = Self::AccessorScalarAccess(AccessorScalarForm::ALL[placed]);
            placed += 1;
        }
        all
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeFamily {
    GlobalDescriptorTableLoad,
    GlobalDescriptorTableStore,
    TaskRegisterActivate,
    TaskRegisterRead,
    InstalledInterruptCpuCheck,
    InstalledInterruptTaskCommit,
    InstalledInterruptTablePublication,

    CodeSegmentReload,
    DataSegmentReload,
    SegmentSelectorRead,
    ControlRegisterRead,
    ControlRegisterWrite,
    PortWrite,
    SystemRegisterRead,
    SystemRegisterWrite,
    PortRegisterRead,
    PortRegisterWrite,
    ModelSpecificRead,
    ModelSpecificWrite,
    ProcessorHalt,
    InterruptControl,
    MemoryFence,

    AtomicFence,
    AtomicAccess(AtomicForm),
    AccessorScalarAccess(AccessorScalarForm),
    Crash,
    PrivilegedCrash,
    CopyBytes,
    BitwiseAndI64,
    BitwiseXorI64,
    CallAggregate,
    ReturnAggregate,
    HostedExitProcessI32,
    LoadPacked3,
    LoadPacked5,
    LoadPacked6,
    LoadPacked7,
    StorePacked,
    Load8,
    Load16,
    Load32,
    Float32ToBits,
    Float64ToBits,
    BitsToFloat32,
    BitsToFloat64,
    Load8Indexed,
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ExactDivideU64,
    ExactSubtractI64Immediate,
    ConditionalBranchNonZero,
    ReturnScalar,
    ReturnUnit,
    CompareI64,
    CompareI64Immediate,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    CallScalar,
    Jump,
    ZeroExtendU8,
    ZeroExtendU32,
    ZeroExtendU16,
    SignExtendI8,
    SignExtendI16,
    SignExtendI32,
    Load64,
    Store64,
    FrameAddress,
    CallUnit,
    ByteViewAddress,
    HostedReadByte,
    HostedWriteByteI32,
    Store,
    AddressOffset,
    MaterializeBooleanEqual,
    MaterializeBooleanU64LessThan,
    MaterializeBooleanI64LessThan,
    MaterializeBooleanU64LessOrEqual,
    MaterializeBooleanI64LessOrEqual,
    WrappingRemainderI64,
    WrappingAddI64,
    SaturatingAdd(SaturatingCarrier),
    SaturatingSubtract(SaturatingCarrier),
    SaturatingDivide(SaturatingCarrier),
    SaturatingRemainder(SaturatingCarrier),
    NormalizedForeignCall,
    ExactMultiplyI64,
    ExactRemainderU64,
    WrappingSubtractI64,
    WrappingMultiplyI64,
    WrappingDivideI64,
    BitwiseOrI64,
    BitwiseNotI64,
    SaveFloatingControl,
    RestoreFloatingControl,
    WrappingShiftLeftI64,
    WrappingShiftRightI64,
    WrappingShiftRightU64,
    ExactShiftLeftI64,
    ExactShiftRightI64,
    ExactShiftRightU64,
    ExactDivideI64,
    ExactRemainderI64,
    SaturatingMultiply(SaturatingCarrier),
    ImageAddress,
    ScanByteSequence,
    LoadEncodedScalar,
    StoreEncodedScalar,
    CompareByteSequences,
    ZeroBytes,
    TrappingInteger(TrappingForm),
    NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm),
    FloatResultPolicy(FloatResultPolicyForm),
    InvokeNoNormal,
}

impl From<MachineSemanticKind> for MachineAlternativeFamily {
    fn from(value: MachineSemanticKind) -> Self {
        match value {
            MachineSemanticKind::InvokeNoNormal => Self::InvokeNoNormal,
            MachineSemanticKind::SystemRegisterRead => Self::SystemRegisterRead,
            MachineSemanticKind::SystemRegisterWrite => Self::SystemRegisterWrite,
            MachineSemanticKind::GlobalDescriptorTableLoad => Self::GlobalDescriptorTableLoad,
            MachineSemanticKind::GlobalDescriptorTableStore => Self::GlobalDescriptorTableStore,
            MachineSemanticKind::TaskRegisterActivate => Self::TaskRegisterActivate,
            MachineSemanticKind::TaskRegisterRead => Self::TaskRegisterRead,
            MachineSemanticKind::InstalledInterruptCpuCheck => Self::InstalledInterruptCpuCheck,
            MachineSemanticKind::InstalledInterruptTaskCommit => Self::InstalledInterruptTaskCommit,
            MachineSemanticKind::InstalledInterruptTablePublication => {
                Self::InstalledInterruptTablePublication
            }

            MachineSemanticKind::CodeSegmentReload => Self::CodeSegmentReload,
            MachineSemanticKind::DataSegmentReload => Self::DataSegmentReload,
            MachineSemanticKind::SegmentSelectorRead => Self::SegmentSelectorRead,
            MachineSemanticKind::ControlRegisterRead => Self::ControlRegisterRead,
            MachineSemanticKind::ControlRegisterWrite => Self::ControlRegisterWrite,
            MachineSemanticKind::PortRegisterRead => Self::PortRegisterRead,
            MachineSemanticKind::PortRegisterWrite => Self::PortRegisterWrite,
            MachineSemanticKind::ModelSpecificRead => Self::ModelSpecificRead,
            MachineSemanticKind::ModelSpecificWrite => Self::ModelSpecificWrite,
            MachineSemanticKind::ProcessorHalt => Self::ProcessorHalt,
            MachineSemanticKind::InterruptControl => Self::InterruptControl,
            MachineSemanticKind::MemoryFence => Self::MemoryFence,

            MachineSemanticKind::CopyBytes => Self::CopyBytes,
            MachineSemanticKind::BitwiseAndI64 => Self::BitwiseAndI64,
            MachineSemanticKind::BitwiseXorI64 => Self::BitwiseXorI64,
            MachineSemanticKind::CallAggregate => Self::CallAggregate,
            MachineSemanticKind::ReturnAggregate => Self::ReturnAggregate,
            MachineSemanticKind::Crash => Self::Crash,
            MachineSemanticKind::PrivilegedCrash => Self::PrivilegedCrash,
            MachineSemanticKind::HostedExitProcessI32 => Self::HostedExitProcessI32,
            MachineSemanticKind::LoadPacked3 => Self::LoadPacked3,
            MachineSemanticKind::LoadPacked5 => Self::LoadPacked5,
            MachineSemanticKind::LoadPacked6 => Self::LoadPacked6,
            MachineSemanticKind::LoadPacked7 => Self::LoadPacked7,
            MachineSemanticKind::StorePacked => Self::StorePacked,
            MachineSemanticKind::Load8 => Self::Load8,
            MachineSemanticKind::Load16 => Self::Load16,
            MachineSemanticKind::Load32 => Self::Load32,
            MachineSemanticKind::Float32ToBits => Self::Float32ToBits,
            MachineSemanticKind::Float64ToBits => Self::Float64ToBits,
            MachineSemanticKind::BitsToFloat32 => Self::BitsToFloat32,
            MachineSemanticKind::BitsToFloat64 => Self::BitsToFloat64,
            MachineSemanticKind::HostedReadByte => Self::HostedReadByte,
            MachineSemanticKind::HostedWriteByteI32 => Self::HostedWriteByteI32,
            MachineSemanticKind::SaveFloatingControl => Self::SaveFloatingControl,
            MachineSemanticKind::RestoreFloatingControl => Self::RestoreFloatingControl,
            MachineSemanticKind::Store => Self::Store,
            MachineSemanticKind::AddressOffset => Self::AddressOffset,
            MachineSemanticKind::ByteViewAddress => Self::ByteViewAddress,
            MachineSemanticKind::Load8Indexed => Self::Load8Indexed,
            MachineSemanticKind::CompareI64Zero => Self::CompareI64Zero,
            MachineSemanticKind::MaterializeI64 => Self::MaterializeI64,
            MachineSemanticKind::CopyI64 => Self::CopyI64,
            MachineSemanticKind::ExactAddI64 => Self::ExactAddI64,
            MachineSemanticKind::ExactAddI64Immediate => Self::ExactAddI64Immediate,
            MachineSemanticKind::ExactSubtractI64 => Self::ExactSubtractI64,
            MachineSemanticKind::ExactDivideU64 => Self::ExactDivideU64,
            MachineSemanticKind::WrappingRemainderI64 => Self::WrappingRemainderI64,
            MachineSemanticKind::WrappingAddI64 => Self::WrappingAddI64,
            MachineSemanticKind::SaturatingAdd(carrier) => Self::SaturatingAdd(carrier),
            MachineSemanticKind::SaturatingSubtract(carrier) => Self::SaturatingSubtract(carrier),
            MachineSemanticKind::SaturatingDivide(carrier) => Self::SaturatingDivide(carrier),
            MachineSemanticKind::SaturatingRemainder(carrier) => Self::SaturatingRemainder(carrier),
            MachineSemanticKind::ExactSubtractI64Immediate => Self::ExactSubtractI64Immediate,
            MachineSemanticKind::ConditionalBranchNonZero => Self::ConditionalBranchNonZero,
            MachineSemanticKind::ReturnScalar => Self::ReturnScalar,
            MachineSemanticKind::ReturnUnit => Self::ReturnUnit,
            MachineSemanticKind::CompareI64 => Self::CompareI64,
            MachineSemanticKind::CompareI64Immediate => Self::CompareI64Immediate,
            MachineSemanticKind::ConditionalBranchU64LessThan => Self::ConditionalBranchU64LessThan,
            MachineSemanticKind::ConditionalBranchI64LessThan => Self::ConditionalBranchI64LessThan,
            MachineSemanticKind::CallScalar => Self::CallScalar,
            MachineSemanticKind::Jump => Self::Jump,
            MachineSemanticKind::ZeroExtendU8 => Self::ZeroExtendU8,
            MachineSemanticKind::ZeroExtendU32 => Self::ZeroExtendU32,
            MachineSemanticKind::ZeroExtendU16 => Self::ZeroExtendU16,
            MachineSemanticKind::SignExtendI8 => Self::SignExtendI8,
            MachineSemanticKind::SignExtendI16 => Self::SignExtendI16,
            MachineSemanticKind::SignExtendI32 => Self::SignExtendI32,
            MachineSemanticKind::MaterializeBooleanEqual => Self::MaterializeBooleanEqual,
            MachineSemanticKind::MaterializeBooleanU64LessThan => {
                Self::MaterializeBooleanU64LessThan
            }
            MachineSemanticKind::MaterializeBooleanI64LessThan => {
                Self::MaterializeBooleanI64LessThan
            }
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual => {
                Self::MaterializeBooleanU64LessOrEqual
            }
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
                Self::MaterializeBooleanI64LessOrEqual
            }

            MachineSemanticKind::Load64 => Self::Load64,
            MachineSemanticKind::Store64 => Self::Store64,
            MachineSemanticKind::FrameAddress => Self::FrameAddress,
            MachineSemanticKind::ImageAddress => Self::ImageAddress,
            MachineSemanticKind::ScanByteSequence => Self::ScanByteSequence,
            MachineSemanticKind::LoadEncodedScalar => Self::LoadEncodedScalar,
            MachineSemanticKind::StoreEncodedScalar => Self::StoreEncodedScalar,
            MachineSemanticKind::CompareByteSequences => Self::CompareByteSequences,
            MachineSemanticKind::ZeroBytes => Self::ZeroBytes,
            MachineSemanticKind::CallUnit => Self::CallUnit,
            MachineSemanticKind::NormalizedForeignCall => Self::NormalizedForeignCall,
            MachineSemanticKind::ExactMultiplyI64 => Self::ExactMultiplyI64,
            MachineSemanticKind::ExactRemainderU64 => Self::ExactRemainderU64,
            MachineSemanticKind::WrappingSubtractI64 => Self::WrappingSubtractI64,
            MachineSemanticKind::WrappingMultiplyI64 => Self::WrappingMultiplyI64,
            MachineSemanticKind::WrappingDivideI64 => Self::WrappingDivideI64,
            MachineSemanticKind::BitwiseOrI64 => Self::BitwiseOrI64,
            MachineSemanticKind::BitwiseNotI64 => Self::BitwiseNotI64,
            MachineSemanticKind::WrappingShiftLeftI64 => Self::WrappingShiftLeftI64,
            MachineSemanticKind::WrappingShiftRightI64 => Self::WrappingShiftRightI64,
            MachineSemanticKind::WrappingShiftRightU64 => Self::WrappingShiftRightU64,
            MachineSemanticKind::ExactShiftLeftI64 => Self::ExactShiftLeftI64,
            MachineSemanticKind::ExactShiftRightI64 => Self::ExactShiftRightI64,
            MachineSemanticKind::ExactShiftRightU64 => Self::ExactShiftRightU64,
            MachineSemanticKind::ExactDivideI64 => Self::ExactDivideI64,
            MachineSemanticKind::ExactRemainderI64 => Self::ExactRemainderI64,
            MachineSemanticKind::SaturatingMultiply(carrier) => Self::SaturatingMultiply(carrier),
            MachineSemanticKind::TrappingInteger(form) => Self::TrappingInteger(form),
            MachineSemanticKind::PortWrite => Self::PortWrite,
            MachineSemanticKind::AtomicFence => Self::AtomicFence,
            MachineSemanticKind::AtomicAccess(form) => Self::AtomicAccess(form),
            MachineSemanticKind::AccessorScalarAccess(form) => Self::AccessorScalarAccess(form),
            MachineSemanticKind::FloatResultPolicy(form) => Self::FloatResultPolicy(form),
            MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                Self::NearestIeeeFloatArithmetic(form)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineAlternativeKey {
    pub family: MachineAlternativeFamily,
    pub variant: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeApplicability {
    Always,
    ResultAliasesOperand {
        result: u16,
        operand: u16,
    },
    ResultAliasesOperandAndDistinctFromOperand {
        result: u16,
        aliased_operand: u16,
        distinct_operand: u16,
    },
    ResultAliasesOperands {
        result: u16,
        left: u16,
        right: u16,
    },
    ResultDistinctFromOperands {
        result: u16,
        left: u16,
        right: u16,
    },
    AtLeastOneOperandDoesNotAliasView {
        left: u16,
        right: u16,
        excluded_view: RegisterViewId,
    },
    OptimizationSelected {
        optimization: optimization_core::Optimization,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineMemoryEffect {
    PrivateCallbackMaterializationV1,
    CatalogFenceV1,
    AtomicFenceV1,
    TemporaryStackStorageV1,
    AtomicPointerV1,
    CopyBytesV1,
    ScanByteSequenceV1,
    ReadEncodedScalarV1,
    WriteEncodedScalarV1,
    CompareByteSequencesV1,
    ZeroBytesV1,
    HostedReadByteV1,
    HostedWriteByteV1,
    TaskRegisterActivationV1,
    InstalledInterruptCpuCheckV1,
    InstalledInterruptTaskCommitV1,
    InstalledInterruptTablePublicationV1,

    SegmentDescriptorReloadV1,
    WritePointerV1,
    NoneV1,
    ReadPointerV1,
    WriteFrameStorageV1,
    ReadFrameStorageV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineTrapBehavior {
    ExplicitCrashV1,
    TrappingIntegerV1,
    HostedExitReturnedV1,
    HostedReadFailureV1,
    HostedWriteFailureV1,
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineBarrier {
    ExternalEffect,
    None,
    ControlFlow,
    Call,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCallEffect {
    NoneV1,
    SupervisorNormalReturnV1,
    InternalNormalReturnV1 { pre_call_stack_alignment: u16 },
    InternalNoNormalV1 { pre_call_stack_alignment: u16 },
    ExternalNormalReturnV1 { pre_call_stack_alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCleanupEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineSizeKnowledge {
    ExactBytes(u16),
    EncoderResolved {
        minimum_bytes: u16,
        maximum_bytes: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineLatencyKnowledge {
    StableBaselineUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEncodedEffects {
    pub external_operand_reads: Vec<u16>,
    pub external_operand_writes: Vec<u16>,
    pub implicit_unit_uses: Vec<register_model::RegisterUnitId>,
    pub implicit_unit_defs: Vec<register_model::RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<register_model::RegisterUnitId>,
    pub memory: MachineEncodedMemoryEffect,
    pub stack: MachineEncodedStackEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub control: MachineEncodedControlEffect,
}

impl MachineEncodedEffects {
    pub fn catalog_fence_v1() -> Self {
        Self {
            memory: MachineEncodedMemoryEffect::CatalogFenceV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    pub fn atomic_fence_v1() -> Self {
        Self {
            memory: MachineEncodedMemoryEffect::AtomicFenceV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    pub fn fallthrough_v1(
        external_operand_reads: Vec<u16>,
        external_operand_writes: Vec<u16>,
    ) -> Self {
        Self {
            external_operand_reads,
            external_operand_writes,
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: Vec::new(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        }
    }

    pub fn supervisor_call_v1(row: &register_model::RegisterInstructionConstraint) -> Self {
        Self {
            external_operand_reads: row.operand_reads(),
            external_operand_writes: row.operand_writes(),
            implicit_unit_uses: row.implicit_uses.clone(),
            implicit_unit_defs: row.implicit_defs.clone(),
            implicit_unit_clobbers: row.clobbers.clone(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::SupervisorCallV1,
        }
    }

    pub fn crash_v1(program_counter: &[register_model::RegisterUnitId]) -> Self {
        Self {
            implicit_unit_uses: program_counter.to_vec(),
            implicit_unit_defs: program_counter.to_vec(),
            trap: MachineEncodedTrapBehavior::ExplicitCrashV1,
            control: MachineEncodedControlEffect::CrashV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    pub fn privileged_stop_v1(
        program_counter: &[register_model::RegisterUnitId],
        control_flags: &[register_model::RegisterUnitId],
    ) -> Self {
        Self {
            implicit_unit_uses: program_counter.to_vec(),
            implicit_unit_defs: program_counter.to_vec(),
            implicit_unit_clobbers: control_flags.to_vec(),
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::PrivilegedStopV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    pub fn float_result_policy_v1(
        inputs: u16,
        clobbers: Vec<register_model::RegisterUnitId>,
        trapping: bool,
    ) -> Self {
        let mut effects = Self {
            implicit_unit_clobbers: clobbers,
            ..Self::fallthrough_v1((0..inputs).collect(), vec![inputs])
        };
        if trapping {
            effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
            effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        }
        effects
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedMemoryEffect {
    PrivateCallbackMaterializationV1,
    CatalogFenceV1,
    ReadTableFieldV1 {
        pointer_operand: u16,
        byte_offset: u32,
    },
    AtomicFenceV1,
    TemporaryStackStorageV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    AtomicPointerV1 {
        pointer_operand: u16,
        form: AtomicForm,
    },
    CopyBytesV1 {
        source_pointer_operand: u16,
        destination_pointer_operand: u16,
        count_operand: u16,
    },
    ScanByteSequenceV1 {
        base_pointer_operand: u16,
        count_operand: u16,
    },
    ReadEncodedScalarV1 {
        pointer_operand: u16,
        codec_operand: u16,
    },
    WriteEncodedScalarV1 {
        pointer_operand: u16,
        codec_operand: u16,
    },
    CompareByteSequencesV1 {
        left_base_operand: u16,
        left_count_operand: u16,
        right_base_operand: u16,
        right_count_operand: u16,
    },
    ZeroBytesV1 {
        destination_pointer_operand: u16,
        count_operand: u16,
    },
    HostedReadByteV1 {
        stack_pointer: RegisterViewId,
    },
    HostedWriteByteV1 {
        stack_pointer: RegisterViewId,
    },
    TaskRegisterActivationV1 {
        table_pointer_operand: u16,
        table_byte_count_operand: u16,
        task_pointer_operand: u16,
        selector_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    InstalledInterruptCpuCheckV1 {
        table_pointer_operand: u16,
        task_pointer_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    InstalledInterruptTaskCommitV1 {
        table_pointer_operand: u16,
        task_pointer_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    InstalledInterruptTablePublicationV1 {
        table_pointer_operand: u16,
        scratch_pointer_operand: u16,
    },
    SegmentDescriptorReloadV1 {
        table_pointer_operand: u16,
        table_byte_count_operand: u16,
        selector_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    WritePointerV1 {
        pointer_operand: u16,
    },
    WriteFixedPointerV1 {
        pointer_operand: u16,
        byte_count: u16,
    },
    ReadIndexedPointerV1 {
        pointer_operand: u16,
        index_operand: u16,
        byte_count: u16,
    },
    NoneV1,
    ReadPointerV1 {
        pointer_operand: u16,
        byte_count: u16,
    },
    WriteFrameStorageV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    ReadFrameStorageV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    ReadActivationStackV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    WriteReturnAddressBelowStackPointerV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedStackEffect {
    TemporaryBytesV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    UnchangedV1,
    PopBytesV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    CallReturnAddressLifecycleV1 {
        stack_pointer: RegisterViewId,
        return_address_byte_count: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedTrapBehavior {
    ExplicitCrashV1,
    TrappingIntegerV1,
    HostedExitReturnedV1,
    HostedReadFailureV1,
    HostedWriteFailureV1,
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedControlEffect {
    CrashV1,
    FallThroughOrTrapV1,
    FallThroughOrStopV1,
    PrivilegedStopV1,
    HostedExitOrTrapV1,
    HostedReadReturnOrTrapV1,
    HostedWriteReturnOrTrapV1,
    FallThroughV1,
    AtomicRetryUntilCommitV1,
    ConditionalRelativeBranchV1,
    ReturnFromActivationStackV1,
    ReturnIndirectRegisterV1 { target: RegisterViewId },
    DirectRelativeCallV1,
    IndirectExternalCallV1,
    IndirectTableCallOrTrapV1,
    SupervisorCallV1,
    UnconditionalRelativeBranchV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineAlternative {
    pub key: MachineAlternativeKey,
    pub applicability: MachineAlternativeApplicability,
    pub size: MachineSizeKnowledge,
    pub latency: MachineLatencyKnowledge,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffectDeclaration {
    pub semantic: MachineSemanticKind,
    pub constraint: RegisterConstraintKey,
    pub memory: MachineMemoryEffect,
    pub trap: MachineTrapBehavior,
    pub barrier: MachineBarrier,
    pub call: MachineCallEffect,
    pub cleanup: MachineCleanupEffect,
    pub alternatives: Vec<MachineAlternative>,
}

impl MachineEffectDeclaration {
    pub fn call(
        semantic: MachineSemanticKind,
        constraint: RegisterConstraintKey,
        pre_call_stack_alignment: u16,
        byte_count: u16,
        foreign_supervisor_bytes: Option<u16>,
        encoded: MachineEncodedEffects,
    ) -> Self {
        let mut declaration = Self {
            semantic,
            constraint,
            memory: MachineMemoryEffect::NoneV1,
            trap: MachineTrapBehavior::NeverV1,
            barrier: MachineBarrier::Call,
            call: if semantic == MachineSemanticKind::InvokeNoNormal {
                MachineCallEffect::InternalNoNormalV1 {
                    pre_call_stack_alignment,
                }
            } else {
                MachineCallEffect::InternalNormalReturnV1 {
                    pre_call_stack_alignment,
                }
            },
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(byte_count),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            }],
        };
        match foreign_supervisor_bytes {
            Some(supervisor_byte_count) if constraint.family == SystemCall => {
                declaration.call = MachineCallEffect::SupervisorNormalReturnV1;
                for alternative in &mut declaration.alternatives {
                    alternative.size = MachineSizeKnowledge::ExactBytes(supervisor_byte_count);
                    alternative.encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                    alternative.encoded.stack = MachineEncodedStackEffect::UnchangedV1;
                    alternative.encoded.control = MachineEncodedControlEffect::SupervisorCallV1;
                }
            }
            Some(_) => {
                declaration.call = MachineCallEffect::ExternalNormalReturnV1 {
                    pre_call_stack_alignment,
                };
            }
            None => {}
        }
        declaration
    }

    pub fn single(
        semantic: MachineSemanticKind,
        constraint: RegisterConstraintKey,
        memory: MachineMemoryEffect,
        trap: MachineTrapBehavior,
        barrier: MachineBarrier,
        family: MachineAlternativeFamily,
        size: MachineSizeKnowledge,
        encoded: MachineEncodedEffects,
    ) -> Self {
        Self {
            semantic,
            constraint,
            memory,
            trap,
            barrier,
            call: MachineCallEffect::NoneV1,
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey { family, variant: 0 },
                applicability: MachineAlternativeApplicability::Always,
                size,
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffectCatalog {
    pub target: NativeTarget,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub selected_keys: SelectedConstraintKeys,
    pub declarations: Vec<MachineEffectDeclaration>,
}

impl MachineEffectCatalog {
    pub fn declaration(
        &self,
        constraint: RegisterConstraintKey,
    ) -> Option<&MachineEffectDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.constraint == constraint)
    }

    pub fn declaration_by_semantic(
        &self,
        semantic: MachineSemanticKind,
    ) -> Option<&MachineEffectDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.semantic == semantic)
    }

    pub fn declaration_by_semantic_mut(
        &mut self,
        semantic: MachineSemanticKind,
    ) -> Option<&mut MachineEffectDeclaration> {
        self.declarations
            .iter_mut()
            .find(|declaration| declaration.semantic == semantic)
    }
}

/// Identity-bound machine effects prepared by their trusted ISA producer or checked on receipt.
/// Preparation alone does not claim independent validation of the ISA description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMachineEffectCatalog {
    catalog: MachineEffectCatalog,
    identity: MachineEffectCatalogIdentity,
}

impl PreparedMachineEffectCatalog {
    pub const fn catalog(&self) -> &MachineEffectCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> MachineEffectCatalogIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    RegisterConstraintRootMismatch,
    DuplicateSelectedConstraintKey,
    NonCanonicalDeclarations,
    DeclarationRosterMismatch,
    UnknownConstraint(MachineSemanticKind),
    NonCanonicalAlternatives(MachineSemanticKind),
    EmptyAlternatives(MachineSemanticKind),
    AlternativeFamilyMismatch(MachineSemanticKind),
    InvalidAlternativeApplicability(MachineSemanticKind),
    InvalidEncodedEffects(MachineSemanticKind),
    InvalidSizeKnowledge(MachineSemanticKind),
    BarrierMismatch(MachineSemanticKind),
}

impl std::fmt::Display for MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid machine-effect catalog: {self:?}")
    }
}

impl std::error::Error for MachineEffectCatalogValidationError {}

/// Reuses the compiler-owned ISA catalog and its identity without replaying its producer.
/// The builder must be the target's trusted ISA owner, not a decoder of supplied data.
/// Separately supplied catalogs enter through `validate_isa_machine_effect_catalog`.
pub fn canonical_machine_effect_catalog<E>(
    target: NativeTarget,
    constraints: &PreparedRegisterConstraintCatalog,
    build: impl FnOnce(
        NativeTarget,
        &PreparedRegisterConstraintCatalog,
    ) -> Result<MachineEffectCatalog, E>,
) -> Result<std::sync::Arc<PreparedMachineEffectCatalog>, E> {
    type Canonical = (
        NativeTarget,
        RegisterConstraintCatalogIdentity,
        std::sync::Arc<PreparedMachineEffectCatalog>,
    );
    static CANONICAL: std::sync::OnceLock<std::sync::Mutex<Vec<Canonical>>> =
        std::sync::OnceLock::new();
    let cache = CANONICAL.get_or_init(Default::default);
    let constraint_identity = constraints.identity();
    if let Some(catalog) = cache.lock().ok().and_then(|cached| {
        cached
            .iter()
            .find(|(seen_target, seen_identity, _)| {
                *seen_target == target && *seen_identity == constraint_identity
            })
            .map(|(_, _, catalog)| catalog.clone())
    }) {
        return Ok(catalog);
    }
    let catalog = build(target, constraints)?;
    let identity = machine_effect_catalog_identity(&catalog);
    let catalog = std::sync::Arc::new(PreparedMachineEffectCatalog { catalog, identity });
    if let Ok(mut cached) = cache.lock() {
        // Concurrent cold preparations may finish together; publish one retained
        // value so every caller borrows the same catalog rather than a second copy.
        if let Some((_, _, retained)) = cached.iter().find(|(seen_target, seen_identity, _)| {
            *seen_target == target && *seen_identity == constraint_identity
        }) {
            return Ok(retained.clone());
        }
        cached.push((target, constraint_identity, catalog.clone()));
    }
    Ok(catalog)
}

#[derive(Debug)]
pub enum IsaCatalogValidation<E> {
    ArchitectureMismatch,
    Structural(MachineEffectCatalogValidationError),
    SemanticMismatch,
    Builder(E),
}

pub fn validate_isa_machine_effect_catalog<E>(
    architecture: target::Architecture,
    target: NativeTarget,
    constraints: &PreparedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
    build: impl Fn(NativeTarget, &PreparedRegisterConstraintCatalog) -> Result<MachineEffectCatalog, E>,
) -> Result<PreparedMachineEffectCatalog, IsaCatalogValidation<E>> {
    if target.architecture != architecture || constraints.architecture() != architecture {
        return Err(IsaCatalogValidation::ArchitectureMismatch);
    }
    let canonical = canonical_machine_effect_catalog(target, constraints, build)
        .map_err(IsaCatalogValidation::Builder)?;
    let validated = validate_machine_effect_catalog(constraints, catalog)
        .map_err(IsaCatalogValidation::Structural)?;
    if validated.catalog() != canonical.catalog() {
        return Err(IsaCatalogValidation::SemanticMismatch);
    }
    Ok(validated)
}

#[macro_export]
macro_rules! isa_machine_effect_catalog_plumbing {
    ($error:ident, $validate:ident, $architecture:expr, $label:literal, $build:path) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $error {
            TargetArchitectureMismatch,
            UnsupportedTargetAbi,
            Structural($crate::MachineEffectCatalogValidationError),
            TargetSemanticMismatch,
        }

        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "invalid {} machine effects: {:?}", $label, self)
            }
        }

        impl std::error::Error for $error {}

        pub fn $validate(
            target: ::target::NativeTarget,
            constraints: &::register_model::PreparedRegisterConstraintCatalog,
            catalog: $crate::MachineEffectCatalog,
        ) -> Result<$crate::PreparedMachineEffectCatalog, $error> {
            $crate::validate_isa_machine_effect_catalog(
                $architecture,
                target,
                constraints,
                catalog,
                $build,
            )
            .map_err(|failure| match failure {
                $crate::IsaCatalogValidation::ArchitectureMismatch => {
                    $error::TargetArchitectureMismatch
                }
                $crate::IsaCatalogValidation::Structural(error) => $error::Structural(error),
                $crate::IsaCatalogValidation::SemanticMismatch => $error::TargetSemanticMismatch,
                $crate::IsaCatalogValidation::Builder(error) => error,
            })
        }
    };
}
