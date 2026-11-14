/*
    The builtin roster: source-free types, their members, builtin functions and
    fixed core declarations, each installed at an exact root-child ordinal.
*/

use super::{SymbolKind, SymbolNameRef};
use numerics::integer_carrier::FixedInteger;
use std::borrow::Cow;

pub const BUILTIN_TYPE_COUNT: usize = 21;

/*
    Closed semantic identity for every compiler-installed source-free type.
    Result is ordinary core-library data, not a source-free type.
    Ordinals are the exact root-child slots installed by
    [`builtin_type_symbols`]; names are diagnostic only.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum BuiltinTypeAtom {
    Bool,
    /*
        A fixed-width integer is its sign and width, at every width. The
        symbol table holds one `BuiltinType` symbol per carrier a compilation
        names: eight have fixed root slots that published positions depend
        on, the others are installed beside the program's declarations
        (`SymbolTable::integer_type_symbol`). Either way the identity is this
        carrier, never the slot or the spelling.
    */
    Integer(FixedInteger),
    /* Pointer-width address, distinct from integer counts. */
    Address,
    F32,
    F64,
    Slice,
    SyscallResult,
    Terminal,
    Never,
    UInt,
    Int,
    /*
        Atomic types retain distinct identities even where layout matches the
        underlying primitive.
    */
    AtomicBool,
    AtomicU32,
    AtomicU64,
}

impl BuiltinTypeAtom {
    pub const ALL: [Self; BUILTIN_TYPE_COUNT] = [
        Self::Bool,
        Self::Integer(FixedInteger::signed(8)),
        Self::Integer(FixedInteger::signed(16)),
        Self::Integer(FixedInteger::signed(32)),
        Self::Integer(FixedInteger::signed(64)),
        Self::Integer(FixedInteger::unsigned(8)),
        Self::Integer(FixedInteger::unsigned(16)),
        Self::Integer(FixedInteger::unsigned(32)),
        Self::Integer(FixedInteger::unsigned(64)),
        Self::Address,
        Self::F32,
        Self::F64,
        Self::Slice,
        Self::SyscallResult,
        Self::Terminal,
        Self::Never,
        Self::UInt,
        Self::Int,
        Self::AtomicBool,
        Self::AtomicU32,
        Self::AtomicU64,
    ];

    pub fn from_ordinal(ordinal: usize) -> Option<Self> {
        Self::ALL.get(ordinal).copied()
    }

    pub const U8: Self = Self::Integer(FixedInteger::U8);
    pub const U64: Self = Self::Integer(FixedInteger::U64);

    /*
        The fixed root slot of an atom written in compiler source: its
        position in [`Self::ALL`]. An integer carrier a program names may
        have no fixed slot, so code holding an arbitrary atom asks
        [`Self::roster_ordinal`] or the symbol table instead.
    */
    pub const fn ordinal(self) -> usize {
        match self.roster_ordinal() {
            Some(ordinal) => ordinal,
            None => panic!("a builtin type outside the root roster has no slot"),
        }
    }

    /* The root slot of a roster member, compared in const context. */
    pub const fn roster_ordinal(self) -> Option<usize> {
        let mut index = 0;
        while index < BUILTIN_TYPE_COUNT {
            if Self::ALL[index].same_atom(self) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    const fn same_atom(self, other: Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => {
                left.width() == right.width() && left.is_signed() == right.is_signed()
            }
            (Self::Integer(_), _) | (_, Self::Integer(_)) => false,
            _ => self.payload_free_tag() == other.payload_free_tag(),
        }
    }

    const fn payload_free_tag(self) -> u8 {
        match self {
            Self::Bool => 0,
            Self::Integer(_) => 1,
            Self::Address => 2,
            Self::F32 => 3,
            Self::F64 => 4,
            Self::Slice => 5,
            Self::SyscallResult => 6,
            Self::Terminal => 7,
            Self::Never => 8,
            Self::UInt => 9,
            Self::Int => 10,
            Self::AtomicBool => 11,
            Self::AtomicU32 => 12,
            Self::AtomicU64 => 13,
        }
    }

    /* The carrier of a fixed-width integer atom. */
    pub const fn fixed_integer(self) -> Option<FixedInteger> {
        match self {
            Self::Integer(carrier) => Some(carrier),
            _ => None,
        }
    }

    /*
        Whether this atom is one of the dedicated atomic core types whose
        storage is operated on through the sealed atomic requirements (see
        `wiki/spec/language/concurrency.md`). Ordinary integers are never
        implicitly atomic, and a package type spelled `Atomic...` is not an
        atom at all, so this is the classification to use instead of a name
        prefix.
    */
    pub const fn is_atomic(self) -> bool {
        matches!(self, Self::AtomicBool | Self::AtomicU32 | Self::AtomicU64)
    }

    /*
        Whether the atom spells a canonical constant's scalar carrier: bool,
        the fixed-width integers, or the pointer-width address — the
        shapes `DecodedCanonicalConstValue` can hold. Arbitrary-width
        `Int`/`UInt`, floats, and composite atoms are not carriers.
    */
    pub const fn is_canonical_const_scalar(self) -> bool {
        matches!(self, Self::Bool | Self::Address) || self.is_fixed_integer()
    }

    /*
        Whether the atom is a fixed-width integer of any sign and width;
        the arbitrary-width `Int`/`UInt` and pointer-width
        `Address` are not members.
    */
    pub const fn is_fixed_integer(self) -> bool {
        matches!(self, Self::Integer(_))
    }

    /*
        One roster row per member in [`Self::ALL`] order: the atom's
        diagnostic identity and its installed symbol spelling. Both readers
        index this roster instead of restating the variant list once each.
    */
    const SPELLINGS: [(Self, &'static str, &'static str); BUILTIN_TYPE_COUNT] = [
        (Self::Bool, "bool", "bool"),
        (Self::Integer(FixedInteger::signed(8)), "i8", "i8"),
        (Self::Integer(FixedInteger::signed(16)), "i16", "i16"),
        (Self::Integer(FixedInteger::signed(32)), "i32", "i32"),
        (Self::Integer(FixedInteger::signed(64)), "i64", "i64"),
        (Self::Integer(FixedInteger::unsigned(8)), "u8", "u8"),
        (Self::Integer(FixedInteger::unsigned(16)), "u16", "u16"),
        (Self::Integer(FixedInteger::unsigned(32)), "u32", "u32"),
        (Self::Integer(FixedInteger::unsigned(64)), "u64", "u64"),
        (Self::Address, "address", "addr"),
        (Self::F32, "f32", "f32"),
        (Self::F64, "f64", "f64"),
        (Self::Slice, "slice", "Slice"),
        (Self::SyscallResult, "syscall-result", "SyscallResult"),
        (Self::Terminal, "terminal", "Terminal"),
        (Self::Never, "never", "Never"),
        (Self::UInt, "uint", "UInt"),
        (Self::Int, "int", "Int"),
        (Self::AtomicBool, "atomic-bool", "AtomicBool"),
        (Self::AtomicU32, "atomic-u32", "AtomicU32"),
        (Self::AtomicU64, "atomic-u64", "AtomicU64"),
    ];

    /*
        The diagnostic identity of any atom. An integer's is its decimal
        spelling at every width, so it needs no roster row.
    */
    pub fn identity(self) -> Cow<'static, str> {
        self.spelling(|row| row.1)
    }

