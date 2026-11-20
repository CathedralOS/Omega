use crate::{Access, CallableType, ValueType};
use num_bigint::Sign;

pub(crate) trait TypeRelations: Sized {
    fn builtin_scalar(name: &str) -> Option<Self>;
    fn integer(name: &str) -> Option<Self>;
    fn fits(
        &self,
        expected: &Self,
    ) -> bool;
    /// An unsuffixed number has no carrier until a destination lands it, and a
    /// tuple literal leaves its components unlanded. A binding or constant
    /// whose type still holds one would let a later use choose that type.
    fn holds_anonymous_number(&self) -> bool;
    fn numeric(&self) -> bool;
    fn admits(
        &self,
        operand: &Self,
    ) -> bool;
    fn supports_closed_snapshot(&self) -> bool;
}

impl TypeRelations for ValueType {
    fn builtin_scalar(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "addr" => Some(Self::Address),
            "f32" => Some(Self::Float32),
            "f64" => Some(Self::Float64),
            _ => Self::integer(name),
        }
    }
    fn integer(name: &str) -> Option<Self> {
        let digits = name.strip_prefix(['u', 'i'])?;
        let width = if digits.is_empty() {
            None
        } else {
            Some(digits.parse::<u64>().ok().filter(|&w| w > 0)?)
        };
        Some(Self::Integer {
            signed: name.starts_with('i'),
            width,
        })
    }
    fn fits(
        &self,
        expected: &Self,
    ) -> bool {
        if let Self::Exact { value, .. } = self {
            match expected {
                Self::Float32 | Self::Float64 => return true,
                Self::IndexedInteger { signed, .. } => return value.is_integer() && (*signed || value.numer().sign() != Sign::Minus),
                Self::Integer { .. } => return value.is_integer() && expected.contains_integer(value.numer()),
                _ => {}
            }
        }
        if let (Self::Callable(actual), Self::Callable(destination)) = (self, expected) {
            return destination.admits_contract(actual);
        }
        self == expected
    }
    fn holds_anonymous_number(&self) -> bool {
        match self {
            Self::Exact { .. } => true,
            Self::Tuple(items) => items.iter().any(Self::holds_anonymous_number),
            _ => false,
        }
    }
    fn numeric(&self) -> bool {
        matches!(self, Self::Integer { .. } | Self::IndexedInteger { .. } | Self::Float32 | Self::Float64)
    }
    fn admits(
        &self,
        operand: &Self,
    ) -> bool {
        if let (Self::Callable(expected), Self::Callable(actual)) = (self, operand) {
            return expected.admits_contract(actual);
        }
        self == operand || (self.numeric() && matches!(operand, Self::Exact { .. }))
    }
    fn supports_closed_snapshot(&self) -> bool {
        matches!(
            self.carrier(),
            Self::Bool | Self::Array(..) | Self::IndexedArray(..) | Self::Nominal(_) | Self::Applied(..)
        )
    }
}

pub(crate) trait CallableContract: Sized {
    fn admits_contract(
        &self,
        actual: &Self,
    ) -> bool;
}

impl CallableContract for CallableType {
    fn admits_contract(
        &self,
        actual: &Self,
    ) -> bool {
        self.parameters == actual.parameters
            && self.result == actual.result
            && (self.requirement.is_none() || self.requirement == actual.requirement)
            && (!actual.pause.block || self.pause.block)
            && (!actual.pause.suspend || self.pause.suspend)
            && (!self.terminates || actual.terminates)
            && self.crash_ceiling.as_ref().is_none_or(|allowed| {
                actual
                    .crash_ceiling
                    .as_ref()
                    .is_some_and(|supplied| supplied.iter().all(|cause| allowed.contains(cause)))
            })
            && self.reach.as_ref().is_none_or(|allowed| {
                actual
                    .reach
                    .as_ref()
                    .is_some_and(|supplied| supplied.iter().all(|s| allowed.contains(s)))
            })
    }
}

pub(crate) trait StoredReference: Sized {
    /// Following a stored reference reads its carrier under the outer loan.
    fn through(
        &self,
        outer: Option<&Access>,
    ) -> Result<Self, &'static str>;
}

impl StoredReference for Access {
    fn through(
        &self,
        outer: Option<&Access>,
    ) -> Result<Self, &'static str> {
        match (outer, self) {
            (Some(Self::Write), _) => Err("cannot read through a write-only reference"),
            (Some(Self::Shared), Self::Write) => Err("place requires writable authority"),
            (Some(Self::Shared), _) => Ok(Self::Shared),
            _ => Ok(self.clone()),
        }
    }
}
