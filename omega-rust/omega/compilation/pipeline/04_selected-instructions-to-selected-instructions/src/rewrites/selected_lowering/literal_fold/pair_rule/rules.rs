//! The selected instruction-pair rule catalog: each `Self` constant names
//! the producer/consumer/rewritten triple a catalog row carries, with the
//! operand shape, immediate bound, result disposition, and declared effect
//! surfaces the rewrite must satisfy. Table-constructor `const fn`s mint
//! carrier-scaled families.

use super::*;

impl SelectedInstructionPairRule {
    pub const EXACT_ADD_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactAddI64,
        rewritten: MachineSemanticKind::ExactAddI64Immediate,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the left operand of `ExactAddI64`:
    /// exact addition commutes, so `literal + x` rewrites to the same
    /// `ExactAddI64Immediate` form `x + literal` uses. The same catalog
    /// selection admits both grammars; the pair disambiguates by which `Use`
    /// position the folded literal occupies.
    pub const EXACT_ADD_LEFT_IMMEDIATE_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::EXACT_ADD_IMMEDIATE_U12
    };
    pub const EXACT_SUBTRACT_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ExactSubtractI64,
        rewritten: MachineSemanticKind::ExactSubtractI64Immediate,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// subtrahend — of `CompareI64` when the folded literal is nonzero:
    /// `x - literal` rewrites to the `CompareI64Immediate` form. The
    /// family partitions its consumer kind and operand position on the
    /// literal's value: the zero literal names the dedicated
    /// `CompareI64Zero` realization [`COMPARE_ZERO`](Self::COMPARE_ZERO)
    /// binds instead, so the immediate grammars admit only nonzero
    /// values and exactly one pair admits any candidate.
    pub const COMPARE_IMMEDIATE_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Immediate,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::EncodingNonZero(4095),
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// minuend — of `CompareI64` when the folded literal is nonzero:
    /// `literal - x` rewrites to the
    /// `CompareI64Immediate` form computing `x - literal`, the
    /// operand-swapped subtraction whose zero condition is identical but
    /// whose ordering predicates invert. The same catalog selection
    /// admits both operand positions; the pair disambiguates by which
    /// `Use` position the folded literal occupies, and a zero literal at
    /// this position names [`COMPARE_LEFT_ZERO`](Self::COMPARE_LEFT_ZERO)
    /// instead. Under
    /// [`OperandSwappedUnitDefs`](PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS)
    /// the rewrite keeps the consumer's implicit unit definitions — the
    /// target condition state — bit-identical while changing the
    /// relation they encode, admitted only while every reader each
    /// defined unit can reach through the function's CFG is
    /// equality-sensing.
    pub const COMPARE_LEFT_IMMEDIATE_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
        machine_effects: PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS,
        ..Self::COMPARE_IMMEDIATE_U12
    };
    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// subtrahend — of `CompareI64` when the folded literal is exactly
    /// zero: `x - 0` rewrites to the dedicated `CompareI64Zero` form
    /// rather than the immediate form, publishing the same condition
    /// state through the narrower realization the selected vocabulary
    /// reserves for a zero comparison. The fold is a realization
    /// refinement of the same computation, not a different computation:
    /// the family's value partition keeps it disjoint from the immediate
    /// grammar so exactly one pair admits the candidate.
    pub const COMPARE_ZERO: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::CompareI64,
        rewritten: MachineSemanticKind::CompareI64Zero,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Exactly(0),
        result: PairResultDisposition::ImplicitUnits,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// The operand-swapped zero compare: `0 - x` rewrites to the
    /// `CompareI64Zero` form computing `x - 0` — the zero condition is
    /// identical while the ordering predicates invert exactly as under
    /// the nonzero left-immediate grammar, so the pair carries the same
    /// reader-flow audit under
    /// [`OperandSwappedUnitDefs`](PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS).
    pub const COMPARE_LEFT_ZERO: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_OPERAND_SWAP,
        machine_effects: PairMachineEffects::OPERAND_SWAPPED_UNIT_DEFS,
        ..Self::COMPARE_ZERO
    };

    /// Shared base of the unary materialization folds: `MaterializeI64`
    /// feeding a sole-`Use` consumer rewrites into a direct
    /// `MaterializeI64` of the constant the consumer computes — the
    /// extension's folded output bits, or the literal itself under `CopyI64`.
    /// `consumer` is a placeholder each concrete rule overrides.
    const UNARY_MATERIALIZE_FOLD: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::MaterializeI64,
        rewritten: MachineSemanticKind::MaterializeI64,
        operand_shape: PairOperandShape::UNARY_LITERAL,
        // The folded value always encodes as a `MaterializeI64` constant; no
        // target immediate bound applies to the source literal itself.
        immediate_bound: PairImmediateBound::Encoding(u64::MAX),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU8`: the result is the
    /// literal's low eight bits materialized directly.
    pub const ZERO_EXTEND_U8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU8,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU16`: the result is the
    /// literal's low sixteen bits materialized directly.
    pub const ZERO_EXTEND_U16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU16,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `ZeroExtendU32`: the result is the
    /// literal's low thirty-two bits materialized directly.
    pub const ZERO_EXTEND_U32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::ZeroExtendU32,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI8`: the result is the
    /// sign extension of the literal's low eight bits materialized directly.
    pub const SIGN_EXTEND_I8_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI8,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI16`: the result is the
    /// sign extension of the literal's low sixteen bits materialized directly.
    pub const SIGN_EXTEND_I16_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI16,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// Eliminate `MaterializeI64` feeding `SignExtendI32`: the result is the
    /// sign extension of the literal's low thirty-two bits materialized
    /// directly.
    pub const SIGN_EXTEND_I32_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::SignExtendI32,
        ..Self::UNARY_MATERIALIZE_FOLD
    };
    /// The six unary extension-elimination rules, in extension-kind order.
    pub const EXTENSION_LITERAL_FOLDS: [Self; 6] = [
        Self::ZERO_EXTEND_U8_LITERAL_FOLD,
        Self::ZERO_EXTEND_U16_LITERAL_FOLD,
        Self::ZERO_EXTEND_U32_LITERAL_FOLD,
        Self::SIGN_EXTEND_I8_LITERAL_FOLD,
        Self::SIGN_EXTEND_I16_LITERAL_FOLD,
        Self::SIGN_EXTEND_I32_LITERAL_FOLD,
    ];

    /// Eliminate `MaterializeI64` feeding `CopyI64`: the copy's output is
    /// the literal itself, so the fold materializes it directly at the
    /// copy's destination register — a copy of a constant is the constant.
    pub const COPY_LITERAL_FOLD: Self = Self {
        consumer: MachineSemanticKind::CopyI64,
        ..Self::UNARY_MATERIALIZE_FOLD
    };

    /// Eliminate `MaterializeI64` feeding the index operand of
    /// `Load8Indexed`: the indexed byte read rewrites to the direct-offset
    /// `Load8` whose `byte_offset` is the folded literal. The rewrite
    /// preserves the read — the consumer's `ReadIndexedPointerV1`
    /// alternatives and the rewritten form's `ReadPointerV1` alternatives
    /// name the same pointer and byte count — while the may-fault trap
    /// surface is identical on both forms, so no trap behavior the fold
    /// removes is left unobserved.
    ///
    /// The immediate bound is the narrowest byte-offset field any target's
    /// `Load8` encoder admits: aarch64 `ldrb` carries a 12-bit unsigned
    /// scaled offset, so a target-independent rule declares 4095 even though
    /// x86-64's disp32 form would admit more.
    pub const LOAD8_INDEXED_U12: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::Load8Indexed,
            rewritten: MachineSemanticKind::Load8,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Encoding(4095),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::indexed_pointer_read_fold(1),
        };
        assert!(
            matches!(
                rule.machine_effects.non_unit,
                PairNonUnitSurface::IndexedPointerRead { index_operand }
                    if index_operand == rule.victim_operand()
            ),
            "the folded operand is the indexed read's index operand"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the offset operand of
    /// `ByteViewAddress`: the base-plus-index address computation rewrites to
    /// the constant-offset `AddressOffset` form whose `byte_offset` is the
    /// folded literal. Both forms are effect-isolated — neither touches
    /// memory, traps, or implicit units — so the rewrite replaces the
    /// consumer's surface wholesale under the ordinary
    /// [`Isolated`](PairMachineEffects::ISOLATED) relationship.
    ///
    /// The immediate bound is the narrowest byte-offset field any target's
    /// `AddressOffset` encoder admits: aarch64 `add xD, xN, #imm12` carries a
    /// 12-bit unsigned displacement, so a target-independent rule declares
    /// 4095 even though x86-64's `lea` disp32 form would admit more.
    pub const BYTE_VIEW_ADDRESS_OFFSET_U12: Self = Self {
        producer: MachineSemanticKind::MaterializeI64,
        consumer: MachineSemanticKind::ByteViewAddress,
        rewritten: MachineSemanticKind::AddressOffset,
        operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
        immediate_bound: PairImmediateBound::Encoding(4095),
        result: PairResultDisposition::ScalarRegister,
        unit_effects: PairUnitEffects::ISOLATED,
        machine_effects: PairMachineEffects::ISOLATED,
    };
    /// Eliminate `MaterializeI64` feeding the operand-0 backing operand of
    /// `ByteViewAddress`: the projection computes `(backing + offset)`
    /// modulo 2^64 — a commutative modular address addition — so a
    /// materialized backing literal rewrites to the same constant-offset
    /// `AddressOffset` form the operand-1 offset fold uses, with the
    /// operand-1 `Use` surviving as the rewritten row's base. The same
    /// catalog selection admits both operand positions; the pair
    /// disambiguates by which `Use` position the folded literal occupies.
    pub const BYTE_VIEW_ADDRESS_BACKING_U12: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::BYTE_VIEW_ADDRESS_OFFSET_U12
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `ExactDivideU64` when the literal is exactly one: an unsigned divide
    /// by one returns the dividend, so the rewrite is a `CopyI64` of the
    /// surviving operand-0 register. The declared surface carries the
    /// dimensions a `div` realization brings: the consumer may encode an
    /// architectural fault — divide by zero or quotient overflow — which
    /// the folded divisor of one discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL),
    /// its operands may carry the register pins the pinned-operand form
    /// requires under
    /// [`BOUND_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_CONSUMER_OPERANDS),
    /// and every `Use` operand past the operand-2 `Def` result — the
    /// zeroed high-half scratch a realization like x86-64 `div` reads — is
    /// dropped under
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES`](PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES),
    /// which requires each such register to be defined only by zero
    /// materializations. Targets whose divide row carries no auxiliary
    /// `Use` — aarch64's `udiv` — admit the same rule with an empty
    /// auxiliary tail.
    pub const EXACT_DIVIDE_ONE_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::ExactDivideU64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the dividend operand of
    /// `ExactDivideU64` when the literal is exactly zero: an unsigned
    /// divide of a zero dividend is always zero — `0 / x` is `0` for
    /// every `x` — so the rewrite is a `MaterializeI64` of the constant
    /// zero at the consumer's result register. The declared surface
    /// carries the dimensions a `div` realization brings, with one
    /// distinction from the divisor-one fold: the folded dividend is not
    /// the value that discharges the consumer's encoded architectural
    /// fault. A quotient of zero can never overflow, but the
    /// divide-by-zero case the encoding could still name is unreachable
    /// only because the `ExactDivideU64` kind carries its proven nonzero
    /// divisor as an accepted obligation — the fold declares
    /// [`FaultDischargedByObligation`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins the pinned-operand
    /// realization requires under
    /// [`BOUND_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_CONSUMER_OPERANDS),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every `Use` operand past the
    /// operand-2 `Def` result — the zeroed high-half input an x86-64
    /// `div` realization reads as the dividend's upper half — is dropped
    /// under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES),
    /// which requires each such register to be defined only by zero
    /// materializations: a literal of zero fixes the low dividend half,
    /// so the fold is exact only when every auxiliary half is provably
    /// zero as well. Targets whose divide row carries no auxiliary `Use`
    /// — aarch64's `udiv` — admit the same rule with an empty auxiliary
    /// tail. The family shares its consumer kind with the divisor-one
    /// fold; the grammars stay disjoint on the folded literal's operand
    /// position.
    pub const EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::ExactDivideU64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `WrappingRemainderI64` when the literal is exactly one: a wrapping
    /// remainder by one is always zero — signed or unsigned, `x % 1` is
    /// `0` and `i64::MIN % 1` cannot overflow — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. The declared surface carries the dimensions an
    /// `idiv`-class realization brings: the consumer may encode an
    /// architectural fault — divide by zero or quotient overflow — which
    /// the folded divisor of one discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL),
    /// its operands may carry the register pins and early-clobber marks a
    /// pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and every operand past the operand-2 `Def` result — the dead
    /// quotient scratch an x86-64 `idiv` realization writes — is a `Def`
    /// the fold drops under
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT),
    /// which requires each such register to occur nowhere else in the
    /// function. The operand-0 dividend `Use` is dropped with the form:
    /// the constant result never reads it. Targets whose remainder row
    /// carries no scratch `Def` — aarch64's `udiv`/`msub` realization —
    /// admit the same rule with an empty scratch tail.
    pub const WRAPPING_REMAINDER_ONE_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the divisor operand of
    /// `WrappingRemainderI64` when the literal is `u64::MAX` — the
    /// normalized-i64 divisor `-1`: a wrapping remainder by minus one is
    /// always zero — `x % -1` is `0` for every `x`, and `i64::MIN % -1`
    /// is the exceptional case the kind's semantics defines to produce
    /// zero rather than trap — so the rewrite is a `MaterializeI64` of
    /// the constant zero at the consumer's result register. The declared
    /// surface carries the dimensions an `idiv`-class realization brings:
    /// the consumer may encode an architectural fault — divide by zero
    /// or quotient overflow — which the folded divisor of minus one
    /// discharges under
    /// [`FaultDischargedByLiteral`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL):
    /// a divisor of `-1` can never divide by zero, and the one dividend
    /// whose `idiv` would overflow is the case the kind defines away —
    /// the x86-64 realization's `-1` guard skips the divide for exactly
    /// it — so the encoded fault cannot fire on this instruction. The
    /// operands may carry the register pins and early-clobber marks a
    /// pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// the operand-0 dividend `Use` is dropped with the form because the
    /// constant result never reads it, and every operand past the
    /// operand-2 `Def` result — the dead quotient scratch an x86-64
    /// `idiv` realization writes — is a `Def` the fold drops under
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT),
    /// which requires each such register to occur nowhere else in the
    /// function. Targets whose remainder row carries no scratch `Def` —
    /// aarch64's `udiv`/`msub` realization — admit the same rule with an
    /// empty scratch tail. The family shares its consumer kind and
    /// operand position with the divisor-one fold; the grammars stay
    /// disjoint on the folded literal's value — `1` admits only the
    /// divisor-one pair, `u64::MAX` only this one.
    pub const WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(u64::MAX)),
            "the fault discharge holds only for the divisor literal minus one"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the dividend operand of
    /// `WrappingRemainderI64` when the literal is exactly zero: a
    /// remainder of a zero dividend is always zero — `0 % x` is `0` for
    /// every `x` — so the rewrite is a `MaterializeI64` of the constant
    /// zero at the consumer's result register. The declared surface
    /// carries the dimensions an `idiv`-class realization brings, with
    /// one distinction from the divisor-one fold: the folded dividend is
    /// not the value that discharges the consumer's encoded
    /// architectural fault. A quotient of zero can never overflow, but
    /// the divide-by-zero case the encoding could still name is
    /// unreachable only because the `WrappingRemainderI64` kind carries
    /// its proven nonzero divisor as an accepted obligation — the fold
    /// declares
    /// [`FaultDischargedByObligation`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION)
    /// so the descriptor never claims the literal did the obligation's
    /// work. The operands may carry the register pins and early-clobber
    /// marks a pinned-scratch realization requires under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// the operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it, and every operand past the
    /// operand-2 `Def` result — the dead quotient scratch an x86-64
    /// `idiv` realization writes — is a `Def` the fold drops under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT),
    /// which requires each such register to occur nowhere else in the
    /// function. Targets whose remainder row carries no scratch `Def` —
    /// aarch64's `udiv`/`msub` realization — admit the same rule with an
    /// empty scratch tail. The family shares its consumer kind with the
    /// divisor-one fold; the grammars stay disjoint on the folded
    /// literal's operand position.
    pub const WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingRemainderI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    };

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseAndI64` when the literal is exactly zero: zero is the
    /// bitwise-and annihilator — `x & 0` is `0` for every `x` — so the
    /// rewrite is a `MaterializeI64` of the constant zero at the
    /// consumer's result register. Both forms are effect-isolated on every
    /// target — the consumer's flag clobber, where one is declared, dies
    /// with the folded form — and neither side pins or binds an operand,
    /// so the ordinary
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` is dropped because the constant result never reads
    /// it, under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's custody: every `Def` operand past the result must occur
    /// nowhere else in the function.
    pub const BITWISE_AND_ZERO_MATERIALIZE: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseAndI64,
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the annihilator fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand annihilator fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseAndI64` when the literal is exactly
    /// zero — `0 & x` is `0` for every `x`. The constant result is a
    /// function of the literal alone, so no commutativity attestation is
    /// needed: under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// the dropped operand-1 `Use` is never read and every `Def` operand
    /// past the result drops under the same occurrence-free custody the
    /// right grammar declares. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ZERO_LEFT_MATERIALIZE: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
        ..Self::BITWISE_AND_ZERO_MATERIALIZE
    };

    /// The two bitwise-and annihilator rules, one per literal `Use`
    /// position.
    pub const BITWISE_AND_ZERO_FOLDS: [Self; 2] = [
        Self::BITWISE_AND_ZERO_MATERIALIZE,
        Self::BITWISE_AND_ZERO_LEFT_MATERIALIZE,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseXorI64` when the literal is exactly zero: zero is the
    /// bitwise-xor identity element — `x ^ 0` is `x` for every `x` — so
    /// the rewrite is a `CopyI64` of the surviving operand-0 register at
    /// the consumer's result register. Both forms are effect-isolated on
    /// every target — the consumer's flag clobber, where one is declared,
    /// dies with the folded form — and neither side pins or binds an
    /// operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const BITWISE_XOR_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseXorI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseXorI64` when the literal is exactly
    /// zero — `0 ^ x` is `x` for every `x`. Bitwise xor commutes, so the
    /// `CopyI64` of the surviving operand-1 register computes the same
    /// value `x ^ 0` does; the
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_XOR_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::BITWISE_XOR_ZERO_COPY
    };

    /// The two bitwise-xor identity rules, one per literal `Use` position.
    pub const BITWISE_XOR_ZERO_COPIES: [Self; 2] = [
        Self::BITWISE_XOR_ZERO_COPY,
        Self::BITWISE_XOR_ZERO_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `WrappingAddI64` when the literal is exactly zero: zero is the
    /// additive identity under modulo-2^64 wrap — `x + 0` is `x` for
    /// every `x` — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. Both forms
    /// are effect-isolated on every target — the wrapping-add consumer
    /// binds the flag-transparent add row, so unlike the bitwise forms
    /// there is no flag clobber to retire — and neither side pins or
    /// binds an operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    pub const WRAPPING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::WrappingAddI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `WrappingAddI64` when the literal is exactly
    /// zero — `0 + x` is `x` for every `x`. Wrapping addition commutes,
    /// so the `CopyI64` of the surviving operand-1 register computes the
    /// same value `x + 0` does; the
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const WRAPPING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::WRAPPING_ADD_ZERO_COPY
    };

    /// The two wrapping-add identity rules, one per literal `Use` position.
    pub const WRAPPING_ADD_ZERO_COPIES: [Self; 2] = [
        Self::WRAPPING_ADD_ZERO_COPY,
        Self::WRAPPING_ADD_ZERO_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `BitwiseAndI64` when the literal is all ones (`u64::MAX`): all-ones
    /// is the bitwise-and identity element — `x & MAX` is `x` for every
    /// `x` — so the rewrite is a `CopyI64` of the surviving operand-0
    /// register at the consumer's result register. Both forms are
    /// effect-isolated on every target — the consumer's flag clobber,
    /// where one is declared, dies with the folded form — and neither
    /// side pins or binds an operand, so the ordinary
    /// [`Isolated`](PairMachineEffects::ISOLATED) and
    /// [`Isolated`](PairUnitEffects::ISOLATED) surfaces apply. The
    /// operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand. The
    /// family shares its consumer kind and operand grammar with the
    /// and-zero annihilator rules; the two families stay disjoint on the
    /// literal's value, which the producer's admission now carries as
    /// part of pair selection.
    pub const BITWISE_AND_ONES_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::BitwiseAndI64,
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(u64::MAX),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::ISOLATED,
            machine_effects: PairMachineEffects::ISOLATED,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(u64::MAX)),
            "the identity fold holds only for the all-ones literal"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `BitwiseAndI64` when the literal is all ones —
    /// `MAX & x` is `x` for every `x`. Bitwise and commutes, so the
    /// `CopyI64` of the surviving operand-1 register computes the same
    /// value `x & MAX` does; the
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const BITWISE_AND_ONES_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::BITWISE_AND_ONES_COPY
    };

    /// The two bitwise-and identity rules, one per literal `Use` position.
    pub const BITWISE_AND_ONES_COPIES: [Self; 2] = [
        Self::BITWISE_AND_ONES_COPY,
        Self::BITWISE_AND_ONES_LEFT_COPY,
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingAdd` on the u64 carrier when the literal is exactly
    /// zero: zero is the additive identity under unsigned saturating
    /// addition — `x +| 0` is `x` for every `x`, already inside the
    /// carrier's bounds — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. This is the
    /// first family whose consumer carries an implicit unit *definition*
    /// the rewrite retires: the three-operand u64 saturating-add row
    /// defines `nzcv` on aarch64 — its flag-setting `adds` realization —
    /// while the isolated `CopyI64` defines nothing. Under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand. The u64
    /// row carries no operand past its `Def` result; the narrower and
    /// signed carriers bind the clamped row whose bound scratch the
    /// scratch-`Def` grammars below drop.
    pub const SATURATING_ADD_ZERO_COPY: Self = {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the identity fold holds only for the literal zero"
        );
        rule
    };

    /// The left-operand identity fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of the u64 saturating add when the literal is
    /// exactly zero — `0 +| x` is `x` for every `x`. Unsigned saturating
    /// addition commutes, so the `CopyI64` of the surviving operand-1
    /// register computes the same value `x +| 0` does; the
    /// [`BINARY_LEFT_LITERAL`](PairOperandShape::BINARY_LEFT_LITERAL) grammar
    /// attests that commutation and binds operand 1 into the rewritten
    /// row's `Use` position. The same catalog selection admits both
    /// operand positions; the pair disambiguates by which `Use` position
    /// the folded literal occupies.
    pub const SATURATING_ADD_ZERO_LEFT_COPY: Self = Self {
        operand_shape: PairOperandShape::BINARY_LEFT_LITERAL,
        ..Self::SATURATING_ADD_ZERO_COPY
    };

    /// The right-operand identity fold for a clamped-carrier saturating
    /// add: `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingAdd` on `carrier` — every carrier but u64 — when the
    /// literal is exactly zero. `x +| 0` is `x` inside the carrier's
    /// bounds under signed or unsigned saturation, so the rewrite is a
    /// `CopyI64` of the surviving operand-0 register. Unlike the u64
    /// row, the clamped row's operand list continues past its `Def`
    /// result with an early-clobber bound scratch — a `Def` output the
    /// realization writes and nothing else may observe — so the rule
    /// declares
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_add_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !matches!(carrier, SaturatingCarrier::U64)
                && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the clamped identity fold holds only for a non-u64 carrier's zero literal"
        );
        rule
    }

    /// The left-operand identity fold for a clamped-carrier saturating
    /// add: `0 +| x` is `x` for every `x`. Saturating addition commutes
    /// under every carrier — its saturation bounds are symmetric around
    /// the operation — so the `CopyI64` of the surviving operand-1
    /// register computes the same value `x +| 0` does; the
    /// [`BINARY_LEFT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS)
    /// grammar attests that commutation and drops the same bound-scratch
    /// `Def` tail under occurrence-free custody.
    const fn saturating_add_zero_clamped_left(carrier: SaturatingCarrier) -> Self {
        Self {
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_SCRATCH_DEFS,
            ..Self::saturating_add_zero_clamped(carrier)
        }
    }

    /// The saturating-add identity rules: one pair per literal `Use`
    /// position for the u64 carrier's three-operand row, and one pair
    /// per position for each carrier binding the clamped row — every
    /// other carrier — whose bound scratch `Def` drops under the
    /// scratch-defs grammar.
    pub const SATURATING_ADD_ZERO_COPIES: [Self; 16] = [
        Self::SATURATING_ADD_ZERO_COPY,
        Self::SATURATING_ADD_ZERO_LEFT_COPY,
        Self::saturating_add_zero_clamped(SaturatingCarrier::I8),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I8),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I16),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I16),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I32),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I32),
        Self::saturating_add_zero_clamped(SaturatingCarrier::I64),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::I64),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U8),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U8),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U16),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U16),
        Self::saturating_add_zero_clamped(SaturatingCarrier::U32),
        Self::saturating_add_zero_clamped_left(SaturatingCarrier::U32),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingAdd` on an unsigned carrier when the literal is exactly
    /// the carrier's maximum: `x +| MAX` is `MAX` for every `x` the
    /// carrier admits, because `x + MAX` reaches the carrier's upper
    /// bound and saturates to it — so the rewrite is a `MaterializeI64`
    /// of that maximum at the consumer's result register. Only unsigned
    /// carriers admit the fold: under signed saturation `x +| MAX` is
    /// `x + MAX` unclamped for every negative `x`, not a constant, so the
    /// family binds no signed carrier and no signed carrier ever names an
    /// admitted maximum-literal grammar. The operand-0 `Use` is dropped
    /// with the form because the constant result never reads it, and
    /// every operand past the operand-2 `Def` result — the bound scratch
    /// `Def` a clamped saturating-add realization computes its saturation
    /// bound through — drops under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody: a scratch output another
    /// instruction read or defined would leave a use of a register the
    /// rewrite stopped defining. The consumer carries the same implicit
    /// unit surface the identity family retires: the saturating-add rows
    /// define `nzcv` on aarch64 — every carrier's realization is
    /// flag-setting — while the isolated `MaterializeI64` defines
    /// nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` marks the saturating realizations declare on their
    /// `Def` outputs — the hazard they name exists only inside the
    /// dropped operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind and operand positions with the
    /// zero-identity fold: the literal's value names which family a
    /// `SaturatingAdd` fold belongs to, and the grammars stay disjoint on
    /// that value.
    const fn saturating_add_upper_bound(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingAdd(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed()
                && matches!(
                    rule.immediate_bound,
                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                ),
            "the upper-bound fold holds only for an unsigned carrier's maximum literal"
        );
        rule
    }

    /// The left-operand upper-bound fold: `MaterializeI64` feeding the
    /// operand-0 `Use` of `SaturatingAdd` on an unsigned carrier when the
    /// literal is exactly the carrier's maximum — `MAX +| x` is `MAX`
    /// for every `x`. The
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// grammar attests the operand-0 literal alone fixes the result —
    /// saturating addition's commutation makes `MAX +| x` and `x +| MAX`
    /// the same fold, but the constant-result grammar needs none of it:
    /// the operand-1 `Use` drops because the constant never reads it, and
    /// every operand past the operand-2 `Def` result drops under the same
    /// occurrence-free custody.
    const fn saturating_add_upper_bound_left(carrier: SaturatingCarrier) -> Self {
        Self {
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            ..Self::saturating_add_upper_bound(carrier)
        }
    }

    /// The saturating-add upper-bound rules: one pair per literal `Use`
    /// position for each unsigned carrier — `x +| MAX` and `MAX +| x`
    /// are both `MAX` under unsigned saturation. The u64 carrier binds
    /// the three-operand row; every other unsigned carrier binds the
    /// clamped row whose bound scratch `Def` drops under the
    /// constant-result grammar's scratch-defs custody. Signed carriers
    /// admit no maximum fold — `x +| MAX` there is `x + MAX` unclamped
    /// for every negative `x` — so the family declares no signed pair.
    /// The family shares its consumer kind and operand positions with
    /// the zero-identity fold; the grammars stay disjoint on the folded
    /// literal's value.
    pub const SATURATING_ADD_UPPER_BOUND_MATERIALIZATIONS: [Self; 8] = [
        Self::saturating_add_upper_bound(SaturatingCarrier::U8),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U8),
        Self::saturating_add_upper_bound(SaturatingCarrier::U16),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U16),
        Self::saturating_add_upper_bound(SaturatingCarrier::U32),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U32),
        Self::saturating_add_upper_bound(SaturatingCarrier::U64),
        Self::saturating_add_upper_bound_left(SaturatingCarrier::U64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingSubtract` on an unsigned carrier when the literal is
    /// exactly zero: zero is the right identity under saturating
    /// subtraction — `x -| 0` is `x` for every `x`, already inside the
    /// carrier's bounds — so the rewrite is a `CopyI64` of the surviving
    /// operand-0 register at the consumer's result register. The grammar
    /// is deliberately asymmetric: `0 -| x` is `-x` clamped to the
    /// carrier's bounds, not `x`, so this family declares no left-literal
    /// pair and a literal recorded at operand 0 names no admitted
    /// grammar. Like the saturating add, the consumer carries an implicit
    /// unit *definition* the rewrite retires: the three-operand unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated `CopyI64`
    /// defines nothing. Under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function —
    /// a conditional branch reading `nzcv` would go stale — and the
    /// consumer's clobbers retire wholesale, as the x86-64 row's `rflags`
    /// clobber does. The consumer's operands may carry the `early_clobber`
    /// mark the x86-64 saturating realization declares on its result —
    /// the hazard it names exists only inside the dropped operand list —
    /// under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives under the ordinary
    /// [`BINARY_RIGHT_LITERAL`](PairOperandShape::BINARY_RIGHT_LITERAL)
    /// grammar: the rewritten row binds it as its `Use` operand.
    const fn saturating_subtract_zero_unsigned(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the unsigned identity fold holds only for an unsigned carrier's zero literal"
        );
        rule
    }

    /// The right-operand identity fold for a clamped-carrier saturating
    /// subtract: `MaterializeI64` feeding the operand-1 `Use` of
    /// `SaturatingSubtract` on `carrier` — every signed carrier — when
    /// the literal is exactly zero. `x -| 0` is `x` inside the carrier's
    /// bounds under signed saturation, so the rewrite is a `CopyI64` of
    /// the surviving operand-0 register; the asymmetric grammar of the
    /// unsigned rules applies here too. Unlike the unsigned row, the
    /// clamped row's operand list continues past its `Def` result with
    /// an early-clobber bound scratch — a `Def` output the realization
    /// writes and nothing else may observe — so the rule declares
    /// [`BINARY_RIGHT_LITERAL_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS):
    /// each dropped `Def` register must occur nowhere else in the
    /// function. The unit surface is the family's own: the aarch64
    /// clamped row still defines `nzcv` — retired under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// only while dead in the function — both targets mark the dropped
    /// result and scratch `early_clobber`, admitted under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS),
    /// and the x86-64 row's `rflags` clobber retires unconditionally.
    const fn saturating_subtract_zero_clamped(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the clamped identity fold holds only for a signed carrier's zero literal"
        );
        rule
    }

    /// The saturating-subtract identity rules: one right-literal pair for
    /// each unsigned carrier's three-operand row, and one for each signed
    /// carrier's clamped row whose bound scratch `Def` drops under the
    /// scratch-defs grammar. Saturating subtraction does not commute —
    /// `0 -| x` is `-x` clamped to the carrier's bounds, not `x` — so the
    /// family declares no left-literal pair at either operand grammar.
    /// The unsigned `0 -| x` constant fold is a separate family below,
    /// not a left-literal member of this one.
    pub const SATURATING_SUBTRACT_ZERO_COPIES: [Self; 8] = [
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U8),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U16),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U32),
        Self::saturating_subtract_zero_unsigned(SaturatingCarrier::U64),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I8),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I16),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I32),
        Self::saturating_subtract_zero_clamped(SaturatingCarrier::I64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// minuend — of `SaturatingSubtract` on an unsigned carrier when the
    /// literal is exactly zero: `0 -| x` is `0` for every `x`, because
    /// `0 - x` underflows the carrier's lower bound and saturates to it —
    /// so the rewrite is a `MaterializeI64` of the constant zero at the
    /// consumer's result register. Only unsigned carriers admit the fold:
    /// under signed saturation `0 -| x` is `-x` clamped to the carrier's
    /// bounds, not a constant, so the family binds the three-operand
    /// unsigned row alone and no signed carrier ever names an admitted
    /// operand-0 grammar. The operand-1 subtrahend `Use` is dropped with
    /// the form because the constant result never reads it, and every
    /// operand past the operand-2 `Def` result — none on the unsigned
    /// row — would drop under the
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody. The consumer carries the same
    /// implicit unit surface the identity family retires: the unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated
    /// `MaterializeI64` defines nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` mark the x86-64 saturating realization declares on
    /// its result — the hazard it names exists only inside the dropped
    /// operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind with the right-zero identity
    /// fold; the grammars stay disjoint on the folded literal's operand
    /// position.
    const fn saturating_subtract_zero_minuend(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed() && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the zero-minuend fold holds only for an unsigned carrier's zero literal"
        );
        rule
    }

    /// The saturating-subtract zero-minuend rules: one left-literal pair
    /// for each unsigned carrier's three-operand row — `0 -| x` is `0`
    /// under unsigned saturation. Signed carriers admit no operand-0 fold
    /// — `0 -| x` there is `-x` clamped to the carrier's bounds, not a
    /// constant — so the family declares no signed pair at all. The
    /// family shares its consumer kind with the right-zero identity fold;
    /// the grammars stay disjoint on the folded literal's operand
    /// position.
    pub const SATURATING_SUBTRACT_ZERO_MINUEND_MATERIALIZATIONS: [Self; 4] = [
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U8),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U16),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U32),
        Self::saturating_subtract_zero_minuend(SaturatingCarrier::U64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// subtrahend — of `SaturatingSubtract` on an unsigned carrier when
    /// the literal is exactly the carrier's maximum: `x -| MAX` is `0`
    /// for every `x` the carrier admits, because `x - MAX` underflows
    /// the carrier's lower bound — saturating to it — for every
    /// `x < MAX` and is exactly zero at `x == MAX` — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. Only unsigned carriers admit the fold: under signed
    /// saturation `x -| MAX` is `x - MAX` clamped to the carrier's lower
    /// bound for every negative `x`, not a constant, so the family binds
    /// the three-operand unsigned row alone and no signed carrier ever
    /// names an admitted maximum-subtrahend grammar. The grammar is
    /// deliberately asymmetric: `MAX -| x` is `MAX - x`, not a constant,
    /// so the family declares no left-literal pair and a maximum literal
    /// recorded at operand 0 names no admitted grammar — the unsigned
    /// `0 -| x` constant fold is the zero-minuend family, disjoint on
    /// the folded literal's operand position. The operand-0 minuend
    /// `Use` is dropped with the form because the constant result never
    /// reads it, and every operand past the operand-2 `Def` result —
    /// none on the unsigned row — would drop under the
    /// [`BINARY_RIGHT_LITERAL_CONSTANT_RESULT`](PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT)
    /// grammar's occurrence-free custody. The consumer carries the same
    /// implicit unit surface the sibling families retire: the unsigned
    /// saturating-subtract row defines `nzcv` on aarch64 — its
    /// flag-setting `subs` realization — while the isolated
    /// `MaterializeI64` defines nothing, so under
    /// [`DeadConsumerUnitDefs`](PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS)
    /// that definition may retire only while it is dead in the function,
    /// and the consumer's clobbers retire wholesale, as the x86-64 row's
    /// `rflags` clobber does. The consumer's operands may carry the
    /// `early_clobber` mark the x86-64 saturating realization declares on
    /// its result — the hazard it names exists only inside the dropped
    /// operand list — under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The family shares its consumer kind and operand position with the
    /// right-zero identity fold: the folded literal's value names which
    /// `SaturatingSubtract` subtrahend family a fold belongs to.
    const fn saturating_subtract_upper_bound(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingSubtract(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_CONSTANT_RESULT,
            immediate_bound: PairImmediateBound::Exactly(carrier.maximum_bits()),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::DEAD_CONSUMER_UNIT_DEFS,
        };
        assert!(
            !carrier.is_signed()
                && matches!(
                    rule.immediate_bound,
                    PairImmediateBound::Exactly(bound) if bound == carrier.maximum_bits()
                ),
            "the upper-bound subtrahend fold holds only for an unsigned carrier's maximum literal"
        );
        rule
    }

    /// The saturating-subtract upper-bound rules: one right-literal pair
    /// for each unsigned carrier's three-operand row — `x -| MAX` is `0`
    /// under unsigned saturation. Signed carriers admit no
    /// maximum-subtrahend fold — `x -| MAX` there is `x - MAX` clamped
    /// to the carrier's lower bound for every negative `x`, not a
    /// constant — so the family declares no signed pair at all. The
    /// family shares its consumer kind and operand position with the
    /// right-zero identity fold; the grammars stay disjoint on the
    /// folded literal's value.
    pub const SATURATING_SUBTRACT_UPPER_BOUND_MATERIALIZATIONS: [Self; 4] = [
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U8),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U16),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U32),
        Self::saturating_subtract_upper_bound(SaturatingCarrier::U64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-1 `Use` — the
    /// divisor — of `SaturatingDivide` on `carrier` when the literal is
    /// exactly one: a saturating divide by one is the dividend — `x /| 1`
    /// is `x` inside every carrier's bounds, and the signed `MIN /| -1`
    /// clamp lies outside the divisor this grammar admits — so the
    /// rewrite is a `CopyI64` of the surviving operand-0 register at the
    /// consumer's result register. The grammar is deliberately
    /// asymmetric: `1 /| x` is not `x`, so the family declares no
    /// left-literal pair and a literal recorded at operand 0 names no
    /// admitted grammar.
    ///
    /// This is the first family whose consumer both may architecturally
    /// fault *and* retires implicit unit definitions the rewritten form
    /// does not carry, so it declares
    /// [`FaultDischargedByLiteralDeadUnitDefs`](PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS):
    /// the divisor literal of one discharges the encoded
    /// `MayArchitecturalFaultV1` an x86-64 `div`/`idiv` realization
    /// carries — a divide by one can neither divide by zero nor overflow,
    /// and unlike the exact-divide family the kind's carried
    /// fault-recovery obligation is not the discharger — while every
    /// implicit unit the consumer record defines, the `nzcv` an aarch64
    /// signed row writes, retires only while dead in the function. The
    /// consumer's operands may carry the register pins the x86-64 `div`
    /// realization requires and the `early_clobber` marks the clamped
    /// aarch64 signed rows declare on their `Def` outputs under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-0 `Use` survives into the rewritten row; every operand
    /// past the operand-2 `Def` result drops under
    /// [`BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS`](PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS) —
    /// the zeroed high-half dividend `Use` an x86-64 `div`/`idiv` reads,
    /// provably defined only by zero materializations, or the bound
    /// scratch `Def` an aarch64 clamped signed row writes, occurring
    /// nowhere else in the function. The unsigned carriers bind the
    /// `divide_u64` row — the bare three-operand `udiv` on aarch64, the
    /// pinned four-operand `div` row carrying its zeroed-rdx auxiliary
    /// `Use` on x86-64 — while every signed carrier binds the
    /// `saturating_divide_signed` row whose tail the grammar drops under
    /// its own access's custody: the pinned `idiv` row's zeroed-rdx
    /// auxiliary `Use` on x86-64, the clamped row's bound scratch `Def`
    /// on aarch64.
    const fn saturating_divide_one(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingDivide(carrier),
            rewritten: MachineSemanticKind::CopyI64,
            operand_shape: PairOperandShape::BINARY_RIGHT_LITERAL_AUXILIARY_USES_OR_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(1),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_LITERAL_DEAD_UNIT_DEFS,
        };
        assert!(
            matches!(rule.immediate_bound, PairImmediateBound::Exactly(1)),
            "the fault discharge holds only for the divisor literal one"
        );
        rule
    }

    /// The saturating-divide identity rules: one right-literal pair for
    /// every carrier — `x /| 1` is `x` under signed or unsigned
    /// saturation. Saturating division does not commute — `1 /| x` is not
    /// `x` — so the family declares no left-literal pair. The
    /// unsigned-carrier rules bind the `divide_u64` row and the
    /// signed-carrier rules the `saturating_divide_signed` row; the mixed
    /// drop-tail grammar covers every realization's operand-3 role — the
    /// auxiliary `Use` x86-64's pinned divide rows read and the bound
    /// scratch `Def` aarch64's clamped signed row writes — as well as the
    /// empty tail aarch64's `udiv` leaves.
    pub const SATURATING_DIVIDE_ONE_COPIES: [Self; 8] = [
        Self::saturating_divide_one(SaturatingCarrier::U8),
        Self::saturating_divide_one(SaturatingCarrier::U16),
        Self::saturating_divide_one(SaturatingCarrier::U32),
        Self::saturating_divide_one(SaturatingCarrier::U64),
        Self::saturating_divide_one(SaturatingCarrier::I8),
        Self::saturating_divide_one(SaturatingCarrier::I16),
        Self::saturating_divide_one(SaturatingCarrier::I32),
        Self::saturating_divide_one(SaturatingCarrier::I64),
    ];

    /// Eliminate `MaterializeI64` feeding the operand-0 `Use` — the
    /// dividend — of `SaturatingDivide` on `carrier` when the literal is
    /// exactly zero: a saturating divide of a zero dividend is always
    /// zero — `0 /| x` is `0` inside every carrier's bounds, and a zero
    /// quotient reaches no saturation edge — so the rewrite is a
    /// `MaterializeI64` of the constant zero at the consumer's result
    /// register. The folded dividend is not the value that discharges
    /// the consumer's encoded architectural fault: a quotient of zero
    /// can never overflow, but the divide-by-zero case the encoding
    /// could still name is unreachable only because the
    /// `SaturatingDivide` kind carries its proven nonzero divisor as an
    /// accepted obligation — the fold declares
    /// [`FaultDischargedByObligationDeadUnitDefs`](PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS)
    /// so the descriptor never claims the literal did the obligation's
    /// work, and every implicit unit the consumer record defines — the
    /// `nzcv` an aarch64 signed row writes — retires only while dead in
    /// the function. The consumer's operands may carry the register pins
    /// the x86-64 `div`/`idiv` realizations require and the
    /// `early_clobber` marks the clamped aarch64 signed rows declare on
    /// their `Def` outputs under
    /// [`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`](PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS).
    /// The operand-1 divisor `Use` is dropped with the form because the
    /// constant result never reads it; every operand past the operand-2
    /// `Def` result drops under
    /// [`BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS`](PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS)
    /// — each `Use`, the zeroed high-half dividend an x86-64 `div`/`idiv`
    /// realization reads, provably defined only by zero materializations
    /// since a literal of zero fixes only the low dividend half, and each
    /// `Def`, the bound scratch an aarch64 clamped signed-divide
    /// realization writes, occurring nowhere else in the function. The
    /// unsigned carriers bind the `divide_u64` row — the bare
    /// three-operand `udiv` on aarch64, the pinned four-operand `div`
    /// row carrying its zeroed-rdx auxiliary `Use` on x86-64 — while
    /// every signed carrier binds the `saturating_divide_signed` row
    /// whose tail the grammar drops under its own access's custody: the
    /// pinned `idiv` row's zeroed-rdx auxiliary `Use` on x86-64, the
    /// clamped row's bound scratch `Def` on aarch64. The family shares
    /// its consumer kind with the divisor-one fold; the grammars stay
    /// disjoint on the folded literal's operand position.
    const fn saturating_divide_zero_dividend(carrier: SaturatingCarrier) -> Self {
        let rule = Self {
            producer: MachineSemanticKind::MaterializeI64,
            consumer: MachineSemanticKind::SaturatingDivide(carrier),
            rewritten: MachineSemanticKind::MaterializeI64,
            operand_shape:
                PairOperandShape::BINARY_LEFT_LITERAL_CONSTANT_RESULT_AUXILIARY_USES_OR_SCRATCH_DEFS,
            immediate_bound: PairImmediateBound::Exactly(0),
            result: PairResultDisposition::ScalarRegister,
            unit_effects: PairUnitEffects::BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS,
            machine_effects: PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS,
        };
        assert!(
            matches!(
                rule.machine_effects,
                PairMachineEffects::FAULT_DISCHARGED_BY_OBLIGATION_DEAD_UNIT_DEFS
            ) && matches!(rule.immediate_bound, PairImmediateBound::Exactly(0)),
            "the obligation discharge holds only for the dividend literal zero"
        );
        rule
    }

    /// The saturating-divide zero-dividend rules: one left-literal pair
    /// for every carrier — `0 /| x` is `0` under signed or unsigned
    /// saturation. The unsigned-carrier rules bind the `divide_u64` row
    /// and the signed-carrier rules the `saturating_divide_signed` row;
    /// the mixed drop-tail grammar covers every realization's operand-3
    /// role — the auxiliary `Use` x86-64's pinned divide rows read and
    /// the bound scratch `Def` aarch64's clamped signed row writes — as
    /// well as the empty tail aarch64's `udiv` leaves.
    pub const SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS: [Self; 8] = [
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U8),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U16),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U32),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::U64),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I8),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I16),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I32),
        Self::saturating_divide_zero_dividend(SaturatingCarrier::I64),
    ];
}