    /* The name the atom's symbol is installed under. */
    pub fn symbol_name(self) -> Cow<'static, str> {
        self.spelling(|row| row.2)
    }

    fn spelling(
        self,
        column: fn(&(Self, &'static str, &'static str)) -> &'static str,
    ) -> Cow<'static, str> {
        match (self.roster_ordinal(), self) {
            (Some(ordinal), _) => Cow::Borrowed(column(&Self::SPELLINGS[ordinal])),
            (None, Self::Integer(carrier)) => Cow::Owned(carrier.to_string()),
            (None, _) => unreachable!("every atom but an integer carrier is a roster member"),
        }
    }
}

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

    /*
        The atom's slot in the shared builtin type inventory. The two enums
        must not keep separate copies of the slot number: the `usize`/`isize`
        retirement shifted these positions once already, and the
        builtin_type_ordinals_track_the_symbol_table unit test pins the
        coupling to the symbol table.
    */
    pub fn ordinal(self) -> usize {
        match self {
            Self::UInt => BuiltinTypeAtom::UInt.ordinal(),
            Self::Int => BuiltinTypeAtom::Int.ordinal(),
        }
    }
}

/*
    Closed semantic identity for every compiler-installed source-free
    function. Ordinals are the exact root-child slots following the builtin
    type slots installed by [`builtin_function_symbols`]; names are diagnostic
    only.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum BuiltinFunction {
    /*
        Unnameable execution identities selected only by admitted floating
        requirements. Ordinary library helpers have no builtin source identity.
    */
    FloatMaximum,
    FloatMinimum,
    FloatSquareRoot,
    /*
        `asm { hlt }`: the x86 privileged halt instruction as a known-contract
        asm intrinsic (wiki/spec/build/permissions.md). Reaches the
        canonical `MachineControl` service. The `#` in the symbol name is not an
        identifier character, so the intrinsic is UNNAMEABLE from source --
        only the parser's asm-block desugar can reference it.
    */
    AsmHlt,
    /*
        `asm { out <port>, <value> }`: x86 port write (`out dx, al`). Reaches
        the canonical `PortIo` service. Unnameable from source (see AsmHlt).
    */
    AsmPortOut,
    /*
        `asm { in <destination>, <port> }`: x86 port read (`in al, dx`).
        Reaches the canonical `PortIo` service. Unnameable from source (see AsmHlt).
    */
    AsmPortIn,
    /*
        x86 memory-ordering fences. They are unnameable zero-operand asm
        intrinsics and carry no service-reach effect.
    */
    AsmLoadFence,
    AsmStoreFence,
    AsmFullFence,
    AsmAarch64LoadFence,
    AsmAarch64StoreFence,
    AsmAarch64FullFence,
    /*
        x86 interrupt-enable flag control. Both are unnameable, zero-operand
        asm intrinsics with canonical `MachineControl` reach.
    */
    AsmDisableInterrupts,
    AsmEnableInterrupts,
    /*
        Compiler-balanced RFLAGS snapshot/restore intrinsics. The source
        spelling uses explicit u64 places; raw stack-mutating push/pop is not
        an admitted intrinsic.
    */
    AsmSnapshotFlags,
    AsmRestoreFlags,
    /*
        Structured x86 model-specific-register access. `rdmsr` returns a u64;
        `wrmsr` consumes a u32 index and u64 value. Both are unnameable
        machine-control intrinsics.
    */
    AsmReadMsr,
    AsmWriteMsr,
    AsmReadCr0,
    AsmReadCr2,
    AsmReadCr3,
    AsmReadCr4,
    AsmWriteCr0,
    AsmWriteCr3,
    AsmWriteCr4,
    /*
        Instruction-stream serialization and scheduling-hint directives
        (`serialize`/`isb`, `pause`/`yield`/`nop`, AArch64
        `wfe`/`wfi`/`sev`/`sevl`). Zero-operand asm intrinsics with no
        service-reach effect and no modeled machine-state write; legal
        pipeline behavior is pinned by the catalog contract.
    */
    AsmSerialize,
    AsmInstructionSyncBarrier,
    AsmSpinPause,
    AsmYieldHint,
    AsmNop,
    AsmWaitForEvent,
    AsmWaitForInterrupt,
    AsmSendEvent,
    AsmSendEventLocal,
    /*
        x86 cache-maintenance operations (`wbinvd`/`invd`/`wbnoinvd`):
        serializing privileged operations that act on the cache hierarchy
        rather than a modeled place. Each reaches the canonical
        `MachineControl` service and requires machine-owner authority.
        Unnameable from source (see AsmHlt).
    */
    AsmWriteBackInvalidate,
    AsmInvalidate,
    AsmWriteBackNoInvalidate,
    /*
        AArch64 system-register access (`asm { read_sctlr_el1 dst }` /
        `asm { write_sctlr_el1 src }` lowers to `mrs`/`msr`): each reaches the
        canonical `MachineControl` service and requires machine-owner
        authority. Syndrome registers (ESR/FAR) are read-only in the catalog.
        Unnameable from source (see AsmHlt).
    */
    AsmReadSctlrEl1,
    AsmReadTcrEl1,
    AsmReadTtbr0El1,
    AsmReadTtbr1El1,
    AsmReadMairEl1,
    AsmReadVbarEl1,
    AsmReadTpidrEl1,
    AsmReadEsrEl1,
    AsmReadFarEl1,
    AsmWriteSctlrEl1,
    AsmWriteTcrEl1,
    AsmWriteTtbr0El1,
    AsmWriteTtbr1El1,
    AsmWriteMairEl1,
    AsmWriteVbarEl1,
    AsmWriteTpidrEl1,
    /*
        Internal unary predicate used only by selected named-float plans. The The
        `#` keeps it unnameable from source during the provider migration.
    */
    FloatIsNan,
    /*
        Internal ternary operations selected only by exact named-float plans.
        Separate symbols retain the source format after expression tables copy
        the checked call into state-local lowering tables.
    */
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
    /*
        Proof-only callable-entry versioning for content-conservation
        contracts. This is source-nameable as contextual `old(place)` only in
        proof facts and never has a runtime implementation.
    */
    ContentOld,
    /*
        Proof-only partial n-ary composition for the compiler-owned content
        algebras. Validation selects the algebra from the exact projection
        terms; packages cannot implement or override this operation.
    */
    ContentSeparate,
    /*
        Internal binary operations selected only by exact directed-float plans.
        The format and rounding direction remain explicit in the symbol after
        state-local expression copying. These append to the stable builtin
        ordinal sequence rather than renumbering existing content operations.
    */
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
    /*
        Total, proof-only projection of a fixed-width integer or address into
        mathematical Int. This has no callable implementation or runtime ABI.
    */
    IntegerEmbed,
    AsmStoreGdtr,
    AsmLoadGdtr,
}

