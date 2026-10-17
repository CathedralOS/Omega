//! Executable scalar evaluation, separate from pure proposition expressions.
use super::{
    Arena, CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, Handle,
    HandleSpan, SymbolHandle,
};
use crate::CheckedUnitStructuralArgumentPlan;
use crate::checked_trees::CheckedMemberIdentity;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::PrimitiveType;

pub type CheckedScalarComputationHandle = Handle<CheckedScalarComputation>;

/// A real structural value whose scalar payload is evaluated in authored order.
/// Field and case symbols retain nominal identity; no tag occupies a scalar
/// parameter slot, and evaluation is not permission to copy an affine value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarCaseConstruction {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub case: SymbolHandle,
    pub fields: HandleSpan<CheckedScalarCaseComputationField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarCaseComputationField {
    pub symbol: SymbolHandle,
    pub value: CheckedScalarComputationHandle,
}

/// Call operands name existing storage or retained structural expression roots.
/// A membership observation may also carry a direct case construction. Neither
/// representation invents source locals or gives structural values scalar slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarComputationStructuralArgument {
    Place(CheckedUnitStructuralArgumentPlan),
    Case(CheckedScalarCaseConstruction),
}

impl Default for CheckedScalarComputationStructuralArgument {
    fn default() -> Self {
        Self::Place(CheckedUnitStructuralArgumentPlan::default())
    }
}

impl CheckedScalarComputationStructuralArgument {
    pub fn as_place(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
        match self {
            Self::Place(place) => Some(place),
            Self::Case(_) => None,
        }
    }
}

/// One admitted temporary owner retired after its consuming call returns.
/// Rows follow reverse establishment order and select the call's structural
/// operand roster, not a scalar position or a source binding ordinal.
/// The exact maximal complement may be empty; the row still names the dying
/// owner's normal continuation. Each action either discards a cleanup-free
/// subtree or invokes its checked nominal hook; their order stays intact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarComputationTemporaryCleanup {
    /// None selects the call's authority value; Some selects an ordinary
    /// structural formal. Authority never consumes a formal position.
    pub argument_ordinal: Option<u32>,
    pub actions: HandleSpan<crate::CheckedNormalCleanupAction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarComputationPlans {
    pub roots: Arena<CheckedScalarComputationRoot>,
    pub nodes: Arena<CheckedScalarComputation>,
    pub operands: Arena<CheckedScalarComputationHandle>,
    pub observation_collections: Arena<typed_trees::types::TypeReferenceHandle>,
    pub structural_arguments: Arena<CheckedScalarComputationStructuralArgument>,
    pub temporary_cleanups: Arena<CheckedScalarComputationTemporaryCleanup>,
    pub temporary_cleanup_actions: Arena<crate::CheckedNormalCleanupAction>,
    pub claim_transfers: Arena<crate::CheckedUnitClaimTransferPlan>,
    pub dispatch_arms: Arena<CheckedScalarDispatchArm>,
    pub case_fields: Arena<CheckedScalarCaseComputationField>,
}

impl CheckedScalarComputationPlans {
    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<&CheckedScalarComputationRoot> {
        let mut roots = self.roots_for_statement_role(state, statement_ordinal, role);
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }

    /// Roots matching one statement's role filter, in arena order — the
    /// shared stem behind role-specific root lookups.
    pub fn roots_for_statement_role<'plans>(
        &'plans self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> impl Iterator<Item = &'plans CheckedScalarComputationRoot> {
        self.roots.iter().map(|(_, root)| root).filter(move |root| {
            root.state == state && root.statement_ordinal == statement_ordinal && root.role == role
        })
    }

    /// Root handles of computations owned by `machine`, in arena order.
    pub fn roots_for_machine(&self, machine: SymbolHandle) -> Vec<CheckedScalarComputationHandle> {
        self.roots
            .iter()
            .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
            .collect()
    }

    /// Whether `root` points at a valid computation node authored by
    /// `authored` with `primitive_type` on `machine` — the shared admission
    /// shape for scalar root lookups.
    pub fn is_authored_root(
        &self,
        root: &CheckedScalarComputationRoot,
        machine: SymbolHandle,
        authored: typed_trees::expression::ExpressionHandle,
        primitive_type: PrimitiveType,
    ) -> bool {
        root.machine == machine
            && self.nodes.is_valid(root.root)
            && self.nodes.get(root.root).authored_root == authored
            && self.nodes.get(root.root).primitive_type == primitive_type
    }

