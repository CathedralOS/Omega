//! The values an execution holds, and how many storage cells each occupies.
//!
//! A value belongs to the Psi unit that typed it: a record or case names its
//! schema by that unit's `DataId`. Nothing here is a layout. A byte view keeps
//! the sponsor's frozen backing by reference count, so a view that reaches the
//! result can be compared with the backing the sponsor supplied.
use num_bigint::BigInt;
use omega_psi::{DataId, IeeeFloatBits, IeeeFloatFormat, LoweredData, LoweredPackage, ScalarType, TypeId, TypeShape};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Boolean(bool),
    Integer(BigInt),
    Float(IeeeFloatBits),
    /// A shared view of the sponsor's immutable bytes.
    Bytes(Arc<[u8]>),
    Record {
        data: DataId,
        fields: Vec<Value>,
    },
    /// One case of a sum. `case` indexes the schema's cases in declaration order.
    Case {
        data: DataId,
        case: usize,
        fields: Vec<Value>,
    },
    /// A fixed array, or a sequence whose length is its element count.
    Array(Vec<Value>),
    /// A shared reference to a completed record or sum.
    Shared(Arc<Value>),
}

/// One actual of the entry machine. The sponsor keeps a shared argument for
/// the whole invocation and gives an owned one away.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Argument {
    Owned(Value),
    Shared(Arc<Value>),
}

impl Value {
    /// Storage cells this value occupies under [`crate::CELL_SCHEDULE`]: one
    /// per scalar, case tag, sequence length and descriptor. A descriptor's
    /// referent and a view's backing bytes belong to whoever holds them.
    /// `None` when the count does not fit the account.
    pub fn cells(
        &self,
        unit: &LoweredPackage,
        ty: TypeId,
    ) -> Option<u64> {
        let mut total = 0u64;
        let mut pending = vec![(self, ty)];
        while let Some((value, ty)) = pending.pop() {
            let shape = unit.types.get(ty)?;
            match (value, shape) {
                (Value::Boolean(_) | Value::Integer(_) | Value::Float(_), TypeShape::PrimitiveScalar(_))
                | (Value::Bytes(_), TypeShape::SharedByteView)
                | (Value::Shared(_), TypeShape::NominalReference { .. }) => total = total.checked_add(1)?,
                (Value::Record { data, fields }, TypeShape::Nominal(id)) if data == id => {
                    let schema = schema(unit, *id)?;
                    if schema.fields.len() != fields.len() {
                        return None;
                    }
                    pending.extend(
                        fields
                            .iter()
                            .zip(schema.fields.iter().map(|field| field.carrier)),
                    );
                }
                (Value::Case { data, case, fields }, TypeShape::Nominal(id)) if data == id => {
                    let payload = &schema(unit, *id)?.cases.get(*case)?.fields;
                    if payload.len() != fields.len() {
                        return None;
                    }
                    total = total.checked_add(1)?;
                    pending.extend(fields.iter().zip(payload.iter().map(|field| field.carrier)));
                }
                (Value::Array(elements), TypeShape::FixedArray { element, .. }) => pending.extend(elements.iter().map(|value| (value, *element))),
                (Value::Array(elements), TypeShape::ElementSequence { element }) => {
                    // One cell for the stored length.
                    total = total.checked_add(1)?;
                    pending.extend(elements.iter().map(|value| (value, *element)));
                }
                _ => return None,
            }
        }
        Some(total)
    }
}

pub(crate) fn schema(
    unit: &LoweredPackage,
    id: DataId,
) -> Option<&LoweredData> {
    unit.data.get(id.get() - 1).filter(|data| data.id == id)
}

pub(crate) fn scalar_conforms(
    value: &Value,
    scalar: ScalarType,
) -> bool {
    match (value, scalar) {
        (Value::Boolean(_), ScalarType::Boolean) => true,
        (Value::Integer(value), ScalarType::Integer { signed, width }) => {
            use num_bigint::Sign;
            match (value.sign(), signed) {
                (Sign::NoSign, _) => width > 0,
                (Sign::Plus, false) => value.bits() <= width,
                (Sign::Plus, true) => value.bits() < width,
                (Sign::Minus, false) => false,
                // The least value of a signed carrier has one more magnitude bit than its greatest.
                (Sign::Minus, true) => (-value - 1u8).bits() < width,
            }
        }
        (Value::Float(bits), ScalarType::IeeeFloat(format)) => bits.format() == format,
        _ => false,
    }
}

/// Whether `value` is a completed value of `ty` in `unit`. A reference must
/// refer to a completed value of its referent.
pub(crate) fn conforms(
    unit: &LoweredPackage,
    value: &Value,
    ty: TypeId,
) -> bool {
    let mut pending = vec![(value, ty)];
    while let Some((value, ty)) = pending.pop() {
        let Some(shape) = unit.types.get(ty) else { return false };
        match (value, shape) {
            (value, TypeShape::PrimitiveScalar(scalar)) if scalar_conforms(value, *scalar) => {}
            (Value::Bytes(_), TypeShape::SharedByteView) => {}
            (Value::Shared(referent), TypeShape::NominalReference { referent: id, .. }) => {
                let Some(schema) = schema(unit, *id) else { return false };
                pending.push((referent, schema.carrier));
            }
            (Value::Record { data, fields }, TypeShape::Nominal(id)) if data == id => {
                let Some(schema) = schema(unit, *id) else { return false };
                if !schema.cases.is_empty() || schema.fields.len() != fields.len() {
                    return false;
                }
                pending.extend(
                    fields
                        .iter()
                        .zip(schema.fields.iter().map(|field| field.carrier)),
                );
            }
            (Value::Case { data, case, fields }, TypeShape::Nominal(id)) if data == id => {
                let Some(payload) = schema(unit, *id).and_then(|schema| schema.cases.get(*case)) else {
                    return false;
                };
                if payload.fields.len() != fields.len() {
                    return false;
                }
                pending.extend(
                    fields
                        .iter()
                        .zip(payload.fields.iter().map(|field| field.carrier)),
                );
            }
            (Value::Array(elements), TypeShape::FixedArray { element, length }) => {
                if u64::try_from(elements.len()) != Ok(*length) {
                    return false;
                }
                pending.extend(elements.iter().map(|value| (value, *element)));
            }
            (Value::Array(elements), TypeShape::ElementSequence { element }) => pending.extend(elements.iter().map(|value| (value, *element))),
            _ => return false,
        }
    }
    true
}

pub(crate) fn zero_scalar(scalar: ScalarType) -> Value {
    match scalar {
        ScalarType::Boolean => Value::Boolean(false),
        ScalarType::Integer { .. } => Value::Integer(BigInt::from(0u8)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Value::Float(IeeeFloatBits::Binary32(0)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Value::Float(IeeeFloatBits::Binary64(0)),
    }
}
