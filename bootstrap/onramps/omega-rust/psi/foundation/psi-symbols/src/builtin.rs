use super::{SymbolKind, SymbolNameRef};

pub const BUILTIN_TYPE_COUNT: usize = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    UInt,
    Int,
}

impl BuiltinType {
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::UInt, Self::Int]
            .into_iter()
            .find(|builtin_type| builtin_type.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::UInt => "UInt",
            Self::Int => "Int",
        }
    }

    pub fn ordinal(self) -> usize {
        // POSITION in `builtin_type_symbols()` -- symbol handles are assigned
        // in table order, so these MUST track that array. (The usize/isize
        // retirement removed two entries above these and shifted them from
        // 19/20/21; the builtin_type_ordinals_track_the_symbol_table unit
        // test pins the coupling.)
        match self {
            Self::UInt => 17,
            Self::Int => 18,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Max,
    Min,
    /// `sqrt(x)`: a UNARY float intrinsic. It reuses the binary float
    /// value-write path with both operands set to `x` (the encoder's Sqrt
    /// arm reads the first SSE register only).
    Sqrt,
    /// `asm { hlt }`: the x86 privileged halt instruction as a known-contract
    /// asm intrinsic (privileged_effects_and_binary_trust brief). Reaches the
    /// canonical `MachineControl` service. The `#` in the symbol name is not an
    /// identifier character, so the intrinsic is UNNAMEABLE from source --
    /// only the parser's asm-block desugar can reference it.
    AsmHlt,
    /// `asm { out <port>, <value> }`: x86 port write (`out dx, al`). Reaches
    /// the canonical `PortIo` service. Unnameable from source (see AsmHlt).
    AsmPortOut,
    /// `asm { in <destination>, <port> }`: x86 port read (`in al, dx`).
    /// Reaches the canonical `PortIo` service. Unnameable from source (see AsmHlt).
    AsmPortIn,
    /// x86 memory-ordering fences. They are unnameable zero-operand asm
    /// intrinsics and carry no service-reach effect.
    AsmLoadFence,
    AsmStoreFence,
    AsmFullFence,
    /// x86 interrupt-enable flag control. Both are unnameable, zero-operand
    /// asm intrinsics with canonical `MachineControl` reach.
    AsmDisableInterrupts,
    AsmEnableInterrupts,
    /// Compiler-balanced RFLAGS snapshot/restore intrinsics. The source
    /// spelling uses explicit u64 places; raw stack-mutating push/pop never
    /// enters the checked tree.
    AsmSnapshotFlags,
    AsmRestoreFlags,
    /// Structured x86 model-specific-register access. `rdmsr` returns a u64;
    /// `wrmsr` consumes a u32 index and u64 value. Both are unnameable
    /// machine-control intrinsics.
    AsmReadMsr,
    AsmWriteMsr,
    AsmReadCr0,
    AsmReadCr2,
    AsmReadCr3,
    AsmReadCr4,
    AsmWriteCr0,
    AsmWriteCr3,
    AsmWriteCr4,
    /// Internal unary predicate used only by selected named-float plans. The
    /// `#` keeps it unnameable from source during the provider migration.
    FloatIsNan,
    /// Internal ternary operations selected only by exact named-float plans.
    /// Separate symbols retain the source format after expression tables copy
    /// the checked call into state-local lowering tables.
    FloatMultiplyThenAddF32,
    FloatMultiplyThenAddF64,
    FloatFusedMultiplyAddF32,
    FloatFusedMultiplyAddF64,
    FloatIsFinite,
    FloatIsInfinite,
    FloatIsNormal,
    FloatIsSubnormal,
    FloatClassifyF32,
    FloatClassifyF64,
    /// Proof-only callable-entry versioning for content-conservation
    /// contracts. This is source-nameable only in proof facts and never has a
    /// runtime implementation.
    ContentEntry,
    /// Proof-only partial n-ary composition for the compiler-owned content
    /// algebras. Validation selects the algebra from the exact projection
    /// terms; packages cannot implement or override this operation.
    ContentSeparate,
    /// Internal binary operations selected only by exact directed-float plans.
    /// The format and rounding direction remain explicit in the symbol after
    /// state-local expression copying. These append to the stable builtin
    /// ordinal sequence rather than renumbering existing content operations.
    FloatAddTowardZeroF32,
    FloatAddTowardZeroF64,
    FloatAddTowardPositiveF32,
    FloatAddTowardPositiveF64,
    FloatAddTowardNegativeF32,
    FloatAddTowardNegativeF64,
    FloatSubtractTowardZeroF32,
    FloatSubtractTowardZeroF64,
    FloatSubtractTowardPositiveF32,
    FloatSubtractTowardPositiveF64,
    FloatSubtractTowardNegativeF32,
    FloatSubtractTowardNegativeF64,
    FloatMultiplyTowardZeroF32,
    FloatMultiplyTowardZeroF64,
    FloatMultiplyTowardPositiveF32,
    FloatMultiplyTowardPositiveF64,
    FloatMultiplyTowardNegativeF32,
    FloatMultiplyTowardNegativeF64,
    FloatDivideTowardZeroF32,
    FloatDivideTowardZeroF64,
    FloatDivideTowardPositiveF32,
    FloatDivideTowardPositiveF64,
    FloatDivideTowardNegativeF32,
    FloatDivideTowardNegativeF64,
    FloatSqrtTowardZeroF32,
    FloatSqrtTowardZeroF64,
    FloatSqrtTowardPositiveF32,
    FloatSqrtTowardPositiveF64,
    FloatSqrtTowardNegativeF32,
    FloatSqrtTowardNegativeF64,
    FloatFusedMultiplyAddTowardZeroF32,
    FloatFusedMultiplyAddTowardZeroF64,
    FloatFusedMultiplyAddTowardPositiveF32,
    FloatFusedMultiplyAddTowardPositiveF64,
    FloatFusedMultiplyAddTowardNegativeF32,
    FloatFusedMultiplyAddTowardNegativeF64,
}

impl BuiltinFunction {
    pub const COUNT: usize = 71;

    pub fn name(self) -> &'static str {
        match self {
            Self::Max => "max",
            Self::Min => "min",
            Self::Sqrt => "sqrt",
            Self::AsmHlt => "asm#hlt",
            Self::AsmPortOut => "asm#port_out",
            Self::AsmPortIn => "asm#port_in",
            Self::AsmLoadFence => "asm#lfence",
            Self::AsmStoreFence => "asm#sfence",
            Self::AsmFullFence => "asm#mfence",
            Self::AsmDisableInterrupts => "asm#cli",
            Self::AsmEnableInterrupts => "asm#sti",
            Self::AsmSnapshotFlags => "asm#pushfq",
            Self::AsmRestoreFlags => "asm#popfq",
            Self::AsmReadMsr => "asm#rdmsr",
            Self::AsmWriteMsr => "asm#wrmsr",
            Self::AsmReadCr0 => "asm#read_cr0",
            Self::AsmReadCr2 => "asm#read_cr2",
            Self::AsmReadCr3 => "asm#read_cr3",
            Self::AsmReadCr4 => "asm#read_cr4",
            Self::AsmWriteCr0 => "asm#write_cr0",
            Self::AsmWriteCr3 => "asm#write_cr3",
            Self::AsmWriteCr4 => "asm#write_cr4",
            Self::FloatIsNan => "float#is_nan",
            Self::FloatMultiplyThenAddF32 => "float#multiply_then_add_f32",
            Self::FloatMultiplyThenAddF64 => "float#multiply_then_add_f64",
            Self::FloatFusedMultiplyAddF32 => "float#fused_multiply_add_f32",
            Self::FloatFusedMultiplyAddF64 => "float#fused_multiply_add_f64",
            Self::FloatIsFinite => "float#is_finite",
            Self::FloatIsInfinite => "float#is_infinite",
            Self::FloatIsNormal => "float#is_normal",
            Self::FloatIsSubnormal => "float#is_subnormal",
            Self::FloatClassifyF32 => "float#classify_f32",
            Self::FloatClassifyF64 => "float#classify_f64",
            Self::FloatAddTowardZeroF32 => "float#add_toward_zero_f32",
            Self::FloatAddTowardZeroF64 => "float#add_toward_zero_f64",
            Self::FloatAddTowardPositiveF32 => "float#add_toward_positive_f32",
            Self::FloatAddTowardPositiveF64 => "float#add_toward_positive_f64",
            Self::FloatAddTowardNegativeF32 => "float#add_toward_negative_f32",
            Self::FloatAddTowardNegativeF64 => "float#add_toward_negative_f64",
            Self::FloatSubtractTowardZeroF32 => "float#subtract_toward_zero_f32",
            Self::FloatSubtractTowardZeroF64 => "float#subtract_toward_zero_f64",
            Self::FloatSubtractTowardPositiveF32 => "float#subtract_toward_positive_f32",
            Self::FloatSubtractTowardPositiveF64 => "float#subtract_toward_positive_f64",
            Self::FloatSubtractTowardNegativeF32 => "float#subtract_toward_negative_f32",
            Self::FloatSubtractTowardNegativeF64 => "float#subtract_toward_negative_f64",
            Self::FloatMultiplyTowardZeroF32 => "float#multiply_toward_zero_f32",
            Self::FloatMultiplyTowardZeroF64 => "float#multiply_toward_zero_f64",
            Self::FloatMultiplyTowardPositiveF32 => "float#multiply_toward_positive_f32",
            Self::FloatMultiplyTowardPositiveF64 => "float#multiply_toward_positive_f64",
            Self::FloatMultiplyTowardNegativeF32 => "float#multiply_toward_negative_f32",
            Self::FloatMultiplyTowardNegativeF64 => "float#multiply_toward_negative_f64",
            Self::FloatDivideTowardZeroF32 => "float#divide_toward_zero_f32",
            Self::FloatDivideTowardZeroF64 => "float#divide_toward_zero_f64",
            Self::FloatDivideTowardPositiveF32 => "float#divide_toward_positive_f32",
            Self::FloatDivideTowardPositiveF64 => "float#divide_toward_positive_f64",
            Self::FloatDivideTowardNegativeF32 => "float#divide_toward_negative_f32",
            Self::FloatDivideTowardNegativeF64 => "float#divide_toward_negative_f64",
            Self::FloatSqrtTowardZeroF32 => "float#sqrt_toward_zero_f32",
            Self::FloatSqrtTowardZeroF64 => "float#sqrt_toward_zero_f64",
            Self::FloatSqrtTowardPositiveF32 => "float#sqrt_toward_positive_f32",
            Self::FloatSqrtTowardPositiveF64 => "float#sqrt_toward_positive_f64",
            Self::FloatSqrtTowardNegativeF32 => "float#sqrt_toward_negative_f32",
            Self::FloatSqrtTowardNegativeF64 => "float#sqrt_toward_negative_f64",
            Self::FloatFusedMultiplyAddTowardZeroF32 => "float#fused_multiply_add_toward_zero_f32",
            Self::FloatFusedMultiplyAddTowardZeroF64 => "float#fused_multiply_add_toward_zero_f64",
            Self::FloatFusedMultiplyAddTowardPositiveF32 => {
                "float#fused_multiply_add_toward_positive_f32"
            }
            Self::FloatFusedMultiplyAddTowardPositiveF64 => {
                "float#fused_multiply_add_toward_positive_f64"
            }
            Self::FloatFusedMultiplyAddTowardNegativeF32 => {
                "float#fused_multiply_add_toward_negative_f32"
            }
            Self::FloatFusedMultiplyAddTowardNegativeF64 => {
                "float#fused_multiply_add_toward_negative_f64"
            }
            Self::ContentEntry => "entry",
            Self::ContentSeparate => "separate",
        }
    }

    pub fn ordinal(self) -> usize {
        match self {
            Self::Max => 0,
            Self::Min => 1,
            Self::Sqrt => 2,
            Self::AsmHlt => 3,
            Self::AsmPortOut => 4,
            Self::AsmPortIn => 5,
            Self::AsmLoadFence => 6,
            Self::AsmStoreFence => 7,
            Self::AsmFullFence => 8,
            Self::AsmDisableInterrupts => 9,
            Self::AsmEnableInterrupts => 10,
            Self::AsmSnapshotFlags => 11,
            Self::AsmRestoreFlags => 12,
            Self::AsmReadMsr => 13,
            Self::AsmWriteMsr => 14,
            Self::AsmReadCr0 => 15,
            Self::AsmReadCr2 => 16,
            Self::AsmReadCr3 => 17,
            Self::AsmReadCr4 => 18,
            Self::AsmWriteCr0 => 19,
            Self::AsmWriteCr3 => 20,
            Self::AsmWriteCr4 => 21,
            Self::FloatIsNan => 22,
            Self::FloatMultiplyThenAddF32 => 23,
            Self::FloatMultiplyThenAddF64 => 24,
            Self::FloatFusedMultiplyAddF32 => 25,
            Self::FloatFusedMultiplyAddF64 => 26,
            Self::FloatIsFinite => 27,
            Self::FloatIsInfinite => 28,
            Self::FloatIsNormal => 29,
            Self::FloatIsSubnormal => 30,
            Self::FloatClassifyF32 => 31,
            Self::FloatClassifyF64 => 32,
            Self::ContentEntry => 33,
            Self::ContentSeparate => 34,
            Self::FloatAddTowardZeroF32 => 35,
            Self::FloatAddTowardZeroF64 => 36,
            Self::FloatAddTowardPositiveF32 => 37,
            Self::FloatAddTowardPositiveF64 => 38,
            Self::FloatAddTowardNegativeF32 => 39,
            Self::FloatAddTowardNegativeF64 => 40,
            Self::FloatSubtractTowardZeroF32 => 41,
            Self::FloatSubtractTowardZeroF64 => 42,
            Self::FloatSubtractTowardPositiveF32 => 43,
            Self::FloatSubtractTowardPositiveF64 => 44,
            Self::FloatSubtractTowardNegativeF32 => 45,
            Self::FloatSubtractTowardNegativeF64 => 46,
            Self::FloatMultiplyTowardZeroF32 => 47,
            Self::FloatMultiplyTowardZeroF64 => 48,
            Self::FloatMultiplyTowardPositiveF32 => 49,
            Self::FloatMultiplyTowardPositiveF64 => 50,
            Self::FloatMultiplyTowardNegativeF32 => 51,
            Self::FloatMultiplyTowardNegativeF64 => 52,
            Self::FloatDivideTowardZeroF32 => 53,
            Self::FloatDivideTowardZeroF64 => 54,
            Self::FloatDivideTowardPositiveF32 => 55,
            Self::FloatDivideTowardPositiveF64 => 56,
            Self::FloatDivideTowardNegativeF32 => 57,
            Self::FloatDivideTowardNegativeF64 => 58,
            Self::FloatSqrtTowardZeroF32 => 59,
            Self::FloatSqrtTowardZeroF64 => 60,
            Self::FloatSqrtTowardPositiveF32 => 61,
            Self::FloatSqrtTowardPositiveF64 => 62,
            Self::FloatSqrtTowardNegativeF32 => 63,
            Self::FloatSqrtTowardNegativeF64 => 64,
            Self::FloatFusedMultiplyAddTowardZeroF32 => 65,
            Self::FloatFusedMultiplyAddTowardZeroF64 => 66,
            Self::FloatFusedMultiplyAddTowardPositiveF32 => 67,
            Self::FloatFusedMultiplyAddTowardPositiveF64 => 68,
            Self::FloatFusedMultiplyAddTowardNegativeF32 => 69,
            Self::FloatFusedMultiplyAddTowardNegativeF64 => 70,
        }
    }

    /// The canonical boundary-service identity reached by an asm intrinsic,
    /// or None when the intrinsic reaches no service (and for value builtins).
    /// Operand, clobber, ordering, and availability metadata lives in the
    /// shared inline-assembly catalog.
    pub fn asm_intrinsic_service_name(self) -> Option<&'static str> {
        match self {
            Self::AsmHlt
            | Self::AsmDisableInterrupts
            | Self::AsmEnableInterrupts
            | Self::AsmRestoreFlags
            | Self::AsmReadMsr
            | Self::AsmWriteMsr
            | Self::AsmReadCr0
            | Self::AsmReadCr2
            | Self::AsmReadCr3
            | Self::AsmReadCr4
            | Self::AsmWriteCr0
            | Self::AsmWriteCr3
            | Self::AsmWriteCr4 => Some("MachineControl"),
            Self::AsmPortOut | Self::AsmPortIn => Some("PortIo"),
            Self::Max
            | Self::Min
            | Self::Sqrt
            | Self::FloatIsNan
            | Self::FloatMultiplyThenAddF32
            | Self::FloatMultiplyThenAddF64
            | Self::FloatFusedMultiplyAddF32
            | Self::FloatFusedMultiplyAddF64
            | Self::FloatIsFinite
            | Self::FloatIsInfinite
            | Self::FloatIsNormal
            | Self::FloatIsSubnormal
            | Self::FloatClassifyF32
            | Self::FloatClassifyF64
            | Self::FloatAddTowardZeroF32
            | Self::FloatAddTowardZeroF64
            | Self::FloatAddTowardPositiveF32
            | Self::FloatAddTowardPositiveF64
            | Self::FloatAddTowardNegativeF32
            | Self::FloatAddTowardNegativeF64
            | Self::FloatSubtractTowardZeroF32
            | Self::FloatSubtractTowardZeroF64
            | Self::FloatSubtractTowardPositiveF32
            | Self::FloatSubtractTowardPositiveF64
            | Self::FloatSubtractTowardNegativeF32
            | Self::FloatSubtractTowardNegativeF64
            | Self::FloatMultiplyTowardZeroF32
            | Self::FloatMultiplyTowardZeroF64
            | Self::FloatMultiplyTowardPositiveF32
            | Self::FloatMultiplyTowardPositiveF64
            | Self::FloatMultiplyTowardNegativeF32
            | Self::FloatMultiplyTowardNegativeF64
            | Self::FloatDivideTowardZeroF32
            | Self::FloatDivideTowardZeroF64
            | Self::FloatDivideTowardPositiveF32
            | Self::FloatDivideTowardPositiveF64
            | Self::FloatDivideTowardNegativeF32
            | Self::FloatDivideTowardNegativeF64
            | Self::FloatSqrtTowardZeroF32
            | Self::FloatSqrtTowardZeroF64
            | Self::FloatSqrtTowardPositiveF32
            | Self::FloatSqrtTowardPositiveF64
            | Self::FloatSqrtTowardNegativeF32
            | Self::FloatSqrtTowardNegativeF64
            | Self::FloatFusedMultiplyAddTowardZeroF32
            | Self::FloatFusedMultiplyAddTowardZeroF64
            | Self::FloatFusedMultiplyAddTowardPositiveF32
            | Self::FloatFusedMultiplyAddTowardPositiveF64
            | Self::FloatFusedMultiplyAddTowardNegativeF32
            | Self::FloatFusedMultiplyAddTowardNegativeF64
            | Self::ContentEntry
            | Self::ContentSeparate
            | Self::AsmLoadFence
            | Self::AsmStoreFence
            | Self::AsmFullFence
            | Self::AsmSnapshotFlags => None,
        }
    }

    pub fn is_asm_intrinsic(self) -> bool {
        matches!(
            self,
            Self::AsmHlt
                | Self::AsmPortOut
                | Self::AsmPortIn
                | Self::AsmLoadFence
                | Self::AsmStoreFence
                | Self::AsmFullFence
                | Self::AsmDisableInterrupts
                | Self::AsmEnableInterrupts
                | Self::AsmSnapshotFlags
                | Self::AsmRestoreFlags
                | Self::AsmReadMsr
                | Self::AsmWriteMsr
                | Self::AsmReadCr0
                | Self::AsmReadCr2
                | Self::AsmReadCr3
                | Self::AsmReadCr4
                | Self::AsmWriteCr0
                | Self::AsmWriteCr3
                | Self::AsmWriteCr4
        )
    }

    pub fn asm_intrinsics() -> [Self; 19] {
        [
            Self::AsmHlt,
            Self::AsmPortOut,
            Self::AsmPortIn,
            Self::AsmLoadFence,
            Self::AsmStoreFence,
            Self::AsmFullFence,
            Self::AsmDisableInterrupts,
            Self::AsmEnableInterrupts,
            Self::AsmSnapshotFlags,
            Self::AsmRestoreFlags,
            Self::AsmReadMsr,
            Self::AsmWriteMsr,
            Self::AsmReadCr0,
            Self::AsmReadCr2,
            Self::AsmReadCr3,
            Self::AsmReadCr4,
            Self::AsmWriteCr0,
            Self::AsmWriteCr3,
            Self::AsmWriteCr4,
        ]
    }
}

