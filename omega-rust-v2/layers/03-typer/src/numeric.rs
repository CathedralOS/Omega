use super::{Access, TypeError, ValueType};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use omega_lexer::{SourceSpan, TokenKind};
use omega_resolver::IndexedSource;

pub(super) fn literal(
    f: &IndexedSource<'_>,
    n: usize,
) -> Result<(ValueType, Option<ValueType>), TypeError> {
    let token = f.nodes[n].leaves[0];
    let start = token.span.get_start_offset();
    let (value, suffix) = match token.kind {
        TokenKind::Quoted => {
            return Ok((
                ValueType::Reference(Access::Shared, Box::new(ValueType::Slice(Box::new(ValueType::integer("u8").unwrap())))),
                None,
            ));
        }
        TokenKind::Integer { radix, suffix_start_offset } => {
            let (digits, suffix) = token.text.split_at(suffix_start_offset - start);
            let magnitude = BigInt::parse_bytes(
                digits[radix.get_prefix_byte_count()..]
                    .replace('_', "")
                    .as_bytes(),
                radix.get_base(),
            )
            .ok_or_else(|| TypeError::at(f, n, "invalid integer literal"))?;
            (BigRational::from_integer(magnitude), suffix)
        }
        TokenKind::Float { suffix_start_offset } => {
            let (digits, suffix) = token.text.split_at(suffix_start_offset - start);
            let digits = digits.replace('_', "");
            let (mantissa, exponent) = match digits.split_once(['e', 'E']) {
                Some((m, e)) => (
                    m,
                    e.parse::<i64>()
                        .map_err(|_| TypeError::at(f, n, "decimal exponent exceeds implementation capacity"))?,
                ),
                None => (digits.as_str(), 0),
            };
            let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
            let significand =
                BigInt::parse_bytes(format!("{integer}{fraction}").as_bytes(), 10).ok_or_else(|| TypeError::at(f, n, "invalid decimal significand"))?;
            let value = if significand.is_zero() {
                BigRational::zero()
            } else {
                let scale = exponent
                    .checked_sub(fraction.len() as i64)
                    .ok_or_else(|| TypeError::at(f, n, "decimal exponent exceeds implementation capacity"))?;
                // Bound the allocation before expanding a compact scientific spelling.
                if scale.unsigned_abs() > 1_000_000 {
                    return Err(TypeError::at(f, n, "decimal exponent exceeds implementation capacity"));
                }
                let power = BigInt::from(10u8).pow(scale.unsigned_abs() as u32);
                if scale < 0 {
                    BigRational::new(significand, power)
                } else {
                    BigRational::from_integer(significand * power)
                }
            };
            (value, suffix)
        }
        _ => return Err(TypeError::unsupported(f, n)),
    };
    let fractional_origin = (!value.is_integer()).then_some(token.span);
    let ty = match suffix {
        "" => None,
        "f32" => Some(ValueType::Float32),
        "f64" => Some(ValueType::Float64),
        other => Some(ValueType::integer(other).ok_or_else(|| TypeError::at(f, n, format!("invalid numeric suffix {other}")))?),
    };
    Ok((ValueType::Exact { value, fractional_origin }, ty))
}

