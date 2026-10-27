//! Checker-owned carry facts. The declaration keeps the authored minimum;
//! this plan records the effective policy derived from the complete stored
//! shape so later liveness, runtime-admission, artifact, and model-export
//! passes never need to reinterpret source syntax.

use arena::{Arena, Handle, HandleSpan};
use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use std::borrow::Cow;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryFacts {
    /// One entry per data declaration, in declaration order.
    pub data: Vec<DataCarryFact>,
    /// Canonical call sites that may suspend, with the most restrictive
    /// four-axis policy contributed by every value live across that exact
    /// crossing. Activation planning consumes this checked result; it must not
    /// re-run syntax-shaped liveness.
    pub suspension_crossings: Vec<SuspensionCrossingCarryFact>,
    /// One envelope per machine over every typed storage slot and call value
    /// that may be live at an instruction boundary. Activation planning keeps
    /// only the CPU/thread preservation this envelope demands; fixed,
    /// nonmoving stack storage supplies address stability structurally.
    pub activation_wide_carry: Vec<MachineActivationCarryFact>,
    /// Effective carry policy retained by the exact linear claim identity.
    /// This is independent of the carrier's structural policy and survives
    /// path-indexed n-ary transformations through the ownership outcome map.
    pub claim_policies: Vec<ClaimCarryPolicyFact>,
    /// Current call-entry loans and the occurrences supplying or parenting
    /// them. These are preservation demands, not runtime backing receipts.
    pub suspension_loans: Arena<SuspensionCrossingLoanFact>,
    /// Per-value edges into the exact invocation's loan roster. Grouping
    /// records complete descriptor-leaf coverage, not parking permission.
    pub suspension_value_loans: Arena<Handle<SuspensionCrossingLoanFact>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimCarryPolicyFact {
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    pub effective: CarryPolicy,
    /// Number of independent qualification evidence origins intersected to
    /// produce `effective`. Each origin begins strict and is relaxed only by
    /// its own exact positive permissions.
    pub contributing_origins: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineActivationCarryFact {
    pub machine: SymbolHandle,
    /// Direct envelope of this machine's explicit storage and call values.
    /// Activation planning composes these over the checked call graph.
    pub effective: CarryPolicy,
    /// False when this machine has a call/value shape that
    /// could not be resolved to a checked type. Absence of a type is never
    /// treated as permissive evidence.
    pub analysis_complete: bool,
    /// Canonical type inputs contributing to the envelope, deduplicated by
    /// handle for artifact/debug consumers.
    pub contributing_types: Vec<typed_trees::types::TypeReferenceHandle>,
    /// Some transient reference-like values have no standalone type handle in
    /// typed expressions; their strict contribution remains explicit.
    pub unnamed_strict_values: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionCrossingCarryFact {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target: SymbolHandle,
    pub effective: CarryPolicy,
    pub live_values: Vec<SuspensionCrossingLiveValueFact>,
    pub borrows: SuspensionCrossingBorrowFrontier,
}

/// The borrow observations already produced for this invocation. Captured
/// actuals include their constructed children and evaluation epochs; keeping
/// that span avoids re-reading an argument after a later operand mutates it.
/// Empty loans do not establish that an incoming or unresolved carrier is
/// borrow-free. Admission still needs the independent lifetime/access join.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuspensionCrossingBorrowFrontier {
    pub loans: HandleSpan<SuspensionCrossingLoanFact>,
    pub operands: HandleSpan<crate::FlowOperandFact>,
}

/// One exact occurrence in a crossing's loan dependency closure. The loan row
/// owns its access, captured referent, carrier path, parent and supplying-loan
/// identities. A supplying loan transports custody; it is not a reborrow
/// parent. Neither edge is inferred from matching storage or source labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuspensionCrossingLoanFact {
    pub source: SuspensionCrossingLoanSource,
    /// False for a dependency retained solely by a current descendant. This
    /// does not reactivate an expired/replaced descriptor or restore access.
    pub active_at_call: bool,
    /// Selected by retained storage or its dependency closure, including
    /// backing needed only by the incomplete callee's captured operands.
    /// This demand neither reactivates a descriptor nor establishes access.
    pub retained_for_crossing: bool,
    pub backing: SuspensionCrossingLoanBacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspensionCrossingLoanSource {
    /// Current invocation-entry permission, not a loan of the parameter slot.
    Incoming {
        identity: Handle<crate::IncomingReferenceIdentity>,
        access: crate::BorrowAccessKind,
    },
    Stored {
        loan: Handle<crate::BorrowLoanFact>,
        resource: crate::CheckedParentBorrowResource,
    },
    CapturedActual {
        resource: Handle<crate::CheckedCallBorrowLoanResource>,
    },
}

impl Default for SuspensionCrossingLoanSource {
    fn default() -> Self {
        Self::Stored {
            loan: Handle::invalid(),
            resource: Default::default(),
        }
    }
}

impl SuspensionCrossingLoanFact {
    pub fn stored_loan(&self) -> Handle<crate::BorrowLoanFact> {
        match self.source {
            SuspensionCrossingLoanSource::Stored { loan, .. } => loan,
            SuspensionCrossingLoanSource::CapturedActual { .. }
            | SuspensionCrossingLoanSource::Incoming { .. } => Handle::invalid(),
        }
    }
}

/// Logical backing identity, kept separate from the descriptor containing a
/// loan. An incoming reference's opaque referent cannot become the parameter
/// slot merely because they have the same root symbol. Stack provisioning and
/// physical address stability remain runtime obligations, not these coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SuspensionCrossingLoanBacking {
    #[default]
    Unresolved,
    StateValue {
        origin: SuspensionCrossingValueOrigin,
    },
    IncomingReferent {
        identity: Handle<crate::IncomingReferenceIdentity>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionCrossingLiveValueFact {
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub storage: SuspensionCrossingStorage,
    /// Exact source coordinate for this value. The coordinate is retained by
    /// the checker because neither its storage class nor its type identifies a
    /// unique value at the crossing.
    pub origin: SuspensionCrossingValueOrigin,
    /// Complete live linear-claim identities attached to this exact place.
    /// An empty roster means no compiler-owned live claim was established; it
    /// must never be reconstructed from the type or storage class.
    pub claims: Vec<language_semantics::PermissionClaimIdentity>,
    /// Per-value policy after a born-strict claim is relaxed by the exact
    /// compiler-owned permissions still attached to this place. This may be
    /// stricter than the transparent carrier's structural policy.
    pub effective: CarryPolicy,
    /// Source checking groups the value's existing loan occurrences or
    /// captured actual once. Lowering consumes this join rather than guessing
    /// from its reference type or from equal backing coordinates.
    pub borrow_frontier: SuspensionValueBorrowFrontier,
}

/// Complete borrowed-leaf coverage by stored and captured occurrences.
/// Coverage alone does not prove lifetime/access/backing preservation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SuspensionValueBorrowFrontier {
    #[default]
    Unresolved,
    ViewFree,
    RetainedLoans {
        /// These edges name this call's retained occurrences, including their
        /// backing and resource join, rather than the historical loan arena.
        loans: HandleSpan<Handle<SuspensionCrossingLoanFact>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionCrossingValueOrigin {
    Parameter {
        symbol: SymbolHandle,
        /// Ordinal among the state's explicit parameters.
        position: usize,
    },
    Local {
        symbol: SymbolHandle,
        /// Source statement that establishes the local.
        statement_index: usize,
        /// Exact position in the checked scalar environment: explicit
        /// parameters followed by preceding local-data bindings.
        environment_position: usize,
    },
    CallArgument {
        /// Dense ordinary argument ordinal at the exact call coordinate.
        position: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionCrossingStorage {
    /// Entry/state parameter that must survive the parked continuation.
    Parameter,
    /// Lexical local that must survive the parked continuation.
    Local,
    /// Value materialized for the suspending call itself.
    CallArgument,
}

/// Reconstruct the established suspension-crossing identity used by Omega's
/// activation planning. Exact live-place and claim coordinates remain
/// separately replayable frontier data; extending that frontier does not
/// silently rename already-published crossing identities.
pub fn canonical_suspension_crossing_id(
    program: &typed_trees::TypedTrees,
    crossing: &SuspensionCrossingCarryFact,
) -> Option<semantic_vocabulary::SuspensionCrossingId> {
    let mut hash = StableCrossingHash::new();
    hash.byte(0x73);
    hash.string(&symbol_identity(program, crossing.machine)?);
    hash.string(&symbol_identity(program, crossing.state)?);
    hash.usize(crossing.statement_index);
    hash.usize(crossing.call_ordinal);
    hash.string(&symbol_identity(program, crossing.target)?);
    hash.policy(crossing.effective);
    for live in &crossing.live_values {
        hash.string(&program.normalized_type_identity_text(live.type_reference));
        hash.byte(match live.storage {
            SuspensionCrossingStorage::Parameter => 2,
            SuspensionCrossingStorage::Local => 3,
            SuspensionCrossingStorage::CallArgument => 4,
        });
        hash.policy(live.effective);
    }
    semantic_vocabulary::SuspensionCrossingId::new(hash.finish())
}

fn symbol_identity(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<Cow<'_, str>> {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return Some(crossing_declaration_name(
                program,
                symbol,
                machine.name.as_str(),
            ));
        }
        if let Some(state) = program.state_in_machine(machine, symbol) {
            return Some(crossing_declaration_name(
                program,
                symbol,
                state.name.as_str(),
            ));
        }
    }
    // A boundary call names the requirement, not its selected implementation.
    // These declarations have no machine/state row. Use their complete logical
    // path even without a module so two owners' equally named requirements do
    // not share a crossing identity. Existing machine/state identities above
    // retain their published spelling.
    program
        .trait_machine_signature(symbol)
        .filter(|(definition, _)| definition.is_boundary)
        .map(|_| Cow::Owned(program.symbols.display_path(symbol, "::")))
}

fn crossing_declaration_name<'program>(
    program: &'program typed_trees::TypedTrees,
    symbol: SymbolHandle,
    legacy_name: &'program str,
) -> Cow<'program, str> {
    // Preserve published identities for declarations without a module; new
    // module-owned crossings require their complete logical declaration path.
    if program.symbols.symbol_module(symbol).is_valid() {
        Cow::Owned(program.symbols.display_path(symbol, "::"))
    } else {
        Cow::Borrowed(legacy_name)
    }
}

struct StableCrossingHash(semantic_vocabulary::Fnv1a);

impl StableCrossingHash {
    fn new() -> Self {
        Self(semantic_vocabulary::Fnv1a::new())
    }

    fn byte(&mut self, byte: u8) {
        self.0.byte(byte);
    }

    fn string(&mut self, value: &str) {
        self.0.string_terminated(value);
    }

    fn usize(&mut self, value: usize) {
        self.0.u64(value as u64);
    }

    fn policy(&mut self, policy: CarryPolicy) {
        self.byte(u8::from(policy.suspension == CarrySuspension::Allowed));
        self.byte(u8::from(policy.cpu == CarryCpu::Origin));
        self.byte(u8::from(policy.host_thread == CarryHostThread::Origin));
        self.byte(match policy.address {
            CarryAddress::Movable => 1,
            CarryAddress::Stable => 2,
        });
    }

    fn finish(self) -> u64 {
        self.0.finish().max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataCarryFact {
    pub data: SymbolHandle,
    /// The optional authored minimum promise retained for diagnostics and
    /// published-contract work. It is not the effective policy.
    pub declared: Option<CarryPolicy>,
    /// The checker-derived policy for this transparent stored shape.
    pub effective: CarryPolicy,
}