macro_rules! roster {
    ($name:ident => $spelling:literal) => {
        (Self::$name, $spelling)
    };
}

impl BuiltinFunction {
    pub const COUNT: usize = 105;

    /*
        The closed inventory in slot order: every compiler-installed builtin
        function paired with its exact spelling. The array position is the
        slot ordinal ([`Self::ordinal`]) and the root-child slot the symbol
        table installs, so ordinals, spellings and symbol rows all read this
        one roster instead of parallel transcriptions of it.

        Package review evidence records a builtin function by this ordinal,
        so the roster is append-only: a new builtin takes the next free slot
        at the end, never a position beside its relatives, or recorded
        evidence would silently decode as a different function.
    */
    pub const ALL: [(Self, &'static str); Self::COUNT] = [
        (Self::FloatMaximum, "float#maximum"),
        (Self::FloatMinimum, "float#minimum"),
        (Self::FloatSquareRoot, "float#square_root"),
        (Self::AsmHlt, "asm#hlt"),
        (Self::AsmPortOut, "asm#port_out"),
        (Self::AsmPortIn, "asm#port_in"),
        (Self::AsmLoadFence, "asm#lfence"),
        (Self::AsmStoreFence, "asm#sfence"),
        (Self::AsmFullFence, "asm#mfence"),
        (Self::AsmDisableInterrupts, "asm#cli"),
        (Self::AsmEnableInterrupts, "asm#sti"),
        (Self::AsmSnapshotFlags, "asm#pushfq"),
        (Self::AsmRestoreFlags, "asm#popfq"),
        (Self::AsmReadMsr, "asm#rdmsr"),
        (Self::AsmWriteMsr, "asm#wrmsr"),
        (Self::AsmReadCr0, "asm#read_cr0"),
        (Self::AsmReadCr2, "asm#read_cr2"),
        (Self::AsmReadCr3, "asm#read_cr3"),
        (Self::AsmReadCr4, "asm#read_cr4"),
        (Self::AsmWriteCr0, "asm#write_cr0"),
        (Self::AsmWriteCr3, "asm#write_cr3"),
        (Self::AsmWriteCr4, "asm#write_cr4"),
        (Self::FloatIsNan, "float#is_nan"),
        (Self::FloatMultiplyThenAddF32, "float#multiply_then_add_f32"),
        (Self::FloatMultiplyThenAddF64, "float#multiply_then_add_f64"),
        roster!(FloatFusedMultiplyAddF32 => "float#fused_multiply_add_f32"),
        roster!(FloatFusedMultiplyAddF64 => "float#fused_multiply_add_f64"),
        (Self::FloatIsFinite, "float#is_finite"),
        (Self::FloatIsInfinite, "float#is_infinite"),
        (Self::FloatIsNormal, "float#is_normal"),
        (Self::FloatIsSubnormal, "float#is_subnormal"),
        (Self::FloatClassifyF32, "float#classify_f32"),
        (Self::FloatClassifyF64, "float#classify_f64"),
        (Self::ContentOld, "old"),
        (Self::ContentSeparate, "separate"),
        (Self::FloatAddTowardZeroF32, "float#add_toward_zero_f32"),
        (Self::FloatAddTowardZeroF64, "float#add_toward_zero_f64"),
        roster!(FloatAddTowardPositiveF32 => "float#add_toward_positive_f32"),
        roster!(FloatAddTowardPositiveF64 => "float#add_toward_positive_f64"),
        roster!(FloatAddTowardNegativeF32 => "float#add_toward_negative_f32"),
        roster!(FloatAddTowardNegativeF64 => "float#add_toward_negative_f64"),
        roster!(FloatSubtractTowardZeroF32 => "float#subtract_toward_zero_f32"),
        roster!(FloatSubtractTowardZeroF64 => "float#subtract_toward_zero_f64"),
        roster!(FloatSubtractTowardPositiveF32 => "float#subtract_toward_positive_f32"),
        roster!(FloatSubtractTowardPositiveF64 => "float#subtract_toward_positive_f64"),
        roster!(FloatSubtractTowardNegativeF32 => "float#subtract_toward_negative_f32"),
        roster!(FloatSubtractTowardNegativeF64 => "float#subtract_toward_negative_f64"),
        roster!(FloatMultiplyTowardZeroF32 => "float#multiply_toward_zero_f32"),
        roster!(FloatMultiplyTowardZeroF64 => "float#multiply_toward_zero_f64"),
        roster!(FloatMultiplyTowardPositiveF32 => "float#multiply_toward_positive_f32"),
        roster!(FloatMultiplyTowardPositiveF64 => "float#multiply_toward_positive_f64"),
        roster!(FloatMultiplyTowardNegativeF32 => "float#multiply_toward_negative_f32"),
        roster!(FloatMultiplyTowardNegativeF64 => "float#multiply_toward_negative_f64"),
        roster!(FloatDivideTowardZeroF32 => "float#divide_toward_zero_f32"),
        roster!(FloatDivideTowardZeroF64 => "float#divide_toward_zero_f64"),
        roster!(FloatDivideTowardPositiveF32 => "float#divide_toward_positive_f32"),
        roster!(FloatDivideTowardPositiveF64 => "float#divide_toward_positive_f64"),
        roster!(FloatDivideTowardNegativeF32 => "float#divide_toward_negative_f32"),
        roster!(FloatDivideTowardNegativeF64 => "float#divide_toward_negative_f64"),
        (Self::FloatSqrtTowardZeroF32, "float#sqrt_toward_zero_f32"),
        (Self::FloatSqrtTowardZeroF64, "float#sqrt_toward_zero_f64"),
        roster!(FloatSqrtTowardPositiveF32 => "float#sqrt_toward_positive_f32"),
        roster!(FloatSqrtTowardPositiveF64 => "float#sqrt_toward_positive_f64"),
        roster!(FloatSqrtTowardNegativeF32 => "float#sqrt_toward_negative_f32"),
        roster!(FloatSqrtTowardNegativeF64 => "float#sqrt_toward_negative_f64"),
        roster!(FloatFusedMultiplyAddTowardZeroF32 => "float#fused_multiply_add_toward_zero_f32"),
        roster!(FloatFusedMultiplyAddTowardZeroF64 => "float#fused_multiply_add_toward_zero_f64"),
        (
            Self::FloatFusedMultiplyAddTowardPositiveF32,
            "float#fused_multiply_add_toward_positive_f32",
        ),
        (
            Self::FloatFusedMultiplyAddTowardPositiveF64,
            "float#fused_multiply_add_toward_positive_f64",
        ),
        (
            Self::FloatFusedMultiplyAddTowardNegativeF32,
            "float#fused_multiply_add_toward_negative_f32",
        ),
        (
            Self::FloatFusedMultiplyAddTowardNegativeF64,
            "float#fused_multiply_add_toward_negative_f64",
        ),
        (Self::IntegerEmbed, "embed"),
        (Self::AsmSerialize, "asm#serialize"),
        (Self::AsmInstructionSyncBarrier, "asm#isb"),
        (Self::AsmSpinPause, "asm#pause"),
        (Self::AsmYieldHint, "asm#yield"),
        (Self::AsmWriteBackInvalidate, "asm#wbinvd"),
        (Self::AsmInvalidate, "asm#invd"),
        (Self::AsmWriteBackNoInvalidate, "asm#wbnoinvd"),
        (Self::AsmNop, "asm#nop"),
        (Self::AsmWaitForEvent, "asm#wfe"),
        (Self::AsmWaitForInterrupt, "asm#wfi"),
        (Self::AsmSendEvent, "asm#sev"),
        (Self::AsmSendEventLocal, "asm#sevl"),
        (Self::AsmReadSctlrEl1, "asm#read_sctlr_el1"),
        (Self::AsmReadTcrEl1, "asm#read_tcr_el1"),
        (Self::AsmReadTtbr0El1, "asm#read_ttbr0_el1"),
        (Self::AsmReadTtbr1El1, "asm#read_ttbr1_el1"),
        (Self::AsmReadMairEl1, "asm#read_mair_el1"),
        (Self::AsmReadVbarEl1, "asm#read_vbar_el1"),
        (Self::AsmReadTpidrEl1, "asm#read_tpidr_el1"),
        (Self::AsmReadEsrEl1, "asm#read_esr_el1"),
        (Self::AsmReadFarEl1, "asm#read_far_el1"),
        (Self::AsmWriteSctlrEl1, "asm#write_sctlr_el1"),
        (Self::AsmWriteTcrEl1, "asm#write_tcr_el1"),
        (Self::AsmWriteTtbr0El1, "asm#write_ttbr0_el1"),
        (Self::AsmWriteTtbr1El1, "asm#write_ttbr1_el1"),
        (Self::AsmWriteMairEl1, "asm#write_mair_el1"),
        (Self::AsmWriteVbarEl1, "asm#write_vbar_el1"),
        (Self::AsmWriteTpidrEl1, "asm#write_tpidr_el1"),
        /* Slots 100..=102: appended after the published 0..=99 roster. */
        (Self::AsmAarch64LoadFence, "asm#dmb_ld"),
        (Self::AsmAarch64StoreFence, "asm#dmb_st"),
        (Self::AsmAarch64FullFence, "asm#dmb_sy"),
        (Self::AsmStoreGdtr, "asm#sgdt"),
        (Self::AsmLoadGdtr, "asm#lgdt"),
    ];

    pub fn from_ordinal(ordinal: usize) -> Option<Self> {
        Self::ALL.get(ordinal).map(|(function, _)| *function)
    }

    /*
        The builtin function spelled `name`, or `None` when no compiler-installed
        function carries that name. Stages that only hold a call's target
        spelling ask here instead of keeping their own copy of the names.
    */
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|(_, spelling)| *spelling == name)
            .map(|(function, _)| function)
    }

    /*
        Whether this builtin is one of the value-position asm intrinsics
        (`in`, `pushfq`, `rdmsr`): the parser desugars the instruction to
        `<dest> = asm#...(..)`, so the call yields a value in expression
        position where every other asm intrinsic is a statement call. A
        receiverless value call to one of these is therefore not an unresolved
        machine call. The control- and system-register reads (`read_crN`,
        `read_<sysreg>`) are value-position too, but their names, mnemonics and
        operand contracts are owned by `language_core::inline_assembly`'s
        register catalogs, which the same sites ask beside this predicate; this
        set is exactly the three value intrinsics that are not register reads.
    */
    pub const fn is_value_position_asm_intrinsic(self) -> bool {
        matches!(
            self,
            Self::AsmPortIn | Self::AsmSnapshotFlags | Self::AsmReadMsr
        )
    }

    /*
        The installation spelling paired with this function in the closed
        inventory.
    */
    pub fn name(self) -> &'static str {
        Self::ALL[self.ordinal()].1
    }

    /*
        The function's slot ordinal: its position in [`Self::ALL`], which is
        also the root-child slot `builtin_function_symbol` resolves.
    */
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|(function, _)| *function == self)
            .expect("builtin function belongs to its closed inventory")
    }

    /*
        The canonical boundary-service identity reached by an asm intrinsic,
        or None when the intrinsic reaches no service (and for value builtins).
        Operand, clobber, ordering, and availability metadata lives in the
        shared inline-assembly catalog.
    */
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
            | Self::AsmWriteCr4
            | Self::AsmStoreGdtr
            | Self::AsmLoadGdtr
            | Self::AsmWriteBackInvalidate
            | Self::AsmInvalidate
            | Self::AsmWriteBackNoInvalidate
            | Self::AsmReadSctlrEl1
            | Self::AsmReadTcrEl1
            | Self::AsmReadTtbr0El1
            | Self::AsmReadTtbr1El1
            | Self::AsmReadMairEl1
            | Self::AsmReadVbarEl1
            | Self::AsmReadTpidrEl1
            | Self::AsmReadEsrEl1
            | Self::AsmReadFarEl1
            | Self::AsmWriteSctlrEl1
            | Self::AsmWriteTcrEl1
            | Self::AsmWriteTtbr0El1
            | Self::AsmWriteTtbr1El1
            | Self::AsmWriteMairEl1
            | Self::AsmWriteVbarEl1
            | Self::AsmWriteTpidrEl1 => Some("MachineControl"),
            Self::AsmPortOut | Self::AsmPortIn => Some("PortIo"),
            service if service.is_float_family() => None,
            Self::ContentOld
            | Self::ContentSeparate
            | Self::IntegerEmbed
            | Self::AsmLoadFence
            | Self::AsmStoreFence
            | Self::AsmFullFence
            | Self::AsmAarch64LoadFence
            | Self::AsmAarch64StoreFence
            | Self::AsmAarch64FullFence
            | Self::AsmSerialize
            | Self::AsmInstructionSyncBarrier
            | Self::AsmSpinPause
            | Self::AsmYieldHint
            | Self::AsmNop
            | Self::AsmWaitForEvent
            | Self::AsmWaitForInterrupt
            | Self::AsmSendEvent
            | Self::AsmSendEventLocal
            | Self::AsmSnapshotFlags => None,
            _ => None,
        }
    }

    /*
        Unnameable selected floating operations, predicates, classifications, and
        every directed-rounding arithmetic and fused-multiply-add spelling.
        Classifiers list the family once; consumers select membership only.
    */
    pub const fn is_float_family(self) -> bool {
        matches!(
            self,
            Self::FloatMaximum
                | Self::FloatMinimum
                | Self::FloatSquareRoot
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
        )
    }

    /*
        Whether a call to this builtin writes no caller storage: it reads its
        operands and produces a value. Write-frame inference gives such a
        receiver-free call an empty frame instead of the whole-receiver clobber
        of an unresolved call. Machine-control and port intrinsics, content
        custody builtins, and `embed` are excluded: their effects are their own
        custody axes, not value reads.
        Floating control-state effects do not write caller storage;
        their separate admission rules remain attached to the numeric operation.
    */
    pub const fn has_empty_write_frame(self) -> bool {
        self.is_float_family()
    }

    /*
        Whether this builtin is one of the instruction-bearing asm
        intrinsics. Membership reads [`Self::ALL`]'s own spellings: every asm
        intrinsic is installed with the unnameable `asm#` prefix.
    */
    pub fn is_asm_intrinsic(self) -> bool {
        self.name().starts_with("asm#")
    }
}

