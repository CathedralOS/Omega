//! Value facts: the checker's record of each value at its use site, and the
//! scalar, structural and proof-term plans built over those values.
//!
//! `CheckedValueFacts` holds the `CheckedValueFact` rows (each value's
//! origin, declared type, selected primitive carrier and discharged integer
//! range) and four plan tables: scalar expression plans with the source
//! bindings that locate each plan in its authored statement
//! (`CheckedScalarExpressionPlans`), scalar computations (`computations`),
//! structural values (`structural_values`), and proof-only erased actuals
//! (`proof_terms`). `typed-trees-to-checked-trees` builds it in
//! `build_check_facts` (`src/facts.rs`), starting from `build_value_facts`,
//! and stores it as `CheckFacts::values`; `checked-trees-to-lowered-psi`
//! reads it through `checked.facts.values`.
//!
//! This file defines the value rows, the scalar expression plans and their
//! `CheckedScalarExpressionRole`s, and reexports the shared scalar and Boolean
//! expression algebra owned by `facts`. `array_construction_source` names the
//! owner of an array constructor. `guard_complement` decides whether a
//! second authored guard holds exactly where the first fails; both
//! `typed-trees-to-checked-trees` and `checked-trees-to-lowered-psi` call it.

use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

pub use facts::{
    CheckedBooleanExpression, CheckedByteSequenceOperand, CheckedIeeeFloatComparisonKind,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind, CheckedIntegerMathExpression,
    CheckedIntegerMathTerm, CheckedScalarExpression, CheckedScalarReadProjection,
    CheckedScalarSelector, CheckedStorageRoot, CheckedStructuralCaseRoot,
    CheckedStructuralCaseSubject, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment,
};

mod computations;
pub use computations::{
    CheckedScalarCallee, CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot,
    CheckedScalarComputationStructuralArgument, CheckedScalarComputationTemporaryCleanup,
    CheckedScalarDispatchArm, CheckedScalarDispatchPattern,
};
mod proof_terms;
pub use proof_terms::{
    CheckedErasedProofParameterPlan, CheckedLocatedProofTerm, CheckedProofTerm,
    CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
};
mod structural_values;
pub use structural_values::{
    CheckedElementViewRegion, CheckedQuotientAdaptationLicense, CheckedStructuralDispatchArm,
    CheckedStructuralRecordField, CheckedStructuralRecordFieldValue, CheckedStructuralValue,
    CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedStructuralValuePlans,
    CheckedStructuralValueRoot,
};
mod array_construction_source;
pub use array_construction_source::CheckedArrayConstructionSource;
pub mod guard_complement;

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
    /// Arrival observation of the independently checked natural rank, before
    /// the state prefix. Its root uses this state's exact formal telescope.
    RankObservation,
    /// Evaluated selector of one structural call argument. The argument's
    /// dense structural ordinal and authored operand keep equal selectors at
    /// different evaluation points distinct.
    CallSelector {
        call_ordinal: u32,
        argument_ordinal: u32,
        expression: ExpressionHandle,
    },
    /// One address operand of a selected named successor. The full target
    /// parameter position keeps its evaluation distinct from sibling actuals.
    TransitionSelector {
        argument_ordinal: u32,
        expression: ExpressionHandle,
    },
    /// Captured byte coordinate of this representation view occurrence.
    RepresentationViewOffset {
        expression: ExpressionHandle,
    },
    /// One evaluated index of an exact structural value formation. The
    /// ordinal follows executable selectors in the retained projection path;
    /// mutable storage is read here, never replayed when the loan is used.
    StructuralValueSelector {
        expression: ExpressionHandle,
        selector_ordinal: u32,
    },
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
    /// One row-major scalar leaf of a closed primitive array supplied inside a
    /// structural value, such as a record literal's `samples: [1, 2]` field.
    /// The key is the array's own authored expression, exactly as `RecordField`
    /// keys by its record literal, so two arrays in one statement stay distinct.
    StructuralArrayElement {
        expression: ExpressionHandle,
        element_ordinal: u32,
    },
    /// The evaluated scalar that initializes one structural scalar home.
    StructuralScalarValue {
        expression: ExpressionHandle,
    },
    /// Initial value written into mutable local storage. This does not append
    /// an immutable runtime binding or change the scalar operand namespace.
    StorageInitializer,
    /// One authored operand of the atomic carrier at this statement, by its
    /// event-order ordinal: the stored or swapped value, the fetch operand,
    /// or the compare-exchange expected (0) and replacement (1).
    AtomicOperand {
        operand_ordinal: u32,
    },
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
    /// Scalar actual for one finite-family binder of a dynamic requirement
    /// call, keyed by the exact call coordinate and the roster's dense
    /// family-binder ordinal. It is the runtime row-selection operand the
    /// dispatch compares against the declared tuple identities.
    DynamicFamilyArgument {
        call_ordinal: u32,
        family_ordinal: u32,
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
    /// Evaluated scalar selector of one indexed step of an assignment target,
    /// before its value. `depth` counts the target's indexed steps from the
    /// target inward: `a[i] = v` and `a[i].f = v` select `i` at depth 0, and
    /// `a[i][j] = v` selects `j` at depth 0 and `i` at depth 1. Every indexed
    /// step has a row, literal or runtime, so a byte store's own selector is
    /// always depth 0.
    AssignmentIndex {
        depth: u32,
    },
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
    /// A computed operand at its exact expression occurrence. It has no call
    /// formal or source local, but its endpoints use the same scalar namespace.
    Expression { expression: ExpressionHandle },
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

#[macro_export]
macro_rules! integer_widen_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedScalarExpression::IntegerBitwiseNot { $($fields)* }
            | $crate::CheckedScalarExpression::IntegerExactCast { $($fields)* }
            | $crate::CheckedScalarExpression::IntegerWiden { $($fields)* }
    };
}