pub(super) fn exact_binary(
    token: &str,
    left: &BigRational,
    right: &BigRational,
    origin: Option<SourceSpan>,
    site: SourceSpan,
) -> Result<ValueType, &'static str> {
    let value = match token {
        "+" => left + right,
        "-" => left - right,
        "*" => left * right,
        "/" if right.is_zero() => return Err("anonymous division by zero"),
        "/" => left / right,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => return Ok(ValueType::Bool),
        _ => return Err("operator requires an already typed operand"),
    };
    let fractional_origin = origin.or_else(|| (value.denom() != &BigInt::one()).then_some(site));
    Ok(ValueType::Exact { value, fractional_origin })
}
pub(super) fn array_length(
    f: &IndexedSource<'_>,
    n: usize,
    number: ValueType,
    suffix: Option<ValueType>,
) -> Result<u64, TypeError> {
    use num_traits::ToPrimitive;
    let ValueType::Exact {
        value,
        fractional_origin: None,
    } = number
    else {
        return Err(TypeError::at(f, n, "array length must be a nonnegative integer"));
    };
    if !value.is_integer()
        || suffix.as_ref().is_some_and(|t| {
            !matches!(t, ValueType::Integer { .. })
                || !(ValueType::Exact {
                    value: value.clone(),
                    fractional_origin: None,
                })
                .fits(t)
        })
    {
        return Err(TypeError::at(f, n, "invalid array length"));
    }
    value
        .numer()
        .to_u64()
        .ok_or_else(|| TypeError::at(f, n, "array length exceeds u64"))
}
pub(super) fn static_number(
    f: &IndexedSource<'_>,
    root: usize,
) -> Result<(ValueType, Option<ValueType>), TypeError> {
    use omega_parser::NodeKind;
    // A token can select a user operation once a typed operand participates. Fold
    // only wholly anonymous arithmetic here. Typed computations need evaluation.
    let mut values = Vec::new();
    let mut events = vec![(root, false)];
    while let Some((n, exit)) = events.pop() {
        let node = &f.nodes[n];
        if !exit {
            if node.kind == NodeKind::Literal {
                values.push(literal(f, n)?);
                continue;
            }
            if !matches!(node.kind, NodeKind::Binary | NodeKind::Paren | NodeKind::Unary) {
                return Err(TypeError::unsupported(f, n));
            }
            events.push((n, true));
            events.extend(node.children.iter().rev().map(|&child| (child, false)));
            continue;
        }
        if node.kind == NodeKind::Paren {
            continue;
        }
        let Some((
            ValueType::Exact {
                value: right,
                fractional_origin: b,
            },
            right_carrier,
        )) = values.pop()
        else {
            return Err(TypeError::at(f, n, "static operation requires a numeric operand"));
        };
        let token = node.leaves.iter().map(|w| w.text).collect::<String>();
        if node.kind == NodeKind::Unary && token == "-" {
            let negated = ValueType::Exact {
                value: -right,
                fractional_origin: b,
            };
            if right_carrier.as_ref().is_some_and(|ty| !negated.fits(ty)) {
                return Err(TypeError::at(f, n, "negated static integer exceeds its carrier"));
            }
            values.push((negated, right_carrier));
            continue;
        }
        let Some((
            ValueType::Exact {
                value: left,
                fractional_origin: a,
            },
            left_carrier,
        )) = values.pop()
        else {
            return Err(TypeError::at(f, n, "static operation requires two numeric operands"));
        };
        if node.kind != NodeKind::Binary {
            return Err(TypeError::unsupported(f, n));
        }
        if let Some(carrier) = left_carrier.as_ref().or(right_carrier.as_ref()) {
            let shift = token.starts_with("<<") || token.starts_with(">>");
            let right_destination = if shift { right_carrier.as_ref().unwrap_or(carrier) } else { carrier };
            if !matches!(carrier, ValueType::Integer { .. })
                || left_carrier
                    .as_ref()
                    .zip(right_carrier.as_ref())
                    .is_some_and(|(l, r)| !shift && l != r)
                || !left.is_integer()
                || !right.is_integer()
                || a.is_some()
                || b.is_some()
                || !crate::integers::fits(left.numer(), carrier)
                || !crate::integers::fits(right.numer(), right_destination)
            {
                return Err(TypeError::at(f, n, "static integer operands require the same representable carrier"));
            }
            if token.ends_with('!') {
                return Err(TypeError::at(f, n, "trapping operation requires proof-static admission"));
            }
            let result = crate::integers::compute(&token, &[left.to_integer(), right.to_integer()], carrier).map_err(|m| TypeError::at(f, n, m))?;
            values.push((
                ValueType::Exact {
                    value: BigRational::from_integer(result),
                    fractional_origin: None,
                },
                Some(carrier.clone()),
            ));
        } else {
            let value = exact_binary(&token, &left, &right, a.or(b), f.span(n)).map_err(|e| TypeError::at(f, n, e))?;
            values.push((value, None));
        }
    }
    values.pop().ok_or_else(|| TypeError::unsupported(f, root))
}