pub fn builtin_type_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BUILTIN_TYPE_COUNT] {
    BuiltinTypeAtom::SPELLINGS
        .map(|(_, _, symbol_name)| (SymbolKind::BuiltinType, SymbolNameRef::Static(symbol_name)))
}

pub fn builtin_function_symbols() -> [(SymbolKind, SymbolNameRef<'static>); BuiltinFunction::COUNT]
{
    BuiltinFunction::ALL
        .map(|(_, spelling)| (SymbolKind::BuiltinFunction, SymbolNameRef::Static(spelling)))
}

/*
    Closed semantic identity for the fixed `core::*` mathematical
    declarations (PROOF-CONTRACT-MIGRATION): the compiler-known universe and
    sort vocabulary the
    [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md) spec
    installs. Ordinals are the exact root-child slots following the builtin
    function slots; names are diagnostic only.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixedCoreDeclaration {
    /* `core::Level` — the universe-level domain. */
    Level,
    /* `core::Type` — the sort of types; `core::Type<u>` applies one level. */
    Type,
    /* `core::Strict` — the strict-proposition sort. */
    Strict,
    /* `core::Squash` — the witness-hiding proposition squash. */
    Squash,
    /// `core::Identity<T>(left, right)` — proof-relevant kernel identity.
    Identity,
}

impl FixedCoreDeclaration {
    pub const COUNT: usize = 5;

    /// The fixed vocabulary in root-child order: every member's
    /// [`ordinal`](Self::ordinal) is its index here.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Level,
        Self::Type,
        Self::Strict,
        Self::Squash,
        Self::Identity,
    ];

    /*
        The compiler-known name this declaration owns. The `core::` prefix
        is the mathematical vocabulary's fixed namespace — not the
        `omega::language::core` library package, and not shadowable by an
        authored declaration.
    */
    pub const fn name(self) -> &'static str {
        match self {
            Self::Level => "core::Level",
            Self::Type => "core::Type",
            Self::Strict => "core::Strict",
            Self::Squash => "core::Squash",
            Self::Identity => "core::Identity",
        }
    }

    /*
        The member's slot within the fixed vocabulary — its index in
        [`Self::ALL`]. The root-child position is
        `BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT + ordinal`.
    */
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .expect("fixed core declaration belongs to its closed inventory")
    }

    pub fn from_ordinal(ordinal: usize) -> Option<Self> {
        Self::ALL.get(ordinal).copied()
    }

    /*
        The fixed declaration an authored `core::*` spelling denotes, if any.
        Names never identify a resolved symbol — this arm exists for the
        resolution pass that binds spelled references to their fixed slots
        and for the transitional unresolved-name classifiers downstream.
    */
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|declaration| declaration.name() == name)
    }
}

