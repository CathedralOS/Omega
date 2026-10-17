//! Unit effect plans: the effect machines and every effect operation a
//! Unit-effect body performs.

use crate::checked_trees::flow::terminal::structural_return::CheckedUnitStructuralReturnPlan;
use crate::checked_trees::flow::terminal::{
    CheckedBoundaryMachinePlan, CheckedCallScalarArgument, CheckedComposedUnitControlMachinePlan,
    CheckedProviderAttachmentRequirementPlan, CheckedScalarStateTerminator,
    CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, CheckedUnitStructuralTypePlan,
};
use crate::checked_trees::values::CheckedErasedProofParameterPlan;
use diagnostics::{CoverageForm, Diagnostic};
use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolTable};
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::PrimitiveType;

/// Cleanup follows an evaluated call, retires an assignment's old value after
/// its operands complete and before its store, or precedes an authored statement
/// whose entry ends captured loans. Replacement is not a fabricated call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitCleanupBoundary {
    Call(CheckedUnitCallCoordinate),
    Replacement {
        statement_index: u32,
        /// The original owner supplies death selection; the displaced home
        /// supplies executable action coordinates. Neither is reconstructed
        /// from neighboring move/store operations.
        destination: CheckedUnitStructuralArgumentPlan,
        old_value: CheckedUnitStructuralArgumentPlan,
        /// Path below the assignment's captured destination. Whole-array
        /// replacement retains each leaf's relative indices here, so an
        /// interpreter never re-evaluates the authored destination selector.
        receiver_path: Vec<CheckedUnitStructuralPathSegment>,
    },
    StatementEntry {
        statement_index: u32,
    },
}

/// Source-handle-free plans for the first general structural/Unit terminal
/// slice. These rows are assembled only after ownership and carry checking
/// have recorded their authoritative facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedUnitEffectPlans {
    /// One condition/path namespace for statement, computed-call and normal
    /// exit cleanup. Action spans never refer to a temporary collector's arena.
    pub cleanup_control: crate::CheckedCleanupControlPlans,
    pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
    pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
    /// The call shape of every instantiated selector-call slot.
    pub dynamic_slots: Vec<crate::CheckedDynamicSlotPlan>,
    pub machines: Vec<CheckedUnitEffectMachinePlan>,
    /// Multi-state Unit machines whose exact control and effect rows were
    /// admitted as one atomic executable plan.
    pub composed_machines: Vec<CheckedComposedUnitControlMachinePlan>,
    /// One row per checked-body machine without an executable Unit plan: the
    /// construct plan construction could not lower, at its authored site, or
    /// the call whose callee has no body. A gap is reported only where
    /// lowering or entry establishment demands that body
    /// ([`Self::gap_diagnostic`]); it grants nothing and selects no fallback.
    pub gaps: Vec<CheckedUnitPlanGap>,
}

