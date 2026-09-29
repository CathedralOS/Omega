//! One ordinary or boundary call from a Unit-effect body.
//!
//! A call has three independent axes. Its route reaches an ordinary checked
//! machine in this module or a bodyless boundary. Its result binds nothing, a
//! primitive in the dense scalar namespace, or a whole value in the structural
//! namespace. Its transfers carry existing claims through completion. Only six
//! (route, result, transfers) combinations exist, and `CheckedUnitCallPlan::new`
//! admits exactly those six:
//!
//! | route    | result     | transfers            | contract commitment | source site  |
//! |----------|------------|----------------------|---------------------|--------------|
//! | module   | Unit       | `Claims`             | absent              | not retained |
//! | module   | scalar     | `Claims`             | present             | not retained |
//! | module   | structural | `StructuralCustody`  | present             | retained     |
//! | boundary | Unit       | `CompletionReceipts` | absent              | retained     |
//! | boundary | scalar     | `CompletionReceipts` | absent              | retained     |
//! | boundary | structural | `CompletionReceipts` | absent              | retained     |
//!
//! The route, result, transfers and source site are private so that no later
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
use symbols::SymbolHandle;

use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedStructuralCallCustodyPlan, CheckedUnitCallCoordinate,
    CheckedUnitClaimTransferPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralResultBindingPlan,
};
use crate::checked_trees::values::CheckedProofTerm;
use crate::{MachineContractCommitment, NominalMachineUseSite};

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
}

/// What a call binds for the operations after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitCallResult {
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
    /// An ordinary structural call's result custody, kept separate from its
    /// value binding.
    StructuralCustody(CheckedStructuralCallCustodyPlan),
    /// A boundary call's completion receipts.
    CompletionReceipts(Vec<CheckedUnitClaimTransferPlan>),
}

/// Every field of one call, unchecked. [`CheckedUnitCallPlan::new`] admits
/// the parts only in one of the six shapes in the module table, and
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
    pub scalar_arguments: Vec<CheckedCallScalarArgument>,
    pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    pub result: CheckedUnitCallResult,
    pub transfers: CheckedUnitCallTransfers,
}

