//! STR4 checked plans (machine_taxonomy.md): the normalized MACHINE
//! SEMANTIC CONTRACT, independent of syntax and lowering -- component
//! manifests, proof artifacts, provider admission, and hot-swap checks
//! reference this identity, never re-derived booleans. The checked public
//! axes publish independently; this carrier retains the remaining contract
//! plans plus their domain-separated commitment. Its compact FNV projection
//! is report/cache data only and still incorporates the published supply and
//! canonical service reach.
//! Prover-independence (acceptance 8: a stronger prover cannot change an
//! exported contract ID) holds BY CONSTRUCTION: only declared/published
//! halves enter either projection, never inferred rows or witnesses.

use psi_language_semantics::{
    BlockingInterface, MachineSupplyMode, SuspensionInterface, SynchronousInvocationInterface,
    TerminationGuarantee, TerminationInterface,
};
use psi_numerics::literals::IntegerLiteral;
use psi_symbols::SymbolHandle;
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, hash::Hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrashInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

/// Source-independent checked syntax retained for one guarded crash route.
///
/// This is intentionally parameter-relative rather than tied to terminal
/// `ValueId`s. The terminal producer assigns those identities and lowers the
/// supported scalar subset into `psi_core::Proposition`; syntax outside that
/// subset remains explicit and fails closed there.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashPredicateExpression {
    Invalid,
    Binary {
        operator: u8,
        left: Box<Self>,
        right: Box<Self>,
    },
    Unary {
        operator: u8,
        operand: Box<Self>,
    },
    Integer(String),
    Boolean(bool),
    Name(Vec<String>),
    Member {
        receiver: Box<Self>,
        member: String,
    },
    Call {
        target: String,
        receiver: Box<Self>,
        arguments: Vec<Self>,
    },
    Opaque(String),
    Parameter(u32),
    ContentConservation(Vec<u8>),
}

impl CrashPredicateExpression {
    pub fn substitute(&self, arguments: &[Option<Self>]) -> Self {
        match self {
            Self::Parameter(index) => arguments
                .get(*index as usize)
                .and_then(Clone::clone)
                .unwrap_or_else(|| self.clone()),
            Self::Binary {
                operator,
                left,
                right,
            } => Self::Binary {
                operator: *operator,
                left: Box::new(left.substitute(arguments)),
                right: Box::new(right.substitute(arguments)),
            },
            Self::Unary { operator, operand } => Self::Unary {
                operator: *operator,
                operand: Box::new(operand.substitute(arguments)),
            },
            Self::Member { receiver, member } => Self::Member {
                receiver: Box::new(receiver.substitute(arguments)),
                member: member.clone(),
            },
            Self::Call {
                target,
                receiver,
                arguments: nested,
            } => Self::Call {
                target: target.clone(),
                receiver: Box::new(receiver.substitute(arguments)),
                arguments: nested
                    .iter()
                    .map(|argument| argument.substitute(arguments))
                    .collect(),
            },
            _ => self.clone(),
        }
    }

    pub fn boolean_value(&self) -> Option<bool> {
        use psi_typed_trees::expression::{BinaryOperator, UnaryOperator};

        match self {
            Self::Boolean(value) => Some(*value),
            Self::Unary { operator, operand } if *operator == UnaryOperator::LogicalNot as u8 => {
                operand.boolean_value().map(|value| !value)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::And as u8 => {
                Some(left.boolean_value()? && right.boolean_value()?)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::Or as u8 => {
                Some(left.boolean_value()? || right.boolean_value()?)
            }
            _ => None,
        }
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::Invalid => out.push(0),
            Self::Binary {
                operator,
                left,
                right,
            } => {
                out.push(1);
                out.push(*operator);
                left.write_canonical(out);
                right.write_canonical(out);
            }
            Self::Unary { operator, operand } => {
                out.push(2);
                out.push(*operator);
                operand.write_canonical(out);
            }
            Self::Integer(value) => {
                out.push(3);
                out.extend(value.as_bytes());
                out.push(0);
            }
            Self::Boolean(value) => {
                out.push(4);
                out.push(u8::from(*value));
            }
            Self::Name(members) => {
                out.push(5);
                for member in members {
                    out.extend(member.as_bytes());
                    out.push(b'.');
                }
                out.push(0);
            }
            Self::Member { receiver, member } => {
                out.push(6);
                receiver.write_canonical(out);
                out.extend(member.as_bytes());
                out.push(0);
            }
            Self::Call {
                target,
                receiver,
                arguments,
            } => {
                out.push(7);
                out.extend(target.as_bytes());
                out.push(0);
                receiver.write_canonical(out);
                for argument in arguments {
                    argument.write_canonical(out);
                }
                out.push(0xfe);
            }
            Self::Opaque(display) => {
                out.push(8);
                out.extend(display.as_bytes());
                out.push(0);
            }
            Self::Parameter(index) => {
                out.push(9);
                out.extend(index.to_le_bytes());
            }
            Self::ContentConservation(bytes) => {
                out.push(0xcc);
                out.extend(bytes);
            }
        }
    }
}

/// Source-independent identity plus the checked syntax that produced it. The
/// canonical bytes remain the equality/hash material used by checked joins;
/// syntax is retained solely so later semantic lowering need not interpret
/// those identity bytes as executable meaning.
#[derive(Debug, Clone)]
pub struct CrashPredicateIdentity {
    canonical_bytes: Vec<u8>,
    expression: Option<CrashPredicateExpression>,
    scalar_expression: Option<crate::CheckedBooleanExpression>,
}

impl CrashPredicateIdentity {
    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Self {
        Self {
            canonical_bytes: bytes,
            expression: None,
            scalar_expression: None,
        }
    }

    pub fn from_expression(expression: CrashPredicateExpression) -> Self {
        let mut canonical_bytes = vec![1]; // ProofFact::Expression
        expression.write_canonical(&mut canonical_bytes);
        Self {
            canonical_bytes,
            expression: Some(expression),
            scalar_expression: None,
        }
    }

    pub fn from_expression_and_scalar(
        expression: CrashPredicateExpression,
        scalar_expression: crate::CheckedBooleanExpression,
    ) -> Self {
        let mut identity = Self::from_expression(expression);
        identity.scalar_expression = Some(scalar_expression);
        identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn expression(&self) -> Option<&CrashPredicateExpression> {
        self.expression.as_ref()
    }

    pub const fn scalar_expression(&self) -> Option<&crate::CheckedBooleanExpression> {
        self.scalar_expression.as_ref()
    }
}

impl PartialEq for CrashPredicateIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_bytes == other.canonical_bytes
    }
}

impl Eq for CrashPredicateIdentity {}

impl PartialOrd for CrashPredicateIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CrashPredicateIdentity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical_bytes.cmp(&other.canonical_bytes)
    }
}

impl Hash for CrashPredicateIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical_bytes.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    /// The canonical route contributed by a route-less clause or an authored
    /// `true` route. It subsumes every guarded alternative in its bucket.
    Truth,
    Predicate(CrashPredicateIdentity),
}

/// Dense, one-based identity of a canonical published route bucket within one
/// machine's crash plan. Bucket normalization happens before these identities
/// are assigned, so clause regrouping and duplicate routes cannot renumber
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucketId(u32);

impl CrashRouteBucketId {
    fn from_index(index: usize) -> Self {
        Self(
            u32::try_from(index)
                .expect("published crash bucket count exceeds checked identity range")
                .checked_add(1)
                .expect("published crash bucket identity is one-based"),
        )
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    fn index(self) -> Option<usize> {
        usize::try_from(self.0.checked_sub(1)?).ok()
    }
}

/// Source-handle-free location of one crash transition within a checked
/// machine body. State identity plus the statement's state-local ordinal is
/// stable against unrelated statement-arena insertions and is sufficient for
/// checked-tree consumers to join the derived site back to its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashSiteLocation {
    state: SymbolHandle,
    statement_ordinal: u32,
}

/// Source-handle-free identity of one invocation within a checked machine
/// body. This deliberately reuses the flow layer's state/statement/call
/// coordinates so later crash propagation never has to rediscover a source
/// expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashCallSiteLocation {
    state: SymbolHandle,
    statement_ordinal: u32,
    call_ordinal: u32,
}

impl CrashCallSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32, call_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
            call_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }

    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }
}

impl CrashSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }
}

