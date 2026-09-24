use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

mod computations;
pub use computations::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarDispatchArm, CheckedScalarDispatchPattern,
};
mod proof_terms;
pub use proof_terms::{
    CheckedErasedProofParameterPlan, CheckedLocatedProofTerm, CheckedProofTerm,
    CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
};
mod structural_values;
pub use structural_values::{
    CheckedStructuralDispatchArm, CheckedStructuralRecordField, CheckedStructuralRecordFieldValue,
    CheckedStructuralValue, CheckedStructuralValueHandle, CheckedStructuralValueKind,
    CheckedStructuralValuePlans, CheckedStructuralValueRoot,
};
mod array_construction_source;
pub use array_construction_source::CheckedArrayConstructionSource;
pub mod guard_complement;

#[cfg(test)]
mod tests;

pub type CheckedValueHandle = Handle<CheckedValueFact>;

/// A checker-established inclusive integer interval for one value at its exact
/// use site. Big integers preserve the full `u64` proof domain.
pub use facts::IntegerRange as CheckedIntegerRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedValueOrigin {
    MachineDecrease {
        machine_symbol: SymbolHandle,
        ordinal: usize,
    },
    MachineOwnedDataInitializer {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateStatement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        role: CheckedValueStatementRole,
    },
    NestedExpression {
        parent: ExpressionHandle,
    },
}

impl Default for CheckedValueOrigin {
    fn default() -> Self {
        Self::NestedExpression {
            parent: ExpressionHandle::invalid(),
        }
    }
}