/// One call operation in one of the six admitted shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitCallPlan {
    pub coordinate: CheckedUnitCallCoordinate,
    source_site: Option<NominalMachineUseSite>,
    pub target: CheckedUnitCallTarget,
    route: CheckedUnitCallRoute,
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
    /// Admit one of the six call shapes in the module table. Returns `None`
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
            scalar_arguments,
            structural_arguments,
            result,
            transfers,
        } = parts;
        let admitted = match (&route, &result, &transfers) {
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
            (CheckedUnitCallRoute::Boundary, _, Transfers::CompletionReceipts(_)) => true,
            _ => false,
        };
        admitted.then_some(Self {
            coordinate,
            source_site,
            target,
            route,
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

    pub fn route(&self) -> &CheckedUnitCallRoute {
        &self.route
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
        self.is_boundary() || matches!(self.result, CheckedUnitCallResult::Structural { .. })
    }

    /// The exact authored call site the row retains for target-owned
    /// occurrence joins. `None` when the call retains no site (an ordinary
    /// Unit or scalar call); `Some(None)` for a transition-target call, which
    /// has no statement or expression arena site.
    pub fn source_site(&self) -> Option<Option<NominalMachineUseSite>> {
        self.retains_source_site().then_some(self.source_site)
    }

    /// The retained source site; `None` when the call retains no site.
    pub fn source_site_mut(&mut self) -> Option<&mut Option<NominalMachineUseSite>> {
        if self.retains_source_site() {
            Some(&mut self.source_site)
        } else {
            None
        }
    }

    /// The target's contract commitment, retained by ordinary scalar and
    /// structural calls only.
    pub fn contract_commitment(&self) -> Option<MachineContractCommitment> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                contract_commitment,
                ..
            } => *contract_commitment,
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    /// The retained contract commitment; `None` when the call retains none.
    /// A retained commitment can change but cannot be removed.
    pub fn contract_commitment_mut(&mut self) -> Option<&mut MachineContractCommitment> {
        match &mut self.route {
            CheckedUnitCallRoute::Module {
                contract_commitment,
                ..
            } => contract_commitment.as_mut(),
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    /// Proof-only erased actuals in the target's erased-formal order; `None`
    /// for a boundary call, which retains no erased lane.
    pub fn erased_scalar_arguments(&self) -> Option<&[CheckedCallScalarArgument]> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                erased_scalar_arguments,
                ..
            } => Some(erased_scalar_arguments),
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    pub fn erased_scalar_arguments_mut(&mut self) -> Option<&mut Vec<CheckedCallScalarArgument>> {
        match &mut self.route {
            CheckedUnitCallRoute::Module {
                erased_scalar_arguments,
                ..
            } => Some(erased_scalar_arguments),
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    /// Erased proof-only actuals in the target's erased-proof roster order;
    /// `None` for a boundary call.
    pub fn erased_proof_arguments(&self) -> Option<&[CheckedProofTerm]> {
        match &self.route {
            CheckedUnitCallRoute::Module {
                erased_proof_arguments,
                ..
            } => Some(erased_proof_arguments),
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    pub fn erased_proof_arguments_mut(&mut self) -> Option<&mut Vec<CheckedProofTerm>> {
        match &mut self.route {
            CheckedUnitCallRoute::Module {
                erased_proof_arguments,
                ..
            } => Some(erased_proof_arguments),
            CheckedUnitCallRoute::Boundary => None,
        }
    }

    /// The primitive result binding of a scalar call.
    pub fn scalar_result(&self) -> Option<&CheckedUnitScalarResultBindingPlan> {
        match &self.result {
            CheckedUnitCallResult::Scalar(result) => Some(result),
            _ => None,
        }
    }

    pub fn scalar_result_mut(&mut self) -> Option<&mut CheckedUnitScalarResultBindingPlan> {
        match &mut self.result {
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
    /// boundary call's completion receipts.
    pub fn claim_transfers(&self) -> &[CheckedUnitClaimTransferPlan] {
        match &self.transfers {
            CheckedUnitCallTransfers::Claims(claims)
            | CheckedUnitCallTransfers::CompletionReceipts(claims) => claims,
            CheckedUnitCallTransfers::StructuralCustody(custody) => &custody.claim_transfers,
        }
    }

    pub fn claim_transfers_mut(&mut self) -> &mut Vec<CheckedUnitClaimTransferPlan> {
        match &mut self.transfers {
            CheckedUnitCallTransfers::Claims(claims)
            | CheckedUnitCallTransfers::CompletionReceipts(claims) => claims,
            CheckedUnitCallTransfers::StructuralCustody(custody) => &mut custody.claim_transfers,
        }
    }

    /// An ordinary structural call's result custody.
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

#[cfg(test)]
mod tests {
    use language_semantics::ServiceReachSummary;
    use symbols::SymbolHandle;

    use super::{
        CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
        CheckedUnitCallTarget, CheckedUnitCallTransfers,
    };
    use crate::checked_trees::flow::terminal::{
        CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan,
        CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralResultBindingPlan,
    };
    use crate::{MachineContractCommitment, NominalMachineUseSite};

    const COORDINATE: CheckedUnitCallCoordinate = CheckedUnitCallCoordinate {
        statement_index: 2,
        call_ordinal: 0,
    };

    fn target() -> CheckedUnitCallTarget {
        CheckedUnitCallTarget {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            contract_report_fingerprint: 7,
            service_reach: ServiceReachSummary::default(),
        }
    }

    fn module(commitment: bool) -> CheckedUnitCallRoute {
        CheckedUnitCallRoute::Module {
            contract_commitment: commitment
                .then(|| MachineContractCommitment::from_digest([3; 32])),
            erased_scalar_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
        }
    }

    fn scalar() -> CheckedUnitCallResult {
        CheckedUnitCallResult::Scalar(CheckedUnitScalarResultBindingPlan {
            statement_index: 2,
            binding_ordinal: 0,
            primitive_type: typed_trees::types::PrimitiveType::U8,
        })
    }

    fn structural() -> CheckedUnitCallResult {
        CheckedUnitCallResult::Structural {
            binding: CheckedUnitStructuralResultBindingPlan {
                statement_index: 2,
                binding_ordinal: 0,
                type_identity: "Token".to_owned(),
                multiplicity: language_semantics::Multiplicity::Affine,
            },
            discard_result_on_return: true,
        }
    }

    fn call(
        source_site: Option<NominalMachineUseSite>,
        route: CheckedUnitCallRoute,
        result: CheckedUnitCallResult,
        transfers: CheckedUnitCallTransfers,
    ) -> Option<CheckedUnitCallPlan> {
        CheckedUnitCallPlan::new(CheckedUnitCallParts {
            coordinate: COORDINATE,
            source_site,
            target: target(),
            route,
            scalar_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            result,
            transfers,
        })
    }

    #[test]
    fn the_constructor_admits_exactly_the_six_call_shapes() {
        let claims = || CheckedUnitCallTransfers::Claims(Vec::new());
        let custody = || CheckedUnitCallTransfers::StructuralCustody(Default::default());
        let receipts = || CheckedUnitCallTransfers::CompletionReceipts(Vec::new());
        let unit = || CheckedUnitCallResult::Unit;
        let site = Some(NominalMachineUseSite::Expression(
            arena::Handle::from_parts(4, 0),
        ));

        assert!(call(None, module(false), unit(), claims()).is_some());
        assert!(call(None, module(true), scalar(), claims()).is_some());
        assert!(call(site, module(true), structural(), custody()).is_some());
        assert!(call(site, CheckedUnitCallRoute::Boundary, unit(), receipts()).is_some());
        assert!(call(None, CheckedUnitCallRoute::Boundary, scalar(), receipts()).is_some());
        assert!(
            call(
                site,
                CheckedUnitCallRoute::Boundary,
                structural(),
                receipts()
            )
            .is_some()
        );

        // Transfers belong to exactly one (route, result) pair.
        assert!(call(None, module(false), unit(), receipts()).is_none());
        assert!(call(None, module(true), structural(), claims()).is_none());
        assert!(call(None, module(true), scalar(), custody()).is_none());
        assert!(call(None, CheckedUnitCallRoute::Boundary, unit(), claims()).is_none());
        assert!(
            call(
                None,
                CheckedUnitCallRoute::Boundary,
                structural(),
                custody()
            )
            .is_none()
        );
        // Only ordinary scalar and structural calls retain the commitment.
        assert!(call(None, module(true), unit(), claims()).is_none());
        assert!(call(None, module(false), scalar(), claims()).is_none());
        assert!(call(None, module(false), structural(), custody()).is_none());
        // Ordinary Unit and scalar calls retain no site.
        assert!(call(site, module(false), unit(), claims()).is_none());
        assert!(call(site, module(true), scalar(), claims()).is_none());
    }

    #[test]
    fn a_call_keeps_the_fields_its_shape_lacks_absent() {
        let unit = CheckedUnitEffectOperationPlan::Call(
            call(
                None,
                module(false),
                CheckedUnitCallResult::Unit,
                CheckedUnitCallTransfers::Claims(Vec::new()),
            )
            .expect("an ordinary Unit call"),
        );
        let planned = unit.call().expect("a Unit call is a call");
        assert!(!planned.is_boundary());
        assert_eq!(planned.coordinate, COORDINATE);
        assert_eq!(planned.target.state, SymbolHandle::from_arena_index(2));
        assert_eq!(planned.erased_scalar_arguments(), Some(&[][..]));
        assert_eq!(planned.contract_commitment(), None);
        assert_eq!(planned.source_site(), None);
        assert_eq!(planned.result(), &CheckedUnitCallResult::Unit);
        assert_eq!(planned.discard_result_on_return(), None);

        let mut boundary = call(
            None,
            CheckedUnitCallRoute::Boundary,
            structural(),
            CheckedUnitCallTransfers::CompletionReceipts(Vec::new()),
        )
        .expect("a boundary structural call");
        assert!(boundary.is_boundary());
        assert_eq!(boundary.erased_scalar_arguments(), None);
        assert_eq!(boundary.structural_custody(), None);
        assert_eq!(boundary.discard_result_on_return(), Some(true));
        // A transition-target boundary call retains a site slot with no site.
        assert_eq!(boundary.source_site(), Some(None));
        let (_, discard) = boundary
            .structural_result_mut()
            .expect("a structural result");
        *discard = false;
        assert_eq!(boundary.discard_result_on_return(), Some(false));

        let port = CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: COORDINATE,
            port: 0x3f8,
            value: 0,
            service_reach: ServiceReachSummary::default(),
        };
        assert_eq!(port.call(), None);
    }
}