/// Body-derived seed for a crash-terminator plan. Structurally unconditional
/// guard coverage is attached immediately; path-conditioned entailment and
/// frontier reconstruction remain independent later passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashSite {
    location: CrashSiteLocation,
    cause: CrashCause,
    /// Exact canonical predicates known to hold on every path into this site.
    /// Their conjunction is the retained derived path guard; implication
    /// consequences remain separate coverage evidence.
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    /// Sound canonical consequences of the exact incoming conjunction. These
    /// witness guarded-route coverage without rewriting the exact guard.
    path_guard_consequences: Vec<CrashPredicateIdentity>,
    /// Published buckets whose guard implication is already established for
    /// this site.
    guard_covering_buckets: Vec<CrashRouteBucketId>,
    /// Stable identities of claims proved live at this exact machine-local
    /// crash site. This is deliberately a lower bound: conditionally live sum
    /// payloads and obligations outside this activation are absent until a
    /// later analysis can prove their membership.
    frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
}

impl CheckedCrashSite {
    pub fn new(
        location: CrashSiteLocation,
        cause: CrashCause,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
        mut frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        Self {
            location,
            cause,
            path_guard_conjuncts: Vec::new(),
            path_guard_consequences: Vec::new(),
            guard_covering_buckets,
            frontier_lower_bound,
        }
    }

    pub const fn location(&self) -> CrashSiteLocation {
        self.location
    }

    pub const fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn with_guard_covering_buckets(
        mut self,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        self.guard_covering_buckets = guard_covering_buckets;
        self
    }

    pub fn with_path_guard_conjuncts(
        mut self,
        mut path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_conjuncts.sort();
        path_guard_conjuncts.dedup();
        self.path_guard_conjuncts = path_guard_conjuncts;
        self
    }

    pub fn with_path_guard_consequences(
        mut self,
        mut path_guard_consequences: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_consequences.sort();
        path_guard_consequences.dedup();
        self.path_guard_consequences = path_guard_consequences;
        self
    }

    pub fn with_frontier_lower_bound(
        mut self,
        mut frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        self.frontier_lower_bound = frontier_lower_bound;
        self
    }

    pub fn guard_covering_buckets(&self) -> &[CrashRouteBucketId] {
        &self.guard_covering_buckets
    }

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_consequences
    }

    pub fn frontier_lower_bound(&self) -> &[psi_language_semantics::PermissionClaimIdentity] {
        &self.frontier_lower_bound
    }
}

/// Invocation-specific refinement of a selected callee crash summary. The
/// summary may be a published ceiling or conservative same-unit checked-body
/// evidence. `surviving_buckets` are already expressed in the caller's
/// canonical scalar value namespace, including direct caller-local arguments.
/// `target_machine` plus `target_contract_commitment` pins the exact
/// parameter-relative route origin when terminal control must bind a staged
/// argument value directly rather than reverse-matching caller expressions.
/// The compact fingerprint remains a compatibility/report coordinate only.
/// Exact incoming conjuncts remain distinct
/// from the sound structural consequences used by ceiling coverage. An empty
/// surviving set is meaningful evidence that the selected summary is
/// crash-free at this invocation, so such records are retained rather than
/// elided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashCallSite {
    location: CrashCallSiteLocation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_report_fingerprint: u64,
    target_contract_commitment: MachineContractCommitment,
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    path_guard_consequences: Vec<CrashPredicateIdentity>,
    surviving_buckets: Vec<CrashRouteBucket>,
}

/// Source-independent published envelope for a callable requirement that has
/// no local `MachineContractPlan`. The commitment pins the complete normalized
/// callable contract; the fingerprint is a compatibility/report coordinate.
/// Independent operational axes and the crash ceiling stay directly queryable
/// without reopening the authored signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashContractCapsule {
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_report_fingerprint: u64,
    target_contract_commitment: MachineContractCommitment,
    published_service_reach: Vec<String>,
    published_synchronous_invocations: Vec<String>,
    published_may_suspend: bool,
    published_may_block: bool,
    published_termination: TerminationGuarantee,
    published_buckets: Vec<CrashRouteBucket>,
}

impl CrashContractCapsule {
    pub fn new(
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        published_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        Self::new_with_commitment(
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            MachineContractCommitment::from_digest([0; 32]),
            published_buckets,
        )
    }

    pub fn new_with_commitment(
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: MachineContractCommitment,
        mut published_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        published_buckets.sort();
        published_buckets.dedup();
        Self {
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            published_service_reach: Vec::new(),
            published_synchronous_invocations: Vec::new(),
            published_may_suspend: false,
            published_may_block: false,
            published_termination: TerminationGuarantee::NoGuarantee,
            published_buckets,
        }
    }

    pub fn with_operational_envelope(
        mut self,
        mut published_service_reach: Vec<String>,
        mut published_synchronous_invocations: Vec<String>,
        published_may_suspend: bool,
        published_may_block: bool,
        published_termination: TerminationGuarantee,
    ) -> Self {
        published_service_reach.sort();
        published_service_reach.dedup();
        published_synchronous_invocations.sort();
        published_synchronous_invocations.dedup();
        self.published_service_reach = published_service_reach;
        self.published_synchronous_invocations = published_synchronous_invocations;
        self.published_may_suspend = published_may_suspend;
        self.published_may_block = published_may_block;
        self.published_termination = published_termination;
        self
    }

    pub const fn target_machine(&self) -> SymbolHandle {
        self.target_machine
    }

    pub const fn target_state(&self) -> SymbolHandle {
        self.target_state
    }

    pub const fn target_contract_report_fingerprint(&self) -> u64 {
        self.target_contract_report_fingerprint
    }

    pub const fn target_contract_commitment(&self) -> MachineContractCommitment {
        self.target_contract_commitment
    }

    pub fn published_buckets(&self) -> &[CrashRouteBucket] {
        &self.published_buckets
    }

    pub fn published_service_reach(&self) -> &[String] {
        &self.published_service_reach
    }

    pub fn published_synchronous_invocations(&self) -> &[String] {
        &self.published_synchronous_invocations
    }

    pub const fn published_may_suspend(&self) -> bool {
        self.published_may_suspend
    }

    pub const fn published_may_block(&self) -> bool {
        self.published_may_block
    }

    pub const fn published_termination(&self) -> &TerminationGuarantee {
        &self.published_termination
    }
}

impl CheckedCrashCallSite {
    pub fn new(
        location: CrashCallSiteLocation,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        surviving_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        Self::new_with_commitment(
            location,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            MachineContractCommitment::from_digest([0; 32]),
            surviving_buckets,
        )
    }

    pub fn new_with_commitment(
        location: CrashCallSiteLocation,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: MachineContractCommitment,
        mut surviving_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        surviving_buckets.sort();
        surviving_buckets.dedup();
        Self {
            location,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            path_guard_conjuncts: Vec::new(),
            path_guard_consequences: Vec::new(),
            surviving_buckets,
        }
    }

    pub const fn location(&self) -> CrashCallSiteLocation {
        self.location
    }

    pub const fn target_machine(&self) -> SymbolHandle {
        self.target_machine
    }

    pub const fn target_state(&self) -> SymbolHandle {
        self.target_state
    }

    pub const fn target_contract_report_fingerprint(&self) -> u64 {
        self.target_contract_report_fingerprint
    }

    pub const fn target_contract_commitment(&self) -> MachineContractCommitment {
        self.target_contract_commitment
    }

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_consequences
    }

    pub fn surviving_buckets(&self) -> &[CrashRouteBucket] {
        &self.surviving_buckets
    }

    pub fn with_path_guard_conjuncts(
        mut self,
        mut path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_conjuncts.sort();
        path_guard_conjuncts.dedup();
        self.path_guard_conjuncts = path_guard_conjuncts;
        self
    }

    pub fn with_path_guard_consequences(
        mut self,
        mut path_guard_consequences: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_consequences.sort();
        path_guard_consequences.dedup();
        self.path_guard_consequences = path_guard_consequences;
        self
    }
}

