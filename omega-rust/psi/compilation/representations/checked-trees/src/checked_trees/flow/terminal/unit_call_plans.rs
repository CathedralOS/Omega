//! One ordinary, boundary or dynamic call from a Unit-effect body.
//!
//! A call has four independent axes. Its route reaches an ordinary checked
//! machine in this module, a bodyless boundary, or whichever implementation a
//! runtime selector names. Its result binds nothing, a primitive in the dense
//! scalar namespace, or a whole value in the structural namespace. Its
//! completion distinguishes a normal return from a closed external outcome;
//! transfers carry existing claims through normal completion or explicitly
//! abandon them. `CheckedUnitCallPlan::new` admits these combinations:
//!
//! | route    | completion     | result     | transfers            | contract commitment | source site  |
//! |----------|----------------|------------|----------------------|---------------------|--------------|
//! | module   | `NormalReturn` | Unit       | `Claims`             | absent              | not retained |
//! | module   | `NormalReturn` | scalar     | `Claims`             | present             | not retained |
//! | module   | `NormalReturn` | structural | `StructuralCustody`  | present             | retained     |
//! | boundary | `NormalReturn` | Unit       | `CompletionReceipts` | absent              | retained     |
//! | boundary | `NormalReturn` | scalar     | `CompletionReceipts` | absent              | retained     |
//! | boundary | `NormalReturn` | structural | `StructuralCustody`  | absent              | retained     |
//! | dynamic  | `NormalReturn` | Unit       | `Claims`             | absent              | not retained |
//! | dynamic  | `NormalReturn` | scalar     | `Claims`             | absent              | not retained |
//! | dynamic  | `NormalReturn` | structural | `StructuralCustody`  | absent              | retained     |
//! | boundary | `External`     | Unit       | `Abandonment`        | absent              | retained     |
//!
//! The normal call rows have `NormalReturn` completion. The final row has
//! `External` completion, boundary route, source Unit omission, `Abandonment`
//! transfers, no contract commitment, and a retained source site. Unit denotes
//! omitted source result syntax here, never a normal external result. Actual
//! domain-bound authority remains an unsatisfied entry/runtime demand.
//!
//! A dynamic call's target is its instantiated slot; no single machine's
//! contract commitment describes every implementation it can reach. Structured
//! results preserve the authored producer site and ordinary custody while the
//! public slot owns their result signature.
//!
//! The route, completion, result, transfers and source site are private so that no later
//! edit can separate them. Each has an accessor, and the parts a pass edits
//! have mutators that cannot change their kind. The target identity (machine,
//! state, contract fingerprint and reach) sits beside the route, not inside
//! it, so it stays plain public data that forging tests and passes can edit
//! without reopening the invariant. A field that some combinations do not
//! retain stays an `Option` in its accessor. Each consumer keeps deciding what
//! the absence means; the plan never fills a field in.
//!
//! `CheckedUnitEffectOperationPlan::call` returns this plan. It replaces the
//! borrowed view the six fused variants needed: once one record carries every
//! call, the record's own fields and accessors are the shared reading.

use language_semantics::ServiceReachSummary;
use semantic_vocabulary::BoundaryCompletion;
use symbols::SymbolHandle;

use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedStructuralCallCustodyPlan, CheckedUnitCallCoordinate,
    CheckedUnitClaimTransferPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralResultBindingPlan,
};
use crate::checked_trees::values::CheckedProofTerm;
use crate::{MachineContractCommitment, NominalMachineUseSite};
use typed_trees::type_identity::TypeIdentityId;

/// The exact machine state a call enters and the service reach of the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitCallTarget {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub contract_report_fingerprint: u64,
    pub service_reach: ServiceReachSummary,
}

