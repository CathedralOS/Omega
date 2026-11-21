use omega_psi::{
    BlockTerminator, FloatOperation, FloatSemantics, IeeeFloatBits, IeeeFloatFormat, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter,
    PlaceProjection, ProjectedPlace, ReturnEdge, ScalarLiteral, ScalarType, TypeShape, ValueId,
};

/// A scalar this reader can compute with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatScalar {
    Bits(IeeeFloatBits),
    Bool(bool),
    Integer(i128),
}

/// Evaluate a one-block body of reads, constants, float operations and
/// exact casts, using only Psi. Each input binds a parameter position: a
/// scalar parameter, or the referent of a shared scalar reference. The format
/// of every float operation comes from its operands' Psi types, and only the
/// nearest-even contract is understood. An exact cast whose value does not
/// survive the change of carrier has no result here: the checker would have
/// to refuse it. `None` means the body uses something else.
pub fn evaluate_float_body(
    package: &LoweredPackage,
    body: &MachineBody,
    inputs: &[FloatScalar],
) -> Option<FloatScalar> {
    let [block] = body.blocks.as_slice() else { return None };
    let scalar_type = |value: ValueId| match package.types.get(body.values.get(value.index())?.ty)? {
        TypeShape::PrimitiveScalar(scalar) => Some(*scalar),
        _ => None,
    };
    let mut known: Vec<Option<FloatScalar>> = vec![None; body.values.len()];
    if body.parameters.len() != inputs.len() {
        return None;
    }
    let mut referents = Vec::new();
    for (parameter, input) in body.parameters.iter().zip(inputs) {
        match parameter {
            Parameter::Scalar(value) => known[value.index()] = Some(fits(scalar_type(*value)?, *input)?),
            Parameter::Structural(place) => match package.types.get(body.places.get(place.index())?.ty)? {
                TypeShape::ScalarReference { referent, .. } => referents.push((*place, fits(*referent, *input)?)),
                _ => return None,
            },
        }
    }
    for op in body.operations.get(block.operations.clone())? {
        let OperationResult::Scalar(result) = op.result else { return None };
        let result_type = scalar_type(result)?;
        let value = match &op.kind {
            OperationKind::ProjectedScalarRead {
                source: ProjectedPlace { root, path },
            } if path.as_slice() == [PlaceProjection::Referent] => referents.iter().find(|(place, _)| place == root)?.1,
            OperationKind::Constant(ScalarLiteral::IeeeFloat(bits)) => fits(result_type, FloatScalar::Bits(*bits))?,
            OperationKind::Constant(ScalarLiteral::Integer(value)) => fits(result_type, FloatScalar::Integer(i128::try_from(value).ok()?))?,
            OperationKind::Float {
                operation,
                semantics: FloatSemantics::Ieee754NearestEvenV1,
                operands,
            } => {
                let format = match scalar_type(*operands.first()?)? {
                    ScalarType::IeeeFloat(format) => format,
                    _ => return None,
                };
                if operands
                    .iter()
                    .any(|operand| scalar_type(*operand) != Some(ScalarType::IeeeFloat(format)))
                {
                    return None;
                }
                let values = operands
                    .iter()
                    .map(|operand| match known[operand.index()]? {
                        FloatScalar::Bits(bits) => Some(bits),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                fits(result_type, apply(*operation, format, &values)?)?
            }
            OperationKind::ExactNumericCast { operand } => exact_cast(known[operand.index()]?, result_type)?,
            _ => return None,
        };
        known[result.index()] = Some(value);
    }
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = &block.terminator
    else {
        return None;
    };
    known[returned.index()]
}

fn fits(
    ty: ScalarType,
    value: FloatScalar,
) -> Option<FloatScalar> {
    let ok = match (ty, value) {
        (ScalarType::IeeeFloat(format), FloatScalar::Bits(bits)) => bits.format() == format,
        (ScalarType::Boolean, FloatScalar::Bool(_)) => true,
        (ScalarType::Integer { signed, width }, FloatScalar::Integer(value)) => {
            let (low, high) = if signed {
                (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
            } else {
                (0, (1i128 << width) - 1)
            };
            width <= 64 && (low..=high).contains(&value)
        }
        _ => false,
    };
    ok.then_some(value)
}

// Host `f32` and `f64` operations are IEEE binary32 and binary64 with
// round-to-nearest-even, the contract this reader accepted above.
pub(super) fn apply(
    operation: FloatOperation,
    format: IeeeFloatFormat,
    operands: &[IeeeFloatBits],
) -> Option<FloatScalar> {
    use FloatOperation::*;
    macro_rules! run {
        ($float:ty, $variant:ident) => {{
            let values = operands
                .iter()
                .map(|bits| match bits {
                    IeeeFloatBits::$variant(bits) => Some(<$float>::from_bits(*bits)),
                    _ => None,
                })
                .collect::<Option<Vec<$float>>>()?;
            let bits = |value: $float| Some(FloatScalar::Bits(IeeeFloatBits::$variant(value.to_bits())));
            let truth = |value: bool| Some(FloatScalar::Bool(value));
            match (operation, values.as_slice()) {
                (Negate, [a]) => bits(-a),
                (Add, [a, b]) => bits(a + b),
                (Subtract, [a, b]) => bits(a - b),
                (Multiply, [a, b]) => bits(a * b),
                (Divide, [a, b]) => bits(a / b),
                (Equal, [a, b]) => truth(a == b),
                (NotEqual, [a, b]) => truth(a != b),
                (Less, [a, b]) => truth(a < b),
                (LessEqual, [a, b]) => truth(a <= b),
                (Greater, [a, b]) => truth(a > b),
                (GreaterEqual, [a, b]) => truth(a >= b),
                _ => None,
            }
        }};
    }
    match format {
        IeeeFloatFormat::Binary32 => run!(f32, Binary32),
        IeeeFloatFormat::Binary64 => run!(f64, Binary64),
    }
}

/// The value must survive unchanged; there is no rounding or truncation.
fn exact_cast(
    value: FloatScalar,
    to: ScalarType,
) -> Option<FloatScalar> {
    match (value, to) {
        (FloatScalar::Integer(integer), ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => {
            let landed = integer as f32;
            (landed.is_finite() && landed as i128 == integer).then_some(FloatScalar::Bits(IeeeFloatBits::Binary32(landed.to_bits())))
        }
        (FloatScalar::Integer(integer), ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => {
            let landed = integer as f64;
            (landed.is_finite() && landed as i128 == integer).then_some(FloatScalar::Bits(IeeeFloatBits::Binary64(landed.to_bits())))
        }
        (FloatScalar::Integer(_), ScalarType::Integer { .. }) => fits(to, value),
        _ => None,
    }
}
