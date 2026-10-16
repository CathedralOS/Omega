//! Shared checked scalar and predicate expressions for source facts and plans.
//!
//! The algebra retains selected primitive meanings and exact operand namespaces.
//! Fact storage and executable plans use the same terms; constructing a term
//! alone establishes neither its truth nor the custody of storage it observes.

use crate::IntegerRange as CheckedIntegerRange;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

mod bindings;
pub use bindings::{BooleanPredicate, BooleanPredicateBinding, BooleanPredicateRole};

mod byte_comparison;
mod integer_math;
pub use integer_math::{CheckedIntegerMathExpression, CheckedIntegerMathTerm};

/// The storage an observation or a view derivation starts from: a structural
/// state parameter, or an immutable borrowed view local that an earlier
/// statement of the same body established (a whole-collection loan or
/// another subslice). A view local is an established view place exactly as a whole
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
    /// The `let` symbol of an owned structural local — a fixed array or
    /// record the body established in place. Lowering resolves it through the
    /// plan's established structural locals; element reads project a runtime
    /// index into its declared extent like a parameter's fixed-array field.
    OwnedLocal { symbol: SymbolHandle },
}

impl CheckedStorageRoot {
    /// The parameter number when this root is a parameter.
    pub fn parameter(self) -> Option<u32> {
        match self {
            Self::Parameter { index } => Some(index),
            Self::ViewLocal { .. } | Self::OwnedLocal { .. } => None,
        }
    }
}

/// One evaluated selector and the authored indexing occurrence it selects for.
/// The occurrence names `collection[index]`, not its index operand: equal
/// operands can select different extents. Generated selectors use the zero
/// handle and cannot consume an authored bounds decision. Source plans retain
/// these rows without interning them by value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarSelector {
    pub expression: CheckedScalarExpression,
    pub occurrence: typed_trees::expression::ExpressionHandle,
}

/// Scalar read paths retain evaluated selectors beside static member geometry.
/// This is executable occurrence custody, not the canonical geometry used as
/// a fact key. Member-separated and consecutive indexes take the same path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarReadProjection {
    Static(CheckedStructuralPredicatePathSegment),
    RuntimeIndex(CheckedScalarSelector),
}