fn crash_frontier_claim_sort_key(
    identity: psi_language_semantics::PermissionClaimIdentity,
) -> [u64; 11] {
    use psi_language_semantics::{PermissionClaimIdentity, PermissionEventSource};

    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = identity
    else {
        return [0; 11];
    };
    let mut key = [0; 11];
    key[0] = 1;
    key[1] = u64::from(machine_symbol.arena_index());
    key[2] = u64::from(machine_symbol.generation());
    key[3] = u64::from(state_symbol.arena_index());
    key[4] = u64::from(state_symbol.generation());
    match source {
        PermissionEventSource::StateEntry => key[5] = 0,
        PermissionEventSource::Statement { statement_index } => {
            key[5] = 1;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            key[5] = 2;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
            key[7] = u64::try_from(call_ordinal).unwrap_or(u64::MAX);
            key[8] = u64::from(target_symbol.arena_index());
            key[9] = u64::from(target_symbol.generation());
        }
        PermissionEventSource::StateExit => key[5] = 3,
    }
    key[10] = u64::from(ordinal);
    key
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    cause: CrashCause,
    /// Canonical nonempty set. `Truth` is always the sole entry when present.
    alternative_guards: Vec<CrashRouteGuard>,
}

impl CrashRouteBucket {
    pub fn new(cause: CrashCause, mut alternative_guards: Vec<CrashRouteGuard>) -> Option<Self> {
        if alternative_guards.contains(&CrashRouteGuard::Truth) {
            alternative_guards = vec![CrashRouteGuard::Truth];
        } else {
            alternative_guards.sort();
            alternative_guards.dedup();
        }
        (!alternative_guards.is_empty()).then(|| Self {
            cause,
            alternative_guards,
        })
    }

    pub fn unconditional(cause: CrashCause) -> Self {
        Self::new(cause, vec![CrashRouteGuard::Truth])
            .expect("the unconditional crash bucket has one canonical guard")
    }

    pub fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn alternative_guards(&self) -> &[CrashRouteGuard] {
        &self.alternative_guards
    }

    pub fn is_unconditional(&self) -> bool {
        self.alternative_guards == [CrashRouteGuard::Truth]
    }
}

/// The published and body-derived halves of CRASH-CONTRACT remain independent:
/// published route buckets are contract identity, while checked sites are
/// implementation evidence and never enter that fingerprint. Path guards,
/// complete covering buckets, and frontier lower bounds enrich
/// the site layer without changing the published interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrashPlan {
    interface: CrashInterface,
    published: Vec<CrashRouteBucket>,
    /// Complete source-independent lowering of the machine and entry-state
    /// `requires` package into the bounded structural scalar vocabulary.
    /// `None` means at least one authored requirement is outside that
    /// vocabulary; consumers must not publish a partial terminal contract.
    structural_runtime_requirements: Option<Vec<crate::CheckedBooleanExpression>>,
    checked_sites: Vec<CheckedCrashSite>,
    checked_calls: Vec<CheckedCrashCallSite>,
}

impl CrashPlan {
    pub fn published_ceiling(mut published: Vec<CrashRouteBucket>) -> Self {
        published.sort();
        published.dedup();
        Self {
            interface: CrashInterface::PublishedCeiling,
            published,
            structural_runtime_requirements: None,
            checked_sites: Vec::new(),
            checked_calls: Vec::new(),
        }
    }

    pub fn with_structural_runtime_requirements(
        mut self,
        requirements: Option<Vec<crate::CheckedBooleanExpression>>,
    ) -> Self {
        self.structural_runtime_requirements = requirements;
        self
    }

    pub fn structural_runtime_requirements(&self) -> Option<&[crate::CheckedBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }

    /// Whether a published structural crash predicate contains proof-gated
    /// arithmetic whose safety may depend on the complete retained runtime
    /// requirement package rather than self-proving literals alone.
    pub fn uses_structural_proof_gated_arithmetic(&self) -> bool {
        fn scalar_uses_proof_gated_arithmetic(expression: &crate::CheckedScalarExpression) -> bool {
            match expression {
                crate::CheckedScalarExpression::IntegerBinary {
                    kind, left, right, ..
                } => {
                    let exact = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactDivide
                            | crate::CheckedIntegerBinaryKind::ExactRemainder
                    );
                    let policy = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::WrappingDivide
                            | crate::CheckedIntegerBinaryKind::WrappingRemainder
                            | crate::CheckedIntegerBinaryKind::SaturatingDivide
                            | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                    );
                    let runtime_divisor = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactDivide
                            | crate::CheckedIntegerBinaryKind::ExactRemainder
                            | crate::CheckedIntegerBinaryKind::WrappingDivide
                            | crate::CheckedIntegerBinaryKind::WrappingRemainder
                            | crate::CheckedIntegerBinaryKind::SaturatingDivide
                            | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                    ) && !matches!(
                        right.as_ref(),
                        crate::CheckedScalarExpression::IntegerLiteral { literal }
                            if literal.landing().is_some_and(|landing| {
                                if landing.landed_type.is_signed() {
                                    literal.value_i64().is_some_and(|value| {
                                        value != 0 && (policy || (exact && value != -1))
                                    })
                                } else {
                                    literal.value_u64().is_some_and(|value| value != 0)
                                }
                            })
                    );
                    let exact_shift = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactShiftLeft
                            | crate::CheckedIntegerBinaryKind::ExactShiftRight
                    );
                    runtime_divisor
                        || exact_shift
                        || scalar_uses_proof_gated_arithmetic(left)
                        || scalar_uses_proof_gated_arithmetic(right)
                }
                crate::CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
                | crate::CheckedScalarExpression::IntegerWiden { operand, .. }
                | crate::CheckedScalarExpression::IntegerExactCast { operand, .. } => {
                    scalar_uses_proof_gated_arithmetic(operand)
                }
                crate::CheckedScalarExpression::Boolean(expression) => {
                    boolean_uses_proof_gated_arithmetic(expression)
                }
                _ => false,
            }
        }

        fn boolean_uses_proof_gated_arithmetic(
            expression: &crate::CheckedBooleanExpression,
        ) -> bool {
            match expression {
                crate::CheckedBooleanExpression::Not(operand) => {
                    boolean_uses_proof_gated_arithmetic(operand)
                }
                crate::CheckedBooleanExpression::Equal { left, right }
                | crate::CheckedBooleanExpression::And { left, right }
                | crate::CheckedBooleanExpression::Or { left, right } => {
                    boolean_uses_proof_gated_arithmetic(left)
                        || boolean_uses_proof_gated_arithmetic(right)
                }
                crate::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                    scalar_uses_proof_gated_arithmetic(left)
                        || scalar_uses_proof_gated_arithmetic(right)
                }
                _ => false,
            }
        }

        self.published.iter().any(|bucket| {
            bucket.alternative_guards().iter().any(|guard| {
                matches!(guard, CrashRouteGuard::Predicate(predicate)
                    if predicate.scalar_expression().is_some_and(boolean_uses_proof_gated_arithmetic))
            })
        })
    }

    pub fn with_checked_sites(mut self, mut checked_sites: Vec<CheckedCrashSite>) -> Option<Self> {
        checked_sites.sort_by_key(|site| {
            (
                site.location.state.arena_index(),
                site.location.state.generation(),
                site.location.statement_ordinal,
                site.cause,
            )
        });
        checked_sites.dedup();
        if checked_sites.windows(2).any(|sites| {
            sites[0].location.state == sites[1].location.state
                && sites[0].location.statement_ordinal == sites[1].location.statement_ordinal
        }) {
            return None;
        }
        if checked_sites.iter().any(|site| {
            site.guard_covering_buckets.iter().any(|bucket| {
                self.published_bucket(*bucket)
                    .is_none_or(|published| published.cause != site.cause)
            }) || site.frontier_lower_bound.iter().any(|identity| {
                *identity == psi_language_semantics::PermissionClaimIdentity::Unknown
            })
        }) {
            return None;
        }
        self.checked_sites = checked_sites;
        Some(self)
    }

    pub fn interface(&self) -> CrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[CrashRouteBucket] {
        &self.published
    }

    pub fn published_with_ids(
        &self,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &CrashRouteBucket)> {
        self.published
            .iter()
            .enumerate()
            .map(|(index, bucket)| (CrashRouteBucketId::from_index(index), bucket))
    }

    pub fn published_bucket(&self, id: CrashRouteBucketId) -> Option<&CrashRouteBucket> {
        self.published.get(id.index()?)
    }

    pub fn checked_sites(&self) -> &[CheckedCrashSite] {
        &self.checked_sites
    }

    pub fn with_checked_calls(
        mut self,
        mut checked_calls: Vec<CheckedCrashCallSite>,
    ) -> Option<Self> {
        checked_calls.sort_by_key(|call| {
            (
                call.location.state.arena_index(),
                call.location.state.generation(),
                call.location.statement_ordinal,
                call.location.call_ordinal,
            )
        });
        checked_calls.dedup();
        if checked_calls
            .windows(2)
            .any(|calls| calls[0].location == calls[1].location)
        {
            return None;
        }
        self.checked_calls = checked_calls;
        Some(self)
    }

    pub fn checked_calls(&self) -> &[CheckedCrashCallSite] {
        &self.checked_calls
    }

    pub fn checked_call_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        call_ordinal: u32,
    ) -> Option<&CheckedCrashCallSite> {
        self.checked_calls.iter().find(|call| {
            call.location.state == state
                && call.location.statement_ordinal == statement_ordinal
                && call.location.call_ordinal == call_ordinal
        })
    }

    /// Published buckets whose guards cover this checked body site.
    pub fn covering_buckets_for_site<'plan>(
        &'plan self,
        site: &'plan CheckedCrashSite,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &'plan CrashRouteBucket)> + 'plan {
        site.guard_covering_buckets.iter().filter_map(move |id| {
            self.published_bucket(*id)
                .and_then(|bucket| (bucket.cause == site.cause).then_some((*id, bucket)))
        })
    }

    pub fn checked_site_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedCrashSite> {
        self.checked_sites.iter().find(|site| {
            site.location.state == state && site.location.statement_ordinal == statement_ordinal
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineContractPlans {
    /// One entry per machine, in machine order.
    pub machines: Vec<MachineContractPlan>,
    /// Trait requirements and compile-time machine-parameter contracts do not
    /// own local machine plans. Their normalized callable identity and crash
    /// projection live here for modular call-site selection.
    pub crash_capsules: Vec<CrashContractCapsule>,
    /// Checked implementation axes assembled under the same exact machine
    /// identity as `machines`. Published requirement capsules remain separate;
    /// this row is the narrower realized endpoint used by callback admission.
    pub realized_envelopes: Vec<RealizedMachineContractEnvelope>,
}

impl MachineContractPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn crash_capsule(
        &self,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
    ) -> Option<&CrashContractCapsule> {
        self.crash_capsules.iter().find(|capsule| {
            capsule.target_machine == target_machine && capsule.target_state == target_state
        })
    }

    pub fn realized_envelope(
        &self,
        machine: SymbolHandle,
    ) -> Option<&RealizedMachineContractEnvelope> {
        self.realized_envelopes
            .iter()
            .find(|envelope| envelope.machine == machine)
    }

    pub fn resource_envelope(
        &self,
        machine: SymbolHandle,
        entry: SymbolHandle,
    ) -> Option<&CheckedEntryResourceEnvelope> {
        self.realized_envelope(machine)
            .and_then(|envelope| envelope.resources.get(entry))
    }

    /// Independently replay the checked resource anchor for every exact
    /// machine contract and compare it with the retained realized row.
    ///
    /// This deliberately rederives only the structural obligations available
    /// at checked-Psi time. It does not accept a stack number, a fuel number,
    /// a target footprint, or an installation receipt as an input.
    pub fn validate_resource_envelopes(&self) -> Result<(), &'static str> {
        for contract in &self.machines {
            if contract.report_fingerprint == 0 {
                return Err(
                    "checked resource envelope requires a nonzero contract report coordinate",
                );
            }
            if contract.commitment.is_zero() {
                return Err(
                    "checked resource envelope requires a nonzero machine contract commitment",
                );
            }
            let mut matching = self
                .realized_envelopes
                .iter()
                .filter(|envelope| envelope.machine == contract.machine);
            let Some(realized) = matching.next() else {
                return Err("checked resource envelope is missing its exact realized machine row");
            };
            if matching.next().is_some() {
                return Err("checked resource envelope has duplicate realized machine rows");
            }
            if realized.contract_report_fingerprint != contract.report_fingerprint
                || realized.contract_commitment != contract.commitment
            {
                return Err(
                    "checked resource envelope contract identity disagrees with its realized machine row",
                );
            }
            realized.resources.validate()?;
            if realized.resources.machine != contract.machine
                || realized.resources.contract_report_fingerprint != contract.report_fingerprint
                || realized.resources.contract_commitment != contract.commitment
            {
                return Err(
                    "checked resource roster does not bind its exact realized machine contract",
                );
            }
        }
        if self.realized_envelopes.iter().any(|realized| {
            !self
                .machines
                .iter()
                .any(|contract| contract.machine == realized.machine)
        }) {
            return Err("checked resource envelope retained an unknown realized machine row");
        }
        Ok(())
    }
}

/// Exact downstream derivation still required for one checked resource axis.
///
/// These are positive structural obligations, not zero-valued resource
/// claims. Checked Psi has neither target stack closure nor a selected fuel
/// schedule/instruction footprint, so claiming a numeric realization here
/// would promote backend or installation authority into the wrong stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedResourceDerivationObligation {
    TerminalAndTargetStackClosure,
    TerminalControlAndFuelSchedule,
    SelectedInstructionMachineStateFootprint,
}

