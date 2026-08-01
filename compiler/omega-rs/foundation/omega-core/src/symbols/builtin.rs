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
    FloatIsFinite,
    FloatIsInfinite,
    FloatIsNormal,
    FloatIsSubnormal,
}

impl BuiltinFunction {
    pub const COUNT: usize = 29;

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
            Self::FloatIsFinite => "float#is_finite",
            Self::FloatIsInfinite => "float#is_infinite",
            Self::FloatIsNormal => "float#is_normal",
            Self::FloatIsSubnormal => "float#is_subnormal",
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
            Self::FloatIsFinite => 25,
            Self::FloatIsInfinite => 26,
            Self::FloatIsNormal => 27,
            Self::FloatIsSubnormal => 28,
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
            | Self::FloatIsFinite
            | Self::FloatIsInfinite
            | Self::FloatIsNormal
            | Self::FloatIsSubnormal
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
            BuiltinFunction::FloatIsFinite,
            BuiltinFunction::FloatIsInfinite,
            BuiltinFunction::FloatIsNormal,
            BuiltinFunction::FloatIsSubnormal,
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
