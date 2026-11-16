//! The checker records a float conversion's integer result as a total relation.
//! A failed conversion makes the relation false; it does not form a partial
//! integer term. Calls import this guarantee only after the converter's actual
//! preconditions and normal completion. Exact casts owe a stronger source-only
//! predicate before making that call.

use numerics::float_projection::{FloatProjectionContractIdentity, FloatProjectionOperation};
use numerics::float_semantics::{
    FloatSemanticContractIdentity, FloatSemanticOperation, FloatSemanticValueKind,
};
use numerics::integer_carrier::FixedInteger;

use crate::{IeeeFloatFormat, IntegerSign, IntegerType, PropositionError, ScalarTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatIntegerConversionError {
    Source(Box<PropositionError>),
    Result(Box<PropositionError>),
    Signature(&'static str),
}

impl FloatIntegerConversionError {
    /// Clause binders that carry static diagnostics still identify which
    /// scalar failed. Proposition/codec errors retain the full typed cause.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Source(_) => "integer conversion source is not a well-formed scalar term",
            Self::Result(_) => "integer conversion result is not a well-formed scalar term",
            Self::Signature(detail) => detail,
        }
    }
}

impl From<&'static str> for FloatIntegerConversionError {
    fn from(detail: &'static str) -> Self {
        Self::Signature(detail)
    }
}

/// The exact graph of a sealed integer-valued FloatSemantics operation over
/// the meaning of one actual IEEE scalar. This is not a FloatMeaning value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct FloatIntegerConversion {
    pub source: ScalarTerm,
    pub result: ScalarTerm,
    pub projection: FloatProjectionContractIdentity,
    pub operation: FloatSemanticContractIdentity,
}

impl FloatIntegerConversion {
    pub fn new(
        source: ScalarTerm,
        result: ScalarTerm,
        operation: FloatSemanticContractIdentity,
    ) -> Result<Self, FloatIntegerConversionError> {
        let projection = projection_contract(&source)?;
        let relation = Self {
            source,
            result,
            projection,
            operation,
        };
        relation.validate()?;
        Ok(relation)
    }

    /// Check the complete catalog signature. The consumer separately binds
    /// source/result identities and proves the callable's entry obligations.
    pub fn validate(&self) -> Result<(), FloatIntegerConversionError> {
        self.source
            .validate()
            .map_err(|error| FloatIntegerConversionError::Source(Box::new(error)))?;
        self.result
            .validate()
            .map_err(|error| FloatIntegerConversionError::Result(Box::new(error)))?;
        if self.projection != projection_contract(&self.source)? {
            return Err("integer conversion has a different float projection contract".into());
        }
        let target = self
            .result
            .scalar_type()
            .as_integer()
            .ok_or("integer conversion result has no integer carrier")?;
        let carrier = fixed_integer_type(target)?;
        let operation = FloatSemanticOperation::for_contract_identity(&self.operation)
            .ok_or("integer conversion has no exact semantic catalog contract")?;
        if operation.parameters() != [FloatSemanticValueKind::Meaning]
            || operation.result != FloatSemanticValueKind::Integer(carrier)
        {
            return Err("integer conversion differs from its complete catalog signature".into());
        }
        Ok(())
    }
}

