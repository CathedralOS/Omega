//! Immutable typed indexes used while reconstructing one terminal machine.
//!
//! Path-fact selection is a work filter, not an admission rule. Every call form
//! whose requirements can use a selected edge must enable the same successor
//! substitution and all-predecessor intersection, regardless of its result form.

use std::collections::BTreeMap;

use semantic_vocabulary::{BlockId, PropositionContext, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{Block, OperationKind, TerminalMachine, TerminalModule, Terminator};

pub(super) struct MachineReconstructionContext<'a> {
    pub(super) reconstruct_path_facts: bool,
    pub(super) value_types: BTreeMap<ValueId, ScalarType>,
    pub(super) blocks: BTreeMap<BlockId, &'a Block>,
    proposition_context: PropositionContext,
    machine: &'a TerminalMachine,
    dominators: std::cell::OnceCell<crate::control_graph::DominatorTree>,
}

impl<'a> MachineReconstructionContext<'a> {
    pub(super) fn new(
        module: &'a TerminalModule,
        machine: &'a TerminalMachine,
        crash_facts: bool,
    ) -> Self {
        let reconstruct_path_facts = (!crash_facts && module.scalar_block_invariants.iter().any(|invariant| invariant.machine == machine.id)) || matches!(machine.ranked_scc, Some(terminal_psi::TerminalRankedScc::Natural(_))) || machine.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ByteSequenceWrite { .. }
                        | OperationKind::ByteSequenceSubslice { .. }
                        | OperationKind::EstablishElementView { .. }
                        | OperationKind::ElementViewLength { .. }
                        | OperationKind::ElementViewRead { .. }
                        | OperationKind::ElementViewSubslice { .. }
                        | OperationKind::StructuralByteSequenceFieldStore { .. }
                        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallDynamicScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::IntegerExactCast { .. }
                        | OperationKind::ExactIntegerShiftLeft { .. }
                        | OperationKind::ExactIntegerShiftRight { .. }
                        | OperationKind::ExactIntegerAdd { .. }
                        | OperationKind::ExactIntegerSubtract { .. }
                        | OperationKind::ExactIntegerMultiply { .. }
                        | OperationKind::ExactIntegerDivide { .. }
                        | OperationKind::ExactIntegerRemainder { .. }
                        | OperationKind::WrappingIntegerDivide { .. }
                        | OperationKind::WrappingIntegerRemainder { .. }
                        | OperationKind::SaturatingIntegerDivide { .. }
                        | OperationKind::SaturatingIntegerRemainder { .. }
                )
            })
        }) || (crash_facts && machine.blocks.iter().any(|block| {
            matches!(&block.terminator, Terminator::Crash { site_guard, .. } if !site_guard.is_empty())
        }));
        let value_types = machine
            .parameters
            .iter()
            .chain(machine.result.scalar_ref())
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter()),
            )
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref())
            }))
            .map(|declaration| (declaration.id, declaration.scalar_type))
            .collect::<BTreeMap<_, _>>();
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        // The same context the machine's obligations are checked under, so a
        // generation-time certificate is decided against exactly the values
        // and places reconstruction may name.
        let proposition_context = crate::validation::machine_value_context(module, machine)
            .expect("validated module retains a consistent proposition context");
        Self {
            reconstruct_path_facts,
            value_types,
            blocks,
            proposition_context,
            machine,
            dominators: std::cell::OnceCell::new(),
        }
    }

    /// The machine-wide proposition context generation-time certificate
    /// checks run under.
    pub(super) fn proposition_context(&self) -> &PropositionContext {
        &self.proposition_context
    }

    /// Reuse full-graph dominance only when an observation needs it. Pure scalar
    /// reconstruction pays no additional graph-analysis cost.
    pub(super) fn dominators(&self) -> &crate::control_graph::DominatorTree {
        self.dominators
            .get_or_init(|| crate::control_graph::dominators(self.machine))
    }

    pub(super) fn value_term(&self, id: ValueId) -> ScalarTerm {
        ScalarTerm::value(
            id,
            *self
                .value_types
                .get(&id)
                .expect("validated module contains every referenced value"),
        )
    }
}