/*
    Root-child seeds for the fixed `core::*` mathematical declarations, in
    `FixedCoreDeclaration::ALL` order: installed immediately after the builtin
    function slots so their positions are stable before any authored
    declaration. `MathematicalDefinition` is their kind — they are the
    vocabulary that surface elaborates, though no authored declaration backs
    them.
*/
pub fn fixed_core_declaration_symbols()
-> [(SymbolKind, SymbolNameRef<'static>); FixedCoreDeclaration::COUNT] {
    FixedCoreDeclaration::ALL.map(|declaration| {
        (
            SymbolKind::MathematicalDefinition,
            SymbolNameRef::Static(declaration.name()),
        )
    })
}

/*
    Mathematical members owned by a builtin carrier. Their child slots, not
    authored names, identify them after resolution.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum BuiltinTypeMember {
    /* The selected address model's exclusive upper bound in proof Int. */
    AddressBound,
}

impl BuiltinTypeMember {
    pub const fn owner(self) -> BuiltinTypeAtom {
        match self {
            Self::AddressBound => BuiltinTypeAtom::Address,
        }
    }

    pub const fn ordinal(self) -> usize {
        0
    }
}

pub fn builtin_type_member_symbols(
    builtin_type: BuiltinTypeAtom,
) -> impl Iterator<Item = (SymbolKind, SymbolNameRef<'static>)> {
    (builtin_type == BuiltinTypeMember::AddressBound.owner())
        .then_some((
            SymbolKind::MathematicalDefinition,
            SymbolNameRef::Static("Bound"),
        ))
        .into_iter()
}