impl CheckedResourceDerivationObligation {
    const fn identity_tag(self) -> u8 {
        match self {
            Self::TerminalAndTargetStackClosure => 1,
            Self::TerminalControlAndFuelSchedule => 2,
            Self::SelectedInstructionMachineStateFootprint => 3,
        }
    }
}

/// One independently replayable checked resource-axis anchor. Every field is
/// source-handle-free and redundantly binds the exact machine entry and
/// normalized contract so later carriage cannot swap a valid axis between
/// implementations or between entries of one machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedResourceAxisAnchor {
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    derivation_obligation: CheckedResourceDerivationObligation,
    report_fingerprint: u64,
}

impl CheckedResourceAxisAnchor {
    fn from_checked_contract(
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        derivation_obligation: CheckedResourceDerivationObligation,
    ) -> Self {
        let report_fingerprint = checked_resource_axis_report_fingerprint(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            derivation_obligation,
        );
        Self {
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            derivation_obligation,
            report_fingerprint,
        }
    }

    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }

    pub const fn contract_report_fingerprint(&self) -> u64 {
        self.contract_report_fingerprint
    }

    pub const fn contract_commitment(&self) -> MachineContractCommitment {
        self.contract_commitment
    }

    pub const fn entry(&self) -> SymbolHandle {
        self.entry
    }

    pub const fn derivation_obligation(&self) -> CheckedResourceDerivationObligation {
        self.derivation_obligation
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    fn validate(
        &self,
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        expected_obligation: CheckedResourceDerivationObligation,
    ) -> Result<(), &'static str> {
        if self.machine != machine
            || self.entry != entry
            || self.contract_report_fingerprint != contract_report_fingerprint
            || self.contract_commitment != contract_commitment
        {
            return Err(
                "checked resource axis does not bind the enclosing exact machine entry contract",
            );
        }
        if self.derivation_obligation != expected_obligation {
            return Err(
                "checked resource envelope substituted or fused an independent resource axis",
            );
        }
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource axis requires a nonzero exact contract identity");
        }
        if self.report_fingerprint
            != checked_resource_axis_report_fingerprint(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
                self.derivation_obligation,
            )
        {
            return Err("checked resource axis report fingerprint does not replay");
        }
        Ok(())
    }
}

/// Checked-Psi resource-envelope prerequisite for one concrete machine entry.
///
/// It records exactly which source-independent derivations remain necessary;
/// it intentionally contains no numeric ceiling, target realization, provider
/// receipt, callback placement, or installation authority. Its report
/// fingerprint is a compilation-local summary/join key, never artifact or
/// admission authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEntryResourceEnvelope {
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    stack: CheckedResourceAxisAnchor,
    logical_structural_work: CheckedResourceAxisAnchor,
    machine_state: CheckedResourceAxisAnchor,
    report_fingerprint: u64,
}