/// Why one checked-body machine has no executable Unit plan. `machine` never
/// appears in `machines` or `composed_machines` of the same record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitPlanGap {
    pub machine: SymbolHandle,
    /// Exact construction site; the zero state handle means the machine header.
    pub state: SymbolHandle,
    /// Zero-based authored statement position, when construction reached one.
    pub statement_index: Option<u32>,
    pub reason: CheckedUnitPlanGapReason,
    /// The authored construct or call the reason names. A compiler-generated
    /// site keeps its enclosing state's or machine's declaration span.
    pub source_span: SourceSpan,
    /// The coverage provision a demand for this body exceeds.
    pub coverage: CoverageForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedUnitPlanGapReason {
    /// Plan construction has no lowering for this construct yet; the text
    /// names the construct, outermost first.
    Unsupported(&'static str),
    /// The body calls `target`, which has no executable body of its own.
    Callee(SymbolHandle),
    /// The body calls boundary `target`, which has no boundary plan.
    BoundaryTarget(SymbolHandle),
    /// The body uses opaque boundary data `opaque` by value, and the build
    /// selects no `OpaqueRepresentation` application for it. A representation
    /// is inert until selected (build/opaque_representations.md): only the
    /// selected carrier supplies the storage a by-value use needs, so this is
    /// the program's missing selection, not a construct the compiler lacks.
    UnselectedOpaqueRepresentation(SymbolHandle),
}

impl CheckedUnitEffectPlans {
    /// Adopt shapes another Unit-family builder collected into this roster,
    /// the one structural type namespace every Unit body lowers against.
    /// Identities are deterministic, so a shape already present must be the
    /// same shape; the conflicting identity is returned otherwise, as is an
    /// empty identity, and the roster keeps every shape adopted before it.
    pub fn adopt_structural_types(
        &mut self,
        types: impl IntoIterator<Item = CheckedUnitStructuralTypePlan>,
    ) -> Result<(), TypeIdentityId> {
        for shape in types {
            if !shape.identity.is_valid() {
                return Err(shape.identity);
            }
            match crate::checked_structural_type_by_identity(&self.structural_types, shape.identity)
            {
                Some(existing) if *existing != shape => return Err(shape.identity),
                Some(_) => {}
                None => self.structural_types.push(shape),
            }
        }
        Ok(())
    }

    /// One published callable entry, independent of operation or graph body.
    /// Competing bodies do not select a fallback signature.
    pub fn callable_entry_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<CheckedUnitCallableEntry<'_>> {
        let mut entries = self
            .machines
            .iter()
            .filter(|plan| plan.machine == machine)
            .map(CheckedUnitEffectMachinePlan::callable_entry)
            .chain(
                self.composed_machines
                    .iter()
                    .filter(|plan| plan.machine == machine)
                    .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
            );
        let entry = entries.next()?;
        entries.next().is_none().then_some(entry)
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitEffectMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn composed_for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedComposedUnitControlMachinePlan> {
        self.composed_machines
            .iter()
            .find(|plan| plan.machine == machine)
    }

    /// The gap row for a machine that has no plan in this record.
    pub fn gap_for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitPlanGap> {
        self.gaps.iter().find(|row| row.machine == machine)
    }

    /// The refusal for demanding `machine`'s body when it has none. Callee
    /// rows are followed to the body whose own construct is unsupported; the
    /// diagnostic sits at that construct and names the call chain from
    /// `machine`. A chain that returns to a machine it already passed stops
    /// at the cycle. `None` when the record names no gap for `machine`.
    pub fn gap_diagnostic(
        &self,
        machine: SymbolHandle,
        symbols: &SymbolTable,
    ) -> Option<Diagnostic> {
        let name = |symbol: SymbolHandle| symbols.display_path(symbol, "::");
        let mut gap = self.gap_for_machine(machine)?;
        let mut chain = vec![machine];
        while let CheckedUnitPlanGapReason::Callee(target) = gap.reason {
            let Some(next) = self.gap_for_machine(target) else {
                break;
            };
            if chain.contains(&target) {
                break;
            }
            chain.push(target);
            gap = next;
        }
        let construct = match gap.coverage {
            CoverageForm::MachineHeader => "its declaration",
            CoverageForm::EntrySignature => "its signature",
            CoverageForm::StateSignature => "a state",
            CoverageForm::StatementForm => "a statement",
            CoverageForm::CallForm => "a call",
            CoverageForm::ExpressionForm => "an expression",
            CoverageForm::TransitionForm => "a transition",
        };
        let reason = match gap.reason {
            CheckedUnitPlanGapReason::Unsupported(detail) => format!(
                "`{}` has no executable body: this compiler does not lower {construct} of it yet ({detail})",
                name(gap.machine)
            ),
            // The chain ended at a callee without a gap row of its own: it
            // has a body, but none this call can use (a scalar body this
            // call shape cannot enter, say).
            CheckedUnitPlanGapReason::Callee(target) => format!(
                "`{}` has no executable body: its call of `{}` reaches no executable callee plan",
                name(gap.machine),
                name(target)
            ),
            CheckedUnitPlanGapReason::BoundaryTarget(target) => format!(
                "`{}` has no executable body: it calls boundary `{}`, which has no boundary plan",
                name(gap.machine),
                name(target)
            ),
            // The program's omission, worded as a rejection: the corpus and
            // sample gates read "has no executable body" as a compiler gap.
            CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation(opaque) => format!(
                "`{}` uses opaque boundary data `{opaque}` by value, but the build selects no `OpaqueRepresentation<{opaque}, _>` application; add `Build::select_representation<{opaque}, <conformance>>(builder)`",
                name(gap.machine),
                opaque = name(opaque)
            ),
        };
        let reason = if gap.state.is_valid() {
            match gap.statement_index {
                Some(statement_index) => format!(
                    "{reason}; in state `{}`, statement {statement_index}",
                    name(gap.state)
                ),
                None => format!("{reason}; in state `{}`", name(gap.state)),
            }
        } else {
            reason
        };
        let message = if chain.len() == 1 {
            reason
        } else {
            format!(
                "`{}` calls {}; {reason}",
                name(machine),
                chain[1..]
                    .iter()
                    .map(|symbol| format!("`{}`", name(*symbol)))
                    .collect::<Vec<_>>()
                    .join(", which calls ")
            )
        };
        // A missing representation selection is a verdict on the program, so
        // it is an ordinary error; every other gap names an exceeded coverage
        // provision and leaves the compilation incomplete, not rejected.
        let diagnostic = match gap.reason {
            CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation(_) => {
                Diagnostic::error(message)
            }
            _ => Diagnostic::incomplete(
                message,
                gap.coverage.exceeded(gap.source_span.span.start as u64),
            ),
        };
        Some(diagnostic.with_source_span(gap.source_span))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitEffectOperationPlan {
    /// Evaluate the assignment's destination before any replacement producer.
    /// Cleanup and stores retain this occurrence, including selector values;
    /// they do not read a selector again after the RHS changes its storage.
    CaptureAssignmentDestination {
        statement_index: u32,
        /// Exact target occurrence. Displacement names this occurrence;
        /// an independently evaluated RHS value never does, even at the same path.
        destination_expression: typed_trees::expression::ExpressionHandle,
        destination: CheckedUnitStructuralArgumentPlan,
    },
    EstablishReference {
        /// Exact checked formation occurrence. The emitter retains its
        /// completed address; later crossings never replay this expression.
        formation_expression: typed_trees::expression::ExpressionHandle,
        backing_type: typed_trees::types::TypeReferenceHandle,
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
        storage: super::CheckedReferenceStorage,
    },
    ReleaseReference {
        statement_index: u32,
        /// Current descriptor storage, not necessarily its original producer.
        /// A replacement keeps the receiving local while changing its value.
        source: CheckedUnitStructuralArgumentSourcePlan,
        /// Exact stored descriptor; an empty path releases the result itself.
        path: Vec<CheckedUnitStructuralPathSegment>,
        /// Enclosing cases not established at this death boundary. The normal
        /// cleanup controller selects them before touching their payload.
        case_conditions: arena::HandleSpan<super::CheckedCleanupCaseCondition>,
        /// A candidate death for computed backing whose producer can belong to
        /// an unevaluated alternative. Select its actual storage occurrence in
        /// the existing ownership continuation graph before Terminal publication.
        /// Ordinary carrier retirement remains unconditional and exact.
        select_evaluated_storage: bool,
        /// A transition observes its operands before its selected edge closes
        /// their custody. Lowering consumes this checked timing directly.
        defer_to_exit: bool,
        /// All alternative sources end together; the descriptor retires once.
        loans: arena::HandleSpan<crate::BorrowLoanFact>,
        /// Anonymous selected complements close with the complete receiving
        /// leaf cohort, independently of their scalar/root exit disposition.
        selection_complements: Vec<CheckedUnitSelectedReferenceRetirementPlan>,
    },
    /// One local data binding: its declared home or result coordinate and
    /// its initializer payload, keyed by the bound value's representation.
    /// What used to be a family of per-shape variants is one operation; the
    /// payload's representation selects the lowering.
    EstablishLocal(CheckedUnitLocalBindingPlan),
    /// Bind the statement's immutable view local to an exclusive range of an
    /// established view. `source` is the same `ByteSequenceSubslice` or
    /// `ElementViewSubslice` argument plan a call argument carries, with its
    /// endpoints keyed at the `LocalBinding` subslice site. The result is a
    /// shared, unrestricted view in the structural result namespace: it owns
    /// no storage, so its end is the end of the loan, never a disposal.
    EstablishViewSubslice {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
    },
    /// Retire affine owners at their authenticated semantic boundary.
    /// Calls and statement entry share edge disposal, while retaining the
    /// distinct source evidence needed to authenticate their lifetimes.
    ContinuationCleanup {
        boundary: CheckedUnitCleanupBoundary,
        /// Exact candidate regions whose old values exist only on retained paths.
        selected_regions: Vec<CheckedUnitStructuralArgumentPlan>,
        cleanup_actions: Vec<crate::CheckedNormalCleanupAction>,
    },
    /// One ordinary or boundary call with a Unit, scalar or structural
    /// result. Selected operators keep their own operand lanes.
    Call(CheckedUnitCallPlan),
    /// One catalogued register event. Operands retain the ordinary evaluated
    /// scalar call lanes; a read binds its scalar result at `coordinate`.
    RegisterAccess {
        coordinate: CheckedUnitCallCoordinate,
        operation: language_core::inline_assembly::AsmRegisterOperation,
        result: Option<CheckedUnitScalarResultBindingPlan>,
        arguments: Vec<CheckedCallScalarArgument>,
        service_reach: ServiceReachSummary,
    },
    /// One store `place = value` over its exact destination place and value:
    /// an authored scalar, byte or sum-field assignment, the repair of an
    /// open borrowed window, or an owned local's rebind. The destination's
    /// leaf representation decides the Terminal operation (see
    /// `store_plans.rs`).
    Store(crate::CheckedStorePlan),
    /// One normalized atomic event on an exact primitive leaf, joining the
    /// source carrier's placeholder local and carrier assignment into one
    /// operation (see `atomic_plans.rs`).
    AtomicAccess(crate::CheckedAtomicAccessPlan),
    /// A proof-static ordering event, after ordinary operand evaluation. It
    /// accesses no resident and produces neither a value nor an observation.
    AtomicFence {
        statement_index: u32,
        declaration: SymbolHandle,
        ordering: language_core::atomic::FenceOrdering,
        /// Evaluated source argument, retained for producer custody even
        /// though the emitted event has no runtime value operand.
        argument: CheckedUnitStructuralArgumentPlan,
    },
    /// Move one whole structural field out of exclusive borrowed storage into
    /// a fresh owned binding, opening a repair window the body must close.
    /// The source retains its exact storage path; the result binds a named
    /// local or the completed value of an anonymous call operand.
    MoveStructuralField {
        result: CheckedUnitStructuralResultBindingPlan,
        source: CheckedUnitStructuralArgumentPlan,
        /// Evaluated value occurrence, or the exact assignment-target occurrence
        /// for internal displacement. The latter uses the destination's earlier
        /// captured address; a real RHS extraction evaluates its own source.
        expression: typed_trees::expression::ExpressionHandle,
    },
    /// Finish the body after cleanup, yielding its retained structural/scalar
    /// binding or scalar control result when present, and Unit otherwise.
    Complete {
        statement_index: u32,
        /// Exact local declaration coordinates cleaned before parameters, in
        /// reverse declaration order.
        trivial_affine_local_discard_ordinals: Vec<u32>,
        /// Owned affine structural parameters discarded on this return edge,
        /// in reverse declaration order.
        trivial_affine_discards: Vec<u32>,
    },
}

/// The payload of [`CheckedUnitEffectOperationPlan::EstablishLocal`]: one
/// local data binding keyed by the bound value's representation. The variant
/// selects the lowering; every field keeps the name and meaning it carried
/// on the former per-shape operation variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitLocalBindingPlan {
    /// Establish initialized mutable storage at its authored declaration.
    /// Later reads and borrows name the symbol, never the initializer value.
    /// The initializer is the `StorageInitializer` value checking retained:
    /// a pure expression, or the computation rooted there when it evaluates
    /// calls or ordered selections (`let mut stored: i32 = echo(a);`).
    Primitive {
        statement_index: u32,
        symbol: SymbolHandle,
        type_identity: TypeIdentityId,
        primitive_type: PrimitiveType,
        value: CheckedCallScalarArgument,
    },
    /// Establish one whole, claim-free affine local's declared storage home.
    TrivialAffine {
        statement_index: u32,
        declaration_ordinal: u32,
        type_identity: TypeIdentityId,
    },
    /// Establish one declared structural local's home at its authored
    /// declaration, publishing the binding that member operations and
    /// borrows rejoin by ordinal. The declaration supplies the home each
    /// member operation establishes into; which members reads may rely on
    /// is the custody ledger's question, not this root's. A shared byte-view
    /// local's home declares its authored byte-domain qualifications: the
    /// invariant every descriptor store into it proves.
    Structural {
        statement_index: u32,
        declaration_ordinal: u32,
        symbol: SymbolHandle,
        result: CheckedUnitStructuralResultBindingPlan,
        qualifications: Vec<SemanticDomainId>,
        discard_result_on_return: bool,
    },
    /// Establish one immutable primitive local from a retained pure expression
    /// or selective computation. The result coordinate names the
    /// same dense scalar namespace used by result-bearing calls; later
    /// consumers may therefore refer to either kind without reconstructing a
    /// source expression.
    Scalar {
        result: CheckedUnitScalarResultBindingPlan,
        value: CheckedCallScalarArgument,
    },
    /// Establish the byte value declared by one local initializer.
    /// The binding retains its declared borrowed or owned carrier. Only the
    /// authored byte-domain qualifications constrain later writes; incidental
    /// predicates of the initial octets do not become declared invariants.
    ByteSequence {
        result: CheckedUnitStructuralResultBindingPlan,
        value: crate::CheckedByteSequenceValuePlan,
        qualifications: Vec<SemanticDomainId>,
    },
    /// Construct an unrestricted primitive fixed array in authored row-major
    /// leaf order. Empty dimensions remain in the exact structural result type.
    ScalarArray {
        qualifications: Vec<SemanticDomainId>,
        source: crate::CheckedArrayConstructionSource,
        result: CheckedUnitStructuralResultBindingPlan,
        elements: Vec<CheckedCallScalarArgument>,
    },
    /// Establish a fresh structural expression in the shared result namespace.
    /// Selected arms transfer one new owner into the expression continuation.
    StructuralValue {
        result: CheckedUnitStructuralResultBindingPlan,
        value: crate::CheckedStructuralValueHandle,
        calls: Vec<CheckedStructuralValueCall>,
        /// `Some` when the value is an inline operand of a call statement,
        /// naming the consuming call coordinate and authored formal position;
        /// absent for statement-level constructions such as local bindings.
        operand_source: Option<crate::CheckedArrayConstructionSource>,
        discard_result_on_return: bool,
    },
}

impl CheckedUnitLocalBindingPlan {
    /// The binding this operation adds to the shared structural namespace;
    /// `None` for the scalar and storage-only representations.
    pub fn structural_result_binding(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
        match self {
            Self::Structural { result, .. }
            | Self::ByteSequence { result, .. }
            | Self::ScalarArray { result, .. }
            | Self::StructuralValue { result, .. } => Some(result),
            Self::Primitive { .. } | Self::TrivialAffine { .. } | Self::Scalar { .. } => None,
        }
    }

    /// The authored statement this binding belongs to in statement order.
    pub fn source_statement_index(&self) -> Option<u32> {
        match self {
            Self::Primitive {
                statement_index, ..
            }
            | Self::TrivialAffine {
                statement_index, ..
            }
            | Self::Structural {
                statement_index, ..
            } => Some(*statement_index),
            Self::Scalar { result, .. } => Some(result.statement_index),
            Self::ByteSequence { result, .. }
            | Self::ScalarArray { result, .. }
            | Self::StructuralValue { result, .. } => Some(result.statement_index),
        }
    }

    /// The structural result whose producer owns an automatic return discard,
    /// with that flag. The other representations answer `None`.
    pub fn discardable_structural_result(
        &self,
    ) -> Option<(&CheckedUnitStructuralResultBindingPlan, bool)> {
        match self {
            Self::Structural {
                result,
                discard_result_on_return,
                ..
            }
            | Self::StructuralValue {
                result,
                discard_result_on_return,
                ..
            } => Some((result, *discard_result_on_return)),
            _ => None,
        }
    }
}

/// One exact selected leaf and its anonymous complement slots. The receipt
/// names the control join; the loan group authenticates the leaf lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitSelectedReferenceRetirementPlan {
    pub selection: arena::Handle<crate::FlowOwnedSelectionReceipt>,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub loans: arena::HandleSpan<crate::BorrowLoanFact>,
}

/// Operand preparation belongs to its consuming call and runs when that call
/// is evaluated. Operation slots use an arena span; scratch vectors are consumed
/// at publication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedOperandSequence {
    operations: arena::Arena<OperandOperation>,
    order: arena::HandleSpan<OperandOperation>,
}