impl CheckedScalarReadProjection {
    /// Closed evidence vocabularies can retain a literal position as static
    /// geometry, but must refuse other executable selectors. This conversion
    /// is not an executable path: the original keeps each bounds occurrence.
    pub fn static_path(path: &[Self]) -> Option<Vec<CheckedStructuralPredicatePathSegment>> {
        path.iter()
            .map(|step| match step {
                Self::Static(step) => Some(step.clone()),
                Self::RuntimeIndex(CheckedScalarSelector {
                    expression: CheckedScalarExpression::IntegerLiteral { literal },
                    ..
                }) => Some(CheckedStructuralPredicatePathSegment::FixedIndex(
                    literal.value_u64()?,
                )),
                Self::RuntimeIndex(_) => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarExpression {
    /// Exact u64 live count at one storage projection, never its capacity.
    /// The root counts authored structural positions; selectors keep their
    /// consuming scalar namespace and indexing proof occurrence. Element views
    /// count elements, byte views bytes, including descriptors stored in records.
    StructuralSequenceLength {
        root: CheckedStorageRoot,
        path: Vec<CheckedScalarReadProjection>,
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
    /// dependency, not a fixed field projection. `element_path` projects from
    /// the selected element to its scalar leaf, interleaving members and further
    /// evaluated selectors. Every selector owns its own `index < extent`
    /// obligation; selectors are never folded into one flattened coordinate.
    StructuralParameterIndexedRead {
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        index: Box<CheckedScalarSelector>,
        element_path: Vec<CheckedScalarReadProjection>,
        primitive_type: typed_trees::types::PrimitiveType,
    },
    IntegerLiteral {
        literal: numerics::literals::IntegerLiteral,
    },
    IeeeFloatLiteral {
        value: semantic_vocabulary::IeeeFloatValue,
    },
    /// `Implementation as dyn Trait<Args>`: a selector naming the
    /// implementation's table for the interface `interface_identity` names
    /// (the normalized selector type). It is a constant: evaluating it reads
    /// no data.
    ImplementationSelection {
        implementation: symbols::SymbolHandle,
        interface_identity: TypeIdentityId,
    },
    IntegerBinary {
        kind: CheckedIntegerBinaryKind,
        primitive_type: typed_trees::types::PrimitiveType,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
        /// Checking decided every formation condition this occurrence owes
        /// (a division's nonzero divisor and, for an Exact signed one, the
        /// excluded `MIN / -1` pair; an Exact sum, difference or product's
        /// representability). Lowering marks the emitted operation so its
        /// obligation consumes the decision; false keeps the certificate
        /// search.
        formation_decided: bool,
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
        /// Checking proved this source conversion's value fits its target
        /// (the retained `range`). Lowering marks the emitted conversion so
        /// its obligation consumes the decision. A conversion lowering or a
        /// substitution synthesizes leaves this false and keeps its search.
        formation_decided: bool,
    },
    Boolean(Box<CheckedBooleanExpression>),
    /// A §5b representation re-view of one whole scalar place
    /// (`&place as &T`, or a read through a loan local bound to one). The
    /// operand is the source place's own read at its declared carrier; the
    /// view observes the same bytes as `primitive_type`. Stage 04 retains
    /// this form only after the recast judgment admitted the pair and only
    /// when the carriers differ -- a same-carrier view changes facts, not
    /// representation, and reads the place directly. Consumers replay the
    /// carrier relation from both types rather than trusting a read retagged
    /// with a carrier its storage does not declare.
    RepresentationView {
        primitive_type: typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
}

/// A decimal `u64` literal landed in the `U8`/`U64` exact domain — the
/// shape every constant-injection site builds for small bounded values.
/// Other primitive types have no authored landing here.
pub fn u64_integer_literal(
    value: u64,
    primitive_type: typed_trees::types::PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let landed_type = match primitive_type {
        typed_trees::types::PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        typed_trees::types::PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        typed_trees::types::PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        typed_trees::types::PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    let literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        &value.to_string(),
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
    });
    Some(CheckedScalarExpression::IntegerLiteral { literal })
}

impl CheckedScalarExpression {
    /// Whether evaluating this expression executes any Trapping primitive,
    /// i.e. whether it owns an operation-level `Trap` crash site.
    /// Boolean subterms are inspected through their integer
    /// comparisons.
    pub fn contains_trapping_operation(&self) -> bool {
        match self {
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
            | Self::RepresentationView { operand, .. } => operand.contains_trapping_operation(),
            Self::StructuralParameterIndexedRead {
                index,
                element_path,
                ..
            } => {
                index.expression.contains_trapping_operation()
                    || element_path.iter().any(|projection| match projection {
                        CheckedScalarReadProjection::Static(_) => false,
                        CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            selector.expression.contains_trapping_operation()
                        }
                    })
            }
            Self::Boolean(expression) => expression.contains_trapping_operation(),
            Self::StructuralSequenceLength { path, .. } => path.iter().any(|step| match step {
                CheckedScalarReadProjection::Static(_) => false,
                CheckedScalarReadProjection::RuntimeIndex(selector) => {
                    selector.expression.contains_trapping_operation()
                }
            }),
            Self::StorageRead { .. }
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IntegerLiteral { .. }
            | Self::IeeeFloatLiteral { .. }
            | Self::ImplementationSelection { .. } => false,
        }
    }

    /// The selected result carrier, independent of executable storage or
    /// target realization. An integer literal must already have a landing.
    pub fn primitive_type(&self) -> Option<typed_trees::types::PrimitiveType> {
        use typed_trees::types::PrimitiveType;
        match self {
            Self::StructuralSequenceLength { .. } => Some(PrimitiveType::U64),
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
            | Self::RepresentationView { primitive_type, .. } => Some(*primitive_type),
            Self::IntegerLiteral { literal } => Some(PrimitiveType::from_landed_integer(
                literal.landing()?.landed_type,
            )),
            Self::IeeeFloatLiteral { value } => Some(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(_) => PrimitiveType::F32,
                semantic_vocabulary::IeeeFloatValue::Binary64(_) => PrimitiveType::F64,
            }),
            Self::Boolean(_) => Some(PrimitiveType::Bool),
            Self::ImplementationSelection { .. } => Some(PrimitiveType::Selector),
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
    SaturatingShiftLeft,
    SaturatingShiftRight,
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
    /// Proof-only comparison over mathematical integers. This form has no
    /// executable Boolean lowering; its finite leaves retain exact subjects.
    IntegerMathComparison {
        kind: CheckedIntegerComparisonKind,
        left: CheckedIntegerMathExpression,
        right: CheckedIntegerMathExpression,
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
    /// Proof-relevant identity of two complete data values in the surrounding
    /// invocation frame. Both paths retain their parameter's exact type, and
    /// Terminal production resolves and checks both endpoint types.
    /// This has no executable comparison and does not enumerate payload fields.
    StructuralIdentity {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    /// Content equality between exact byte-sequence structural leaves.
    ByteSequenceEqual {
        left: CheckedByteSequenceOperand,
        right: CheckedByteSequenceOperand,
    },
    /// Evaluate one declared byte predicate over an exact byte-sequence operand.
    /// The root and projection path name existing storage or a prepared view;
    /// the operand is evaluated once before the predicate's reads.
    ByteSequencePredicate {
        predicate: language_semantics::byte_predicates::ByteSequencePredicate,
        root: CheckedStorageRoot,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    /// Exact active-case test over a structural sum subject. The path reaches
    /// the sum itself; payload paths use an explicit following Case segment.
    StructuralCaseMembership {
        subject: CheckedStructuralCaseSubject,
        case: typed_trees::data::DataMemberIdentity,
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
            Self::IntegerMathComparison { left, right, .. } => {
                left.contains_trapping_operation() || right.contains_trapping_operation()
            }
            Self::StorageRead { .. }
            | Self::Constant(_)
            | Self::Parameter { .. }
            | Self::ErasedParameter { .. }
            | Self::Local { .. }
            | Self::StructuralParameterField { .. }
            | Self::IeeeFloatComparison { .. }
            | Self::StructuralIdentity { .. }
            | Self::ByteSequenceEqual { .. }
            | Self::ByteSequencePredicate { .. }
            | Self::StructuralCaseMembership { .. } => false,
        }
    }
}

pub use semantic_vocabulary::IeeeFloatComparisonKind as CheckedIeeeFloatComparisonKind;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedStructuralParameterField {
    pub parameter_position: u32,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

/// A content observation names current descriptor storage, independently of
/// whether a parameter, local record or local view supplies that storage.
/// Entry-contract facts can project this to a parameter field; local runtime
/// observations cannot acquire entry provenance through that conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceOperand {
    pub root: CheckedStorageRoot,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

impl From<CheckedStructuralParameterField> for CheckedByteSequenceOperand {
    fn from(field: CheckedStructuralParameterField) -> Self {
        Self {
            root: CheckedStorageRoot::Parameter {
                index: field.parameter_position,
            },
            path: field.path,
        }
    }
}

impl CheckedByteSequenceOperand {
    /// Only entry-backed observations can participate in exported contracts.
    pub fn parameter_field(&self) -> Option<CheckedStructuralParameterField> {
        Some(CheckedStructuralParameterField {
            parameter_position: self.root.parameter()?,
            path: self.path.clone(),
        })
    }
}

/// Case facts name storage, not a dense scalar operand. A normal-return result
/// has its own declared place; it must never alias an entry parameter ordinal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedStructuralCaseRoot {
    Parameter { position: u32 },
    Result,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedStructuralCaseSubject {
    pub root: CheckedStructuralCaseRoot,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

impl From<CheckedStructuralParameterField> for CheckedStructuralCaseSubject {
    fn from(field: CheckedStructuralParameterField) -> Self {
        Self {
            root: CheckedStructuralCaseRoot::Parameter {
                position: field.parameter_position,
            },
            path: field.path,
        }
    }
}

impl CheckedStructuralCaseSubject {
    /// Executable entry observations cannot read a contract-only result.
    pub fn parameter_field(&self) -> Option<CheckedStructuralParameterField> {
        let CheckedStructuralCaseRoot::Parameter { position } = self.root else {
            return None;
        };
        Some(CheckedStructuralParameterField {
            parameter_position: position,
            path: self.path.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedStructuralPredicatePathSegment {
    Field(typed_trees::data::DataMemberIdentity),
    Case(typed_trees::data::DataMemberIdentity),
    FixedIndex(u64),
    /// One runtime element selection inside a member's element path —
    /// `cells[i].data[j]` names `data`'s element through the `Field` step's
    /// own array. The coordinate is the same `CheckedRuntimeIndex` a store
    /// carrier path spells; a runtime-selected element is not a fact key, so
    /// fact and roster consumers decline it.
    RuntimeIndex(CheckedRuntimeIndex),
}

impl Default for CheckedStructuralPredicatePathSegment {
    fn default() -> Self {
        // Arena zero is absence; an empty identity cannot resolve a source field.
        Self::Field(typed_trees::data::DataMemberIdentity::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

/// The checked scalar a `RuntimeIndex` segment selects with.
///
/// The segment carries no bound: the checked producer admitted the selector,
/// and Terminal re-proves `0 <= index < extent` from the facts that hold
/// before the operation (the caller's published requires for an entry
/// parameter, a dominating guard, a stored field's snapshot). Restating an
/// interval here would be a second, trusted copy of that evidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckedRuntimeIndex {
    /// Read one established mutable integer home at the selector's program
    /// point. Loan formation snapshots this value; later writes to this home
    /// cannot retarget that loan. It is not an immutable binding coordinate.
    StorageRead { symbol: SymbolHandle },
    /// The dense direct scalar parameter position of the calling state -- the
    /// coordinate `CheckedScalarExpression::Parameter` uses.
    Parameter { position: u32 },
    /// The owning assignment's evaluated `AssignmentIndex { depth }` scalar:
    /// the selector of that indexed step of its target, evaluated before the
    /// stored value.
    AssignmentIndex { depth: u32 },
    /// A current program-point read of one integer scalar field below a
    /// structural entry parameter of the calling state -- a stored-field
    /// snapshot, the same subject
    /// `CheckedScalarExpression::StructuralParameterField` names in scalar
    /// observations. Terminal lowering emits the field read and selects with
    /// its value; the leaf's scalar type resolves from the field's declared
    /// type, so it is not restated here.
    StructuralField(CheckedStructuralParameterField),
    /// An immutable integer scalar local of the calling state, at the dense
    /// position after its scalar parameters -- the coordinate
    /// `CheckedScalarExpression::Local` uses. Symbol resolution hoists a
    /// computed selector (`grid[r * 6 + c]`) into such a local; Terminal
    /// lowering selects with the local's established value.
    Local { position: u32 },
}

#[macro_export]
macro_rules! checked_boolean_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedBooleanExpression::Equal { $($fields)* }
            | $crate::CheckedBooleanExpression::And { $($fields)* }
            | $crate::CheckedBooleanExpression::Or { $($fields)* }
    };
}

impl PartialOrd for CheckedRuntimeIndex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CheckedRuntimeIndex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let tag = |index: &Self| match index {
            Self::Parameter { .. } => 0,
            Self::AssignmentIndex { .. } => 1,
            Self::StructuralField(_) => 2,
            Self::Local { .. } => 3,
            Self::StorageRead { .. } => 4,
        };
        match tag(self).cmp(&tag(other)) {
            std::cmp::Ordering::Equal => match (self, other) {
                (Self::Parameter { position: left }, Self::Parameter { position: right })
                | (Self::Local { position: left }, Self::Local { position: right }) => {
                    left.cmp(right)
                }
                (Self::AssignmentIndex { depth: left }, Self::AssignmentIndex { depth: right }) => {
                    left.cmp(right)
                }
                (Self::StructuralField(left), Self::StructuralField(right)) => left.cmp(right),
                (Self::StorageRead { symbol: left }, Self::StorageRead { symbol: right }) => {
                    (left.arena_index(), left.generation())
                        .cmp(&(right.arena_index(), right.generation()))
                }
                _ => unreachable!("equal coordinate tags retain their variant"),
            },
            order => order,
        }
    }
}