impl CheckedValueOrigin {
    pub const fn machine_symbol(self) -> Option<SymbolHandle> {
        match self {
            Self::MachineDecrease { machine_symbol, .. }
            | Self::MachineOwnedDataInitializer { machine_symbol, .. }
            | Self::StateStatement { machine_symbol, .. } => Some(machine_symbol),
            Self::NestedExpression { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CheckedValueStatementRole {
    #[default]
    Expression,
    AssignmentTargetSubexpression,
    AssignmentValue,
    CallArgument,
    LocalInitializer,
    TransitionGuard,
    TransitionTargetArgument,
    TransitionTargetValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    /// The checker-resolved declared type of this value at its exact use site.
    /// Invalid means the expression has no standalone declared type (for
    /// example, an anonymous literal) or resolution conservatively failed.
    /// Later lowering may consume validated declaration facts through this
    /// handle; it must never reconstruct a stronger type from storage shape.
    pub type_reference: TypeReferenceHandle,
    /// Exact scalar carrier selected at this use site. This remains available
    /// when a context-typed literal has no standalone type-reference handle
    /// (notably validated builtin operands).
    pub primitive_type: Option<typed_trees::types::PrimitiveType>,
    /// The range discharged by Psi for this value in its origin context,
    /// including stable flow guards and retained boundary witnesses.
    pub integer_range: Option<CheckedIntegerRange>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFacts {
    pub values: Arena<CheckedValueFact>,
    pub scalar_expressions: CheckedScalarExpressionPlans,
    pub scalar_computations: CheckedScalarComputationPlans,
    pub structural_values: CheckedStructuralValuePlans,
    /// Proof-only erased actuals recorded under `CheckedProofTermRole`
    /// coordinates. They own no runtime operand rows.
    pub proof_terms: CheckedProofTerms,
}

impl CheckedValueFacts {
    pub fn with_roots(values: Arena<CheckedValueFact>) -> Self {
        Self {
            values,
            scalar_expressions: CheckedScalarExpressionPlans::default(),
            scalar_computations: CheckedScalarComputationPlans::default(),
            structural_values: CheckedStructuralValuePlans::default(),
            proof_terms: CheckedProofTerms::default(),
        }
    }

    pub fn expression_values(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = (CheckedValueHandle, &CheckedValueFact)> + '_ {
        self.values
            .iter()
            .filter(move |(_, value)| value.expression == expression)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarExpressionPlans {
    pub expressions: Vec<CheckedLocatedScalarExpression>,
    /// Source custody for selected scalar plans consumed by proof and lowering.
    /// Plans without a retained row cannot reconstruct a positional scope.
    pub source_bindings: Arena<CheckedScalarExpressionBindings>,
    pub binding_symbols: Arena<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarExpressionBindings {
    /// Exact local, bare assignment destination, or target state parameter;
    /// zero for a returned value, other argument, or projected store.
    /// This is separate from operand identities.
    pub destination: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub expression: ExpressionHandle,
    /// The producer's dense scalar namespace: parameters followed by locals.
    /// This records declarations only, never initializer expressions to replay.
    pub symbols: HandleSpan<SymbolHandle>,
}

impl Default for CheckedScalarExpressionBindings {
    fn default() -> Self {
        Self {
            destination: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            expression: ExpressionHandle::invalid(),
            symbols: HandleSpan::empty(),
        }
    }
}

impl CheckedScalarExpressionPlans {
    /// Select one custody row and one plan at the exact source coordinate.
    /// Expected handles and destinations are checked only after uniqueness;
    /// filtering by those values could conceal conflicting custody rows.
    pub fn bound_expression_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<(&CheckedScalarExpressionBindings, &CheckedScalarExpression)> {
        let mut bindings = self.source_bindings.iter().filter(|(_, binding)| {
            binding.state == state
                && binding.statement_ordinal == statement_ordinal
                && binding.role == role
        });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some() {
            return None;
        }
        let mut expressions = self.expressions.iter().filter(|expression| {
            expression.state == state
                && expression.statement_ordinal == statement_ordinal
                && expression.role == role
        });
        let expression = &expressions.next()?.expression;
        if expressions.next().is_some() {
            return None;
        }
        Some((binding, expression))
    }

    pub fn expression_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<&CheckedScalarExpression> {
        self.expressions
            .iter()
            .find(|expression| {
                expression.state == state
                    && expression.statement_ordinal == statement_ordinal
                    && expression.role == role
            })
            .map(|expression| &expression.expression)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLocatedScalarExpression {
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub expression: CheckedScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedScalarExpressionRole {
    /// One scalar field of an exact case leaf, including inside a selected arm.
    StructuralValueField {
        expression: ExpressionHandle,
        field_ordinal: u32,
    },
    /// One scalar field operand in its authored record construction.
    RecordField {
        expression: ExpressionHandle,
        field_ordinal: u32,
    },
    /// Saved scalar subject of this structural value's exact Match occurrence.
    StructuralValueSubject {
        expression: ExpressionHandle,
    },
    /// Ordered scalar pattern of this structural Match arm.
    StructuralValuePattern {
        source_arm: Handle<typed_trees::expression::TableMatchArm>,
    },
    /// One row-major scalar operand of an array construction at this statement.
    ArrayElement {
        source: CheckedArrayConstructionSource,
        element_ordinal: u32,
    },
    /// Scalar operand of a returned case construction, in authored order.
    ReturnCaseField {
        field_ordinal: u32,
    },
    /// Initial value written into mutable local storage. This does not append
    /// an immutable runtime binding or change the scalar operand namespace.
    StorageInitializer,
    LocalInitializer {
        binding_ordinal: u32,
    },
    CallArgument {
        binding_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Proof-only scalar actual for one erased callee formal, keyed by the
    /// call's binding ordinal and the dense erased-formal ordinal in the
    /// callee's contract roster. It owns no runtime operand.
    ErasedCallArgument {
        binding_ordinal: u32,
        erased_ordinal: u32,
    },
    /// Primitive argument to a bodyless boundary call, keyed by the exact
    /// call coordinate within its statement and dense scalar-parameter order.
    BoundaryCallArgument {
        call_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Primitive argument to an in-module Unit call, keyed by the exact call
    /// coordinate and dense scalar-parameter order. Structural arguments keep
    /// their separate checked custody rows.
    UnitCallArgument {
        call_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Scalar operand of the selected boundary-operator application that is
    /// this statement's immutable local initializer, keyed by its authored
    /// operand position; structural operands keep their positions but own no
    /// row. Provider settlement rewrites the application into a call over the
    /// same operand expressions, so the key and the bound expression survive
    /// it. `CheckedOperatorFacts::boundary_application_operands` names the
    /// operand and its carrier for both the producer and the replay.
    SelectedOperatorOperand {
        operand_ordinal: u32,
    },
    /// Proof-only scalar actual for one erased formal of an in-module Unit
    /// call, keyed by the exact call coordinate and the dense erased-formal
    /// ordinal in the callee's contract roster. It owns no runtime operand.
    ErasedUnitCallArgument {
        call_ordinal: u32,
        erased_ordinal: u32,
    },
    /// Present start of one exclusive `view[start..end]` range at its site.
    /// An omitted endpoint has no expression or source-binding row.
    SubsliceStart {
        site: CheckedSubsliceSite,
    },
    /// Present exclusive end of the same range at the same site.
    SubsliceEnd {
        site: CheckedSubsliceSite,
    },
    /// Right-hand side of one direct typed assignment. The coordinate remains
    /// statement-local and does not imply that every assignment is admitted
    /// by a later executable plan.
    AssignmentValue,
    /// Evaluated scalar selector of an indexed assignment target, before its value.
    AssignmentIndex,
    Return,
    /// Value returned by the false sibling of a combined transition.
    ContinuationReturn,
    Guard,
    TransitionArgument {
        argument_ordinal: u32,
    },
    /// False-arm continuation operands have distinct custody from the primary
    /// target even when their formal positions are identical.
    TransitionContinuationArgument {
        argument_ordinal: u32,
    },
}

/// The authored position of one exclusive `view[start..end]` range within its
/// statement. A call argument, a transition argument and a `let` initializer
/// all narrow a view with the same builtin range; only where the range sits
/// differs, so their endpoints share `SubsliceStart`/`SubsliceEnd` and this
/// one coordinate rather than a role pair per site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedSubsliceSite {
    /// A structural argument of the statement's `call_ordinal`th call, keyed
    /// by its dense structural argument ordinal, not its mixed authored
    /// position.
    CallArgument {
        call_ordinal: u32,
        argument_ordinal: u32,
    },
    /// A transition target argument at its authored parameter position.
    TransitionArgument { argument_ordinal: u32 },
    /// The initializer of the statement's own immutable view local.
    LocalBinding,
}

/// The storage an observation or a view derivation starts from: a structural
/// state parameter, or an immutable borrowed view local that an earlier
/// statement of the same body established (an `as_slice` loan or another
/// subslice). A view local is an established view place exactly as a whole
/// view parameter is, so lengths, element reads and subslices take this one
/// root instead of a parameter-only coordinate plus a local-only twin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStorageRoot {
    /// A structural state parameter, numbered in the holder's own parameter
    /// namespace: subslice sources count the dense structural parameter
    /// list; scalar observations count authored state positions, like every
    /// other scalar parameter read.
    Parameter { index: u32 },
    /// The `let` symbol of an immutable borrowed view local. Lowering resolves
    /// it to the place its establishment published; it never names owned
    /// storage and never carries a projection path.
    ViewLocal { symbol: SymbolHandle },
}

impl CheckedStorageRoot {
    /// The parameter number when this root is a parameter.
    pub fn parameter(self) -> Option<u32> {
        match self {
            Self::Parameter { index } => Some(index),
            Self::ViewLocal { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarExpression {
    /// Exact u64 live length of one borrowed view or bounded byte field. A
    /// parameter root counts authored state positions: an empty path selects
    /// the whole view parameter, a nonempty path the exact bounded-owned byte
    /// field, never its capacity. A view-local root is always the whole view
    /// with an empty path. Element views count elements, byte views bytes.
    StructuralParameterByteLength {
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    /// Read the current value of exact local storage at this computation's
    /// program point. Storage never occupies an immutable binding position.
    StorageRead {
        symbol: SymbolHandle,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// Dense position in the consuming plan's scalar parameter namespace. A
    /// mixed structural/scalar producer must separately retain the authored
    /// source-position partition.
    Parameter {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// Dense position in the machine's proof-only erased-formal roster. The
    /// term names an erased scalar parameter inside `requires` propositions;
    /// it has no runtime storage, position, or argument lane.
    ErasedParameter {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// Dense position after scalar parameters and earlier immutable primitive locals.
    Local {
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// Nonempty path to a relevant primitive field below one structural entry
    /// parameter in its authored namespace. Structural predicates and current
    /// program-point value snapshots retain this form; ordinary scalar native
    /// execution plans still reject it.
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    /// Selected builtin element read from current structural storage. A
    /// parameter root counts authored state positions and an empty path
    /// selects the whole parameter, not a nominal field; a view-local root is
    /// the whole view with an empty path. The index remains an evaluated
    /// dependency, not a fixed field projection.
    StructuralParameterIndexedRead {
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        index: Box<CheckedScalarExpression>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    IntegerLiteral {
        literal: numerics::literals::IntegerLiteral,
    },
    IeeeFloatLiteral {
        value: semantic_vocabulary::IeeeFloatValue,
    },
    IntegerBinary {
        kind: CheckedIntegerBinaryKind,
        primitive_type: typed_trees::types::PrimitiveType,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    IntegerBitwiseNot {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerWiden {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerExactCast {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
        range: CheckedIntegerRange,
    },
    /// Selected modular conversion at the target width, never an Exact proof.
    IntegerWrappingCast {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    /// Selected clamping conversion at the target width; total on every
    /// source value. Only unsigned-to-unsigned narrowings carry this form —
    /// other saturating pairs have no checked cast kind.
    IntegerSaturatingCast {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    /// A selected trapping conversion, not a proof of exactness or termination.
    /// Known representable inputs may supply a conditional normal-return fact;
    /// runtime consumers must retain the policy or reject this form.
    IntegerTrappingCast {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    Boolean(Box<CheckedBooleanExpression>),
}

impl CheckedScalarExpression {
    /// Whether evaluating this expression executes any Trapping primitive or
    /// Trapping conversion, i.e. whether it owns an operation-level `Trap`
    /// crash site. Boolean subterms are inspected through their integer
    /// comparisons.
    pub fn contains_trapping_operation(&self) -> bool {
        match self {
            Self::IntegerTrappingCast { .. } => true,
            Self::IntegerBinary {
                kind, left, right, ..
            } => {
                kind.is_trapping()
                    || left.contains_trapping_operation()
                    || right.contains_trapping_operation()
            }
            Self::IntegerBitwiseNot { operand, .. }
            | Self::IntegerWiden { operand, .. }
            | Self::IntegerExactCast { operand, .. }
            | Self::IntegerWrappingCast { operand, .. }
            | Self::IntegerSaturatingCast { operand, .. } => operand.contains_trapping_operation(),
            Self::StructuralParameterIndexedRead { index, .. } => {
                index.contains_trapping_operation()
            }
            Self::Boolean(expression) => expression.contains_trapping_operation(),
            Self::StructuralParameterByteLength { .. }
            | Self::StorageRead { .. }
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IntegerLiteral { .. }
            | Self::IeeeFloatLiteral { .. } => false,
        }
    }

    /// The selected result carrier, independent of executable storage or
    /// target realization. An integer literal must already have a landing.
    pub fn primitive_type(&self) -> Option<typed_trees::types::PrimitiveType> {
        use numerics::literals::LandedIntegerType;
        use typed_trees::types::PrimitiveType;
        match self {
            Self::StructuralParameterByteLength { .. } => Some(PrimitiveType::U64),
            Self::Parameter { primitive_type, .. }
            | Self::ErasedParameter { primitive_type, .. }
            | Self::StorageRead { primitive_type, .. }
            | Self::Local { primitive_type, .. }
            | Self::StructuralParameterField { primitive_type, .. }
            | Self::StructuralParameterIndexedRead { primitive_type, .. }
            | Self::IntegerBinary { primitive_type, .. }
            | Self::IntegerBitwiseNot { primitive_type, .. }
            | Self::IntegerWiden { primitive_type, .. }
            | Self::IntegerExactCast { primitive_type, .. }
            | Self::IntegerTrappingCast { primitive_type, .. }
            | Self::IntegerWrappingCast { primitive_type, .. }
            | Self::IntegerSaturatingCast { primitive_type, .. } => Some(*primitive_type),
            Self::IntegerLiteral { literal } => Some(match literal.landing()?.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => PrimitiveType::Addr,
            }),
            Self::IeeeFloatLiteral { value } => Some(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(_) => PrimitiveType::F32,
                semantic_vocabulary::IeeeFloatValue::Binary64(_) => PrimitiveType::F64,
            }),
            Self::Boolean(_) => Some(PrimitiveType::Bool),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    /// Runtime-checked arithmetic: a failed primitive predicate is an
    /// executable trap, never a silent wrap or saturation. These forms carry
    /// only normal-return facts; consumers without a Trapping realization
    /// must reject them rather than weaken the policy.
    TrappingShiftLeft,
    TrappingShiftRight,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    /// Traps on a zero divisor or signed `MIN / -1` (the settled Trapping
    /// catalog row), rather than owing a nonzero-divisor proof.
    TrappingDivide,
    TrappingRemainder,
}

impl CheckedIntegerBinaryKind {
    /// Whether this selected builtin is a Trapping primitive: its execution
    /// may crash with cause `Trap` at its own operation site.
    pub const fn is_trapping(self) -> bool {
        matches!(
            self,
            Self::TrappingShiftLeft
                | Self::TrappingShiftRight
                | Self::TrappingAdd
                | Self::TrappingSubtract
                | Self::TrappingMultiply
                | Self::TrappingDivide
                | Self::TrappingRemainder
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBooleanExpression {
    StorageRead {
        symbol: SymbolHandle,
    },
    Constant(bool),
    Parameter {
        position: usize,
    },
    /// Proof-only erased formal in authored erased order. Dense index into the
    /// machine contract's erased-scalar roster; carries no runtime operand.
    ErasedParameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    /// Nonempty path to a relevant Boolean field below one structural entry
    /// parameter. Terminal production resolves every authored identity to the
    /// canonical structural field ID at that exact type level.
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    Not(Box<CheckedBooleanExpression>),
    Equal {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    IntegerComparison {
        kind: CheckedIntegerComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    /// Atomic IEEE comparison between scalar terms (parameters and composed
    /// float expressions), as opposed to the structural-leaf form below. It is
    /// deliberately not represented as generic/reflexive scalar equality: IEEE
    /// equality is non-reflexive at NaN and distinguishes signed zero.
    ScalarIeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    /// Atomic IEEE comparison between exact relevant structural leaves. It is
    /// deliberately not represented as generic/reflexive scalar equality.
    IeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind,
        primitive_type: typed_trees::types::PrimitiveType,
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    /// Content equality between exact byte-sequence structural leaves.
    ByteSequenceEqual {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    /// Equality of two structural sums. Terminal lowering expands
    /// this closed case roster into canonical per-case membership equivalence.
    PayloadlessSumEqual {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
        cases: Vec<String>,
    },
    /// Exact active-case test over a structural sum subject. The path reaches
    /// the sum itself; payload paths use an explicit following Case segment.
    StructuralCaseMembership {
        subject: CheckedStructuralParameterField,
        case: String,
    },
    And {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    Or {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
}

impl CheckedBooleanExpression {
    /// Whether evaluating this condition executes a Trapping primitive in
    /// one of its integer operands.
    pub fn contains_trapping_operation(&self) -> bool {
        match self {
            Self::Not(operand) => operand.contains_trapping_operation(),
            Self::Equal { left, right } | Self::And { left, right } | Self::Or { left, right } => {
                left.contains_trapping_operation() || right.contains_trapping_operation()
            }
            Self::IntegerComparison { left, right, .. }
            | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                left.contains_trapping_operation() || right.contains_trapping_operation()
            }
            Self::StorageRead { .. }
            | Self::Constant(_)
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IeeeFloatComparison { .. }
            | Self::ByteSequenceEqual { .. }
            | Self::PayloadlessSumEqual { .. }
            | Self::StructuralCaseMembership { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIeeeFloatComparisonKind {
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedStructuralParameterField {
    pub parameter_position: u32,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedStructuralPredicatePathSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
