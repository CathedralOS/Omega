use super::{SymbolKind, SymbolNameRef};

pub const BUILTIN_TYPE_COUNT: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    UInt,
    Int,
    Real,
}

impl BuiltinType {
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::UInt, Self::Int, Self::Real]
            .into_iter()
            .find(|builtin_type| builtin_type.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::UInt => "UInt",
            Self::Int => "Int",
            Self::Real => "Real",
        }
    }

    pub fn ordinal(self) -> usize {
        // POSITION in `builtin_type_symbols()` -- symbol handles are assigned
        // in table order, so these MUST track that array. (The usize/isize
        // retirement removed two entries above these and shifted them from
        // 19/20/21; the builtin_type_ordinals_track_the_symbol_table unit
        // test pins the coupling.)
        match self {
            Self::UInt => 18,
            Self::Int => 19,
            Self::Real => 20,
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
    /// asm intrinsic (privileged_effects_and_binary_trust brief). Emits the
    /// `machine_control` effect. The `#` in the symbol name is not an
    /// identifier character, so the intrinsic is UNNAMEABLE from source --
    /// only the parser's asm-block desugar can reference it.
    AsmHlt,
    /// `asm { out <port>, <value> }`: x86 port write (`out dx, al`). Emits
    /// the `device_io` effect. Unnameable from source (see AsmHlt).
    AsmPortOut,
    /// `asm { in <destination>, <port> }`: x86 port read (`in al, dx`).
    /// Emits the `device_io` effect. Unnameable from source (see AsmHlt).
    AsmPortIn,
    /// x86 memory-ordering fences. They are unnameable zero-operand asm
    /// intrinsics and carry no service-reach effect.
    AsmLoadFence,
    AsmStoreFence,
    AsmFullFence,
    /// x86 interrupt-enable flag control. Both are unnameable, zero-operand
    /// asm intrinsics with `machine_control` reach.
    AsmDisableInterrupts,
    AsmEnableInterrupts,
}

impl BuiltinFunction {
    pub const COUNT: usize = 11;

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
        }
    }

    /// The service-reach effect component of an asm intrinsic contract, or
    /// None when the intrinsic is effect-free (and for value builtins).
    /// Operand, clobber, ordering, and availability metadata lives in the
    /// shared inline-assembly catalog.
    pub fn asm_intrinsic_effect_name(self) -> Option<&'static str> {
        match self {
            Self::AsmHlt | Self::AsmDisableInterrupts | Self::AsmEnableInterrupts => {
                Some("machine_control")
            }
            Self::AsmPortOut | Self::AsmPortIn => Some("device_io"),
            Self::Max
            | Self::Min
            | Self::Sqrt
            | Self::AsmLoadFence
            | Self::AsmStoreFence
            | Self::AsmFullFence => None,
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
        )
    }

    pub fn asm_intrinsics() -> [Self; 8] {
        [
            Self::AsmHlt,
            Self::AsmPortOut,
            Self::AsmPortIn,
            Self::AsmLoadFence,
            Self::AsmStoreFence,
            Self::AsmFullFence,
            Self::AsmDisableInterrupts,
            Self::AsmEnableInterrupts,
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
        (SymbolKind::BuiltinType, SymbolNameRef::Static("String")),
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
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static(BuiltinType::Real.name()),
        ),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("string")),
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
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinTypeMember {
    RealFrom,
}

impl BuiltinTypeMember {
    pub fn owner(self) -> BuiltinType {
        match self {
            Self::RealFrom => BuiltinType::Real,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::RealFrom => "from",
        }
    }
}

pub fn builtin_type_member_symbols(
    builtin_type: BuiltinType,
) -> impl Iterator<Item = (SymbolKind, SymbolNameRef<'static>)> {
    [BuiltinTypeMember::RealFrom]
        .into_iter()
        .filter(move |member| member.owner() == builtin_type)
        .map(|member| {
            (
                SymbolKind::BuiltinFunction,
                SymbolNameRef::Static(member.name()),
            )
        })
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
        // "unknown layout-bearing type `UInt`").
        let table = builtin_type_symbols();
        for builtin_type in [BuiltinType::UInt, BuiltinType::Int, BuiltinType::Real] {
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
            BuiltinFunction::AsmHlt.asm_intrinsic_effect_name(),
            Some("machine_control")
        );
        assert_eq!(
            BuiltinFunction::AsmPortOut.asm_intrinsic_effect_name(),
            Some("device_io")
        );
        assert_eq!(
            BuiltinFunction::AsmPortIn.asm_intrinsic_effect_name(),
            Some("device_io")
        );
        assert_eq!(
            BuiltinFunction::AsmFullFence.asm_intrinsic_effect_name(),
            None
        );
        assert_eq!(
            BuiltinFunction::AsmDisableInterrupts.asm_intrinsic_effect_name(),
            Some("machine_control")
        );
    }
}
