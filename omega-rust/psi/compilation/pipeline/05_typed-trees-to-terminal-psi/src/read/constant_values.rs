//! Constant values arrive as materialized source data, never as a source
//! checking verdict. Source completion joins computed values to their real
//! invocation receipts before freezing. Reading independently checks the
//! carrier, each scalar leaf of a scalar, array or record value, and the
//! exact retained initializer entry; that entry is
//! an ordinary subject machine whose body the lowered checker also judges.

use language_semantics::const_value::DecodedCanonicalConstValue;
use semantic_vocabulary::{IntegerSign, IntegerValue, MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::constant::ConstDeclaration;

use super::Program;
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(super) fn subject_constant(
        &mut self,
        id: MachineId,
        constant: &ConstDeclaration,
    ) -> Checked<()> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let carrier = self.value_type(id, constant.declared_type)?;
        let qualification = match carrier {
            ValueType::Scalar(_, qualification) | ValueType::Record(_, qualification, _) => {
                qualification
            }
            ValueType::Rigid(_) => {
                return Err(unserved("constant requires its pure snapshot materializer"));
            }
        };
        if !qualification.is_none() {
            return Err(unserved(
                "qualified constant requires its declared value obligations",
            ));
        }
        let value = typed_trees::canonical_const_declaration_value(self.typed, constant.symbol)
            .and_then(|value| value.decode_encoding())
            .ok_or_else(|| unserved("constant has no evaluated canonical value"))?;
        if !self
            .typed
            .expression_table
            .expression_is_valid(constant.materialized_initializer)
        {
            return Err(unserved("constant has no materialized initializer"));
        }
        let materialized =
            self.expression(id, &[], constant.materialized_initializer, Some(carrier))?;
        if materialized.result != Some(carrier)
            || !self.constant_agrees(id, &value, &materialized)?
        {
            return Err(unserved(
                "constant encoding and materialized carrier differ",
            ));
        }
        if constant.evaluation_entry.is_valid() {
            if !self
                .typed
                .tables
                .machines
                .is_valid(constant.evaluation_entry)
            {
                return Err(unserved("constant lost its retained initializer entry"));
            }
            let retained = self.typed.tables.machines.get(constant.evaluation_entry);
            let actual = self
                .typed
                .machine_by_symbol(retained.symbol)
                .filter(|actual| *actual == retained)
                .ok_or_else(|| unserved("constant initializer is not its actual source machine"))?;
            let [state] = self.typed.machine_states(actual) else {
                return Err(unserved("constant initializer has no unique entry"));
            };
            let [typed_trees::statement::StatementNode::Transition(transition)] =
                self.typed.statement_table.statements(state.statement_nodes)
            else {
                return Err(unserved(
                    "constant initializer has no exact value transition",
                ));
            };
            if state.return_type != constant.declared_type
                || !self.typed.state_parameters(state).is_empty()
                || self
                    .typed
                    .statement_table
                    .transition_target(transition.target)
                    != &typed_trees::statement::TransitionTargetNode::Value(
                        constant.evaluation_expression,
                    )
            {
                return Err(unserved(
                    "constant initializer differs from its retained computation",
                ));
            }
        }
        Ok(())
    }

    /// Whether the materialized initializer, as the reader typed it, is the
    /// evaluated canonical value leaf by leaf: each integer has the
    /// encoding's carrier and value, each array element and each record
    /// field agrees in turn, and a record field is matched to the value's
    /// field by the declared field symbol at its position. A data type with
    /// `requires`, a qualified leaf or a sum value has a domain obligation or
    /// a case this reading does not judge, so it is refused, not compared.
    fn constant_agrees(
        &self,
        id: MachineId,
        value: &DecodedCanonicalConstValue,
        materialized: &Expression,
    ) -> Checked<bool> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let qualification = match materialized.result {
            Some(ValueType::Scalar(_, qualification) | ValueType::Record(_, qualification, _)) => {
                qualification
            }
            _ => return Ok(false),
        };
        if !qualification.is_none() {
            return Err(unserved(
                "qualified constant requires its declared value obligations",
            ));
        }
        Ok(match (value, &materialized.kind) {
            (
                DecodedCanonicalConstValue::Integer { type_name, value },
                ExpressionKind::Integer(current),
            ) => {
                let Some(ValueType::Scalar(ScalarType::Integer(integer), _)) = materialized.result
                else {
                    return Ok(false);
                };
                let Some(bits) = integer.bits() else {
                    return Ok(false);
                };
                let expected = format!(
                    "{}{}",
                    match integer.sign() {
                        IntegerSign::Signed => 'i',
                        IntegerSign::Unsigned => 'u',
                    },
                    bits
                );
                !integer.is_address()
                    && *type_name == expected
                    && match current {
                        IntegerValue::Signed(current) => *value == *current,
                        IntegerValue::Unsigned(current) => {
                            i128::try_from(*current).ok() == Some(*value)
                        }
                    }
            }
            (DecodedCanonicalConstValue::Boolean(value), ExpressionKind::Boolean(current)) => {
                *value == *current
            }
            (
                DecodedCanonicalConstValue::Array { values, .. },
                ExpressionKind::Array { elements },
            ) => {
                if values.len() != elements.len() {
                    return Ok(false);
                }
                for (value, element) in values.iter().zip(elements) {
                    if !self.constant_agrees(id, value, element)? {
                        return Ok(false);
                    }
                }
                true
            }
            (
                DecodedCanonicalConstValue::Record { fields, .. },
                ExpressionKind::Record { fields: operands },
            ) => {
                let Some(ValueType::Record(record, ..)) = materialized.result else {
                    return Ok(false);
                };
                let (Some(row), Some(source)) =
                    (self.records.get(record), self.record_sources.get(record))
                else {
                    return Ok(false);
                };
                if !row.requires.is_empty() {
                    return Err(unserved(
                        "constant of data with requires needs its default-domain proof",
                    ));
                }
                if operands.len() != fields.len() || source.fields.len() != fields.len() {
                    return Ok(false);
                }
                for (position, operand) in operands {
                    let Some(symbol) = source.fields.get(*position) else {
                        return Ok(false);
                    };
                    let name = self.typed.symbols.name(*symbol);
                    let mut named = fields.iter().filter(|(field, _)| field.as_str() == name);
                    let (Some((_, value)), None) = (named.next(), named.next()) else {
                        return Ok(false);
                    };
                    if !self.constant_agrees(id, value, operand)? {
                        return Ok(false);
                    }
                }
                true
            }
            (DecodedCanonicalConstValue::Variant { .. }, _) => {
                return Err(unserved(
                    "constant of a sum type requires its case materializer",
                ));
            }
            _ => false,
        })
    }
}
