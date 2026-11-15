use numerics::integer_carrier::FixedInteger;

/*
    Scalar meaning comes from the compiler-installed builtin identity, not
    diagnostic text carried by a type reference. Both tree representations
    and the lowered reader use this projection; Atomic wrappers are not their
    ordinary scalar storage carriers. Selector describes a dynamic-trait value,
    not another source-free builtin type.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum PrimitiveType {
    Bool,
    F32,
    F64,
    Integer(FixedInteger),
    Addr,
    Selector,
}

impl PrimitiveType {
    pub const U8: Self = Self::Integer(FixedInteger::U8);
    pub const U64: Self = Self::Integer(FixedInteger::U64);

    pub fn from_builtin_atom(atom: crate::BuiltinTypeAtom) -> Option<Self> {
        match atom {
            crate::BuiltinTypeAtom::Bool => Some(Self::Bool),
            crate::BuiltinTypeAtom::Integer(carrier) => Some(Self::Integer(carrier)),
            crate::BuiltinTypeAtom::Address => Some(Self::Addr),
            crate::BuiltinTypeAtom::F32 => Some(Self::F32),
            crate::BuiltinTypeAtom::F64 => Some(Self::F64),
            crate::BuiltinTypeAtom::Slice
            | crate::BuiltinTypeAtom::SyscallResult
            | crate::BuiltinTypeAtom::Terminal
            | crate::BuiltinTypeAtom::UInt
            | crate::BuiltinTypeAtom::Int
            | crate::BuiltinTypeAtom::AtomicBool
            | crate::BuiltinTypeAtom::AtomicU32
            | crate::BuiltinTypeAtom::AtomicU64 => None,
        }
    }

    /* The carrier of a fixed-width integer; the address is not one. */
    #[inline]
    pub fn fixed_integer(self) -> Option<FixedInteger> {
        match self {
            Self::Integer(carrier) => Some(carrier),
            Self::Bool | Self::F32 | Self::F64 | Self::Addr | Self::Selector => None,
        }
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    #[inline]
    pub fn is_unsigned_integer(self) -> bool {
        matches!(self, Self::Integer(carrier) if !carrier.is_signed())
    }

    pub fn name(self) -> String {
        self.to_string()
    }

    /*
        Host-integer endpoints, or `None` for a carrier whose endpoints
        exceed them; such a caller refuses the width.
    */
    pub fn integer_bounds(self) -> Option<(i128, i128)> {
        match self {
            Self::Integer(carrier) => carrier.bounds_i128(),
            Self::Addr => FixedInteger::U64.bounds_i128(),
            Self::Bool | Self::F32 | Self::F64 | Self::Selector => None,
        }
    }

    pub fn accepts_integer_literal(self) -> bool {
        self.accepts_range_constraint() && !self.accepts_finite_constraint()
    }

    pub fn accepts_float_literal(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn is_fixed_integer(self) -> bool {
        self.is_integer()
    }

    pub fn is_signed_integer(self) -> bool {
        match self {
            Self::Integer(carrier) => carrier.is_signed(),
            Self::Bool | Self::F32 | Self::F64 => true,
            Self::Addr | Self::Selector => false,
        }
    }

    /*
        Bytes of the native scalar. An integer whose width has no native
        scalar yet answers `None`, and its user refuses that width.
    */
    pub fn scalar_byte_size(self) -> Option<usize> {
        match self {
            Self::Bool => Some(1),
            Self::Integer(carrier) => carrier.native_scalar_bytes(),
            Self::F32 => Some(4),
            Self::F64 | Self::Addr | Self::Selector => Some(8),
        }
    }

    pub fn accepts_range_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64 | Self::Integer(_) | Self::Addr)
    }

    pub fn accepts_finite_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn from_landed_integer(kind: numerics::literals::LandedIntegerType) -> Self {
        match kind {
            numerics::literals::LandedIntegerType::Fixed(carrier) => Self::Integer(carrier),
            numerics::literals::LandedIntegerType::Addr => Self::Addr,
        }
    }

    pub fn landed_landing(self) -> Option<numerics::literals::LandedIntegerType> {
        use numerics::literals::LandedIntegerType;
        match self {
            Self::Integer(carrier) => Some(LandedIntegerType::Fixed(carrier)),
            Self::Addr => Some(LandedIntegerType::Addr),
            Self::Selector | Self::Bool | Self::F32 | Self::F64 => None,
        }
    }

    pub fn landed_landing_fixed(self) -> Option<numerics::literals::LandedIntegerType> {
        self.fixed_integer()
            .map(numerics::literals::LandedIntegerType::Fixed)
    }

    pub fn non_address_landed_integer(kind: numerics::literals::LandedIntegerType) -> Option<Self> {
        match kind {
            numerics::literals::LandedIntegerType::Addr => None,
            kind => Some(Self::from_landed_integer(kind)),
        }
    }
}

impl std::fmt::Display for PrimitiveType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool => formatter.write_str("bool"),
            Self::F32 => formatter.write_str("f32"),
            Self::F64 => formatter.write_str("f64"),
            Self::Integer(carrier) => carrier.fmt(formatter),
            Self::Addr => formatter.write_str("addr"),
            Self::Selector => formatter.write_str("selector"),
        }
    }
}
