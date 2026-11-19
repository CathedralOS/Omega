/*
    Closed identities for compiler-installed functions. The function ordinal indexes
    this catalog. Source resolution installs the catalog after the built-in types
    in the root namespace.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
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
    AddressOf,
    CodeLayout,
    QuotientLift,
    QuotientDefine,
}

macro_rules! roster {
    ($name:ident => $spelling:literal) => {
        (Self::$name, $spelling)
    };
}

impl BuiltinFunction {
    pub const COUNT: usize = 109;

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
        (Self::AddressOf, "address_of"),
        (Self::CodeLayout, "code_layout"),
        (Self::QuotientLift, "Quotient::lift"),
        (Self::QuotientDefine, "Quotient::define"),
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

    /// These source forms validate their operands at their operation owner, not
    /// against an ordinary callable signature. Their names still resolve normally,
    /// so an authored declaration can shadow them without receiving this identity.
    pub const fn is_source_special_form(self) -> bool {
        matches!(
            self,
            Self::AddressOf | Self::CodeLayout | Self::QuotientLift | Self::QuotientDefine
        )
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
        Whether this builtin is one of the instruction-bearing asm
        intrinsics. Membership reads [`Self::ALL`]'s own spellings: every asm
        intrinsic is installed with the unnameable `asm#` prefix.
    */
    pub fn is_asm_intrinsic(self) -> bool {
        self.name().starts_with("asm#")
    }
}
