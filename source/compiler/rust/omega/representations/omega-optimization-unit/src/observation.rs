use omega_optimization_core::OptimizationUnitIdentity;
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;

use crate::{
    EffectLink, FuelSettlement, OwnershipEvent, PsiOptimizationUnit, PsiProvenance,
    ValueDefinition, ValueUse,
};

/// Whether one exit class is absent, possible through opaque work, or certain
/// at this exact node. `May` is deliberately not narrowed from call syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationKnowledge {
    No,
    May,
    Yes,
}

/// Externally relevant event axis represented by a Terminal-Psi-derived node.
/// The complete operation is retained beside the class so comparisons cannot
/// accidentally erase a callee, boundary, service, edge, cleanup, or crash
/// payload while considering only its broad category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationEventClass {
    StructuralState,
    InternalCall,
    BoundaryCall,
    Service,
    ControlTransfer,
    NormalExit,
    CrashExit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiObservableEvent {
    pub class: ObservationEventClass,
    pub operation: O,
}

/// Compiler-owned observation row for one optimization node.
///
/// Empty `events` means the operation is a pure scalar computation under its
/// already-verified exact Terminal semantics. It does not mean provenance,
/// logical fuel, definitions, or live uses may be dropped: those axes remain
/// explicit in every row and a closed-region validator must join liveness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiNodeObservation {
    pub machine: psi_core::MachineId,
    pub block: psi_core::BlockId,
    pub node: u32,
    pub definitions: Vec<ValueDefinition>,
    pub uses: Vec<ValueUse>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
    pub provenance: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
    pub crash: ObservationKnowledge,
    pub suspension: ObservationKnowledge,
    pub events: Vec<PsiObservableEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiObservationModel {
    pub revision: OptimizationUnitIdentity,
    pub nodes: Vec<PsiNodeObservation>,
}

/// Reconstruct the observation boundary from the immutable unit. The exhaustive
/// operation match is intentional: adding a Terminal abstract operation cannot
/// compile until its observation class is chosen here.
pub fn reconstruct_psi_observation_model(unit: &PsiOptimizationUnit) -> PsiObservationModel {
    let mut nodes = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node, operation) in block.nodes.iter().enumerate() {
                let (events, crash, suspension) = operation_observations(&operation.operation);
                nodes.push(PsiNodeObservation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node).expect("optimization node index is u32"),
                    definitions: operation.definitions.clone(),
                    uses: operation.uses.clone(),
                    effect: operation.effect,
                    ownership: operation.ownership.clone(),
                    provenance: operation.provenance.clone(),
                    fuel: operation.fuel.clone(),
                    crash,
                    suspension,
                    events,
                });
            }
        }
    }
    PsiObservationModel {
        revision: unit.identity,
        nodes,
    }
}

fn operation_observations(
    operation: &O,
) -> (
    Vec<PsiObservableEvent>,
    ObservationKnowledge,
    ObservationKnowledge,
) {
    use ObservationEventClass as C;
    use ObservationKnowledge::{May, No, Yes};

    let event = |class| PsiObservableEvent {
        class,
        operation: operation.clone(),
    };
    match operation {
        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::ExactIntegerDivide { .. }
        | O::ExactIntegerRemainder { .. }
        | O::WrappingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::SaturatingIntegerMultiply { .. } => (Vec::new(), No, No),
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::BooleanStructuralField { .. } => (vec![event(C::StructuralState)], No, No),
        O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::Call { .. } => (vec![event(C::InternalCall)], May, May),
        O::BoundaryCall { .. } => (vec![event(C::BoundaryCall)], May, May),
        O::PortWrite { .. } => (vec![event(C::Service)], No, No),
        O::Jump { .. } | O::Conditional { .. } => (vec![event(C::ControlTransfer)], No, No),
        O::Return { .. } | O::ReturnUnit { .. } => (vec![event(C::NormalExit)], No, No),
        O::ReturnStructural { .. } => (
            vec![event(C::StructuralState), event(C::NormalExit)],
            No,
            No,
        ),
        O::Crash { .. } => (vec![event(C::CrashExit)], Yes, No),
    }
}
