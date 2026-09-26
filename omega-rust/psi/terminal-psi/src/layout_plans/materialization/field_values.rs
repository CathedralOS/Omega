//! Ordinary scalar and aggregate field values supplied to a materializer,
//! with the schemas that validate them against a plan.

use crate::layout_plans::materialization::MaterializationDiagnostic;
use crate::layout_plans::materialization::stored_integer_writes::low_mask;

/// One ordinary scalar supplied to a validated dictated-layout materializer.
/// Positional fields select compiler-validated plan entries by name; numbered
/// fields use their stable member identity. Callers never provide a byte
/// offset or destination bit position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldValue {
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    pub width_bits: u16,
    pub value: u64,
}

impl ScalarFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        value: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        if width_bits < 64 && value > low_mask(width_bits) {
            return Err(MaterializationDiagnostic(format!(
                "scalar value {value:#x} does not fit its {width_bits}-bit field"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            width_bits,
            value,
        })
    }

    /// Constructs a scalar value carrying its compiler-retained stable member
    /// identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
        value: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new(field, width_bits, value)?;
        value.member_identity = Some(member_identity);
        Ok(value)
    }
}

/// One complete aggregate supplied to a validated dictated-layout
/// materializer. The compiler derives `bytes` from an owned typed value; source
/// programs do not gain an arbitrary byte-patching operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFieldValue {
    pub field: String,
    pub bytes: Vec<u8>,
}

/// Compiler-derived physical extent of one aggregate schema field. This is
/// kept separate from the value so caller-provided bytes cannot claim their
/// own completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFieldSchema {
    pub field: String,
    /// Stable schema identity when this field belongs to a numbered record.
    /// The validated layout and compiler-derived schema rejoin through this
    /// identity, so a source rename cannot change placement authority.
    pub(crate) member_identity: Option<u64>,
    pub byte_size: u64,
    pub(crate) shape: AggregateFieldShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateFieldShape {
    Whole,
    Repeated {
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    },
}

impl AggregateFieldSchema {
    pub fn new(
        field: impl Into<String>,
        byte_size: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if byte_size == 0 {
            return Err(MaterializationDiagnostic(
                "aggregate field schema requires a nonzero physical extent".into(),
            ));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            byte_size,
            shape: AggregateFieldShape::Whole,
        })
    }

    /// Constructs one whole aggregate field with its compiler-retained stable
    /// member identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        byte_size: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema = Self::new(field, byte_size)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }

    /// Constructs the compiler-derived shape of one outer fixed array. A
    /// validated layout may retain one whole-field `At`, or use exactly one
    /// `At` per element at a constant destination stride. The policy never
    /// supplies the element extent, alignment, or count.
    pub fn new_repeated(
        field: impl Into<String>,
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        if element_byte_size == 0 || element_count == 0 {
            return Err(MaterializationDiagnostic(
                "repeated aggregate schema requires nonzero element extent and count".into(),
            ));
        }
        if element_align == 0 || !element_align.is_power_of_two() {
            return Err(MaterializationDiagnostic(format!(
                "repeated aggregate element alignment {element_align} is not a positive power of two"
            )));
        }
        let byte_size = element_byte_size
            .checked_mul(element_count)
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "repeated aggregate compiler-derived physical extent overflows".into(),
                )
            })?;
        Ok(Self {
            field: field.into(),
            member_identity: None,
            byte_size,
            shape: AggregateFieldShape::Repeated {
                element_byte_size,
                element_align,
                element_count,
            },
        })
    }

    /// Constructs one numbered outer fixed array. Element geometry remains
    /// compiler-derived while the stable identity rejoins renamed layout rows.
    pub fn new_repeated_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_byte_size: u64,
        element_align: u64,
        element_count: u64,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema =
            Self::new_repeated(field, element_byte_size, element_align, element_count)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }
}

impl AggregateFieldValue {
    pub fn new(
        field: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, MaterializationDiagnostic> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(MaterializationDiagnostic(
                "aggregate field materialization requires a nonempty complete value".into(),
            ));
        }
        Ok(Self {
            field: field.into(),
            bytes,
        })
    }
}

/// Declared scalar shape used when decoding bytes through a validated layout.
/// The width comes from the compiler-materialized schema, not from the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFieldSchema {
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    pub width_bits: u16,
}

impl ScalarFieldSchema {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            width_bits,
        })
    }

    /// Constructs a decode schema carrying its compiler-retained stable member
    /// identity. Decoded values use the current schema spelling.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut schema = Self::new(field, width_bits)?;
        schema.member_identity = Some(member_identity);
        Ok(schema)
    }
}
