//! The fields every call operation shares, borrowed from one operation row.
//!
//! `CheckedUnitEffectOperationPlan` fuses a call's target (an ordinary
//! checked machine or a bodyless boundary) with its result (Unit, scalar or
//! structural) into six variants. A consumer that reads only the shared
//! fields asks `call()` once instead of destructuring all six. A field that
//! only some variants retain stays an `Option` here, so each consumer keeps
//! deciding what its absence means; the view never fills one in.

use language_semantics::ServiceReachSummary;
use symbols::SymbolHandle;

use crate::NominalMachineUseSite;
use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralResultBindingPlan,
};

/// What a call binds for the operations after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedUnitCallResult<'a> {
    /// Nothing: `CallUnit` and `BoundaryCall`.
    Unit,
    /// A primitive in the dense scalar namespace: `ScalarCall` and
    /// `BoundaryScalarCall`.
    Scalar(&'a CheckedUnitScalarResultBindingPlan),
    /// A whole value in the structural namespace: `StructuralCall` and
    /// `BoundaryStructuralCall`.
    Structural(&'a CheckedUnitStructuralResultBindingPlan),
}

/// One call operation's shared fields, built only by
/// `CheckedUnitEffectOperationPlan::call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitCallView<'a> {
    operation: &'a CheckedUnitEffectOperationPlan,
    boundary: bool,
    coordinate: CheckedUnitCallCoordinate,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_report_fingerprint: u64,
    service_reach: ServiceReachSummary,
    scalar_arguments: &'a [CheckedCallScalarArgument],
    erased_scalar_arguments: Option<&'a [CheckedCallScalarArgument]>,
    structural_arguments: &'a [CheckedUnitStructuralArgumentPlan],
    source_site: Option<Option<NominalMachineUseSite>>,
    result: CheckedUnitCallResult<'a>,
    claim_transfers: &'a [CheckedUnitClaimTransferPlan],
}

impl<'a> CheckedUnitCallView<'a> {
    /// The operation row this view borrows.
    pub fn operation(self) -> &'a CheckedUnitEffectOperationPlan {
        self.operation
    }

    /// Whether the target is a bodyless boundary rather than an ordinary
    /// checked machine.
    pub fn is_boundary(self) -> bool {
        self.boundary
    }

    pub fn coordinate(self) -> CheckedUnitCallCoordinate {
        self.coordinate
    }

    pub fn target_machine(self) -> SymbolHandle {
        self.target_machine
    }

    pub fn target_state(self) -> SymbolHandle {
        self.target_state
    }

    pub fn target_contract_report_fingerprint(self) -> u64 {
        self.target_contract_report_fingerprint
    }

    pub fn service_reach(self) -> ServiceReachSummary {
        self.service_reach
    }

    /// Runtime primitive actuals: in the target's retained scalar-parameter
    /// order, or the boundary declaration's dense scalar-parameter order.
    pub fn scalar_arguments(self) -> &'a [CheckedCallScalarArgument] {
        self.scalar_arguments
    }

    /// Proof-only erased actuals in the target's erased-formal order; `None`
    /// for a boundary call, which retains no erased lane.
    pub fn erased_scalar_arguments(self) -> Option<&'a [CheckedCallScalarArgument]> {
        self.erased_scalar_arguments
    }

    /// Structural actuals in authored argument order.
    pub fn structural_arguments(self) -> &'a [CheckedUnitStructuralArgumentPlan] {
        self.structural_arguments
    }

    /// The exact authored call site the row retains for target-owned
    /// occurrence joins. `None` when the variant retains no site (`CallUnit`,
    /// `ScalarCall`); `Some(None)` for a transition-target call, which has no
    /// statement or expression arena site.
    pub fn source_site(self) -> Option<Option<NominalMachineUseSite>> {
        self.source_site
    }

    pub fn result(self) -> CheckedUnitCallResult<'a> {
        self.result
    }

    /// The claims the call carries through completion: an ordinary call's
    /// transfers (a structural call keeps them in its custody record) or a
    /// boundary call's completion receipts.
    pub fn claim_transfers(self) -> &'a [CheckedUnitClaimTransferPlan] {
        self.claim_transfers
    }
}