pub fn builtin_type_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BUILTIN_TYPE_COUNT] {
    [
        (SymbolKind::BuiltinType, SymbolNameRef::Static("bool")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
        // `addr` -- a pointer-width ADDRESS type, distinct from u64 counts
        // (index_count_and_address_model brief: address and count are separate
        // axes). Naive pointer-width for now (rides the 8-byte path); the
        // in-region/aligned capability discipline is a later rung.
        (SymbolKind::BuiltinType, SymbolNameRef::Static("addr")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Slice")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Result")),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static("SyscallResult"),
        ),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Terminal")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Never")),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::UInt.name()),
        ),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::Int.name()),
        ),
        // Atomic types (chapter 17, concurrency stage 1). Layout matches the
        // underlying primitive; the type name is retained so atomic method
        // calls (load/store/fetch_add/compare_exchange) can be resolved by
        // name in later stages.
        (SymbolKind::BuiltinType, SymbolNameRef::Static("AtomicBool")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("AtomicU32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("AtomicU64")),
    ]
}

pub fn builtin_function_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BuiltinFunction::COUNT]
{
    [
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::Max.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::Min.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::Sqrt.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmHlt.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmPortOut.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmPortIn.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmLoadFence.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmStoreFence.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmFullFence.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmDisableInterrupts.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmEnableInterrupts.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmSnapshotFlags.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmRestoreFlags.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmReadMsr.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmWriteMsr.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmReadCr0.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmReadCr2.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmReadCr3.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmReadCr4.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmWriteCr0.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmWriteCr3.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::AsmWriteCr4.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatIsNan.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyThenAddF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyThenAddF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatIsFinite.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatIsInfinite.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatIsNormal.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatIsSubnormal.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatClassifyF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatClassifyF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::ContentEntry.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::ContentSeparate.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatAddTowardNegativeF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSubtractTowardNegativeF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatMultiplyTowardNegativeF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatDivideTowardNegativeF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatSqrtTowardNegativeF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32.name()),
        ),
        (
            SymbolKind::BuiltinFunction,
            SymbolNameRef::Static(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64.name()),
        ),
    ]
}