/// How a call reaches its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitCallRoute {
    /// A sealed compiler operation performed in this caller's activation.
    /// Its source invocation shares operand evaluation and result custody with
    /// calls, but enters no callee frame and demands no provider completion.
    ActivationStorage {
        operation: language_core::activation_storage::ActivationStorageOperation,
        /// Present exactly on establishment; this is a closed byte demand,
        /// never the runtime committed extent or a diagnostic identity.
        bound: Option<u64>,
    },
    /// An ordinary checked machine (or a bodied boundary adapter) whose
    /// authored body this module compiles.
    Module {
        /// The target's public contract commitment. Scalar and structural
        /// results retain it; a Unit call does not.
        contract_commitment: Option<MachineContractCommitment>,
        /// Proof-only erased actuals in the target's erased-formal order.
        erased_scalar_arguments: Vec<CheckedCallScalarArgument>,
        /// Erased proof-only actuals in the target's erased-proof roster
        /// order. They carry semantic identity, never a runtime operand.
        erased_proof_arguments: Vec<CheckedProofTerm>,
    },
    /// A bodyless boundary declaration. It owns no erased lane and no
    /// contract commitment.
    Boundary,
    /// The implementation a runtime selector names
    /// (`Trait::member(selector, data...)`). The selector is the dispatch
    /// operand, kept apart from the data arguments every implementation
    /// receives; the target names the call's instantiated slot.
    Dynamic {
        selector: CheckedCallScalarArgument,
        /// Normalized identity of the selector's `dyn Trait<Args>` type.
        interface_identity: TypeIdentityId,
        /// The exact requirement the slot instantiates.
        requirement: SymbolHandle,
        /// Evaluated actuals of a finite-family call in the roster's
        /// family-binder order; the dispatch compares them against each
        /// candidate row's declared tuple identities to select one row at
        /// runtime. Empty on an ordinary requirement call.
        family_operands: Vec<CheckedCallScalarArgument>,
    },
}

/// What a call binds for the operations after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitCallResult {
    /// The exact checked module invocation has no normal continuation. Its
    /// declared callee signature remains intact; no value or normal custody
    /// receipt enters this caller. This supplies no abandonment authority.
    NoNormal,
    /// Nothing.
    Unit,
    /// A primitive in the dense scalar namespace. A boundary result binds the
    /// exact immutable local its statement declares, and downstream lowering
    /// publishes it for later operations.
    Scalar(CheckedUnitScalarResultBindingPlan),
    /// A whole value in the structural namespace. The result signature stays
    /// target-neutral; physical placement belongs to native realization.
    Structural {
        binding: CheckedUnitStructuralResultBindingPlan,
        /// One affine result that no later operation consumes is discarded
        /// explicitly on the caller's final Unit return.
        discard_result_on_return: bool,
    },
}

/// The existing claims a call carries through completion. Transfers never
/// establish fresh authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitCallTransfers {
    /// An ordinary Unit or scalar call's claim transfers.
    Claims(Vec<CheckedUnitClaimTransferPlan>),
    /// A structural call's ingress and receiving custody, kept separate from
    /// its value binding. Boundary ingress remains completion receipts in
    /// this roster; dispatch never issues the result's claims.
    StructuralCustody(CheckedStructuralCallCustodyPlan),
    /// A boundary call's completion receipts.
    CompletionReceipts(Vec<CheckedUnitClaimTransferPlan>),
    /// External completion abandons in-domain claims without normal receipts.
    /// This decision grants no containing-process-domain authority.
    Abandonment,
}

/// Every field of one call, unchecked. [`CheckedUnitCallPlan::new`] admits
/// the parts only in one of the nine shapes in the module table, and
/// [`CheckedUnitCallPlan::into_parts`] returns them for a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitCallParts {
    pub coordinate: CheckedUnitCallCoordinate,
    /// Exact authored call site retained for target-owned occurrence joins.
    /// Transition-target calls have no statement or expression arena site.
    /// Ordinary Unit and scalar calls retain no site and must pass `None`.
    pub source_site: Option<NominalMachineUseSite>,
    pub target: CheckedUnitCallTarget,
    pub route: CheckedUnitCallRoute,
    pub completion: BoundaryCompletion,
    pub scalar_arguments: Vec<CheckedCallScalarArgument>,
    pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    pub result: CheckedUnitCallResult,
    pub transfers: CheckedUnitCallTransfers,
}

/// One call operation in an admitted route, result and custody shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitCallPlan {
    pub coordinate: CheckedUnitCallCoordinate,
    source_site: Option<NominalMachineUseSite>,
    pub target: CheckedUnitCallTarget,
    route: CheckedUnitCallRoute,
    completion: BoundaryCompletion,
    /// Runtime primitive actuals: in the target's retained scalar-parameter
    /// order, or the boundary declaration's dense scalar-parameter order.
    /// Structural actuals keep their separate custody namespace.
    pub scalar_arguments: Vec<CheckedCallScalarArgument>,
    /// Structural actuals in authored argument order.
    pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    result: CheckedUnitCallResult,
    transfers: CheckedUnitCallTransfers,
}

