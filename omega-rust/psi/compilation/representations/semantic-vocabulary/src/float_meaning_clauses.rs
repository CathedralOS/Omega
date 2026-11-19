//! The checker records what a float operation returns before a caller supplies
//! its actual values. Calls substitute those values into this equation. Flat,
//! connected rows retain exact projection and semantic catalog commitments;
//! declaration templates do not invent runtime values or projection owners.

use std::collections::{BTreeMap, BTreeSet};

use numerics::float_projection::{FloatProjectionContractIdentity, FloatProjectionOperation};
use numerics::float_semantics::{
    FloatSemanticContractIdentity, FloatSemanticOperation, FloatSemanticValueKind,
};

use crate::{IeeeFloatFormat, ScalarTerm, ScalarType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct FloatMeaningSlot(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum FloatMeaningClauseOperand {
    Format(IeeeFloatFormat),
    Meaning(FloatMeaningSlot),
    Integer(ScalarTerm),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum FloatMeaningClauseValue {
    Projection {
        source: ScalarTerm,
        contract: FloatProjectionContractIdentity,
    },
    Application {
        contract: FloatSemanticContractIdentity,
        format: IeeeFloatFormat,
        operands: Vec<FloatMeaningClauseOperand>,
    },
}

impl FloatMeaningClauseValue {
    pub fn format(&self) -> Option<IeeeFloatFormat> {
        match self {
            Self::Projection { source, .. } => source.scalar_type().as_ieee_float(),
            Self::Application { format, .. } => Some(*format),
        }
    }
}

/// Structural equality of FloatMeaning, separate from IEEE comparison and raw
/// representation identity. Rows are unique and topological; every row belongs
/// to the equality. Slot numbers have meaning only inside this template.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct FloatMeaningClause {
    pub values: Vec<FloatMeaningClauseValue>,
    pub left: FloatMeaningSlot,
    pub right: FloatMeaningSlot,
}

impl FloatMeaningClause {
    fn projection(source: ScalarTerm) -> Result<FloatMeaningClauseValue, &'static str> {
        let operation = match source.scalar_type() {
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => FloatProjectionOperation::Meaning32,
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => FloatProjectionOperation::Meaning64,
            _ => return Err("float meaning equation has a non-float source"),
        };
        Ok(FloatMeaningClauseValue::Projection {
            source,
            contract: operation.contract_identity(),
        })
    }

    /// The source is unchanged by conversion to `through` and back to its own
    /// format. This states representability before a runtime result exists;
    /// neither constructing the equation nor naming it proves it.
    pub fn conversion_roundtrip(
        source: ScalarTerm,
        through: IeeeFloatFormat,
    ) -> Result<Self, &'static str> {
        use FloatSemanticValueKind::{Format, Meaning};
        let format = source
            .scalar_type()
            .as_ieee_float()
            .ok_or("float conversion roundtrip has a non-float source")?;
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "convert",
            &[Format, Meaning],
            Meaning,
        )
        .ok_or("float conversion roundtrip has no exact semantic catalog row")?;
        let contract = operation.contract_identity();
        let application = |format, operand| FloatMeaningClauseValue::Application {
            contract,
            format,
            operands: vec![
                FloatMeaningClauseOperand::Format(format),
                FloatMeaningClauseOperand::Meaning(FloatMeaningSlot(operand)),
            ],
        };
        let clause = Self {
            values: vec![
                Self::projection(source)?,
                application(through, 0),
                application(format, 1),
            ],
            left: FloatMeaningSlot(0),
            right: FloatMeaningSlot(2),
        };
        clause.intern_sources::<&'static str>(|source| Ok(source.clone()))
    }

    /// The result projection equals one complete semantic application. This
    /// builds the same typed equation for unary, binary and ternary meaning
    /// operations; it does not select or authorize runtime execution.
    pub fn result_equation(
        result: ScalarTerm,
        contract: FloatSemanticContractIdentity,
        format: IeeeFloatFormat,
        arguments: impl IntoIterator<Item = ScalarTerm>,
    ) -> Result<Self, &'static str> {
        let operation = FloatSemanticOperation::for_contract_identity(&contract)
            .ok_or("float result equation has no exact semantic catalog contract")?;
        if operation.result != FloatSemanticValueKind::Meaning {
            return Err("float result equation has a non-meaning catalog result");
        }
        let mut values = vec![Self::projection(result)?];
        let mut arguments = arguments.into_iter();
        let mut operands = Vec::new();
        for kind in operation.parameters() {
            operands.push(match kind {
                FloatSemanticValueKind::Format => FloatMeaningClauseOperand::Format(format),
                FloatSemanticValueKind::Meaning => {
                    let source = arguments
                        .next()
                        .ok_or("float result equation is missing an argument")?;
                    let slot = FloatMeaningSlot(
                        u32::try_from(values.len()).map_err(|_| "too many float meaning values")?,
                    );
                    values.push(Self::projection(source)?);
                    FloatMeaningClauseOperand::Meaning(slot)
                }
                FloatSemanticValueKind::Integer(_) => FloatMeaningClauseOperand::Integer(
                    arguments
                        .next()
                        .ok_or("float result equation is missing an integer argument")?,
                ),
                _ => return Err("float result equation has an unsupported argument kind"),
            });
        }
        if arguments.next().is_some() {
            return Err("float result equation has extra arguments");
        }
        let right = FloatMeaningSlot(
            u32::try_from(values.len()).map_err(|_| "too many float meaning values")?,
        );
        values.push(FloatMeaningClauseValue::Application {
            contract,
            format,
            operands,
        });
        // Initial templates may repeat a source (for example an authored a+a).
        // Canonicalization uses the same simultaneous map as call substitution.
        let clause = Self {
            values,
            left: FloatMeaningSlot(0),
            right,
        };
        clause.intern_sources::<&'static str>(|source| Ok(source.clone()))
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        use FloatMeaningClauseOperand as Operand;
        use FloatMeaningClauseValue as Value;
        use FloatSemanticValueKind as Kind;

        let mut unique = BTreeSet::new();
        for (position, value) in self.values.iter().enumerate() {
            if !unique.insert(value) {
                return Err("float meaning template repeats a value");
            }
            match value {
                Value::Projection { source, contract } => {
                    source
                        .validate()
                        .map_err(|_| "float meaning projection has an invalid scalar source")?;
                    let operation = match source.scalar_type() {
                        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => {
                            FloatProjectionOperation::Meaning32
                        }
                        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => {
                            FloatProjectionOperation::Meaning64
                        }
                        _ => return Err("float meaning projection source is not an IEEE float"),
                    };
                    if *contract != operation.contract_identity() {
                        return Err("float meaning projection lost its exact catalog contract");
                    }
                }
                Value::Application {
                    contract,
                    format,
                    operands,
                } => {
                    let operation = FloatSemanticOperation::for_contract_identity(contract).ok_or(
                        "float meaning application has no exact semantic catalog contract",
                    )?;
                    if operation.result != Kind::Meaning
                        || operation.parameters().len() != operands.len()
                    {
                        return Err("float meaning application changes its catalog signature");
                    }
                    let has_format = operation.parameters().contains(&Kind::Format);
                    for (kind, operand) in operation.parameters().iter().zip(operands) {
                        match (kind, operand) {
                            (Kind::Format, Operand::Format(actual)) if actual == format => {}
                            (Kind::Meaning, Operand::Meaning(slot))
                                if (slot.0 as usize) < position =>
                            {
                                if !has_format
                                    && self.values[slot.0 as usize].format() != Some(*format)
                                {
                                    return Err(
                                        "float meaning application changes its operand format",
                                    );
                                }
                            }
                            (Kind::Integer(carrier), Operand::Integer(source)) => {
                                source.validate().map_err(
                                    |_| "float meaning application has an invalid integer operand",
                                )?;
                                if crate::IntegerType::from_fixed(*carrier).map(ScalarType::Integer)
                                    != Some(source.scalar_type())
                                {
                                    return Err(
                                        "float meaning application changes its integer carrier",
                                    );
                                }
                            }
                            _ => {
                                return Err(
                                    "float meaning application changes its ordered operands",
                                );
                            }
                        }
                    }
                }
            }
        }
        let left = self
            .values
            .get(self.left.0 as usize)
            .ok_or("float meaning equality has no left value")?;
        let right = self
            .values
            .get(self.right.0 as usize)
            .ok_or("float meaning equality has no right value")?;
        if left.format() != right.format() {
            return Err("float meaning equality mixes permanent formats");
        }
        if self.left > self.right {
            return Err("float meaning equality operands are not canonical");
        }
        let mut reached = vec![false; self.values.len()];
        let mut pending = vec![self.left, self.right];
        while let Some(slot) = pending.pop() {
            let index = slot.0 as usize;
            if std::mem::replace(&mut reached[index], true) {
                continue;
            }
            if let Value::Application { operands, .. } = &self.values[index] {
                pending.extend(operands.iter().filter_map(|operand| match operand {
                    Operand::Meaning(slot) => Some(*slot),
                    Operand::Format(_) | Operand::Integer(_) => None,
                }));
            }
        }
        if reached.iter().any(|seen| !seen) {
            return Err("float meaning template has a disconnected value");
        }
        Ok(())
    }

    /// Resolve each source exactly once. Re-intern rows after substitution so
    /// aliased actuals share a slot, without recursively substituting an actual
    /// again as though it were another formal.
    pub fn map_sources<E: From<&'static str>>(
        &self,
        source: impl FnMut(&ScalarTerm) -> Result<ScalarTerm, E>,
    ) -> Result<Self, E> {
        self.validate().map_err(E::from)?;
        self.intern_sources(source)
    }

    // Source rows are topological before this walk: either validate() checked
    // them or result_equation() built them. Re-interning then validates the
    // complete result, including source formats and connected equality roots.
    fn intern_sources<E: From<&'static str>>(
        &self,
        mut source: impl FnMut(&ScalarTerm) -> Result<ScalarTerm, E>,
    ) -> Result<Self, E> {
        let mut values = Vec::with_capacity(self.values.len());
        let mut positions = BTreeMap::new();
        let mut slots = Vec::with_capacity(self.values.len());
        for value in &self.values {
            let value = match value {
                FloatMeaningClauseValue::Projection {
                    source: input,
                    contract,
                } => FloatMeaningClauseValue::Projection {
                    source: source(input)?,
                    contract: *contract,
                },
                FloatMeaningClauseValue::Application {
                    contract,
                    format,
                    operands,
                } => FloatMeaningClauseValue::Application {
                    contract: *contract,
                    format: *format,
                    operands: operands
                        .iter()
                        .map(|operand| {
                            Ok(match operand {
                                FloatMeaningClauseOperand::Format(format) => {
                                    FloatMeaningClauseOperand::Format(*format)
                                }
                                FloatMeaningClauseOperand::Meaning(slot) => {
                                    FloatMeaningClauseOperand::Meaning(slots[slot.0 as usize])
                                }
                                FloatMeaningClauseOperand::Integer(input) => {
                                    FloatMeaningClauseOperand::Integer(source(input)?)
                                }
                            })
                        })
                        .collect::<Result<Vec<_>, E>>()?,
                },
            };
            let next = FloatMeaningSlot(
                u32::try_from(values.len())
                    .map_err(|_| E::from("too many float meaning values"))?,
            );
            let slot = *positions.entry(value.clone()).or_insert_with(|| {
                values.push(value);
                next
            });
            slots.push(slot);
        }
        let mut left = slots[self.left.0 as usize];
        let mut right = slots[self.right.0 as usize];
        if left > right {
            std::mem::swap(&mut left, &mut right);
        }
        let mapped = Self {
            values,
            left,
            right,
        };
        mapped.validate().map_err(E::from)?;
        Ok(mapped)
    }

    /// Legacy renaming APIs cannot return an error. Preserve that interface by
    /// producing an unmistakably malformed equation when a source map fails;
    /// receiving validation must reject it instead of using an unchanged fact.
    pub fn map_sources_or_invalid(
        &self,
        mut source: impl FnMut(&ScalarTerm) -> ScalarTerm,
    ) -> Self {
        match self.map_sources::<&'static str>(|input| Ok(source(input))) {
            Ok(mapped) => mapped,
            Err(_) => Self {
                values: Vec::new(),
                left: FloatMeaningSlot(0),
                right: FloatMeaningSlot(0),
            },
        }
    }

    pub fn sources(&self) -> impl DoubleEndedIterator<Item = &ScalarTerm> {
        self.values.iter().flat_map(|value| {
            let (projection, operands): (Option<&ScalarTerm>, &[_]) = match value {
                FloatMeaningClauseValue::Projection { source, .. } => (Some(source), &[]),
                FloatMeaningClauseValue::Application { operands, .. } => (None, operands),
            };
            projection
                .into_iter()
                .chain(operands.iter().filter_map(|operand| match operand {
                    FloatMeaningClauseOperand::Integer(source) => Some(source),
                    _ => None,
                }))
        })
    }

    /// Only IEEE projections need the disjoint float-producer correspondence.
    /// Integer operands are bound by ordinary scalar argument substitution.
    pub fn projection_sources(&self) -> impl Iterator<Item = &ScalarTerm> {
        self.values.iter().filter_map(|value| match value {
            FloatMeaningClauseValue::Projection { source, .. } => Some(source),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Proposition, PropositionContext, ValueId};

    fn equation() -> FloatMeaningClause {
        use FloatSemanticValueKind::{Format, Meaning};
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "subtract",
            &[Format, Meaning, Meaning],
            Meaning,
        )
        .unwrap();
        let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        FloatMeaningClause::result_equation(
            value(3),
            operation.contract_identity(),
            IeeeFloatFormat::Binary32,
            [value(1), value(2)],
        )
        .unwrap()
    }

    #[test]
    fn integer_inputs_keep_their_exact_catalog_carrier_during_substitution() {
        use FloatSemanticValueKind::{Format, Integer, Meaning};
        use numerics::integer_carrier::FixedInteger;
        let carrier = FixedInteger::signed(32);
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[Format, Integer(carrier)],
            Meaning,
        )
        .unwrap();
        let integer = ScalarType::Integer(crate::IntegerType::from_fixed(carrier).unwrap());
        let result = ScalarTerm::value(
            ValueId::new(2).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        );
        let source = ScalarTerm::value(ValueId::new(1).unwrap(), integer);
        let clause = FloatMeaningClause::result_equation(
            result,
            operation.contract_identity(),
            IeeeFloatFormat::Binary64,
            [source],
        )
        .unwrap();
        let mapped = clause
            .map_sources::<&'static str>(|source| {
                Ok(match source {
                    ScalarTerm::Value { id, scalar_type } => {
                        ScalarTerm::value(ValueId::new(id.get() + 10).unwrap(), *scalar_type)
                    }
                    _ => source.clone(),
                })
            })
            .unwrap();
        assert!(mapped.sources().any(|source| matches!(source, ScalarTerm::Value { id, scalar_type } if id.get()==11 && *scalar_type==integer)));
        assert_eq!(mapped.projection_sources().count(), 1);
        assert!(
            clause
                .map_sources::<&'static str>(|source| Ok(if source.scalar_type() == integer {
                    ScalarTerm::value(
                        ValueId::new(1).unwrap(),
                        ScalarType::Integer(
                            crate::IntegerType::from_fixed(FixedInteger::unsigned(32)).unwrap(),
                        ),
                    )
                } else {
                    source.clone()
                }))
                .is_err(),
            "equal widths do not make signed and unsigned inputs interchangeable"
        );
    }

    #[test]
    fn float_meaning_equation_remapping_retains_all_sources_and_poisoned_input_rejects() {
        let original = Proposition::FloatMeaningEqual(equation());
        let mut renamed = original.clone();
        renamed.remap_value_ids(|id| ValueId::new(id.get() + 10).unwrap());
        let mut ids = Vec::new();
        renamed.visit_value_ids(|id| ids.push(id.get()));
        assert_eq!(ids, vec![13, 11, 12]);
        let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let context = PropositionContext::from_value_types(
            (11..=13).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
        )
        .unwrap();
        context.validate(&renamed).unwrap();
        let mut broken = equation();
        let FloatMeaningClauseValue::Projection { contract, .. } = &mut broken.values[0] else {
            unreachable!()
        };
        contract.commitment[0] ^= 1;
        let mut broken = Proposition::FloatMeaningEqual(broken);
        broken.remap_value_ids(|id| ValueId::new(id.get() + 10).unwrap());
        assert!(broken.validate().is_err());
        assert!(context.validate(&broken).is_err());
    }
}