// The arena's zero slot denotes absence. Published spans contain only occupied
// operations, and the private fields keep that construction invariant local.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct OperandOperation(Option<crate::CheckedUnitEffectOperationPlan>);

impl CheckedOperandSequence {
    pub fn new(
        operations: impl IntoIterator<Item = crate::CheckedUnitEffectOperationPlan>,
    ) -> Self {
        let mut sequence = Self::default();
        for operation in operations {
            sequence
                .operations
                .append_to_span(&mut sequence.order, OperandOperation(Some(operation)));
        }
        sequence
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &crate::CheckedUnitEffectOperationPlan> {
        self.operations
            .span_or_empty(self.order)
            .iter()
            .map(|operation| {
                operation
                    .0
                    .as_ref()
                    .expect("published operand span contains operations")
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralValueCall {
    pub value: crate::CheckedStructuralValueHandle,
    operands: CheckedOperandSequence,
    operation: CheckedUnitEffectOperationPlan,
    /// Normal-return retirement of anonymous descriptors this call did not retain.
    completions: CheckedOperandSequence,
}

// Only the arena's absent slot uses this value. Published rows still enter
// through `with_operands`, which requires a structural invocation result.
impl Default for CheckedStructuralValueCall {
    fn default() -> Self {
        Self {
            value: crate::CheckedStructuralValueHandle::invalid(),
            operands: CheckedOperandSequence::default(),
            completions: CheckedOperandSequence::default(),
            operation: CheckedUnitEffectOperationPlan::Complete {
                statement_index: 0,
                trivial_affine_local_discard_ordinals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }
    }
}

impl CheckedStructuralValueCall {
    pub fn new(
        value: crate::CheckedStructuralValueHandle,
        operation: CheckedUnitEffectOperationPlan,
    ) -> Option<Self> {
        Self::with_operands(
            value,
            CheckedOperandSequence::default(),
            operation,
            CheckedOperandSequence::default(),
        )
    }

    pub fn with_operands(
        value: crate::CheckedStructuralValueHandle,
        operands: CheckedOperandSequence,
        operation: CheckedUnitEffectOperationPlan,
        completions: CheckedOperandSequence,
    ) -> Option<Self> {
        // The call's admitted result owns its value, independently of where
        // executable supply comes from or which expression consumes it. The
        // operation retains that route and its exact completion custody.
        operation
            .call()
            .is_some_and(|call| call.structural_result().is_some())
            .then_some(Self {
                value,
                operands,
                operation,
                completions,
            })
    }

    pub fn operands(&self) -> &CheckedOperandSequence {
        &self.operands
    }

    pub fn operation(&self) -> &CheckedUnitEffectOperationPlan {
        &self.operation
    }

    pub fn completions(&self) -> &CheckedOperandSequence {
        &self.completions
    }

    /// The value's result binding. `with_operands` admits only a call that
    /// produces one, so every published row answers.
    pub fn structural_result(&self) -> &CheckedUnitStructuralResultBindingPlan {
        self.operation
            .call()
            .and_then(CheckedUnitCallPlan::structural_result)
            .expect("a published structural value call produces a structural result")
    }
}

impl CheckedUnitEffectOperationPlan {
    /// The exact source of an established reference carrier. Direct completion
    /// and structural value construction name the same semantic establishment;
    /// its expression position does not choose another custody interface.
    pub fn established_reference_source<'plans>(
        &'plans self,
        values: &'plans crate::CheckedStructuralValuePlans,
    ) -> Option<&'plans CheckedUnitStructuralArgumentPlan> {
        match self {
            Self::EstablishReference { source, .. } => Some(source),
            Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                value, ..
            }) if values.nodes.is_valid(*value) => match &values.nodes.get(*value).kind {
                crate::CheckedStructuralValueKind::Reference { source, .. } => Some(source),
                _ => None,
            },
            _ => None,
        }
    }

    /// The binding this operation adds to the shared structural namespace.
    /// Literal declarations and constructed/called values use the same dense
    /// ordinal space; their distinct storage and ownership remain on the variant.
    pub fn structural_result_binding(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
        match self {
            Self::EstablishReference { result, .. }
            | Self::EstablishViewSubslice { result, .. }
            | Self::MoveStructuralField { result, .. } => Some(result),
            Self::EstablishLocal(binding) => binding.structural_result_binding(),
            Self::Call(call) => call.structural_result(),
            _ => None,
        }
    }

    /// The statement's structural value, excluding intermediate call operands.
    /// An operand and the local receiving its enclosing call share a statement
    /// index. Only the latter can identify that declaration's cleanup owner.
    /// Source correspondence still authenticates the retained occurrence.
    pub fn statement_structural_result_binding(
        &self,
    ) -> Option<&CheckedUnitStructuralResultBindingPlan> {
        match self {
            Self::EstablishLocal(
                CheckedUnitLocalBindingPlan::StructuralValue {
                    operand_source: Some(_),
                    ..
                }
                | CheckedUnitLocalBindingPlan::ScalarArray {
                    source: crate::CheckedArrayConstructionSource::CallArgument { .. },
                    ..
                },
            ) => None,
            Self::Call(call) if call.coordinate.call_ordinal != 0 => None,
            _ => self.structural_result_binding(),
        }
    }

    /// The authored assignment a store writes. One assignment may own several
    /// stores (each element of an array literal). A window repair or local
    /// rebind and an atomic event keep their own statement custody and answer
    /// `None`, as does every operation other than a store.
    pub fn store_statement_index(&self) -> Option<u32> {
        match self {
            Self::Store(store) if store.installed_value().is_none() => Some(store.statement_index),
            _ => None,
        }
    }

    /// One store of `value` into the `leaf` that `path` names below `root`.
    pub fn new_store(
        statement_index: u32,
        root: crate::CheckedStoreRoot,
        path: Vec<CheckedUnitStructuralPathSegment>,
        leaf: crate::CheckedStoreLeaf,
        value: crate::CheckedStoreValue,
    ) -> Self {
        Self::Store(crate::CheckedStorePlan {
            statement_index,
            destination: crate::CheckedStoreDestination { root, path, leaf },
            value,
        })
    }

    /// The store this operation performs; `None` for every other operation.
    pub fn store(&self) -> Option<&crate::CheckedStorePlan> {
        match self {
            Self::Store(store) => Some(store),
            _ => None,
        }
    }

    /// The whole owned binding a window repair or local rebind installs.
    pub fn installed_value(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
        self.store()
            .and_then(crate::CheckedStorePlan::installed_value)
    }

    /// The byte value a contents or descriptor store writes.
    pub fn stored_bytes(&self) -> Option<&crate::CheckedByteSequenceValuePlan> {
        self.store().and_then(|store| store.value.bytes())
    }

    /// The authored statement this operation belongs to in the plan's
    /// statement order: emission captures loans before it and places nested
    /// operand evaluation by it. A structural call result names its binding
    /// statement; every other call names its invocation statement. A
    /// continuation cleanup, a reference release and a completion run between
    /// statements and answer `None`.
    pub fn source_statement_index(&self) -> Option<u32> {
        match self {
            Self::EstablishReference { result, .. }
            | Self::EstablishViewSubslice { result, .. }
            | Self::MoveStructuralField { result, .. } => Some(result.statement_index),
            Self::EstablishLocal(binding) => binding.source_statement_index(),
            Self::Call(call) => Some(
                call.structural_result()
                    .map_or(call.coordinate.statement_index, |result| {
                        result.statement_index
                    }),
            ),
            Self::CaptureAssignmentDestination {
                statement_index, ..
            } => Some(*statement_index),
            Self::RegisterAccess { coordinate, .. } => Some(coordinate.statement_index),
            Self::AtomicAccess(access) => Some(access.statement_index),
            Self::AtomicFence {
                statement_index, ..
            } => Some(*statement_index),
            Self::Store(store) => Some(store.statement_index),
            Self::ContinuationCleanup { .. }
            | Self::ReleaseReference { .. }
            | Self::Complete { .. } => None,
        }
    }

    /// Evaluated structural call arguments, including a primitive whose
    /// proved argument is absent from its runtime event. Selected-operator
    /// arithmetic keeps its own operand lanes.
    pub fn call_structural_arguments(&self) -> &[CheckedUnitStructuralArgumentPlan] {
        match self {
            Self::Call(call) => &call.structural_arguments,
            Self::AtomicFence { argument, .. } => std::slice::from_ref(argument),
            _ => &[],
        }
    }

    /// The ordinary or boundary call this operation performs; `None` for
    /// every other operation. Selected operators keep their own operand lanes.
    pub fn call(&self) -> Option<&CheckedUnitCallPlan> {
        match self {
            Self::Call(call) => Some(call),
            _ => None,
        }
    }

    /// The authored invocation this operation performs: an ordinary or
    /// boundary call, or a checked port write. Continuations after either
    /// share one call boundary; every other operation answers `None`.
    pub fn invocation_coordinate(&self) -> Option<CheckedUnitCallCoordinate> {
        match self {
            Self::Call(call) => Some(call.coordinate),
            Self::RegisterAccess { coordinate, .. } => Some(*coordinate),
            _ => None,
        }
    }

    pub fn call_mut(&mut self) -> Option<&mut CheckedUnitCallPlan> {
        match self {
            Self::Call(call) => Some(call),
            _ => None,
        }
    }

    /// The structural result binding of a call or a constructed value, the
    /// producers that own an automatic return discard, with that flag. The
    /// other structural producers answer `None`.
    pub fn discardable_structural_result(
        &self,
    ) -> Option<(&CheckedUnitStructuralResultBindingPlan, bool)> {
        match self {
            Self::EstablishLocal(binding) => binding.discardable_structural_result(),
            Self::Call(call) => call
                .structural_result()
                .zip(call.discard_result_on_return()),
            _ => None,
        }
    }

    /// [`Self::discardable_structural_result`] with its flag writable.
    pub fn discardable_structural_result_mut(
        &mut self,
    ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
        match self {
            Self::EstablishLocal(
                CheckedUnitLocalBindingPlan::StructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitLocalBindingPlan::Structural {
                    result,
                    discard_result_on_return,
                    ..
                },
            ) => Some((result, discard_result_on_return)),
            Self::Call(call) => call.structural_result_mut(),
            _ => None,
        }
    }

    /// Enumerate retained operation dependencies for signature/catalog readers.
    /// This is not execution order: structural expressions evaluate their call
    /// operands in their own authored field and selected-arm order.
    pub fn with_value_calls(&self) -> impl Iterator<Item = &Self> {
        let mut pending = vec![self];
        std::iter::from_fn(move || {
            let operation = pending.pop()?;
            if let Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                calls,
                ..
            }) = operation
            {
                for call in calls.iter().rev() {
                    pending.push(call.operation());
                    pending.extend(call.operands().iter().rev());
                }
            }
            Some(operation)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitScalarControlPlan {
    pub primitive_type: PrimitiveType,
    /// Exact authored conditional and return coordinates after the ordered body.
    pub terminator: CheckedScalarStateTerminator,
}

/// Storage for one `mut` scalar formal. The body establishes it before its
/// first statement, initialized from the incoming value, exactly as a
/// `let mut` declaration establishes a primitive local's storage: reads are
/// `StorageRead` of `symbol`, assignments store through
/// `CheckedStoreRoot::Local { symbol }`, and borrows lend this storage, never
/// the immutable incoming value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitParameterStoragePlan {
    /// The authored parameter position; one `scalar_parameters` row names it.
    pub source_position: u32,
    pub symbol: SymbolHandle,
    pub type_identity: TypeIdentityId,
    pub primitive_type: PrimitiveType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEffectMachinePlan {
    /// Source-owned normal exits, selected individually by scalar control completion.
    pub normal_exit_cleanups: arena::HandleSpan<super::CheckedNormalExitCleanupPlan>,
    /// Final ordinary scalar binding, mutually exclusive with the other completions.
    pub scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
    /// Scalar control completion, mutually exclusive with either result binding.
    pub scalar_control: Option<CheckedUnitScalarControlPlan>,
    /// Whole structural value returned after ordinary sequencing, with its
    /// actual source and multiplicity retained by the completion plan.
    pub structural_result: Option<CheckedUnitStructuralReturnPlan>,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    /// Exact attached data carrier, or `None` for an ordinary free machine.
    pub attachment_type_identity: Option<TypeIdentityId>,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    /// Primitive parameters in authored order after removing structural
    /// parameters into their independent custody namespace.
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Storage for the `mut` scalar formals, in authored order.
    pub parameter_storage: Vec<CheckedUnitParameterStoragePlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Erased formals whose carriers are proof-only and admit no scalar
    /// lane. The contract term lane carries their semantic type identities.
    pub erased_proof_parameters: Vec<CheckedErasedProofParameterPlan>,
    /// Exact boundary requirements replacing one authored provider-backed
    /// attachment field. Empty means no attachment specialization occurred.
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    /// Dense source-order declarations for the bounded empty-record affine
    /// local prefix.
    pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Canonical sorted domains from `QualificationFacts`.
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
    /// The number of authored statements in the body, the position at which
    /// a record loan still live at body end is captured.
    pub statement_count: u32,
    /// The body's silent place loans in statement order. They plan no
    /// operation; each names the place its reads and stores rejoin.
    pub place_loans: Vec<super::CheckedPlaceLoanPlan>,
}

/// Borrowed entry vocabulary shared by ordinary sequences and state graphs.
/// This is a view of the owning plan, not another durable body or signature.
/// Cleanup and authored calls select the same entry; the complete body stays
/// in its original representation and travels through the ordinary closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitCallableEntry<'plan> {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub returns_unit: bool,
    pub attachment_type_identity: Option<TypeIdentityId>,
    pub structural_parameters: &'plan [CheckedUnitStructuralParameterPlan],
    pub scalar_parameters: &'plan [CheckedStructuralScalarParameterPlan],
    pub erased_scalar_parameters: &'plan [CheckedStructuralScalarParameterPlan],
    pub erased_proof_parameters: &'plan [CheckedErasedProofParameterPlan],
    pub entry_claims: &'plan [CheckedUnitEntryClaimPlan],
    pub contract_report_fingerprint: u64,
    pub contract_service_reach: ServiceReachPlan,
}

impl CheckedUnitEffectMachinePlan {
    pub fn callable_entry(&self) -> CheckedUnitCallableEntry<'_> {
        CheckedUnitCallableEntry {
            machine: self.machine,
            state: self.state,
            returns_unit: self.scalar_result.is_none()
                && self.scalar_control.is_none()
                && self.structural_result.is_none(),
            attachment_type_identity: self.attachment_type_identity,
            structural_parameters: &self.structural_parameters,
            scalar_parameters: &self.scalar_parameters,
            erased_scalar_parameters: &self.erased_scalar_parameters,
            erased_proof_parameters: &self.erased_proof_parameters,
            entry_claims: &self.entry_claims,
            contract_report_fingerprint: self.contract_report_fingerprint,
            contract_service_reach: self.contract_service_reach,
        }
    }
}

impl CheckedComposedUnitControlMachinePlan {
    /// The state plan keyed `state`.
    pub fn state(
        &self,
        state: SymbolHandle,
    ) -> Option<&super::CheckedComposedUnitControlStatePlan> {
        self.states
            .iter()
            .find(|candidate| candidate.state == state)
    }

    pub fn callable_entry(&self) -> Option<CheckedUnitCallableEntry<'_>> {
        let state = self.states.first()?;
        Some(CheckedUnitCallableEntry {
            machine: self.machine,
            state: state.state,
            returns_unit: matches!(self.result, super::CheckedControlResultPlan::Unit),
            attachment_type_identity: self.attachment_type_identity,
            structural_parameters: &state.structural_parameters,
            scalar_parameters: &state.scalar_parameters,
            erased_scalar_parameters: &state.erased_scalar_parameters,
            erased_proof_parameters: &state.erased_proof_parameters,
            entry_claims: &state.entry_claims,
            contract_report_fingerprint: self.contract_report_fingerprint,
            contract_service_reach: self.contract_service_reach,
        })
    }
}

/// The two byte-sequence establishment plans consumers step over together.
#[macro_export]
macro_rules! byte_sequence_establishment_pattern {
    () => {
        $crate::CheckedUnitEffectOperationPlan::EstablishLocal(
            $crate::CheckedUnitLocalBindingPlan::ByteSequence { .. },
        ) | $crate::CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
    };
}

/// Reference establishment/release and the two structural-field write plans
/// consumers step over together.
#[macro_export]
macro_rules! reference_hold_pattern {
    () => {
        $crate::CheckedUnitEffectOperationPlan::EstablishReference { .. }
            | $crate::CheckedUnitEffectOperationPlan::ReleaseReference { .. }
    };
}

#[macro_export]
macro_rules! structural_field_write_pattern {
    () => {
        $crate::CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
            | $crate::CheckedUnitEffectOperationPlan::CaptureAssignmentDestination { .. }
    };
}