impl CheckedEntryResourceEnvelope {
    pub fn from_checked_contract(
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
    ) -> Self {
        let stack = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
        );
        let logical_structural_work = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule,
        );
        let machine_state = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
        );
        let report_fingerprint = checked_resource_envelope_report_fingerprint(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            stack.report_fingerprint,
            logical_structural_work.report_fingerprint,
            machine_state.report_fingerprint,
        );
        Self {
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            stack,
            logical_structural_work,
            machine_state,
            report_fingerprint,
        }
    }

    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }

    pub const fn contract_report_fingerprint(&self) -> u64 {
        self.contract_report_fingerprint
    }

    pub const fn contract_commitment(&self) -> MachineContractCommitment {
        self.contract_commitment
    }

    pub const fn entry(&self) -> SymbolHandle {
        self.entry
    }

    pub const fn stack(&self) -> &CheckedResourceAxisAnchor {
        &self.stack
    }

    pub const fn logical_structural_work(&self) -> &CheckedResourceAxisAnchor {
        &self.logical_structural_work
    }

    pub const fn machine_state(&self) -> &CheckedResourceAxisAnchor {
        &self.machine_state
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource envelope requires a nonzero exact contract identity");
        }
        self.stack.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
        )?;
        self.logical_structural_work.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule,
        )?;
        self.machine_state.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
        )?;
        if self.report_fingerprint
            != checked_resource_envelope_report_fingerprint(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
                self.stack.report_fingerprint,
                self.logical_structural_work.report_fingerprint,
                self.machine_state.report_fingerprint,
            )
        {
            return Err("checked resource envelope report fingerprint does not replay");
        }
        Ok(())
    }
}

/// Sealed declaration-order roster of the resource prerequisites for one
/// machine's entries. The private roster report fingerprint is compilation-
/// local summary evidence: it detects deletion and reordering during checked-
/// fact carriage but grants no artifact or installation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMachineResourceEnvelopes {
    machine: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    entries: Vec<CheckedEntryResourceEnvelope>,
    roster_report_fingerprint: u64,
}

