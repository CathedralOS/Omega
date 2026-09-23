//! Element-view coordinates keep u64 counts; element reads carry whatever
//! scalar type the view's element declaration selected, so the result check
//! is definitional agreement, not a fixed width.
use crate::BTreeMap;
use crate::IntegerSign;
use crate::IntegerType;
use crate::O;
use crate::ScalarType;
use crate::ValueDefinition;
use crate::ValueId;

pub(super) fn types_match(operation: &O, definitions: &BTreeMap<ValueId, ValueDefinition>) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let u64_scalar = |value: &ValueId| {
        matches!(scalar(*value), Some(ScalarType::Integer(integer))
            if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
    };
    match operation {
        O::ElementViewSubslice {
            start, end, length, ..
        } => u64_scalar(start) && u64_scalar(end) && u64_scalar(length),
        O::ElementViewRead {
            result,
            index,
            length,
            ..
        } => {
            scalar(result.value) == Some(result.scalar_type)
                && u64_scalar(index)
                && scalar(*index) == scalar(*length)
        }
        O::ElementViewLength { result, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
        }
        O::EstablishElementView { .. } => true,
        _ => false,
    }
}