impl CheckedUnitCallPlan {
    /// Admit one of the call shapes in the module table. Returns `None`
    /// for any other combination of route, result, transfers, contract
    /// commitment and source site.
    pub fn new(parts: CheckedUnitCallParts) -> Option<Self> {
        use CheckedUnitCallResult as Result;
        use CheckedUnitCallTransfers as Transfers;
        let CheckedUnitCallParts {
            coordinate,
            source_site,
            target,
            route,
            completion,
            scalar_arguments,
            structural_arguments,
            result,
            transfers,
        } = parts;
        let admitted = match completion {
            BoundaryCompletion::External(_) => matches!(
                (&route, &result, &transfers),
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Unit,
                    Transfers::Abandonment
                )
            ),
            BoundaryCompletion::NormalReturn => match (&route, &result, &transfers) {
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::NoNormal,
                    Transfers::Claims(_),
                ) => contract_commitment.is_some(),
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation:
                            language_core::activation_storage::ActivationStorageOperation::Establish,
                        bound: Some(_),
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation:
                            language_core::activation_storage::ActivationStorageOperation::BorrowBytes,
                        bound: None,
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation:
                            language_core::activation_storage::ActivationStorageOperation::Release,
                        bound: None,
                    },
                    Result::Unit,
                    Transfers::Claims(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation:
                            language_core::activation_storage::ActivationStorageOperation::Split,
                        bound: Some(_),
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation:
                            language_core::activation_storage::ActivationStorageOperation::Merge,
                        bound: None,
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Unit,
                    Transfers::Claims(_),
                ) => contract_commitment.is_none() && source_site.is_none(),
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Scalar(_),
                    Transfers::Claims(_),
                ) => contract_commitment.is_some() && source_site.is_none(),
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => contract_commitment.is_some(),
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Unit | Result::Scalar(_),
                    Transfers::CompletionReceipts(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Dynamic { .. },
                    Result::Unit | Result::Scalar(_),
                    Transfers::Claims(_),
                ) => source_site.is_none(),
                (
                    CheckedUnitCallRoute::Dynamic { .. },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                _ => false,
            },
        };
        admitted.then_some(Self {
            coordinate,
            source_site,
            target,
            route,
            completion,
            scalar_arguments,
            structural_arguments,
            result,
            transfers,
        })
    }

    /// Every field, for a rebuild through [`CheckedUnitCallPlan::new`].
    pub fn into_parts(self) -> CheckedUnitCallParts {
        CheckedUnitCallParts {
            coordinate: self.coordinate,
            source_site: self.source_site,
            target: self.target,
            route: self.route,
            completion: self.completion,
            scalar_arguments: self.scalar_arguments,
            structural_arguments: self.structural_arguments,
            result: self.result,
            transfers: self.transfers,
        }
    }

    /// Whether the target is a bodyless boundary rather than an ordinary
    /// checked machine.
    pub fn is_boundary(&self) -> bool {
        matches!(self.route, CheckedUnitCallRoute::Boundary)
    }

    /// Whether the call enters an ordinary checked machine this module
    /// compiles: neither a bodyless boundary nor a runtime-selected
    /// implementation.
    pub fn is_module(&self) -> bool {
        matches!(self.route, CheckedUnitCallRoute::Module { .. })
    }

    pub fn route(&self) -> &CheckedUnitCallRoute {
        &self.route
    }

    /// The exact checked requirement's completion, independent of provider
    /// selection and source result omission. External completion demands actual
    /// domain-bound authority from the admitted entry/runtime environment.
    pub fn completion(&self) -> BoundaryCompletion {
        self.completion
    }

    /// Continuation, unlike abandonment, follows the invocation outcome. A
    /// checked nonreturning cycle cannot grant disposal or loan completion.
    pub fn has_normal_continuation(&self) -> bool {
        self.completion == BoundaryCompletion::NormalReturn
            && self.result != CheckedUnitCallResult::NoNormal
    }

    pub fn result(&self) -> &CheckedUnitCallResult {
        &self.result
    }

    pub fn transfers(&self) -> &CheckedUnitCallTransfers {
        &self.transfers
    }

    /// Whether this route and result retain the authored call site: every
    /// boundary call and every ordinary structural call.
    fn retains_source_site(&self) -> bool {
        self.is_boundary()
            || matches!(self.route, CheckedUnitCallRoute::ActivationStorage { .. })
            || matches!(
                self.result,
                CheckedUnitCallResult::Structural { .. } | CheckedUnitCallResult::NoNormal
            )
    }

    /// The exact authored call site the row retains for target-owned
    /// occurrence joins. `None` when the call retains no site (an ordinary
    /// Unit or scalar call); `Some(None)` for a transition-target call, which
    /// has no statement or expression arena site.
    pub fn source_site(&self) -> Option<Option<NominalMachineUseSite>> {
        self.retains_source_site().then_some(self.source_site)
    }

    /// The target's contract commitment, retained by ordinary scalar and
    /// structural calls only.
    pub fn contract_commitment(&self) -> Option<MachineContractCommitment> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                contract_commitment,
                ..
            } => *contract_commitment,
            CheckedUnitCallRoute::ActivationStorage { .. }
            | CheckedUnitCallRoute::Boundary
            | CheckedUnitCallRoute::Dynamic { .. } => None,
        }
    }

    /// Proof-only erased actuals in the target's erased-formal order; `None`
    /// for a boundary or dynamic call, which retains no erased lane.
    pub fn erased_scalar_arguments(&self) -> Option<&[CheckedCallScalarArgument]> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                erased_scalar_arguments,
                ..
            } => Some(erased_scalar_arguments),
            CheckedUnitCallRoute::ActivationStorage { .. }
            | CheckedUnitCallRoute::Boundary
            | CheckedUnitCallRoute::Dynamic { .. } => None,
        }
    }

    /// Erased proof-only actuals in the target's erased-proof roster order;
    /// `None` for a boundary or dynamic call.
    pub fn erased_proof_arguments(&self) -> Option<&[CheckedProofTerm]> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                erased_proof_arguments,
                ..
            } => Some(erased_proof_arguments),
            CheckedUnitCallRoute::ActivationStorage { .. }
            | CheckedUnitCallRoute::Boundary
            | CheckedUnitCallRoute::Dynamic { .. } => None,
        }
    }

    /// Every runtime scalar operand in evaluation order: a dynamic call's
    /// selector, authored first, then the scalar arguments.
    pub fn scalar_operands(&self) -> Vec<&CheckedCallScalarArgument> {
        self.dynamic_selector()
            .into_iter()
            .chain(&self.scalar_arguments)
            .collect()
    }

    /// A dynamic call's selector operand; `None` for a call whose target is
    /// fixed.
    pub fn dynamic_selector(&self) -> Option<&CheckedCallScalarArgument> {
        match &self.route {
            CheckedUnitCallRoute::Dynamic { selector, .. } => Some(selector),
            _ => None,
        }
    }

    /// The primitive result binding of a scalar call.
    pub fn scalar_result(&self) -> Option<&CheckedUnitScalarResultBindingPlan> {
        match &self.result {
            CheckedUnitCallResult::Scalar(result) => Some(result),
            _ => None,
        }
    }

    /// The whole-value result binding of a structural call.
    pub fn structural_result(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
        match &self.result {
            CheckedUnitCallResult::Structural { binding, .. } => Some(binding),
            _ => None,
        }
    }

    /// A structural call's result binding and its return-discard flag.
    pub fn structural_result_mut(
        &mut self,
    ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
        match &mut self.result {
            CheckedUnitCallResult::Structural {
                binding,
                discard_result_on_return,
            } => Some((binding, discard_result_on_return)),
            _ => None,
        }
    }

    /// Whether a structural call discards its result on the caller's final
    /// Unit return; `None` for a Unit or scalar call.
    pub fn discard_result_on_return(&self) -> Option<bool> {
        match &self.result {
            CheckedUnitCallResult::Structural {
                discard_result_on_return,
                ..
            } => Some(*discard_result_on_return),
            _ => None,
        }
    }

    /// The claims the call carries through completion: an ordinary call's
    /// transfers (a structural call keeps them in its custody record) or a
    /// boundary call's normal completion receipts. Abandonment has none; an
    /// empty slice does not establish normal completion or discharge claims.
    pub fn claim_transfers(&self) -> &[CheckedUnitClaimTransferPlan] {
        match &self.transfers {
            CheckedUnitCallTransfers::Claims(claims)
            | CheckedUnitCallTransfers::CompletionReceipts(claims) => claims,
            CheckedUnitCallTransfers::StructuralCustody(custody) => &custody.claim_transfers,
            CheckedUnitCallTransfers::Abandonment => &[],
        }
    }

    /// A structural call's result custody, including boundary completion.
    pub fn structural_custody(&self) -> Option<&CheckedStructuralCallCustodyPlan> {
        match &self.transfers {
            CheckedUnitCallTransfers::StructuralCustody(custody) => Some(custody),
            _ => None,
        }
    }

    pub fn structural_custody_mut(&mut self) -> Option<&mut CheckedStructuralCallCustodyPlan> {
        match &mut self.transfers {
            CheckedUnitCallTransfers::StructuralCustody(custody) => Some(custody),
            _ => None,
        }
    }
}
