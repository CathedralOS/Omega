//! This reader checks each materialized constant's type, case and values.
//! Source completion links computed values to their invocation receipts
//! before it freezes the typed source. The lowered checker judges the exact
//! retained initializer as an ordinary machine.

use language_semantics::const_value::DecodedCanonicalConstValue;
use semantic_vocabulary::{IntegerValue, MachineId, ScalarType};
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
        if self.typed.stored_value_multiplicity(constant.declared_type)
            != language_semantics::Multiplicity::Unrestricted
        {
            return Err(crate::reject(
                id,
                crate::SourceOrigin::of(constant.initializer_source_span),
                "a constant's type must permit copying",
            ));
        }
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
    /// encoding's carrier and value. An aggregate must name its declared
    /// type and any selected case, then each field agrees with the value
    /// named by its declared symbol. Data with `requires` and qualified
    /// leaves still need their checked domain obligations.
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
                DecodedCanonicalConstValue::Integer {
                    integer_type,
                    value,
                },
                ExpressionKind::Integer(current),
            ) => {
                let Some(ValueType::Scalar(ScalarType::Integer(integer), _)) = materialized.result
                else {
                    return Ok(false);
                };
                let Some(bits) = integer.bits() else {
                    return Ok(false);
                };
                let expected =
                    numerics::integer_carrier::FixedInteger::new(integer.sign(), u64::from(bits));
                !integer.is_address()
                    && matches!(integer_type, numerics::literals::LandedIntegerType::Fixed(carrier) if Some(*carrier) == expected)
                    && match current {
                        IntegerValue::Signed(current) => {
                            *value == numerics::bignum::BigInt::from(*current)
                        }
                        IntegerValue::Unsigned(current) => {
                            *value == numerics::bignum::BigInt::from(*current)
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
                DecodedCanonicalConstValue::Record { type_name, fields },
                ExpressionKind::Record { fields: operands },
            ) => self.constant_aggregate_agrees(
                id,
                materialized.result,
                type_name,
                None,
                fields,
                operands,
            )?,
            (
                DecodedCanonicalConstValue::Variant {
                    type_name,
                    case_name,
                    fields,
                },
                ExpressionKind::Case {
                    case,
                    fields: operands,
                },
            ) => self.constant_aggregate_agrees(
                id,
                materialized.result,
                type_name,
                Some((*case, case_name)),
                fields,
                operands,
            )?,
            _ => false,
        })
    }

    fn constant_aggregate_agrees(
        &self,
        id: MachineId,
        result: Option<ValueType>,
        type_name: &str,
        case: Option<(usize, &str)>,
        fields: &[(String, DecodedCanonicalConstValue)],
        operands: &[(usize, Expression)],
    ) -> Checked<bool> {
        let Some(ValueType::Record(record, ..)) = result else {
            return Ok(false);
        };
        let (Some(row), Some(source)) = (self.records.get(record), self.record_sources.get(record))
        else {
            return Ok(false);
        };
        let Some(declaration) = self.typed.data_definition_by_symbol(source.declaration) else {
            return Ok(false);
        };
        if declaration.name.as_str() != type_name {
            return Ok(false);
        }
        if !row.requires.is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Declaration(
                    "constant of data with requires needs its default-domain proof",
                ),
            ));
        }
        let declared_fields = if let Some((position, name)) = case {
            let Some((symbol, payload)) = source.cases.get(position) else {
                return Ok(false);
            };
            if !symbol.is_valid() || self.typed.symbols.name(*symbol) != name {
                return Ok(false);
            }
            payload
        } else {
            &source.fields
        };
        if operands.len() != fields.len() || declared_fields.len() != fields.len() {
            return Ok(false);
        }
        for (position, operand) in operands {
            let Some(symbol) = declared_fields
                .get(*position)
                .filter(|symbol| symbol.is_valid())
            else {
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
        Ok(true)
    }
}