pub fn builtin_type_member_symbols(
    _builtin_type: BuiltinType,
) -> impl Iterator<Item = (SymbolKind, SymbolNameRef<'static>)> {
    std::iter::empty()
}

#[cfg(test)]
mod builtin_ordinal_tests {
    use super::*;

    #[test]
    fn builtin_type_ordinals_track_the_symbol_table() {
        // Symbol handles are assigned in builtin_type_symbols() order;
        // BuiltinType::ordinal() hardcodes positions and silently breaks
        // when the table gains or loses entries (the usize retirement
        // shifted UInt/Int/Real by two and layout resolution failed with
        // "unknown layout-bearing type `UInt`"). `Real` is deliberately an
        // ordinary core package, not part of this table.
        let table = builtin_type_symbols();
        for builtin_type in [BuiltinType::UInt, BuiltinType::Int] {
            assert_eq!(
                table[builtin_type.ordinal()].1.as_str(),
                builtin_type.name(),
                "ordinal for {:?} does not match the symbol table position",
                builtin_type
            );
        }
    }

    #[test]
    fn builtin_function_ordinals_track_the_symbol_table() {
        // builtin_function_symbol() resolves by BUILTIN_TYPE_COUNT + ordinal
        // over root children inserted in builtin_function_symbols() order --
        // the same silent-shift hazard the type test pins.
        let table = builtin_function_symbols();
        for function in [
            BuiltinFunction::Max,
            BuiltinFunction::Min,
            BuiltinFunction::Sqrt,
            BuiltinFunction::AsmHlt,
            BuiltinFunction::AsmPortOut,
            BuiltinFunction::AsmPortIn,
            BuiltinFunction::AsmLoadFence,
            BuiltinFunction::AsmStoreFence,
            BuiltinFunction::AsmFullFence,
            BuiltinFunction::AsmDisableInterrupts,
            BuiltinFunction::AsmEnableInterrupts,
            BuiltinFunction::AsmSnapshotFlags,
            BuiltinFunction::AsmRestoreFlags,
            BuiltinFunction::FloatIsNan,
            BuiltinFunction::FloatMultiplyThenAddF32,
            BuiltinFunction::FloatMultiplyThenAddF64,
            BuiltinFunction::FloatFusedMultiplyAddF32,
            BuiltinFunction::FloatFusedMultiplyAddF64,
            BuiltinFunction::FloatIsFinite,
            BuiltinFunction::FloatIsInfinite,
            BuiltinFunction::FloatIsNormal,
            BuiltinFunction::FloatIsSubnormal,
            BuiltinFunction::FloatClassifyF32,
            BuiltinFunction::FloatClassifyF64,
            BuiltinFunction::ContentEntry,
            BuiltinFunction::ContentSeparate,
            BuiltinFunction::FloatAddTowardZeroF32,
            BuiltinFunction::FloatAddTowardZeroF64,
            BuiltinFunction::FloatAddTowardPositiveF32,
            BuiltinFunction::FloatAddTowardPositiveF64,
            BuiltinFunction::FloatAddTowardNegativeF32,
            BuiltinFunction::FloatAddTowardNegativeF64,
            BuiltinFunction::FloatSubtractTowardZeroF32,
            BuiltinFunction::FloatSubtractTowardZeroF64,
            BuiltinFunction::FloatSubtractTowardPositiveF32,
            BuiltinFunction::FloatSubtractTowardPositiveF64,
            BuiltinFunction::FloatSubtractTowardNegativeF32,
            BuiltinFunction::FloatSubtractTowardNegativeF64,
            BuiltinFunction::FloatMultiplyTowardZeroF32,
            BuiltinFunction::FloatMultiplyTowardZeroF64,
            BuiltinFunction::FloatMultiplyTowardPositiveF32,
            BuiltinFunction::FloatMultiplyTowardPositiveF64,
            BuiltinFunction::FloatMultiplyTowardNegativeF32,
            BuiltinFunction::FloatMultiplyTowardNegativeF64,
            BuiltinFunction::FloatDivideTowardZeroF32,
            BuiltinFunction::FloatDivideTowardZeroF64,
            BuiltinFunction::FloatDivideTowardPositiveF32,
            BuiltinFunction::FloatDivideTowardPositiveF64,
            BuiltinFunction::FloatDivideTowardNegativeF32,
            BuiltinFunction::FloatDivideTowardNegativeF64,
            BuiltinFunction::FloatSqrtTowardZeroF32,
            BuiltinFunction::FloatSqrtTowardZeroF64,
            BuiltinFunction::FloatSqrtTowardPositiveF32,
            BuiltinFunction::FloatSqrtTowardPositiveF64,
            BuiltinFunction::FloatSqrtTowardNegativeF32,
            BuiltinFunction::FloatSqrtTowardNegativeF64,
            BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32,
            BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64,
            BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32,
            BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64,
            BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32,
            BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64,
        ] {
            assert_eq!(
                table[function.ordinal()].1.as_str(),
                function.name(),
                "ordinal for {:?} does not match the symbol table position",
                function
            );
        }
    }