impl CheckedMachineResourceEnvelopes {
    pub fn from_checked_contract_entries(
        machine: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        entries: impl IntoIterator<Item = SymbolHandle>,
    ) -> Self {
        let entries = entries
            .into_iter()
            .map(|entry| {
                CheckedEntryResourceEnvelope::from_checked_contract(
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                )
            })
            .collect::<Vec<_>>();
        let roster_report_fingerprint = checked_resource_roster_report_fingerprint(
            machine,
            contract_report_fingerprint,
            contract_commitment,
            &entries,
        );
        Self {
            machine,
            contract_report_fingerprint,
            contract_commitment,
            entries,
            roster_report_fingerprint,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, CheckedEntryResourceEnvelope> {
        self.entries.iter()
    }

    pub fn get(&self, entry: SymbolHandle) -> Option<&CheckedEntryResourceEnvelope> {
        self.entries.iter().find(|resource| resource.entry == entry)
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource roster requires a nonzero exact contract identity");
        }
        for (index, resource) in self.entries.iter().enumerate() {
            resource.validate()?;
            if resource.machine != self.machine
                || resource.contract_report_fingerprint != self.contract_report_fingerprint
                || resource.contract_commitment != self.contract_commitment
            {
                return Err(
                    "checked entry resource envelope does not bind its exact machine contract",
                );
            }
            if self.entries[..index]
                .iter()
                .any(|prior| prior.entry == resource.entry)
            {
                return Err("checked resource roster retained a duplicate machine entry");
            }
            if resource
                != &CheckedEntryResourceEnvelope::from_checked_contract(
                    self.machine,
                    resource.entry,
                    self.contract_report_fingerprint,
                    self.contract_commitment,
                )
            {
                return Err("checked entry resource envelope does not independently replay");
            }
        }
        if self.roster_report_fingerprint
            != checked_resource_roster_report_fingerprint(
                self.machine,
                self.contract_report_fingerprint,
                self.contract_commitment,
                &self.entries,
            )
        {
            return Err("checked resource roster report fingerprint does not replay");
        }
        Ok(())
    }
}

fn checked_resource_axis_report_fingerprint(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    derivation_obligation: CheckedResourceDerivationObligation,
) -> u64 {
    let mut bytes = b"omega.checked.resource-axis.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(entry.arena_index().to_le_bytes());
    bytes.extend(entry.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.push(derivation_obligation.identity_tag());
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_envelope_report_fingerprint(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    stack: u64,
    logical_structural_work: u64,
    machine_state: u64,
) -> u64 {
    let mut bytes = b"omega.checked.resource-envelope.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(entry.arena_index().to_le_bytes());
    bytes.extend(entry.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.extend(stack.to_le_bytes());
    bytes.extend(logical_structural_work.to_le_bytes());
    bytes.extend(machine_state.to_le_bytes());
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_roster_report_fingerprint(
    machine: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    entries: &[CheckedEntryResourceEnvelope],
) -> u64 {
    let mut bytes = b"omega.checked.resource-roster.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.extend(
        u64::try_from(entries.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in entries {
        bytes.extend(entry.entry.arena_index().to_le_bytes());
        bytes.extend(entry.entry.generation().to_le_bytes());
        bytes.extend(entry.report_fingerprint.to_le_bytes());
    }
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_report_fingerprint(bytes: impl IntoIterator<Item = u8>) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.into_iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// Complete currently-checkable implementation envelope for one concrete
/// machine. These axes are evidence, not a replacement public contract
/// commitment. The resource field is an exact derivation-obligation anchor,
/// not a claim that downstream resource realizations already exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedMachineContractEnvelope {
    pub machine: SymbolHandle,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
    pub effective_service_reach: Vec<String>,
    /// Concrete reach with installation-selected upper-bound contributions
    /// removed. Root composition unions resolved rows into this provenance-
    /// preserving base.
    pub concrete_service_reach: Vec<String>,
    /// Installation-selected reach requirements still awaiting provider-row
    /// substitution. These are implementation evidence and therefore do not
    /// enter the machine's published contract identity.
    pub unresolved_installation_reaches: Vec<psi_effects::InstallationReachRequirement>,
    pub effective_synchronous_invocations: Vec<String>,
    pub checked_may_suspend: bool,
    pub checked_may_block: bool,
    pub checked_termination: TerminationGuarantee,
    pub checked_crash: CrashPlan,
    pub mutation: Vec<crate::StateWriteFramePlan>,
    pub capabilities: Vec<psi_effects::CapabilityFlowFact>,
    /// One row per concrete entry, in typed declaration order.
    pub resources: CheckedMachineResourceEnvelopes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPlan {
    pub machine: SymbolHandle,
    /// Source-handle-free projection of authored value clauses into the
    /// closed reflexive scalar equality subset. An unrecognized clause is
    /// retained as `None`, so consumers fail closed without reopening typed
    /// proof expressions.
    pub closed_scalar_values: ClosedScalarValueContractPlan,
    /// Canonical published crash ceiling plus independent checked body sites.
    /// Clause grouping, ordering, duplicate predicates, and `true` spelling do
    /// not survive into the published carrier; sites do not enter identity.
    pub crash: CrashPlan,
    /// Historical compact compatibility coordinate. This is report/cache data
    /// and never sufficient contract authority without `commitment`.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the complete canonical public
    /// contract structure.
    pub commitment: MachineContractCommitment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineContractCommitment([u8; 32]);

impl MachineContractCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineContractIdentity {
    pub report_fingerprint: u64,
    pub commitment: MachineContractCommitment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedScalarContractValue {
    Boolean(bool),
    Integer(IntegerLiteral),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedScalarValueContractPlan {
    requires: Vec<Option<ClosedScalarContractValue>>,
    ensures: Vec<Option<ClosedScalarContractValue>>,
    has_crash_clauses: bool,
    has_outcome_specific_clauses: bool,
}

impl ClosedScalarValueContractPlan {
    pub fn new(
        requires: Vec<Option<ClosedScalarContractValue>>,
        ensures: Vec<Option<ClosedScalarContractValue>>,
        has_crash_clauses: bool,
        has_outcome_specific_clauses: bool,
    ) -> Self {
        Self {
            requires,
            ensures,
            has_crash_clauses,
            has_outcome_specific_clauses,
        }
    }

    pub fn requires(&self) -> &[Option<ClosedScalarContractValue>] {
        &self.requires
    }

    pub fn ensures(&self) -> &[Option<ClosedScalarContractValue>] {
        &self.ensures
    }

    pub const fn has_crash_clauses(&self) -> bool {
        self.has_crash_clauses
    }

    pub const fn has_outcome_specific_clauses(&self) -> bool {
        self.has_outcome_specific_clauses
    }
}

/// Canonical public contract identity. The historical FNV-1a coordinate is
/// retained for reports and caches; authority uses the domain-separated
/// SHA-256 commitment over the same complete normalized byte stream.
pub fn contract_identity(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash: &CrashPlan,
    termination: &TerminationInterface,
    canonical_facts: &[Vec<u8>],
) -> MachineContractIdentity {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut strong = Sha256::new();
    strong.update(b"omega.checked-machine-public-contract.v1\0");
    let mut fold = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        strong.update([byte]);
    };
    fold(match supply_mode {
        MachineSupplyMode::CheckedBody => 1,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::AdmissionClaim => 4,
        // PRV4: the leaf's supply tag; the binding identity folds separately
        // below so two leaves with different bindings differ.
        MachineSupplyMode::ExternalRealization { .. } => 5,
        MachineSupplyMode::TopLevelRequirement => 6,
    });
    if let MachineSupplyMode::ExternalRealization { binding, mechanism } = supply_mode {
        match mechanism {
            Some(mechanism) => {
                fold(1);
                fold(mechanism.identity_tag());
            }
            None => fold(0),
        }
        match binding {
            Some(binding) => {
                fold(1);
                for byte in binding.0.to_le_bytes() {
                    fold(byte);
                }
            }
            None => fold(0),
        }
    }
    // Boundary-service declaration identity, rendered canonically rather than
    // folding per-program row or service-table indices.
    fold(0xfb);
    let mut canonical_service_names = published_service_names.iter().collect::<Vec<_>>();
    canonical_service_names.sort_unstable();
    canonical_service_names.dedup();
    for name in canonical_service_names {
        for byte in name.as_bytes() {
            fold(*byte);
        }
        fold(0xfa);
    }
    fold(match invocation_interface {
        SynchronousInvocationInterface::InternalInferred => 1,
        SynchronousInvocationInterface::PublishedCeiling => 2,
    });
    let mut canonical_invocations = published_invocations.iter().collect::<Vec<_>>();
    canonical_invocations.sort_unstable();
    canonical_invocations.dedup();
    for invocation in canonical_invocations {
        for byte in invocation.as_bytes() {
            fold(*byte);
        }
        fold(0xf9);
    }
    fold(match suspension_interface {
        SuspensionInterface::InternalInferred => 1,
        SuspensionInterface::PublishedMaySuspend(false) => 2,
        SuspensionInterface::PublishedMaySuspend(true) => 3,
    });
    fold(match blocking_interface {
        BlockingInterface::InternalInferred => 1,
        BlockingInterface::PublishedMayBlock(false) => 2,
        BlockingInterface::PublishedMayBlock(true) => 3,
    });
    fold(0xf8);
    fold(match crash.interface {
        CrashInterface::InternalInferred => 1,
        CrashInterface::PublishedCeiling => 2,
    });
    let mut crash_buckets = crash.published.clone();
    crash_buckets.sort();
    crash_buckets.dedup();
    for bucket in crash_buckets {
        fold(match bucket.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
        for guard in bucket.alternative_guards {
            match guard {
                CrashRouteGuard::Truth => fold(0),
                CrashRouteGuard::Predicate(predicate) => {
                    fold(1);
                    for byte in predicate.canonical_bytes() {
                        fold(*byte);
                    }
                }
            }
            fold(0xf7);
        }
        fold(0xf6);
    }
    fold(0xff);
    match termination {
        TerminationInterface::InternalDerived => fold(0),
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => fold(1),
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            fold(2);
            for premise in premises {
                for byte in premise.profile.0.to_le_bytes() {
                    fold(byte);
                }
                for byte in premise.subject.root.arena_index().to_le_bytes() {
                    fold(byte);
                }
                fold(0xfb);
                for projection in &premise.subject.projections {
                    for byte in projection.arena_index().to_le_bytes() {
                        fold(byte);
                    }
                }
                fold(0xfa);
            }
        }
    }
    // Slice 2: the declared requires/ensures facts, pre-sorted by the
    // caller (clause order never enters the identity).
    fold(0xfd);
    for fact in canonical_facts {
        for byte in fact {
            fold(*byte);
        }
        fold(0xfc);
    }
    MachineContractIdentity {
        report_fingerprint: hash,
        commitment: MachineContractCommitment::from_digest(strong.finalize().into()),
    }
}

pub fn contract_report_fingerprint(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash: &CrashPlan,
    termination: &TerminationInterface,
    canonical_facts: &[Vec<u8>],
) -> u64 {
    contract_identity(
        supply_mode,
        published_service_names,
        invocation_interface,
        published_invocations,
        suspension_interface,
        blocking_interface,
        crash,
        termination,
        canonical_facts,
    )
    .report_fingerprint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_equal_machine_contract_substitution_rejects() {
        let machine = SymbolHandle::from_arena_index(17);
        let entry = SymbolHandle::from_arena_index(18);
        let compact = 0xfeed;
        let expected = MachineContractCommitment::from_digest([0x11; 32]);
        let substituted = MachineContractCommitment::from_digest([0x22; 32]);
        let plans = MachineContractPlans {
            machines: vec![MachineContractPlan {
                machine,
                closed_scalar_values: ClosedScalarValueContractPlan::default(),
                crash: CrashPlan::default(),
                report_fingerprint: compact,
                commitment: expected,
            }],
            crash_capsules: Vec::new(),
            realized_envelopes: vec![RealizedMachineContractEnvelope {
                machine,
                contract_report_fingerprint: compact,
                contract_commitment: substituted,
                effective_service_reach: Vec::new(),
                concrete_service_reach: Vec::new(),
                unresolved_installation_reaches: Vec::new(),
                effective_synchronous_invocations: Vec::new(),
                checked_may_suspend: false,
                checked_may_block: false,
                checked_termination: TerminationGuarantee::NoGuarantee,
                checked_crash: CrashPlan::default(),
                mutation: Vec::new(),
                capabilities: Vec::new(),
                resources: CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                    machine,
                    compact,
                    substituted,
                    [entry],
                ),
            }],
        };

        assert!(
            plans
                .validate_resource_envelopes()
                .expect_err("compact-equal structural contract substitution must reject")
                .contains("contract identity disagrees")
        );
    }

    #[test]
    fn zero_machine_contract_commitment_rejects_even_with_nonzero_report_coordinate() {
        let machine = SymbolHandle::from_arena_index(17);
        let entry = SymbolHandle::from_arena_index(18);
        let compact = 0xfeed;
        let plans = MachineContractPlans {
            machines: vec![MachineContractPlan {
                machine,
                closed_scalar_values: ClosedScalarValueContractPlan::default(),
                crash: CrashPlan::default(),
                report_fingerprint: compact,
                commitment: MachineContractCommitment::from_digest([0; 32]),
            }],
            crash_capsules: Vec::new(),
            realized_envelopes: vec![RealizedMachineContractEnvelope {
                machine,
                contract_report_fingerprint: compact,
                contract_commitment: MachineContractCommitment::from_digest([0; 32]),
                effective_service_reach: Vec::new(),
                concrete_service_reach: Vec::new(),
                unresolved_installation_reaches: Vec::new(),
                effective_synchronous_invocations: Vec::new(),
                checked_may_suspend: false,
                checked_may_block: false,
                checked_termination: TerminationGuarantee::NoGuarantee,
                checked_crash: CrashPlan::default(),
                mutation: Vec::new(),
                capabilities: Vec::new(),
                resources: CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                    machine,
                    compact,
                    MachineContractCommitment::from_digest([0; 32]),
                    [entry],
                ),
            }],
        };

        assert_eq!(
            plans
                .validate_resource_envelopes()
                .expect_err("zero strong authority must reject before compact replay"),
            "checked resource envelope requires a nonzero machine contract commitment",
        );
    }

    #[test]
    fn machine_contract_fnv_is_report_only_and_strong_identity_is_domain_separated() {
        let source = include_str!("contract_plans.rs");
        let compact_marker = ["const OFFSET: u64 = 0x", "cbf29ce484222325"].concat();
        assert_eq!(
            source.matches(compact_marker.as_str()).count(),
            2,
            "the reviewed checked-contract/resource compact inventory changed"
        );
        assert!(
            source.contains("omega.checked-machine-public-contract.v1\\0")
                && source.contains("pub fn contract_identity(")
                && source.contains("pub commitment: MachineContractCommitment"),
            "public machine contracts must retain a domain-separated strong commitment"
        );
    }

    #[test]
    fn checked_resource_envelope_replays_three_distinct_derivation_obligations() {
        let machine = SymbolHandle::from_arena_index(21);
        let entry = SymbolHandle::from_arena_index(22);
        let commitment = MachineContractCommitment::from_digest([0x11; 32]);
        let envelope =
            CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, commitment);

        envelope.validate().expect("canonical resource anchor");
        assert_eq!(envelope.machine(), machine);
        assert_eq!(envelope.entry(), entry);
        assert_eq!(envelope.contract_report_fingerprint(), 0xfeed);
        assert_eq!(
            envelope.stack().derivation_obligation(),
            CheckedResourceDerivationObligation::TerminalAndTargetStackClosure
        );
        assert_eq!(
            envelope.logical_structural_work().derivation_obligation(),
            CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule
        );
        assert_eq!(
            envelope.machine_state().derivation_obligation(),
            CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint
        );
        assert_ne!(
            envelope.stack().report_fingerprint(),
            envelope.logical_structural_work().report_fingerprint()
        );
        assert_ne!(
            envelope.logical_structural_work().report_fingerprint(),
            envelope.machine_state().report_fingerprint()
        );

        let mut fused = envelope.clone();
        fused.stack = fused.logical_structural_work.clone();
        assert!(
            fused
                .validate()
                .expect_err("one resource axis cannot substitute for another")
                .contains("substituted or fused")
        );

        let mut tampered = envelope;
        tampered.machine_state.report_fingerprint ^= 1;
        assert!(
            tampered
                .validate()
                .expect_err("axis identity must independently replay")
                .contains("axis report fingerprint")
        );

        let other = CheckedEntryResourceEnvelope::from_checked_contract(
            machine,
            SymbolHandle::from_arena_index(23),
            0xfeed,
            commitment,
        );
        tampered = other.clone();
        tampered.stack =
            CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, commitment)
                .stack;
        assert!(
            tampered
                .validate()
                .expect_err("an axis cannot move between entries")
                .contains("exact machine entry contract")
        );
    }

    #[test]
    fn checked_resource_envelope_rejects_compact_equal_contract_commitment_substitution() {
        let machine = SymbolHandle::from_arena_index(24);
        let entry = SymbolHandle::from_arena_index(25);
        let expected = MachineContractCommitment::from_digest([0x31; 32]);
        let substituted = MachineContractCommitment::from_digest([0x32; 32]);
        let canonical =
            CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, expected);
        let foreign = CheckedEntryResourceEnvelope::from_checked_contract(
            machine,
            entry,
            0xfeed,
            substituted,
        );

        canonical.validate().expect("canonical resource envelope");
        foreign
            .validate()
            .expect("independently canonical foreign envelope");
        assert_ne!(canonical.report_fingerprint(), foreign.report_fingerprint());

        let mut mixed = canonical;
        mixed.stack = foreign.stack;
        assert!(
            mixed
                .validate()
                .expect_err("compact-equal contract substitution must reject")
                .contains("exact machine entry contract")
        );
    }

    #[test]
    fn checked_resource_roster_rejects_entry_deletion_and_reordering() {
        let machine = SymbolHandle::from_arena_index(31);
        let entries = [
            SymbolHandle::from_arena_index(32),
            SymbolHandle::from_arena_index(33),
            SymbolHandle::from_arena_index(34),
        ];
        let commitment = MachineContractCommitment::from_digest([0x22; 32]);
        let roster = CheckedMachineResourceEnvelopes::from_checked_contract_entries(
            machine, 0xbeef, commitment, entries,
        );
        roster.validate().expect("canonical resource roster");
        assert_eq!(roster.len(), 3);
        assert_eq!(
            roster
                .iter()
                .map(CheckedEntryResourceEnvelope::entry)
                .collect::<Vec<_>>(),
            entries
        );

        let mut deleted = roster.clone();
        deleted.entries.remove(1);
        assert!(
            deleted
                .validate()
                .expect_err("entry deletion must invalidate the sealed roster")
                .contains("roster report fingerprint")
        );

        let mut reordered = roster;
        reordered.entries.swap(0, 2);
        assert!(
            reordered
                .validate()
                .expect_err("entry reordering must invalidate the sealed roster")
                .contains("roster report fingerprint")
        );
    }

    #[test]
    fn crash_route_carriers_enforce_canonical_nonempty_sets() {
        assert!(CrashRouteBucket::new(CrashCause::Trap, Vec::new()).is_none());

        let predicate = CrashPredicateIdentity::from_canonical_bytes(vec![1, 2, 3]);
        let guarded = CrashRouteBucket::new(
            CrashCause::Trap,
            vec![
                CrashRouteGuard::Predicate(predicate.clone()),
                CrashRouteGuard::Predicate(predicate),
            ],
        )
        .expect("a guarded bucket is nonempty");
        assert_eq!(guarded.alternative_guards().len(), 1);

        let unconditional = CrashRouteBucket::new(
            CrashCause::Trap,
            vec![
                CrashRouteGuard::Predicate(CrashPredicateIdentity::from_canonical_bytes(vec![4])),
                CrashRouteGuard::Truth,
            ],
        )
        .expect("truth contributes a route");
        assert!(unconditional.is_unconditional());

        let plan = CrashPlan::published_ceiling(vec![unconditional.clone(), unconditional]);
        assert_eq!(plan.published().len(), 1);
    }

    #[test]
    fn crash_sites_are_canonical_implementation_evidence() {
        let first_state = SymbolHandle::from_arena_index(4);
        let second_state = SymbolHandle::from_arena_index(9);
        let first_claim = psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: first_state,
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            ordinal: 0,
        };
        let second_claim = psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: first_state,
            source: psi_language_semantics::PermissionEventSource::Statement { statement_index: 1 },
            ordinal: 1,
        };
        let path_guard = CrashPredicateIdentity::from_canonical_bytes(vec![1, 9, 0, 0, 0, 0]);
        let first = CheckedCrashSite::new(
            CrashSiteLocation::new(first_state, 2),
            CrashCause::Abort,
            Vec::new(),
            vec![second_claim, first_claim, second_claim],
        )
        .with_path_guard_conjuncts(vec![path_guard.clone(), path_guard.clone()]);
        let second = CheckedCrashSite::new(
            CrashSiteLocation::new(second_state, 0),
            CrashCause::Trap,
            Vec::new(),
            Vec::new(),
        );
        let plan = CrashPlan::default()
            .with_checked_sites(vec![second.clone(), first.clone(), first.clone()])
            .expect("one crash cause occupies each source site");

        assert_eq!(plan.checked_sites(), &[first.clone(), second]);
        assert_eq!(
            plan.checked_sites()[0].frontier_lower_bound(),
            &[first_claim, second_claim],
            "frontier identity is canonical and duplicate-free"
        );
        assert_eq!(
            plan.checked_sites()[0].path_guard_conjuncts(),
            &[path_guard]
        );
        assert_eq!(
            plan.checked_site_at(first_state, 2)
                .map(|site| site.cause()),
            Some(CrashCause::Abort)
        );
        assert_eq!(plan.interface(), CrashInterface::InternalInferred);

        assert!(
            CrashPlan::default()
                .with_checked_sites(vec![
                    first.clone(),
                    CheckedCrashSite::new(
                        first.location(),
                        CrashCause::Trap,
                        Vec::new(),
                        Vec::new(),
                    ),
                ])
                .is_none()
        );
        assert!(
            CrashPlan::default()
                .with_checked_sites(vec![CheckedCrashSite::new(
                    CrashSiteLocation::new(first_state, 3),
                    CrashCause::Abort,
                    Vec::new(),
                    vec![psi_language_semantics::PermissionClaimIdentity::Unknown],
                )])
                .is_none(),
            "an unknown claim identity cannot enter checked crash evidence"
        );
    }

    #[test]
    fn crash_calls_retain_empty_refinements_and_reject_coordinate_collisions() {
        let machine = SymbolHandle::from_arena_index(2);
        let state = SymbolHandle::from_arena_index(3);
        let location = CrashCallSiteLocation::new(state, 4, 1);
        let call = CheckedCrashCallSite::new(location, machine, state, 17, Vec::new());
        let plan = CrashPlan::default()
            .with_checked_calls(vec![call.clone(), call.clone()])
            .expect("an identical duplicate canonicalizes away");
        assert_eq!(plan.checked_calls(), &[call.clone()]);
        assert!(plan.checked_calls()[0].surviving_buckets().is_empty());
        assert!(plan.checked_call_at(state, 4, 1).is_some());

        let conflicting = CheckedCrashCallSite::new(
            location,
            SymbolHandle::from_arena_index(8),
            state,
            18,
            vec![CrashRouteBucket::unconditional(CrashCause::Abort)],
        );
        assert!(
            CrashPlan::default()
                .with_checked_calls(vec![call, conflicting])
                .is_none(),
            "one invocation coordinate cannot name two checked crash refinements"
        );
    }

    #[test]
    fn crash_contract_capsules_are_canonical_and_addressable() {
        let target_machine = SymbolHandle::from_arena_index(11);
        let target_state = SymbolHandle::from_arena_index(12);
        let capsule = CrashContractCapsule::new(
            target_machine,
            target_state,
            0xfeed,
            vec![
                CrashRouteBucket::unconditional(CrashCause::Abort),
                CrashRouteBucket::unconditional(CrashCause::Trap),
                CrashRouteBucket::unconditional(CrashCause::Abort),
            ],
        )
        .with_operational_envelope(
            vec!["Window".to_owned(), "Clock".to_owned(), "Window".to_owned()],
            vec!["service:Events".to_owned(), "service:Events".to_owned()],
            true,
            false,
            TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
        );
        assert_eq!(capsule.published_buckets().len(), 2);
        assert_eq!(capsule.published_service_reach(), ["Clock", "Window"]);
        assert_eq!(
            capsule.published_synchronous_invocations(),
            ["service:Events"]
        );
        assert!(capsule.published_may_suspend());
        assert!(!capsule.published_may_block());
        assert!(matches!(
            capsule.published_termination(),
            TerminationGuarantee::Terminates { .. }
        ));
        let plans = MachineContractPlans {
            machines: Vec::new(),
            crash_capsules: vec![capsule],
            realized_envelopes: Vec::new(),
        };
        assert_eq!(
            plans
                .crash_capsule(target_machine, target_state)
                .map(CrashContractCapsule::target_contract_report_fingerprint),
            Some(0xfeed)
        );
    }

    #[test]
    fn crash_bucket_ids_join_checked_sites_to_their_published_contract() {
        let plan = CrashPlan::published_ceiling(vec![
            CrashRouteBucket::unconditional(CrashCause::Abort),
            CrashRouteBucket::unconditional(CrashCause::Trap),
        ]);
        let ids = plan
            .published_with_ids()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        assert_eq!(ids.iter().map(|id| id.get()).collect::<Vec<_>>(), [1, 2]);
        for (id, bucket) in plan.published_with_ids() {
            assert_eq!(plan.published_bucket(id), Some(bucket));
        }

        let abort_id = plan
            .published_with_ids()
            .find_map(|(id, bucket)| (bucket.cause() == CrashCause::Abort).then_some(id))
            .expect("published abort bucket");
        let site = CheckedCrashSite::new(
            CrashSiteLocation::new(SymbolHandle::from_arena_index(4), 0),
            CrashCause::Abort,
            vec![abort_id, abort_id],
            Vec::new(),
        );
        let plan = plan
            .with_checked_sites(vec![site])
            .expect("site coverage cites a same-cause bucket");
        assert_eq!(
            plan.checked_sites()[0].guard_covering_buckets(),
            &[abort_id]
        );
    }

    #[test]
    fn operational_interfaces_participate_independently_in_contract_identity() {
        let fingerprint = |suspension, blocking| {
            contract_report_fingerprint(
                MachineSupplyMode::Boundary,
                &[],
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                suspension,
                blocking,
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let neither = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
        );
        let suspending = fingerprint(
            SuspensionInterface::PublishedMaySuspend(true),
            BlockingInterface::PublishedMayBlock(false),
        );
        let blocking = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(true),
        );
        assert_ne!(neither, suspending);
        assert_ne!(neither, blocking);
        assert_ne!(suspending, blocking);
    }

    #[test]
    fn machine_supply_vocabulary_participates_independently_in_contract_identity() {
        let fingerprint = |supply_mode| {
            contract_report_fingerprint(
                supply_mode,
                &[],
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let fingerprints = [
            MachineSupplyMode::CheckedBody,
            MachineSupplyMode::Requirement,
            MachineSupplyMode::Boundary,
            MachineSupplyMode::AdmissionClaim,
            MachineSupplyMode::ExternalRealization {
                binding: Some(psi_language_semantics::ExternalBindingId(1)),
                mechanism: Some(
                    psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic,
                ),
            },
            MachineSupplyMode::TopLevelRequirement,
        ]
        .map(fingerprint);

        for (index, fingerprint) in fingerprints.iter().enumerate() {
            assert!(
                fingerprints[..index]
                    .iter()
                    .all(|earlier| earlier != fingerprint),
                "machine supply modes must have distinct contract identities"
            );
        }
    }

    #[test]
    fn external_binding_mechanism_participates_in_contract_identity() {
        let fingerprint = |mechanism| {
            contract_report_fingerprint(
                MachineSupplyMode::ExternalRealization {
                    binding: Some(psi_language_semantics::ExternalBindingId(1)),
                    mechanism: Some(mechanism),
                },
                &[],
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };

        assert_ne!(
            fingerprint(psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
            fingerprint(psi_language_semantics::ExternalBindingMechanism::Import),
            "one per-program binding ordinal must not collapse distinct mechanisms"
        );
    }

    #[test]
    fn termination_implementation_evidence_is_contract_invisible() {
        let interface = TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new(),
        });
        let unresolved = psi_language_semantics::MachineTerminationPlan {
            interface: interface.clone(),
            checked_summary: TerminationGuarantee::NoGuarantee,
            implementation_witness: None,
        };
        let established = psi_language_semantics::MachineTerminationPlan {
            interface,
            checked_summary: TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            implementation_witness: Some(psi_language_semantics::RankingWitness {
                subjects: vec!["remaining".to_owned()],
                ranking_view: psi_language_semantics::RankingViewId::NAT_DESCENDING,
                view_path: "Nat::Descending".to_owned(),
                view_arguments: Vec::new(),
                rank_range: None,
            }),
        };
        let fingerprint = |plan: &psi_language_semantics::MachineTerminationPlan| {
            contract_report_fingerprint(
                MachineSupplyMode::CheckedBody,
                &[],
                SynchronousInvocationInterface::InternalInferred,
                &[],
                SuspensionInterface::InternalInferred,
                BlockingInterface::InternalInferred,
                &CrashPlan::default(),
                &plan.interface,
                &[],
            )
        };

        assert_ne!(unresolved, established);
        assert_eq!(fingerprint(&unresolved), fingerprint(&established));
    }

    #[test]
    fn symbol_resolved_service_names_participate_in_contract_identity() {
        let fingerprint = |services: &[String]| {
            contract_report_fingerprint(
                MachineSupplyMode::Boundary,
                services,
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let empty = fingerprint(&[]);
        let readable = fingerprint(&["Readable".to_owned()]);
        let queryable = fingerprint(&["Queryable".to_owned()]);
        let composite = fingerprint(&["Readable".to_owned(), "Queryable".to_owned()]);
        let reordered = fingerprint(&["Queryable".to_owned(), "Readable".to_owned()]);
        assert_ne!(empty, readable);
        assert_ne!(readable, queryable);
        assert_eq!(composite, reordered);
    }

    #[test]
    fn synchronous_invocation_ceiling_participates_in_contract_identity() {
        let fingerprint = |interface, invocations: &[String]| {
            contract_report_fingerprint(
                MachineSupplyMode::Boundary,
                &[],
                interface,
                invocations,
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let omitted = fingerprint(SynchronousInvocationInterface::PublishedCeiling, &[]);
        let handler = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["parameter:0".to_owned()],
        );
        let composite = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["service:Clock".to_owned(), "parameter:0".to_owned()],
        );
        let reordered = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["parameter:0".to_owned(), "service:Clock".to_owned()],
        );
        let private = fingerprint(SynchronousInvocationInterface::InternalInferred, &[]);
        assert_ne!(omitted, handler);
        assert_ne!(omitted, private);
        assert_eq!(composite, reordered);
    }

    #[test]
    fn internal_derivation_differs_from_published_omission() {
        let fingerprint = |termination| {
            contract_report_fingerprint(
                MachineSupplyMode::CheckedBody,
                &[],
                SynchronousInvocationInterface::InternalInferred,
                &[],
                SuspensionInterface::InternalInferred,
                BlockingInterface::InternalInferred,
                &CrashPlan::default(),
                termination,
                &[],
            )
        };
        assert_ne!(
            fingerprint(&TerminationInterface::InternalDerived),
            fingerprint(&TerminationInterface::Published(
                TerminationGuarantee::NoGuarantee
            ))
        );
    }
}