#[cfg(test)]
mod builtin_ordinal_tests {
    use super::{
        BuiltinFunction, BuiltinType, BuiltinTypeAtom, FixedCoreDeclaration, SymbolKind,
        SymbolNameRef, builtin_function_symbols, builtin_type_symbols,
        fixed_core_declaration_symbols,
    };

    #[test]
    fn builtin_type_ordinals_track_the_symbol_table() {
        let table = builtin_type_symbols();
        for builtin_type in [BuiltinType::UInt, BuiltinType::Int] {
            assert_eq!(
                table[builtin_type.ordinal()].1.as_str(),
                builtin_type.name(),
                "ordinal for {:?} does not match the symbol table position",
                builtin_type
            );
        }
        for atom in BuiltinTypeAtom::ALL {
            assert_eq!(
                (table[atom.ordinal()].0, table[atom.ordinal()].1.as_str()),
                (SymbolKind::BuiltinType, atom.symbol_name().as_ref()),
                "closed builtin atom {:?} must retain its exact root slot",
                atom,
            );
        }
    }

    /*
        An integer atom is its carrier. The root roster installs a slot for
        each conventional spelling, so those carriers have an ordinal and a
        symbol name; a carrier outside the roster is still an integer atom
        but has no slot to index.
    */
    #[test]
    fn integer_atoms_are_carriers_and_only_roster_spellings_have_slots() {
        use numerics::integer_carrier::FixedInteger;
        let mut installed = 0;
        for (ordinal, atom) in BuiltinTypeAtom::ALL.into_iter().enumerate() {
            assert_eq!(atom.roster_ordinal(), Some(ordinal));
            assert_eq!(BuiltinTypeAtom::from_ordinal(ordinal), Some(atom));
            let Some(carrier) = atom.fixed_integer() else {
                assert!(!atom.is_fixed_integer());
                continue;
            };
            installed += 1;
            assert!(atom.is_fixed_integer() && atom.is_canonical_const_scalar());
            assert_eq!(atom.symbol_name(), carrier.to_string());
            assert_eq!(atom.identity(), carrier.to_string());
            assert_eq!(
                FixedInteger::from_decimal_spelling(&atom.symbol_name()),
                Some(carrier)
            );
        }
        assert_eq!(installed, 8);
        assert_eq!(BuiltinTypeAtom::U64.symbol_name(), "u64");
        assert_eq!(BuiltinTypeAtom::U8.symbol_name(), "u8");

        let odd = BuiltinTypeAtom::Integer(FixedInteger::unsigned(13));
        assert!(odd.is_fixed_integer());
        assert_eq!(odd.roster_ordinal(), None);
        assert_eq!(odd.symbol_name(), "u13");
        assert_eq!(odd.identity(), "u13");
        assert_ne!(
            BuiltinTypeAtom::Integer(FixedInteger::signed(32)).roster_ordinal(),
            BuiltinTypeAtom::Integer(FixedInteger::unsigned(32)).roster_ordinal()
        );
        for atom in [
            BuiltinTypeAtom::Address,
            BuiltinTypeAtom::UInt,
            BuiltinTypeAtom::Int,
        ] {
            assert_eq!(atom.fixed_integer(), None);
        }
    }