    #[test]
    fn asm_intrinsics_are_unnameable_and_contract_bearing() {
        // Every asm intrinsic name contains `#` (not an identifier character),
        // so source code cannot reference it -- only the parser's asm-block
        // desugar can. Every member is recognized as an asm intrinsic even
        // when its service-reach effect component is empty (as for fences).
        for function in BuiltinFunction::asm_intrinsics() {
            assert!(
                function.name().contains('#'),
                "{:?} must be unnameable from source",
                function
            );
            assert!(
                function.is_asm_intrinsic(),
                "{:?} must carry an instruction contract",
                function
            );
        }
        assert_eq!(
            BuiltinFunction::AsmHlt.asm_intrinsic_service_name(),
            Some("MachineControl")
        );
        assert_eq!(
            BuiltinFunction::AsmPortOut.asm_intrinsic_service_name(),
            Some("PortIo")
        );
        assert_eq!(
            BuiltinFunction::AsmPortIn.asm_intrinsic_service_name(),
            Some("PortIo")
        );
        assert_eq!(
            BuiltinFunction::AsmFullFence.asm_intrinsic_service_name(),
            None
        );
        assert_eq!(
            BuiltinFunction::AsmDisableInterrupts.asm_intrinsic_service_name(),
            Some("MachineControl")
        );
    }
}
