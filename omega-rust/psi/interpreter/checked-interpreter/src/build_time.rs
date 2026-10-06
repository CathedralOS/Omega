//! Source-bound selected operations and the remaining tree engine value bridge.
//!
//! Immutable result data is independently owned by build-time-values. These
//! private conversions still belong to the old executor and are deleted with
//! it; no storage or execution algorithm is relocated into the data owner.

use crate::value::{Cell, Value};
use build_time_values::BuildTimeValue;
use std::collections::BTreeMap;

/// Materialize into an interpreter value tree using the evaluator's
/// allocation authority. Build-time arguments never alias compiler state.
pub(crate) fn materialize_build_time_value<E>(
    argument: BuildTimeValue,
    allocate: &impl Fn(Value) -> Result<Cell, E>,
    allocate_text: &impl Fn(Vec<u8>) -> Result<Value, E>,
) -> Result<Value, E> {
    match argument {
        BuildTimeValue::Unit => Ok(Value::Unit),
        BuildTimeValue::Int(value) => Ok(Value::Int(value)),
        BuildTimeValue::Bool(value) => Ok(Value::Bool(value)),
        BuildTimeValue::Float(value) => Ok(Value::Float(value)),
        BuildTimeValue::UndeterminedNaN(format) => Ok(Value::UndeterminedNaN(format)),
        BuildTimeValue::Text(bytes) => allocate_text(bytes),
        BuildTimeValue::Struct { type_name, fields } => {
            let mut cells: BTreeMap<crate::value::ValueName, Cell> = BTreeMap::new();
            for (name, value) in fields {
                let value = materialize_build_time_value(value, allocate, allocate_text)?;
                cells.insert(name.into(), allocate(value)?);
            }
            Ok(Value::Struct {
                type_symbol: symbols::SymbolHandle::invalid(),
                type_name: type_name.into(),
                fields: cells,
            })
        }
        BuildTimeValue::Case { variant, payload } => Ok(Value::Enum {
            // The build-time boundary carries no type identity (same as
            // the Struct arm above); tag-ordinal resolution falls back to
            // the name-global scan for these values.
            type_symbol: symbols::SymbolHandle::invalid(),
            variant_name: variant.into(),
            payload: payload
                .into_iter()
                .map(|(name, value)| {
                    let value = materialize_build_time_value(value, allocate, allocate_text)?;
                    Ok((name.into(), allocate(value)?))
                })
                .collect::<Result<_, E>>()?,
        }),
        BuildTimeValue::Array(elements) => Ok(Value::Array(
            elements
                .into_iter()
                .map(|element| {
                    let value = materialize_build_time_value(element, allocate, allocate_text)?;
                    allocate(value)
                })
                .collect::<Result<_, E>>()?,
        )),
    }
}

/// Deep-read an interpreter value back out. References deref (a build-time
/// result is a VALUE; nothing it points at survives the evaluation).
pub(crate) fn observe_build_time_value(value: &Value) -> BuildTimeValue {
    match value {
        Value::Unit => BuildTimeValue::Unit,
        Value::Int(value) => BuildTimeValue::Int(*value),
        Value::Bool(value) => BuildTimeValue::Bool(*value),
        Value::Float(value) => BuildTimeValue::Float(*value),
        Value::UndeterminedNaN(format) => BuildTimeValue::UndeterminedNaN(*format),
        Value::Str(bytes) => BuildTimeValue::Text(bytes.borrow().to_vec()),
        Value::Struct {
            type_name, fields, ..
        } => BuildTimeValue::Struct {
            type_name: type_name.to_string(),
            fields: fields
                .iter()
                .map(|(name, cell)| (name.to_string(), observe_build_time_value(&cell.borrow())))
                .collect(),
        },
        Value::Enum {
            variant_name,
            payload,
            ..
        } => BuildTimeValue::Case {
            variant: variant_name.to_string(),
            payload: payload
                .iter()
                .map(|(name, cell)| (name.to_string(), observe_build_time_value(&cell.borrow())))
                .collect(),
        },
        Value::Array(elements) => BuildTimeValue::Array(
            elements
                .iter()
                .map(|cell| observe_build_time_value(&cell.borrow()))
                .collect(),
        ),
        Value::Ref(cell) => observe_build_time_value(&cell.borrow()),
    }
}

/// One supplied semantic execution, tied to the exact checked operator use.
/// The caller owns selected-provider validation; this interpreter never selects
/// a provider from a token or declaration name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBuildTimeBinaryOperator {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement: symbols::SymbolHandle,
    pub operation: typed_trees::expression::BinaryOperator,
    pub operands: [typed_trees::expression::ExpressionHandle; 2],
    pub format: numerics::literals::FloatFormat,
    pub policy: checked_trees::CheckedArithmeticPolicyAdapter,
    pub provider: checked_trees::CheckedProviderPlanCommitment,
}

impl SelectedBuildTimeBinaryOperator {
    /// Prepare semantic execution after Omega has selected the exact intrinsic.
    /// Psi owns the operation, carriers, and authored operand order; this does
    /// not select a provider or replace independent admission of the row.
    pub fn prepare(
        program: &typed_trees::TypedTrees,
        expression: typed_trees::expression::ExpressionHandle,
        origin: checked_trees::CheckedValueOrigin,
        requirement: symbols::SymbolHandle,
        policy: checked_trees::CheckedArithmeticPolicyAdapter,
        provider: checked_trees::CheckedProviderPlanCommitment,
    ) -> Result<Self, String> {
        let operator = typed_trees::operator::declaration_by_symbol(program, requirement)
            .ok_or("selected build-time Float requirement disappeared")?;
        let (operation, format) =
            typed_trees::operator::primitive_float_binary_semantics(program, operator)
                .ok_or("selected build-time operation has no matching sealed Float meaning")?;
        let operands = match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Binary(binary) => [binary.left, binary.right],
            typed_trees::expression::ExpressionNode::Call(call) => {
                let [left, right] = program.expression_table.expression_handles(call.arguments)
                else {
                    return Err("selected build-time Float call has wrong arity".into());
                };
                [*left, *right]
            }
            _ => return Err("selected build-time Float expression has no binary operands".into()),
        };
        Ok(Self {
            expression,
            origin,
            requirement,
            operation,
            operands,
            format,
            policy,
            provider,
        })
    }

    /// Inspect only this supplied selection, never unrelated operator overloads.
    pub fn has_crash_contract(&self, program: &typed_trees::TypedTrees) -> bool {
        program
            .operators()
            .iter()
            .chain(program.machine_token_bindings())
            .filter(|operator| operator.symbol == self.requirement)
            .any(|operator| {
                program
                    .signature_contracts
                    .span_or_empty(operator.contracts)
                    .iter()
                    .any(|contract| {
                        matches!(
                            contract.kind,
                            typed_trees::signature::SignatureContractKind::Crashes { .. }
                        )
                    })
            })
            || program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == self.requirement)
                .any(|machine| {
                    program
                        .machine_contracts(machine)
                        .iter()
                        .chain(
                            program
                                .machine_states(machine)
                                .iter()
                                .flat_map(|state| program.state_contracts(state)),
                        )
                        .any(|contract| {
                            matches!(
                                contract.kind,
                                typed_trees::signature::SignatureContractKind::Crashes { .. }
                            )
                        })
                })
    }
}