    /// Root handles of computations owned by any machine in `machines`, in
    /// arena order.
    pub fn roots_in_machines(
        &self,
        machines: &[SymbolHandle],
    ) -> Vec<CheckedScalarComputationHandle> {
        self.roots
            .iter()
            .filter_map(|(_, root)| machines.contains(&root.machine).then_some(root.root))
            .collect()
    }

    /// Root entries owned by any machine in `machines`, in arena order.
    pub fn machine_roots<'plans>(
        &'plans self,
        machines: &'plans [SymbolHandle],
    ) -> impl Iterator<Item = &'plans CheckedScalarComputationRoot> {
        self.roots
            .iter()
            .filter_map(move |(_, root)| machines.contains(&root.machine).then_some(root))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputationRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub root: CheckedScalarComputationHandle,
}

impl Default for CheckedScalarComputationRoot {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            root: Handle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputation {
    /// Exact authored expression for a destination or call-argument root.
    /// Other intermediate nodes use the zero handle; source spans are not identity.
    pub authored_root: typed_trees::expression::ExpressionHandle,
    /// Pure value occurrence, separate from an enclosing folded destination.
    /// Generated short-circuit constants and non-value nodes use the zero handle.
    pub value_source: typed_trees::expression::ExpressionHandle,
    pub primitive_type: PrimitiveType,
    pub kind: CheckedScalarComputationKind,
}

impl Default for CheckedScalarComputation {
    fn default() -> Self {
        Self {
            authored_root: Handle::invalid(),
            value_source: Handle::invalid(),
            primitive_type: PrimitiveType::Bool,
            kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarComputationKind {
    /// Compare the contents of one evaluated byte carrier with a finite literal.
    /// The operand owns any descriptor preparation; indexed observations happen
    /// only after that preparation and the length comparison succeed.
    ByteSequenceLiteralComparison {
        operand_type: typed_trees::types::TypeReferenceHandle,
        source_expression: typed_trees::expression::ExpressionHandle,
        /// The finite literal's bytes.
        literal: std::sync::Arc<[u8]>,
        /// A declaration-owned upper extent bounds generated byte reads. The
        /// full literal length comparison is retained even for wider literals.
        maximum_length: Option<u64>,
        /// Whether the authored comparison is `!=`.
        negated: bool,
        subject: CheckedUnitStructuralArgumentPlan,
    },
    /// Evaluate one declared byte predicate over an evaluated byte carrier at
    /// its authored evaluation point. The operand owns any descriptor
    /// preparation; the predicate's reads happen only after that preparation.
    ByteSequencePredicate {
        predicate: language_semantics::byte_predicates::ByteSequencePredicate,
        operand_type: typed_trees::types::TypeReferenceHandle,
        source_expression: typed_trees::expression::ExpressionHandle,
        subject: CheckedUnitStructuralArgumentPlan,
    },
    /// Read one scalar field, or one literal element of a scalar array, at its
    /// authored evaluation point without moving the containing owner or
    /// assigning it a scalar parameter position.
    StructuralField {
        source_expression: typed_trees::expression::ExpressionHandle,
        /// Exact receiver occurrence before place normalization. Zero for a
        /// synthesized formal coordinate; lowering never reopens the member AST
        /// to associate this observation with an already completed operand.
        receiver_expression: typed_trees::expression::ExpressionHandle,
        /// Root storage carrier after borrowed ingress has been resolved.
        source_type: typed_trees::types::TypeReferenceHandle,
        /// The checked path retains stored-reference crossings and ends at the
        /// dereferenced declaration owning the final field, or at the scalar
        /// element itself when `field` is zero.
        subject: CheckedUnitStructuralArgumentPlan,
        /// Evaluated structural dependency. Zero observes existing storage;
        /// otherwise the complete value is established once before the read
        /// and its temporary is retired only after the observation completes.
        receiver_value: super::structural_values::CheckedStructuralValueHandle,
        /// The final scalar field. Zero when the subject path's last literal
        /// `FixedIndex` already selects the scalar element.
        field: SymbolHandle,
        /// Exact final field identity, appended to the checked subject path;
        /// the default identity when `field` is zero.
        field_identity: CheckedMemberIdentity,
    },
    /// Observe an established structural operand without consuming its owner.
    CaseMembership {
        source_expression: typed_trees::expression::ExpressionHandle,
        /// Complete evaluated enum owner, established once before tag observation.
        /// Invalid for an existing place or a direct case constructor.
        receiver_value: super::structural_values::CheckedStructuralValueHandle,
        subject: CheckedScalarComputationStructuralArgument,
        case: SymbolHandle,
    },
    SelectedComparison {
        operator_use: Handle<crate::CheckedOperatorUseFact>,
        left: CheckedScalarComputationHandle,
        right: CheckedScalarComputationHandle,
    },
    /// Representation-identical semantic qualification transfer at one authored
    /// cast, including explicit non-owning erasure to a bare result.
    /// The full result reference retains domain instances; this node does not
    /// establish membership or authorize erasing the qualification at publication.
    Qualification {
        source_expression: typed_trees::expression::ExpressionHandle,
        operand: CheckedScalarComputationHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
    },
    /// A checked scalar constant whose value belongs to target closure: a
    /// retained `size_of`/`align_of` query, or a `const` whose materialized
    /// initializer is one. The carrier is already known; the selected
    /// target's fold lands the `u64` literal at `expression` after checking
    /// (`CheckedTrees::landed_integer_constant`), and lowering emits that
    /// literal, never a callee or a source spelling.
    RetainedConstant {
        expression: typed_trees::expression::ExpressionHandle,
    },
    /// Pure source expression in the enclosing state's scalar namespace.
    Value(CheckedScalarExpression),
    /// Authored conversion at one cast occurrence that cannot travel the pure
    /// lane: non-vacuous detags, policy-carrying attaches, and narrowing
    /// destinations all require source custody. The node keeps the cast's
    /// source occurrence and complete result type; checked qualification facts
    /// and boundary authorizations rejoin the exact use at publication.
    AuthoredCast {
        source_expression: typed_trees::expression::ExpressionHandle,
        operand: CheckedScalarComputationHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
        /// Source checking proved this exact occurrence's partial integer
        /// conversion, at these source and destination carriers. Completing
        /// its operand through private blocks does not withdraw that decision.
        formation_decided: bool,
    },
    /// Authored Boolean-to-integer conversion at one cast occurrence. Every
    /// arithmetic domain produces the same payload: `false` becomes 0 and
    /// `true` becomes 1 in the destination carrier, so this node keeps the
    /// cast's source occurrence without carrying a policy.
    BooleanToInteger {
        source_expression: typed_trees::expression::ExpressionHandle,
        operand: CheckedScalarComputationHandle,
    },
    /// Save the subject once, then test arms in order and evaluate one result.
    Dispatch {
        source_expression: typed_trees::expression::ExpressionHandle,
        subject: CheckedScalarComputationHandle,
        arms: HandleSpan<CheckedScalarDispatchArm>,
    },
    /// One scalar invocation. Every callee kind owns the same ordered mixed
    /// operands and normal temporary retirement; the callee names which
    /// executable runs and, for a boundary, retains its separate contract and
    /// provider identity.
    Call {
        /// The invocation's flow row; its `static_requirement_dispatch`
        /// names a checked ordinary static selection. Named proof outputs
        /// and portable boundary calls retain their separate execution and
        /// evidence routes.
        source_call: Handle<crate::FlowCallFact>,
        callee: CheckedScalarCallee,
        /// Authored occurrence identity, not its position in execution order.
        call_ordinal: u32,
        /// Dense scalar operands. The exact callee signature interleaves these
        /// with structural arguments in authored formal-position order.
        arguments: HandleSpan<CheckedScalarComputationHandle>,
        structural_arguments: HandleSpan<CheckedScalarComputationStructuralArgument>,
        /// Checked owner retirement on this invocation's normal continuation.
        /// Borrowed access alone does not authorize an owned temporary's drop.
        temporary_cleanups: HandleSpan<CheckedScalarComputationTemporaryCleanup>,
        /// Every live linear claim the call moves out of the caller, joined to
        /// the owned structural operand whose owner holds it, as an ordinary
        /// call operation's transfers are. Empty when no claim crosses.
        claim_transfers: HandleSpan<crate::CheckedUnitClaimTransferPlan>,
    },
    Select {
        /// Exact conditional occurrence, retained when an enclosing constant
        /// condition folds away and the destination root names that outer form.
        source_expression: typed_trees::expression::ExpressionHandle,
        condition: CheckedScalarComputationHandle,
        when_true: CheckedScalarComputationHandle,
        when_false: CheckedScalarComputationHandle,
    },
    /// Observe storage in source order: capture each collection before its
    /// selector, then continue the projection from that captured referent.
    /// Pure templates describe geometry; operands retain execution. Collection
    /// carriers are exact declaration views, parallel to selector operands.
    StorageObservation {
        root_type: typed_trees::types::TypeReferenceHandle,
        source_expression: typed_trees::expression::ExpressionHandle,
        expression: CheckedScalarExpression,
        operands: HandleSpan<CheckedScalarComputationHandle>,
        collections: HandleSpan<typed_trees::types::TypeReferenceHandle>,
    },
    /// Evaluate operands left-to-right once, then apply a pure template whose
    /// Parameter positions name only these computed operands, not source locals.
    Apply {
        /// Exact operation occurrence, retained when an enclosing selection folds.
        source_expression: typed_trees::expression::ExpressionHandle,
        expression: CheckedScalarExpression,
        operands: HandleSpan<CheckedScalarComputationHandle>,
    },
}

/// Which executable one scalar invocation enters. Operand order, custody and
/// temporary retirement are independent of the callee kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarCallee {
    /// A checked body in this program.
    Body {
        machine: SymbolHandle,
        state: SymbolHandle,
    },
    /// A bodyless boundary declaration, legacy boundary operator, or external
    /// leaf's satisfied requirement: the declaration owns the contract and an
    /// installed provider supplies the body. An operator or requirement names
    /// one symbol for both coordinates.
    Boundary {
        machine: SymbolHandle,
        state: SymbolHandle,
        /// Evaluated call authority, separate from the requirement's formal
        /// arguments. Its value completes before every ordinary operand.
        authority_value: super::structural_values::CheckedStructuralValueHandle,
    },
    /// A requirement invoked through a runtime implementation selector
    /// (`Trait::member(selector, data...)`). `selector` is evaluated before
    /// the data operands and is never one of the call's `arguments`.
    Dynamic {
        selector: CheckedScalarComputationHandle,
        /// The instantiated slot the call targets: the requirement under the
        /// selector's trait application, with the selector as its first
        /// operand.
        slot: SymbolHandle,
        /// The exact requirement the slot instantiates.
        requirement: SymbolHandle,
        /// Normalized identity of the selector's `dyn Trait<Args>` type.
        interface_identity: TypeIdentityId,
    },
}

impl CheckedScalarCallee {
    pub fn authority_value(&self) -> super::structural_values::CheckedStructuralValueHandle {
        match self {
            Self::Boundary {
                authority_value, ..
            } => *authority_value,
            Self::Body { .. } | Self::Dynamic { .. } => Default::default(),
        }
    }

    /// The statically named machine and state of a body or boundary callee.
    /// A dynamic callee selects its executable at runtime and names none.
    pub fn static_target(&self) -> Option<(SymbolHandle, SymbolHandle)> {
        match self {
            Self::Body { machine, state } | Self::Boundary { machine, state, .. } => {
                Some((*machine, *state))
            }
            Self::Dynamic { .. } => None,
        }
    }

    /// The signature the authored call resolves to: a static callee's state,
    /// or the slot a dynamic call targets. Validation compares it with the
    /// retained call fact's resolved target.
    pub fn called_signature(&self) -> SymbolHandle {
        match self {
            Self::Body { state, .. } | Self::Boundary { state, .. } => *state,
            Self::Dynamic { slot, .. } => *slot,
        }
    }

    /// The runtime implementation selector, evaluated before every data operand.
    pub fn selector(&self) -> Option<CheckedScalarComputationHandle> {
        match self {
            Self::Dynamic { selector, .. } => Some(*selector),
            Self::Body { .. } | Self::Boundary { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarDispatchArm {
    /// Zero for builtin comparisons; otherwise the exact implicit arm use.
    pub equality_use: Handle<crate::CheckedOperatorUseFact>,
    pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
    pub pattern: CheckedScalarDispatchPattern,
    pub value: CheckedScalarComputationHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CheckedScalarDispatchPattern {
    Value(CheckedScalarComputationHandle),
    #[default]
    Wildcard,
}

#[macro_export]
macro_rules! cast_or_qualification_triple_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedScalarComputationKind::AuthoredCast { $($fields)* }
            | $crate::CheckedScalarComputationKind::BooleanToInteger { $($fields)* }
            | $crate::CheckedScalarComputationKind::Qualification { $($fields)* }
    };
}
