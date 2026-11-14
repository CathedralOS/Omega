/*
Realizing a Terminal Psi boundary machine on a native target requires binding the selected provider or compiler builtin to its exact execution coordinates, normalized foreign or system-call mechanism, and conventional structural output geometry. We organize boundary execution bindings (`execution`), concrete builtin and foreign-call realizations (`realizations`), boundary invocation arguments, and conventional sum layouts for float classification and atomic compare-exchange outcomes under `boundary`.

Keeping conventional `FloatClass` and single-attempt atomic outcome layout derivations alongside boundary settlement arguments ensures that admission and instruction selection derive identical sum geometry from structural declarations without relying on source type names.
*/

use calling_conventions::{ConventionalSumLayout, MachineRegister, ValueShape};
use semantic_vocabulary::{BoundaryMachineId, IntegerValue, OperationId, ScalarType, ValueId};
use terminal_psi::{StructuralArgument, StructuralTypeDeclaration};

mod execution;
pub use execution::{
    BoundaryExecutionBinding, CompilerBuiltinExecution, IeeeFloatArithmeticOperation,
    IeeeFloatBinaryOperation, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection,
    IeeeFloatToIntegerDomain, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
mod realizations;
pub use realizations::{
    AccessorScalarAccessRealization, BoundaryRealization, BoundarySettlementRealization,
    ClaimCompletionOnlyRealization, DirectPortReadU8Realization, ForeignCallOperands,
    HostedExitProcessI32Realization, HostedReadByteRealization, HostedWriteByteI32Realization,
    LinuxWriteLineRealization, MetadataOnlyPortRealization, NearestIeeeFloatArithmeticRealization,
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryScalarArgument {
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub immediate: IntegerValue,
    pub destination: MachineRegister,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgument {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementBinding {
    pub boundary: BoundaryMachineId,
    pub execution: BoundaryExecutionBinding,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalCompletionBinding {
    Boundary(BoundarySettlementBinding),
    Checked {
        callee: semantic_vocabulary::MachineId,
        origin: crate::NativeCallOrigin,
        call_plan: calling_conventions::CallPlan,
    },
}

pub fn conventional_float_class_layout(
    declaration: &StructuralTypeDeclaration,
) -> Option<ConventionalSumLayout> {
    if !declaration.is_float_class_shape() {
        return None;
    }
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[
            vec![],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
            vec![ValueShape::integer(1, 1)],
        ],
    )
    .ok()?;
    if layout.shape != ValueShape::integer(8, 4)
        || layout.tag_byte_offset != 0
        || layout.tag_shape != ValueShape::integer(4, 4)
        || layout.cases[1..].iter().any(|case| {
            !matches!(
                case.fields.as_slice(),
                [field] if field.byte_offset == 4 && field.shape == ValueShape::integer(1, 1)
            )
        })
    {
        return None;
    }
    Some(layout)
}

pub fn conventional_atomic_once_layout(
    declaration: &StructuralTypeDeclaration,
    integer: semantic_vocabulary::IntegerType,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> Option<ConventionalSumLayout> {
    let integer = semantic_vocabulary::ClosedIntegerType::try_from(integer).ok()?;
    conventional_atomic_once_layout_closed(declaration, integer, custody)
}

pub fn conventional_atomic_once_layout_closed(
    declaration: &StructuralTypeDeclaration,
    integer: semantic_vocabulary::ClosedIntegerType,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> Option<ConventionalSumLayout> {
    terminal_psi::types::atomic_once_outcome_cases(
        declaration,
        ScalarType::Integer(integer.as_checking()),
        custody,
    )?;
    if integer.is_address() {
        return None;
    }
    let field = ValueShape::for_closed_integer(integer)?;
    let cases = custody
        .outcome_identity
        .cases()
        .iter()
        .map(|case| {
            if case.payload.is_some() {
                vec![field]
            } else {
                Vec::new()
            }
        })
        .collect::<Vec<_>>();
    calling_conventions::evaluate_conventional_sum_layout(&[], &cases).ok()
}

#[cfg(test)]
mod tests {
    use super::conventional_float_class_layout;
    use calling_conventions::ValueShape;
    use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };

    fn declaration() -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Sum {
                cases: (0..5)
                    .map(|index| StructuralCaseDeclaration {
                        id: StructuralCaseId::new(index + 1).unwrap(),
                        identity: terminal_psi::TerminalIdentityIndex(10 + index as u32),
                        fields: if index == 0 {
                            vec![]
                        } else {
                            vec![StructuralFieldDeclaration {
                                inline_carrier: None,
                                id: StructuralFieldId::new(index).unwrap(),
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                                storage: None,
                            }]
                        },
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn conventional_float_class_geometry_is_derived_without_identity_selection() {
        let layout = conventional_float_class_layout(&declaration()).unwrap();
        assert_eq!(layout.shape, ValueShape::integer(8, 4));
        assert_eq!(layout.tag_byte_offset, 0);
        assert_eq!(layout.tag_shape, ValueShape::integer(4, 4));
        assert!(layout.cases[0].fields.is_empty());
        for case in &layout.cases[1..] {
            assert_eq!(case.fields[0].byte_offset, 4);
            assert_eq!(case.fields[0].shape, ValueShape::integer(1, 1));
        }
    }

    #[test]
    fn conventional_float_class_geometry_refuses_malformed_payloads() {
        for mutation in 0..7 {
            let mut declaration = declaration();
            let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                unreachable!();
            };
            match mutation {
                0 => {
                    cases.pop();
                }
                1 => {
                    cases.push(cases[4].clone());
                }
                2 => {
                    cases[0].fields = cases[1].fields.clone();
                }
                3 => {
                    cases[1].fields.clear();
                }
                4 => {
                    cases[1].fields[0].field_type = StructuralFieldType::Scalar(
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
                    );
                }
                5 => {
                    cases[1].fields[0].relevance = BindingRelevance::Erased;
                }
                6 => {
                    let second = cases[2].fields[0].clone();
                    cases[1].fields.push(second);
                }
                _ => unreachable!(),
            }
            assert!(
                conventional_float_class_layout(&declaration).is_none(),
                "mutation {mutation}"
            );
        }
        let mut declaration = declaration();
        declaration.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        assert!(conventional_float_class_layout(&declaration).is_none());
    }
}
