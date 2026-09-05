use crate::{
    ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition, EntryClaim,
    MachineContract, Operation, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    TerminalMachineResult, TerminalRankedScc, Terminator, ValueDeclaration,
};
use semantic_vocabulary::{BlockId, MachineId, ServiceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachine {
    pub id: MachineId,
    /// Nominal type to which this machine is attached. An attached static
    /// machine need not have a runtime `self` parameter.
    pub attachment: Option<StructuralTypeId>,
    pub parameters: Vec<ValueDeclaration>,
    /// Ordered runtime structural parameters, separate from scalar values.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Canonical source-handle-free ranking evidence for the first admitted
    /// cyclic control component. Representation validation reconstructs the
    /// closed identity, guard, successor arithmetic, and exact structural-
    /// frontier preservation fixed point. Execution remains independently
    /// unavailable until interpreter, fuel, and native support land.
    pub ranked_scc: Option<TerminalRankedScc>,
    /// Unit carries no value; scalar results have a stable pseudo-value bound
    /// by every scalar return edge and available to `ensures`.
    pub result: TerminalMachineResult,
    /// Proof-visible roots for structural-place propositions. Runtime scalar
    /// parameters remain independently declared above.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Dense one-based machine-local claims present at entry, independent of
    /// content projections. Content claims below refine these identities when
    /// present.
    pub entry_claims: Vec<EntryClaim>,
    /// Strictly ordered normalized executable boundary-service ceiling. Public
    /// machines retain their authored ceiling; private machines and executable
    /// entries retain their exact checked inferred reach.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical machine-local identities for claims present at entry. These
    /// rows name content independently of any later output equality.
    pub content_entry_claims: Vec<ContentEntryClaim>,
    /// Canonical one-to-one claim mappings. These are semantic ownership facts,
    /// not authored algebra theorems: each exact projection below yields one
    /// verifier-reconstructed equality between `input` and `output`.
    pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
    /// Exact substitutions of already-authored partition theorems. These rows
    /// retain the source theorem and do not permit a producer to introduce a
    /// new `Separate` node in the derived equation.
    pub content_partition_compositions: Vec<ContentPartitionComposition>,
    pub entry: BlockId,
    pub blocks: Vec<Block>,
    pub contract: MachineContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}
