//! Public boundary value graphs and their policy-specific ABI classification.
//!
//! Source materialization supplies shapes, field offsets and roots. This owner
//! determines their ABI class and checks an authored class without consulting
//! source declarations or provider selection. Classification does not establish
//! complete graph validity, parameter placement, or calling-plan authority.

use crate::{CallingPolicy, ValueClass, ValueShape};

mod classification;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryValueClass {
    Integer,
    Float,
    Reference,
    FixedArray { element: u16, length: u16 },
    Record { first_field: u16, field_count: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueShape {
    pub class: BoundaryValueClass,
    pub byte_size: u16,
    pub alignment: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueField {
    pub shape: u16,
    pub byte_offset: u16,
}

impl BoundaryValueShape {
    pub const fn class(self) -> BoundaryValueClass {
        self.class
    }

    pub const fn byte_size(self) -> u16 {
        self.byte_size
    }

    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

impl BoundaryValueField {
    pub const fn shape(self) -> u16 {
        self.shape
    }

    pub const fn byte_offset(self) -> u16 {
        self.byte_offset
    }
}

/// Borrowed normalized structure shared by source-policy materialization and
/// ABI checking. Child indices and record field ranges refer to these slices.
pub struct BoundaryValueGraph<'a> {
    pub shapes: &'a [BoundaryValueShape],
    pub fields: &'a [BoundaryValueField],
}

impl BoundaryValueGraph<'_> {
    /// Classify one value for the exact supplied policy. Recursive aggregates
    /// retain their leaf classes and offsets; equal byte size is not enough.
    pub fn classify(&self, root: u16, policy: CallingPolicy) -> Result<ValueShape, String> {
        let shape = self
            .shapes
            .get(usize::from(root))
            .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
        let class = match shape.class {
            BoundaryValueClass::Integer | BoundaryValueClass::Reference => ValueClass::Integer,
            BoundaryValueClass::Float => ValueClass::Float,
            BoundaryValueClass::FixedArray { .. } | BoundaryValueClass::Record { .. } => {
                classification::classify_boundary_aggregate(self, root, policy)?
            }
        };
        Ok(ValueShape {
            class,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        })
    }

    /// Check size and alignment before comparing the policy-specific ABI class.
    /// This checks class compatibility only; placement and full calling-plan
    /// validation remain separate obligations.
    pub fn validate_abi_shape(
        &self,
        root: u16,
        abi: ValueShape,
        policy: CallingPolicy,
    ) -> Result<(), String> {
        let shape = self
            .shapes
            .get(usize::from(root))
            .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
        if abi.byte_size != u32::from(shape.byte_size) || abi.alignment != shape.alignment {
            return Err(format!(
                "ABI shape {}/{} does not preserve semantic size/alignment {}/{}",
                abi.byte_size, abi.alignment, shape.byte_size, shape.alignment
            ));
        }
        let expected = self.classify(root, policy)?.class;
        if abi.class != expected {
            return Err(format!(
                "policy published ABI class {:?}, but its recursive public shape requires {:?} under {:?}",
                abi.class, expected, policy
            ));
        }
        Ok(())
    }
}