    #[test]
    fn builtin_function_ordinals_track_the_symbol_table() {
        let table = builtin_function_symbols();
        for (ordinal, (function, spelling)) in BuiltinFunction::ALL.into_iter().enumerate() {
            assert_eq!(
                function.ordinal(),
                ordinal,
                "closed function inventory must retain the canonical ordinal for {:?}",
                function,
            );
            assert_eq!(
                table[function.ordinal()].1.as_str(),
                spelling,
                "ordinal for {:?} does not match the symbol table position",
                function
            );
            assert_eq!(
                BuiltinFunction::from_ordinal(function.ordinal()),
                Some(function),
                "ordinal for {:?} must recover its closed identity",
                function,
            );
        }
        for (function, ordinal) in [
            (BuiltinFunction::FloatMaximum, 0),
            (BuiltinFunction::AsmHlt, 3),
            (BuiltinFunction::ContentOld, 33),
            (BuiltinFunction::IntegerEmbed, 71),
            (BuiltinFunction::AsmWriteTpidrEl1, 99),
            (BuiltinFunction::AsmAarch64LoadFence, 100),
            (BuiltinFunction::AsmAarch64FullFence, 102),
        ] {
            assert_eq!(
                function.ordinal(),
                ordinal,
                "{function:?} must keep its published slot number"
            );
        }
        assert_eq!(BuiltinFunction::from_ordinal(BuiltinFunction::COUNT), None);
    }

