#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::BTreeMap;

use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalTargetFunction, TerminalTargetOperation,
    TerminalTargetOperationPlan,
};
use psi_core::{IntegerType, IntegerValue, MachineId, ScalarType, ValueId};

pub fn lower_to_target_operations(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalTargetOperationPlan {
        terminal_psi: plan.terminal_psi,
        target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(lower_function)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(
    function: &TerminalAbstractFunction,
) -> Result<TerminalTargetFunction, LoweringError> {
    let mut values = BTreeMap::new();
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;

    for operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
                };
                if !integer_type.admits(*value) {
                    return Err(LoweringError::IntegerConstantOutsideType(*result));
                }
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *integer_type,
                        value: *value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::Jump {
                psi_edge, bindings, ..
            } => {
                let transferred = bindings
                    .iter()
                    .map(|binding| {
                        let value = values
                            .get(&binding.argument)
                            .copied()
                            .ok_or(LoweringError::UnknownValue(binding.argument))?;
                        if binding.scalar_type != value.scalar_type() {
                            return Err(LoweringError::ValueTypeMismatch(binding.parameter));
                        }
                        Ok((binding.parameter, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (parameter, value) in transferred {
                    insert_value(&mut values, parameter, value)?;
                }
                provenance.edges.push(*psi_edge);
            }
            TerminalAbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
            } => {
                let returned_value = values
                    .get(value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*value))?;
                if *scalar_type != returned_value.scalar_type() {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                }
                provenance.edges.push(*psi_edge);
                returned = Some(match returned_value {
                    KnownScalar::Boolean(boolean) => {
                        TerminalTargetOperation::ReturnBooleanImmediate {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            value: boolean,
                        }
                    }
                    KnownScalar::Integer {
                        scalar_type,
                        value: integer,
                    } => TerminalTargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                });
            }
        }
    }

    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}

fn insert_value(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    id: ValueId,
    value: KnownScalar,
) -> Result<(), LoweringError> {
    if values.insert(id, value).is_some() {
        return Err(LoweringError::DuplicateValue(id));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownScalar {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

impl KnownScalar {
    const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EntryFunctionMissing(MachineId),
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    DuplicateValue(ValueId),
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
    };
    use psi_core::{BlockId, EdgeId};
    use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

    #[test]
    fn refuses_a_return_whose_value_was_never_materialized() {
        let machine = MachineId::new(1).expect("machine");
        let unknown = ValueId::new(1).expect("unknown value");
        let result = ValueId::new(2).expect("result");
        let i32_type = IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(1).expect("block"),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    result,
                    value: unknown,
                    scalar_type: ScalarType::Integer(i32_type),
                }],
            }],
        };

        assert_eq!(
            lower_to_target_operations(&plan, NativeTarget::linux_x64()),
            Err(LoweringError::UnknownValue(unknown))
        );
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            semantic_version: SemanticVersion::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }
}