impl CheckedUnitEffectOperationPlan {
    /// The shared fields of an ordinary or boundary call with a Unit, scalar
    /// or structural result; `None` for every other operation. Selected
    /// operators keep their own operand lanes.
    pub fn call(&self) -> Option<CheckedUnitCallView<'_>> {
        let view = match self {
            Self::CallUnit {
                coordinate,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                erased_scalar_arguments,
                structural_arguments,
                claim_transfers,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: false,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: Some(erased_scalar_arguments),
                structural_arguments,
                source_site: None,
                result: CheckedUnitCallResult::Unit,
                claim_transfers,
            },
            Self::ScalarCall {
                coordinate,
                result,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                erased_scalar_arguments,
                structural_arguments,
                claim_transfers,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: false,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: Some(erased_scalar_arguments),
                structural_arguments,
                source_site: None,
                result: CheckedUnitCallResult::Scalar(result),
                claim_transfers,
            },
            Self::StructuralCall {
                coordinate,
                source_site,
                result,
                custody,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                erased_scalar_arguments,
                structural_arguments,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: false,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: Some(erased_scalar_arguments),
                structural_arguments,
                source_site: Some(*source_site),
                result: CheckedUnitCallResult::Structural(result),
                claim_transfers: &custody.claim_transfers,
            },
            Self::BoundaryCall {
                coordinate,
                source_site,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: true,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: None,
                structural_arguments,
                source_site: Some(*source_site),
                result: CheckedUnitCallResult::Unit,
                claim_transfers: completion_receipts,
            },
            Self::BoundaryScalarCall {
                coordinate,
                source_site,
                result,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: true,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: None,
                structural_arguments,
                source_site: Some(*source_site),
                result: CheckedUnitCallResult::Scalar(result),
                claim_transfers: completion_receipts,
            },
            Self::BoundaryStructuralCall {
                coordinate,
                source_site,
                result,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            } => CheckedUnitCallView {
                operation: self,
                boundary: true,
                coordinate: *coordinate,
                target_machine: *target_machine,
                target_state: *target_state,
                target_contract_report_fingerprint: *target_contract_report_fingerprint,
                service_reach: *service_reach,
                scalar_arguments,
                erased_scalar_arguments: None,
                structural_arguments,
                source_site: Some(*source_site),
                result: CheckedUnitCallResult::Structural(result),
                claim_transfers: completion_receipts,
            },
            _ => return None,
        };
        Some(view)
    }
}

#[cfg(test)]
mod tests {
    use language_semantics::ServiceReachSummary;
    use symbols::SymbolHandle;

    use super::CheckedUnitCallResult;
    use crate::checked_trees::flow::terminal::{
        CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan,
    };

    const COORDINATE: CheckedUnitCallCoordinate = CheckedUnitCallCoordinate {
        statement_index: 2,
        call_ordinal: 0,
    };

    #[test]
    fn a_view_keeps_the_fields_a_variant_lacks_absent() {
        let unit = CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: COORDINATE,
            target_machine: SymbolHandle::from_arena_index(1),
            target_state: SymbolHandle::from_arena_index(2),
            target_contract_report_fingerprint: 7,
            service_reach: ServiceReachSummary::default(),
            scalar_arguments: Vec::new(),
            erased_scalar_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        };
        let call = unit.call().expect("a Unit call has a view");
        assert!(!call.is_boundary());
        assert_eq!(call.coordinate(), COORDINATE);
        assert_eq!(call.target_state(), SymbolHandle::from_arena_index(2));
        assert_eq!(call.erased_scalar_arguments(), Some(&[][..]));
        assert_eq!(call.source_site(), None);
        assert_eq!(call.result(), CheckedUnitCallResult::Unit);

        let boundary = CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: COORDINATE,
            source_site: None,
            target_machine: SymbolHandle::from_arena_index(3),
            target_state: SymbolHandle::from_arena_index(4),
            target_contract_report_fingerprint: 9,
            service_reach: ServiceReachSummary::default(),
            scalar_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        };
        let call = boundary.call().expect("a boundary call has a view");
        assert!(call.is_boundary());
        assert_eq!(call.target_machine(), SymbolHandle::from_arena_index(3));
        assert_eq!(call.erased_scalar_arguments(), None);
        // A transition-target boundary call retains a site slot with no site.
        assert_eq!(call.source_site(), Some(None));
        assert!(std::ptr::eq(call.operation(), &boundary));

        let port = CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: COORDINATE,
            port: 0x3f8,
            value: 0,
            service_reach: ServiceReachSummary::default(),
        };
        assert_eq!(port.call(), None);
    }
}