    #[test]
    fn builtin_function_names_round_trip_through_from_name() {
        for (function, spelling) in BuiltinFunction::ALL {
            assert_eq!(
                BuiltinFunction::from_name(spelling),
                Some(function),
                "{:?} must be recoverable from its own spelling",
                function,
            );
            assert_eq!(
                function.name(),
                spelling,
                "{:?} must read the spelling paired with it",
                function,
            );
        }
        assert_eq!(BuiltinFunction::from_name(""), None);
        assert_eq!(BuiltinFunction::from_name("Min"), None);
        for name in ["min", "max", "abs", "clamp", "sqrt"] {
            assert_eq!(BuiltinFunction::from_name(name), None);
        }
    }

    #[test]
    fn value_position_asm_intrinsics_are_exactly_port_in_pushfq_rdmsr() {
        let value_position: Vec<BuiltinFunction> = BuiltinFunction::ALL
            .into_iter()
            .map(|(function, _)| function)
            .filter(|function| function.is_value_position_asm_intrinsic())
            .collect();
        assert_eq!(
            value_position,
            [
                BuiltinFunction::AsmPortIn,
                BuiltinFunction::AsmSnapshotFlags,
                BuiltinFunction::AsmReadMsr
            ]
        );
        assert_eq!(
            value_position
                .iter()
                .map(|function| function.name())
                .collect::<Vec<_>>(),
            ["asm#port_in", "asm#pushfq", "asm#rdmsr"]
        );
        assert!(
            value_position
                .iter()
                .all(|function| function.is_asm_intrinsic())
        );
    }

    #[test]
    fn content_old_owns_the_stable_callable_entry_builtin_slot() {
        let table = builtin_function_symbols();
        assert_eq!(BuiltinFunction::ContentOld.ordinal(), 33);
        assert_eq!(BuiltinFunction::ContentOld.name(), "old");
        assert_eq!(table[33].1.as_str(), "old");
        assert!(
            table.iter().all(
                |(kind, name)| *kind != SymbolKind::BuiltinFunction || name.as_str() != "entry"
            ),
            "the retired ContentEntry builtin must not survive under its source spelling"
        );
    }

    #[test]
    fn asm_intrinsics_are_unnameable_and_contract_bearing() {
        let roster: Vec<BuiltinFunction> = BuiltinFunction::ALL
            .into_iter()
            .filter(|(_, spelling)| spelling.starts_with("asm#"))
            .map(|(function, _)| function)
            .collect();
        assert_eq!(
            roster.len(),
            52,
            "the unnameable instruction roster is closed"
        );
        assert!(roster.contains(&BuiltinFunction::AsmStoreGdtr));
        assert!(roster.contains(&BuiltinFunction::AsmLoadGdtr));
        for function in roster {
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
        for (function, _) in BuiltinFunction::ALL {
            assert!(
                function.asm_intrinsic_service_name().is_none() || function.is_asm_intrinsic(),
                "{:?} reaches a boundary service but is missing from the asm roster",
                function,
            );
        }
    }

    #[test]
    fn fixed_core_declaration_ordinals_track_the_symbol_table() {
        let table = fixed_core_declaration_symbols();
        for (ordinal, declaration) in FixedCoreDeclaration::ALL.into_iter().enumerate() {
            assert_eq!(
                declaration.ordinal(),
                ordinal,
                "closed fixed-core inventory must retain the canonical ordinal for {:?}",
                declaration,
            );
            assert_eq!(
                table[declaration.ordinal()],
                (
                    SymbolKind::MathematicalDefinition,
                    SymbolNameRef::Static(declaration.name()),
                ),
                "fixed-core declaration {:?} must retain its exact root slot",
                declaration,
            );
            assert_eq!(
                FixedCoreDeclaration::from_ordinal(declaration.ordinal()),
                Some(declaration),
                "ordinal for {:?} must recover its closed identity",
                declaration,
            );
            assert_eq!(
                FixedCoreDeclaration::from_name(declaration.name()),
                Some(declaration),
                "{:?} must be recoverable from its own spelling",
                declaration,
            );
        }
        assert_eq!(
            FixedCoreDeclaration::from_ordinal(FixedCoreDeclaration::COUNT),
            None
        );
        assert_eq!(FixedCoreDeclaration::from_name("core::Vec"), None);
    }
}
