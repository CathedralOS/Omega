//! The proof plan: the constraints, obligations and owners it records, and
//! the bounded obligation carriers checking consumes.

use crate::obligations::collection::estimated_proof_obligation_capacity;
use crate::obligations::ranges::float_range_bound;
use arena::{Arena, HandleSpan};
pub(crate) use facts::IntegerRange;
use numerics::bignum::BigInt;
use std::fmt;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, FloatLiteral};
use typed_trees::name::Identifier;
use typed_trees::statement::TransitionGuardNode;
use typed_trees::types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan<'program> {
    pub program: &'program dyn ::validation::CheckingInput,
    pub obligations: Arena<ProofObligation>,
    pub type_constraints: Arena<ProofConstraint>,
    /// Range-analysis inputs for every assignment whose target type resolves,
    /// including assignments to unconstrained semantic carriers. Bounded proof
    /// obligations remain the constrained subset; this complete arena lets the
    /// checked-tree boundary retain useful, already-validated flow evidence for
    /// physical encodings without creating a new language obligation.
    pub assignment_value_ranges: Arena<BoundedAssignmentObligation>,
}

impl<'program> ProofPlan<'program> {
    pub(crate) fn new(program: &'program dyn ::validation::CheckingInput) -> Self {
        let obligation_capacity = estimated_proof_obligation_capacity(program);
        let constraint_capacity = program
            .type_reference_table
            .constraint_count()
            .saturating_add(obligation_capacity);

        Self {
            program,
            obligations: Arena::with_capacity(obligation_capacity),
            type_constraints: Arena::with_capacity(constraint_capacity),
            assignment_value_ranges: Arena::with_capacity(obligation_capacity),
        }
    }

    pub(crate) fn push_obligation(&mut self, obligation: ProofObligation) {
        self.obligations.append(obligation);
    }

    pub(crate) fn store_constraints(
        &mut self,
        constraints: ConstraintBuffer,
    ) -> HandleSpan<ProofConstraint> {
        self.type_constraints.insert_many(constraints)
    }