/// The authored toward-zero converter requires a finite input whose truncated
/// value fits the destination. Exact cast eligibility implies every row, but
/// remains a separate, stronger proposition.
pub fn float_integer_preconditions(
    source: ScalarTerm,
    target: FixedInteger,
) -> Result<Vec<crate::Proposition>, &'static str> {
    use crate::{IeeeFloatComparisonKind, IeeeFloatValue, Proposition};
    use numerics::bignum::BigInt;
    use numerics::float_semantics::{FloatFormat, FloatSemantics};
    let format = source
        .scalar_type()
        .as_ieee_float()
        .ok_or("integer conversion source has no IEEE format")?;
    let native_format = match format {
        IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
        IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
    };
    let literal = |value: &BigInt| {
        let value = FloatSemantics::from_integer(native_format, value);
        ScalarTerm::IeeeFloatLiteral(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(value.to_f32().to_bits()),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(value.to_f64().to_bits()),
        })
    };
    let exponent = target.width() - u64::from(target.is_signed());
    let upper = BigInt::from_u64(1).shl_bits(
        usize::try_from(exponent)
            .map_err(|_| "integer conversion width exceeds its materialization budget")?,
    );
    let minimum = if target.is_signed() {
        upper.negate()
    } else {
        BigInt::zero()
    };
    let exclusive = minimum.sub(&BigInt::from_u64(1));
    let lower_literal = literal(&exclusive);
    let (kind, lower) = if target.is_signed() && lower_literal == literal(&minimum) {
        (IeeeFloatComparisonKind::LessOrEqual, literal(&minimum))
    } else {
        (IeeeFloatComparisonKind::LessThan, lower_literal)
    };
    let predicate = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    // The three rows are in canonical wire order: equality, upper <, lower.
    Ok(vec![
        predicate(
            IeeeFloatComparisonKind::Equal,
            source.clone(),
            source.clone(),
        ),
        predicate(
            IeeeFloatComparisonKind::LessThan,
            source.clone(),
            literal(&upper),
        ),
        predicate(kind, lower, source.clone()),
    ])
}

fn projection_contract(
    source: &ScalarTerm,
) -> Result<FloatProjectionContractIdentity, &'static str> {
    let format = source
        .scalar_type()
        .as_ieee_float()
        .ok_or("integer conversion source has no IEEE float format")?;
    Ok(match format {
        IeeeFloatFormat::Binary32 => FloatProjectionOperation::Meaning32,
        IeeeFloatFormat::Binary64 => FloatProjectionOperation::Meaning64,
    }
    .contract_identity())
}

/// Preserve signedness and the complete closed width; never round an integer
/// endpoint into a float to decide whether an integer result is representable.
pub fn fixed_integer_type(target: IntegerType) -> Result<FixedInteger, &'static str> {
    if target.is_address() {
        return Err("float conversion cannot infer an address carrier");
    }
    let bits = target
        .bits()
        .ok_or("float conversion needs a closed integer width")?;
    Ok(match target.sign() {
        IntegerSign::Signed => FixedInteger::signed(u64::from(bits)),
        IntegerSign::Unsigned => FixedInteger::unsigned(u64::from(bits)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScalarType, ValueId};

    #[test]
    fn integer_result_relation_authenticates_projection_operation_and_carrier() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let source = ScalarTerm::value(
            ValueId::new(1).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        );
        let result = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(integer));
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "to_i32",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(FixedInteger::signed(32)),
        )
        .unwrap();
        let relation =
            FloatIntegerConversion::new(source, result, operation.contract_identity()).unwrap();
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        let malformed = FloatIntegerConversion {
            source: relation.source.clone(),
            result: ScalarTerm::Integer {
                scalar_type: i8_type,
                value: crate::IntegerValue::Signed(128),
            },
            projection: relation.projection.clone(),
            operation: FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                "to_i8",
                &[FloatSemanticValueKind::Meaning],
                FloatSemanticValueKind::Integer(FixedInteger::signed(8)),
            )
            .unwrap()
            .contract_identity(),
        };
        assert!(matches!(
            malformed.validate(),
            Err(FloatIntegerConversionError::Result(_))
        ));
        let mut changed = relation.clone();
        changed.operation.commitment[0] ^= 1;
        assert!(changed.validate().is_err());
        let mut changed = relation.clone();
        changed.source = ScalarTerm::value(
            ValueId::new(1).unwrap(),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        );
        assert!(changed.validate().is_err());
        let mut changed = relation;
        changed.result = ScalarTerm::value(
            ValueId::new(2).unwrap(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        );
        assert!(changed.validate().is_err());
    }
}
