//! Ranked control graph and the semantic evidence required for replay.

use crate::AbstractOperationPlan;
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, ObligationId, OperationId, PlaceId,
    Proposition, ValueId,
};
use terminal_psi::{
    StructuralMultiplicity, StructuralPathSegment, TerminalModule, TerminalPsiIdentity,
    TerminalRankedScc,
};

/// Native-ranked admission kept beside, rather than inside, the ordinary
/// abstract-operation plan. Existing acyclic plan and function constructors
/// therefore remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedNativeAbstractOperationPlan {
    pub plan: AbstractOperationPlan,
    pub countdown: RankedU32CountdownCustody,
}

/// Exact source-free custody for the first native-ranked control slice.
///
/// The SCC owns rank and covered-edge meaning. `graph` retains the remaining
/// concrete operation/edge coordinates needed for later lowering to replay
/// that meaning without recognizing source syntax. Canonical semantic/proof
/// bytes and complete relevant frontier projections are data, not authority:
/// the object boundary re-runs native and fixed-fuel verification over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedU32CountdownCustody {
    pub semantic_replay: TerminalModule,
    pub proof_replay: Vec<u8>,
    pub ranked_scc: TerminalRankedScc,
    pub fixed_fuel: RankedFixedEntryFuel,
    pub graph: RankedU32CountdownGraph,
    pub structural_frontiers: RankedMachineStructuralFrontiers,
}

/// Exact public projection of the independently derived ranked entry theorem.
/// This record is intentionally constructible data; consumers acquire
/// authority only by deriving the semantic certificate again and comparing
/// every field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedFixedEntryFuel {
    pub terminal_psi: TerminalPsiIdentity,
    pub schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    pub relevant_preconditions: Vec<Proposition>,
    pub ceiling_units: u64,
}

impl RankedFixedEntryFuel {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedOwnedStructuralPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedPartialStructuralCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedStructuralOwnershipFrontier {
    pub claims: Vec<RankedLiveClaim>,
    pub owned_places: Vec<RankedOwnedStructuralPlace>,
    pub partial_custody: Vec<RankedPartialStructuralCustody>,
}

impl RankedStructuralOwnershipFrontier {
    pub fn claims(&self) -> &[RankedLiveClaim] {
        &self.claims
    }

    pub fn owned_places(&self) -> &[RankedOwnedStructuralPlace] {
        &self.owned_places
    }

    pub fn partial_custody(&self) -> &[RankedPartialStructuralCustody] {
        &self.partial_custody
    }
}

/// The two ownership snapshots relevant to the one admitted ranked backedge.
/// Coordinates are retained explicitly so unrelated block/edge queries fail
/// closed instead of aliasing either snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedMachineStructuralFrontiers {
    pub machine: MachineId,
    pub header: BlockId,
    pub backedge: EdgeId,
    pub header_entry: RankedStructuralOwnershipFrontier,
    pub backedge_exit: RankedStructuralOwnershipFrontier,
}

impl RankedMachineStructuralFrontiers {
    pub fn block_entry(&self, block: BlockId) -> Option<&RankedStructuralOwnershipFrontier> {
        (block == self.header).then_some(&self.header_entry)
    }

    pub fn edge_exit(&self, edge: EdgeId) -> Option<&RankedStructuralOwnershipFrontier> {
        (edge == self.backedge).then_some(&self.backedge_exit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedU32CountdownGraph {
    pub entry: BlockId,
    pub preheader_edge: EdgeId,
    pub initial_value: ValueId,
    pub zero_operation: OperationId,
    pub zero_value: ValueId,
    pub compare_operation: OperationId,
    pub false_exit_edge: EdgeId,
    pub done_block: BlockId,
    pub one_operation: OperationId,
    pub one_value: ValueId,
    pub subtract_operation: OperationId,
    pub subtract_obligation: ObligationId,
    pub return_edge: EdgeId,
}