    pub(crate) fn store_constraint_nodes(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<ProofConstraint> {
        self.store_constraint_nodes_with(
            program,
            base_type,
            constraints,
            ProofConstraint::from_node,
        )
    }

    /// [`Self::store_constraint_nodes`] for the one obligation that DEMANDS an
    /// exact-interval declared domain as its interval; see
    /// [`ProofConstraint::from_node_with_domain_intervals`].
    pub(crate) fn store_constraint_nodes_with_domain_intervals(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<ProofConstraint> {
        self.store_constraint_nodes_with(
            program,
            base_type,
            constraints,
            ProofConstraint::from_node_with_domain_intervals,
        )
    }

    fn store_constraint_nodes_with(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
        from_node: impl Fn(
            &dyn ::validation::CheckingInput,
            TypeReferenceHandle,
            &TypeConstraintNode,
        ) -> Option<ProofConstraint>,
    ) -> HandleSpan<ProofConstraint> {
        self.type_constraints.insert_many(
            program
                .type_reference_table
                .constraints(constraints)
                .iter()
                .filter_map(|constraint| from_node(program, base_type, constraint)),
        )
    }
}

/// One endpoint of a declared integer range whose other endpoint, or itself,
/// is a retained constant recipe: closed where the shared judgment can read
/// it, otherwise the recipe plus the inclusive offset its spelling implies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeRangeBound {
    Closed(BigInt),
    Recipe {
        recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
        offset: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofConstraint {
    Named(Identifier),
    IntegerRange {
        minimum: numerics::bignum::BigInt,
        maximum: numerics::bignum::BigInt,
    },
    /// A declared range with a retained constant recipe at an endpoint
    /// (`u64[0..=limit()]`). The bounded checks prove the value's own range
    /// and record the comparison against the recipe for the target closure.
    IntegerRangeRecipe {
        minimum: RecipeRangeBound,
        maximum: RecipeRangeBound,
    },
    /// R1 dependent range (`[0..=self.count]`): the maximum names a `self`
    /// FIELD's entry value plus a literal offset. Minted only for the
    /// recognizer's admissible class (`typed-trees::dependent_ranges`);
    /// consumed by the bounded-argument checks, which must DISCHARGE it --
    /// an unrecognized carrier refuses at the validation fence long before
    /// proofs run, so this variant never silently widens.
    IntegerRangeSymbolicMax {
        minimum: i64,
        max_field: Identifier,
        max_offset: i64,
    },
    /// R1 sibling-length range (`[0..items.len]`, Buffer::get): the maximum
    /// names a SIBLING PARAMETER's slice length plus a literal offset. The
    /// obligation builders resolve the sibling's ARGUMENT at build time
    /// (`sibling_argument` on the obligation); discharge is the co-located
    /// guard route only -- slice lengths have no static floor.
    IntegerRangeSiblingLenMax {
        minimum: i64,
        sibling: Identifier,
        max_offset: i64,
    },
    /// A floating range with an inclusive minimum and an authored maximum.
    /// `maximum_inclusive = false` means the value must be less than the
    /// maximum under IEEE order, so NaN satisfies neither endpoint. The
    /// authored maximum remains verbatim because floats have no predecessor.
    FloatRange {
        minimum: FloatLiteral,
        maximum: FloatLiteral,
        maximum_inclusive: bool,
    },
    /// Operand-driven arithmetic behavior carried through expression
    /// derivation. This is metadata for deciding which value facts an
    /// operation establishes, not itself a proof predicate.
    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
}

impl Default for ProofConstraint {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

impl ProofConstraint {
    /// `base_type` is the constrained reference's carrier handle: floating
    /// range endpoints read their authored spelling at that declared carrier
    /// (validation's `closed_float_range_endpoint` twin), never at the
    /// transitional f64 window.
    pub(crate) fn from_node(
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        constraint: &TypeConstraintNode,
    ) -> Option<Self> {
        match constraint {
            TypeConstraintNode::Named(name) => Some(Self::Named(name.clone())),
            // Compiler-known VALUE domains use honest domain notation at the
            // surface while retaining their canonical scalar proof identity.
            // This lets `f64 in Finite` reuse the established finite-literal,
            // float-range, and invariant-window machinery without pretending
            // that an authored carrier domain was declared.
            TypeConstraintNode::Domain(domain) => {
                language_semantics::value_domain::ValueDomain::from_name(domain.as_str())
                    .map(|domain| Self::Named(Identifier::generated_static(domain.proof_name())))
            }
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => Self::range_from_expression_handles(
                program,
                base_type,
                *minimum,
                *maximum,
                *end_inclusive,
            ),
            // Arithmetic policy is not a predicate, but the proof derivation
            // needs it to judge facts established by the operation (for
            // example, finite Saturating add/subtract/multiply stays Finite).
            TypeConstraintNode::ArithmeticDomain(domain) => Some(Self::ArithmeticDomain(*domain)),
        }
    }

    /// [`Self::from_node`] reading an exact-interval DECLARED domain as the
    /// interval it means.
    ///
    /// A VALUE of such a domain satisfies that interval, so this is honest
    /// evidence wherever a value's known constraints are gathered. As an
    /// obligation's DEMAND it belongs only to a written place, which an
    /// exact-interval domain restricts exactly as the bracketed range it
    /// replaced did. Everywhere else the demand stays the domain, because the
    /// checks that own membership name it in their diagnostics and enforce its
    /// establishment route, and an interval standing in for it takes both
    /// away.
    pub(crate) fn from_node_with_domain_intervals(
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        constraint: &TypeConstraintNode,
    ) -> Option<Self> {
        Self::from_node(program, base_type, constraint).or_else(|| match constraint {
            TypeConstraintNode::Domain(domain) => {
                Self::exact_domain_interval(program, base_type, domain)
            }
            _ => None,
        })
    }

    fn exact_domain_interval(
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        domain: &typed_trees::types::DomainConstraint,
    ) -> Option<Self> {
        let (minimum, maximum) = validation::exact_declared_domain_carrier_interval(
            program,
            program.primitive_type_reference(base_type)?,
            domain,
        )?;
        Some(Self::IntegerRange { minimum, maximum })
    }

    fn range_from_expression_handles(
        program: &dyn ::validation::CheckingInput,
        base_type: TypeReferenceHandle,
        minimum: ExpressionHandle,
        maximum: ExpressionHandle,
        end_inclusive: bool,
    ) -> Option<Self> {
        // A floating carrier's authored endpoints are float bounds even when
        // both spellings read as exact integers: anonymous `0.0`/`100.0`
        // literals evaluate integrally, so the integer read alone would mint
        // an integer window no float value could satisfy. The float bound
        // reader already converts integer-spelled endpoints at the declared
        // carrier, so no integer constraint is ever honest evidence here.
        if !matches!(
            program.primitive_type_reference(base_type),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        ) {
            // Endpoint evaluation keeps its selected arithmetic. Only the
            // completed upper value is converted to an inclusive
            // proof-integer bound.
            let integer_bound = validation::closed_integer_range_bound(program, minimum);
            if let (Some(minimum), Some(maximum)) = (
                integer_bound.as_ref(),
                validation::closed_integer_range_maximum(program, maximum, end_inclusive),
            ) {
                return Some(Self::IntegerRange {
                    minimum: minimum.clone(),
                    maximum,
                });
            }
            // A rostered recipe endpoint stays symbolic; its closure fixes it.
            let recipe_bound = |endpoint: ExpressionHandle, offset: i64| {
                let recipe = program.constant_recipe_for_range_endpoint(endpoint);
                recipe
                    .is_valid()
                    .then_some(RecipeRangeBound::Recipe { recipe, offset })
            };
            let minimum_bound = integer_bound
                .clone()
                .map(RecipeRangeBound::Closed)
                .or_else(|| recipe_bound(minimum, 0));
            let maximum_bound =
                validation::closed_integer_range_maximum(program, maximum, end_inclusive)
                    .map(RecipeRangeBound::Closed)
                    .or_else(|| recipe_bound(maximum, if end_inclusive { 0 } else { -1 }));
            if let (Some(minimum), Some(maximum)) = (minimum_bound, maximum_bound)
                && (matches!(minimum, RecipeRangeBound::Recipe { .. })
                    || matches!(maximum, RecipeRangeBound::Recipe { .. }))
            {
                return Some(Self::IntegerRangeRecipe { minimum, maximum });
            }
            // Symbolic proof atoms retain inclusive offsets, not executable
            // subtraction. Validation fences an unrepresentable offset first.
            if let Some(minimum) = integer_bound.as_ref().and_then(BigInt::to_i64)
                && let Some(symbolic) = typed_trees::dependent_ranges::symbolic_range_maximum(
                    program,
                    maximum,
                    end_inclusive,
                )
            {
                return Some(Self::IntegerRangeSymbolicMax {
                    minimum,
                    max_field: symbolic.field,
                    max_offset: symbolic.offset,
                });
            }
            // R1 sibling-length maximum (`[0..items.len]` -> len - 1).
            if let Some(minimum) = integer_bound.as_ref().and_then(BigInt::to_i64)
                && let Some(sibling) = typed_trees::dependent_ranges::sibling_range_maximum(
                    program,
                    maximum,
                    end_inclusive,
                )
            {
                return Some(Self::IntegerRangeSiblingLenMax {
                    minimum,
                    sibling: sibling.sibling,
                    max_offset: sibling.offset,
                });
            }
        }

        Some(Self::FloatRange {
            minimum: FloatLiteral::new(float_range_bound(program, base_type, minimum)?),
            maximum: FloatLiteral::new(float_range_bound(program, base_type, maximum)?),
            maximum_inclusive: end_inclusive,
        })
    }
}

const INLINE_PROOF_CONSTRAINTS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintBuffer {
    inline: [Option<ProofConstraint>; INLINE_PROOF_CONSTRAINTS],
    overflow: Vec<ProofConstraint>,
    count: usize,
}

impl ConstraintBuffer {
    pub(crate) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            overflow: Vec::new(),
            count: 0,
        }
    }

    pub(crate) fn push(&mut self, constraint: ProofConstraint) {
        if self.count < INLINE_PROOF_CONSTRAINTS {
            self.inline[self.count] = Some(constraint);
        } else {
            self.overflow.push(constraint);
        }

        self.count = self
            .count
            .checked_add(1)
            .expect("proof constraint count overflow");
    }

    pub(crate) fn extend(&mut self, constraints: ConstraintBuffer) {
        self.extend_iter(constraints);
    }

    pub(crate) fn extend_iter(&mut self, constraints: impl IntoIterator<Item = ProofConstraint>) {
        for constraint in constraints {
            self.push(constraint);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn iter(&self) -> ConstraintBufferIter<'_> {
        self.inline.iter().flatten().chain(self.overflow.iter())
    }
}

pub(crate) type ConstraintBufferIter<'a> = std::iter::Chain<
    std::iter::Flatten<std::slice::Iter<'a, Option<ProofConstraint>>>,
    std::slice::Iter<'a, ProofConstraint>,
>;

impl<'a> IntoIterator for &'a ConstraintBuffer {
    type IntoIter = ConstraintBufferIter<'a>;
    type Item = &'a ProofConstraint;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Default for ConstraintBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for ConstraintBuffer {
    type IntoIter = std::iter::Chain<
        std::iter::Flatten<std::array::IntoIter<Option<ProofConstraint>, INLINE_PROOF_CONSTRAINTS>>,
        std::vec::IntoIter<ProofConstraint>,
    >;
    type Item = ProofConstraint;

    fn into_iter(self) -> Self::IntoIter {
        self.inline.into_iter().flatten().chain(self.overflow)
    }
}

impl std::iter::FromIterator<ProofConstraint> for ConstraintBuffer {
    fn from_iter<T: IntoIterator<Item = ProofConstraint>>(iter: T) -> Self {
        let mut constraints = Self::new();
        constraints.extend_iter(iter);
        constraints
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    BoundedAssignment(BoundedAssignmentObligation),
    BoundedCallArgument(BoundedCallArgumentObligation),
    BoundedInitializer(BoundedInitializerObligation),
    BoundedStateReturn(BoundedStateReturnObligation),
    BoundedValue(BoundedValueObligation),
    BoundedTransitionArgument(BoundedTransitionArgumentObligation),
    GuardedTransition(GuardedTransitionObligation),
}

impl Default for ProofObligation {
    fn default() -> Self {
        Self::BoundedValue(BoundedValueObligation {
            owner: ProofObligationOwner::default(),
            base_type: TypeReferenceHandle::invalid(),
            constraints: HandleSpan::empty(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofObligationOwner {
    #[default]
    Unknown,
    MachineOwnedData {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        data_symbol: SymbolHandle,
        data: Identifier,
    },
    StateParameter {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
        parameter_symbol: SymbolHandle,
        parameter: Identifier,
    },
    StateReturn {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
    },
    /// The local that statement `statement_index` of the state declares. Its
    /// initializer is the local's first store, so its position is the
    /// occurrence the store's facts are read at.
    StateLocal {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
        local_symbol: SymbolHandle,
        local: Identifier,
        statement_index: usize,
    },
    /// One selected field's first store in an aggregate constructor. The
    /// declaration owns the receiving constraint, and the statement coordinate
    /// selects the evaluated operand's current facts; this is not a local binder.
    ConstructorField {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
        field_symbol: SymbolHandle,
        field: Identifier,
        statement_index: usize,
    },
    /// A literal element's first store. Keep its parent and ordinal as well
    /// as the statement: sibling operands may observe different live facts.
    ArrayElement {
        machine_symbol: SymbolHandle,
        machine: Identifier,
        state_symbol: SymbolHandle,
        state: Identifier,
        statement_index: usize,
        construction: ExpressionHandle,
        element_index: usize,
    },
}

impl ProofObligationOwner {
    /// `(machine, state, statement index)` of the statement that evaluates
    /// the owned value. Local and constructor-field initializers have one; owned data,
    /// parameters and returns are not evaluated at a single statement.
    pub(crate) fn state_statement(&self) -> Option<(SymbolHandle, SymbolHandle, usize)> {
        match self {
            Self::StateLocal {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | Self::ConstructorField {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | Self::ArrayElement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } => Some((*machine_symbol, *state_symbol, *statement_index)),
            Self::Unknown
            | Self::MachineOwnedData { .. }
            | Self::StateParameter { .. }
            | Self::StateReturn { .. } => None,
        }
    }
}

impl fmt::Display for ProofObligationOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => formatter.write_str("unknown"),
            Self::MachineOwnedData { machine, data, .. } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")
            }
            Self::StateParameter {
                machine,
                state,
                parameter,
                ..
            } => write!(
                formatter,
                "machine `{machine}` state `{state}` parameter `{parameter}`"
            ),
            Self::StateReturn { machine, state, .. } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` return value"
                )
            }
            Self::StateLocal {
                machine,
                state,
                local,
                ..
            } => write!(
                formatter,
                "machine `{machine}` state `{state}` local `{local}`"
            ),
            Self::ConstructorField {
                machine,
                state,
                field,
                ..
            } => write!(
                formatter,
                "machine `{machine}` state `{state}` constructor field `{field}`"
            ),
            Self::ArrayElement {
                machine,
                state,
                element_index,
                ..
            } => write!(
                formatter,
                "machine `{machine}` state `{state}` array element {element_index}"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedValueObligation {
    pub owner: ProofObligationOwner,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTransitionObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub guard: TransitionGuardNode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub statement_index: usize,
    pub state_guard: Option<TransitionGuardNode>,
    /// The SOURCE state the incoming `state_guard` was taken from, for
    /// resolving guard-side hoisted locals (`__hoist_N` in the guard's own
    /// scope). Invalid when `state_guard` is None.
    pub state_guard_source: SymbolHandle,
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    /// Present when `value` is a top-level integer BINARY: its operands with
    /// their declared ranges, for the checker's guard-assisted refold.
    pub binary_operands: Option<BinaryValueOperands>,
    /// R4 containment intake: `(place display, INCLUSIVE upper bound)` for
    /// every boundary-ensures witness live at this assignment (see
    /// `ensures_witness_bounds_at`).
    pub ensures_witness_bounds: Vec<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    /// The statement that evaluates the call: a statement call, or any
    /// statement whose expressions contain the call. Occurrence facts for the
    /// argument are read at this statement.
    pub statement_index: usize,
    pub receiver: Option<Identifier>,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub parameter_symbol: SymbolHandle,
    pub parameter: Identifier,
    pub argument: ExpressionHandle,
    pub argument_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    /// See BoundedTransitionArgumentObligation::sibling_argument.
    pub sibling_argument: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInitializerObligation {
    pub owner: ProofObligationOwner,
    pub value: ExpressionHandle,
    /// Constraints the value carries where it is evaluated. A state local's
    /// initializer reads them in its state; owned data has no state, so its
    /// initializer carries none.
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStateReturnObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub statement_index: usize,
    pub value: ExpressionHandle,
    pub value_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    pub binary_operands: Option<BinaryValueOperands>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: Identifier,
    pub state_symbol: SymbolHandle,
    pub state: Identifier,
    pub statement_index: usize,
    pub parameter_symbol: SymbolHandle,
    pub parameter: Identifier,
    pub argument: ExpressionHandle,
    pub argument_constraints: HandleSpan<ProofConstraint>,
    pub base_type: TypeReferenceHandle,
    pub constraints: HandleSpan<ProofConstraint>,
    pub guard: TransitionGuardNode,
    /// Guards of PRIOR in-state EXIT transitions (guarded, valid target, no
    /// fall-through arm): control reaching THIS transition refutes each one,
    /// so the checker may narrow argument places by their complements (the
    /// MR2 fall-through shape: `transition n == 0 { true -> exit }` then the
    /// rewritten loop-back's `n - 1`). Only collected when the arguments are
    /// call-free (same stability rule as `guard`).
    pub refuted_exit_guards: Vec<ExpressionHandle>,
    /// The caller's ARGUMENT for the sibling a sibling-length atom names
    /// (invalid when the parameter has no such atom or the sibling is
    /// absent) -- resolved at build time, where the full parameter and
    /// argument lists are in hand.
    pub sibling_argument: ExpressionHandle,
}

/// A top-level BINARY assignment value's operands with their DECLARED ranges,
/// resolved at obligation-build time. The checker refolds them with the
/// stability-gated edge guard filling in an operand the declaration leaves
/// unbounded (`self.y = self.p + self.dir` with `p: [0..=8]` declared and
/// `dir` bounded only by the incoming guard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryValueOperands {
    pub operator: BinaryOperator,
    pub left: ExpressionHandle,
    pub(crate) left_range: Option<IntegerRange>,
    pub right: ExpressionHandle,
    pub(crate) right_range: Option<IntegerRange>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloatRange {
    pub(crate) minimum: f64,
    pub(crate) maximum: f64,
    pub(crate) maximum_inclusive: bool,
}

impl FloatRange {
    pub(crate) fn closed(value: f64) -> Self {
        Self {
            minimum: value,
            maximum: value,
            maximum_inclusive: true,
        }
    }

    pub(crate) fn intersect(self, other: Self) -> Self {
        let maximum = self.maximum.min(other.maximum);
        let maximum_inclusive = if self.maximum == other.maximum {
            self.maximum_inclusive && other.maximum_inclusive
        } else if self.maximum < other.maximum {
            self.maximum_inclusive
        } else {
            other.maximum_inclusive
        };
        Self {
            minimum: self.minimum.max(other.minimum),
            maximum,
            maximum_inclusive,
        }
    }

    pub(crate) fn contains_range(&self, value: &Self) -> bool {
        value.minimum >= self.minimum
            && (value.maximum < self.maximum
                || (value.maximum == self.maximum
                    && (self.maximum_inclusive || !value.maximum_inclusive)))
    }

    /// Whether membership in this range is finite evidence. IEEE endpoint
    /// comparisons never hold for NaN, so membership already excludes it;
    /// what remains are the infinities. The inclusive minimum must itself
    /// be finite, and the maximum must be finite unless the bound is
    /// EXCLUSIVE -- `x < +inf` rejects +inf while `x <= +inf` admits it.
    pub(crate) fn proves_finite(&self) -> bool {
        self.minimum.is_finite() && (self.maximum.is_finite() || !self.maximum_inclusive)
    }
}
