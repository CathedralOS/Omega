//! Record the values and borrowed data that a call must keep available if it pauses.
//!
//! Keep each operand's location and read/write access together. Owned and
//! borrowed storage have different frontier commitments even when their pointer
//! representations agree. These demands record existing authority; loan ancestry,
//! backing lifetime and runtime preservation still require their own joins.

use crate::{
    StructuralAccess, StructuralArgument, StructuralPathSegment,
    terminal_module::TerminalIdentityTable,
};
use arena::{Arena, Handle, HandleSpan};
use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, IeeeFloatFormat, IntegerCarrier, IntegerSign, MachineId,
    OperationId, ScalarType, StructuralTypeId, SuspensionCrossingId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSuspensionCallSite {
    pub operation: OperationId,
    pub crossing: SuspensionCrossingId,
    pub target: TerminalSuspensionCallTarget,
    pub frontier_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSuspensionCallPlan {
    pub operation: OperationId,
    pub crossing: SuspensionCrossingId,
    pub target: TerminalSuspensionCallTarget,
    pub effective: CarryPolicy,
    /// Independent checked-frontier cardinality commitment. The verifier
    /// compares this with the encoded roster before inspecting any member.
    pub live_value_count: u32,
    /// Ordered demand occurrences, not a set of physical storage coordinates.
    /// Distinct actual positions may retain the same storage; each occurrence
    /// keeps its claims and policy and contributes to the committed count.
    pub live_values: Vec<TerminalSuspensionLiveValue>,
    /// Source-free occurrence, capture and backing relationships. This is a
    /// retention demand; the runtime still owes physical preservation.
    pub loans: TerminalSuspensionLoans,
}

/// Each crossing owns its loan namespace. Parentage and transport remain
/// distinct even when the two occurrences denote pointer-equivalent storage.
/// Durable child paths and value-to-loan edges live in these arenas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalSuspensionLoans {
    pub occurrences: Arena<TerminalSuspensionLoan>,
    pub paths: Arena<StructuralPathSegment>,
    pub value_links: Arena<Handle<TerminalSuspensionLoan>>,
    pub supplier_links: Arena<Handle<TerminalSuspensionLoan>>,
    pub input_links: Arena<Handle<TerminalSuspensionLoan>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSuspensionLoan {
    pub capture: TerminalSuspensionLoanCapture,
    pub backing: TerminalSuspensionLoanBacking,
    pub resource: TerminalSuspensionLoanResource,
    pub parent: Handle<TerminalSuspensionLoan>,
    /// Immediate alternative suppliers, not reborrow parents or flattened
    /// transport histories. Every dependency stays present after convergence.
    pub suppliers: HandleSpan<Handle<TerminalSuspensionLoan>>,
    /// All original input obligations, separate from spatial parent/suppliers.
    pub inputs: HandleSpan<Handle<TerminalSuspensionLoan>>,
    pub access: StructuralAccess,
    /// A retired dependency remains constrained by its descendant. This flag
    /// never reactivates that dependency's descriptor or restores access.
    pub active_at_call: bool,
}

impl Default for TerminalSuspensionLoan {
    fn default() -> Self {
        Self {
            capture: Default::default(),
            backing: Default::default(),
            resource: Default::default(),
            parent: Handle::invalid(),
            suppliers: HandleSpan::empty(),
            inputs: HandleSpan::empty(),
            access: StructuralAccess::SharedBorrow,
            active_at_call: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalSuspensionLoanResource {
    #[default]
    Unresolved,
    DirectRoot,
    Reborrow,
    IncomingReference,
    CompletedOutput,
    LifetimeDependency,
    OutputInput,
    DescriptorTransfer,
}

/// The address captured by the named operation, not a path to reevaluate at
/// the crossing. The carrier and its selector dependencies must remain in the
/// activation's nonmoving backing while the call is incomplete. No retirement,
/// restoration or cancellation settlement is implied by retaining this row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TerminalSuspensionLoanCapture {
    #[default]
    Unresolved,
    Reference {
        formation: OperationId,
        carrier: semantic_vocabulary::PlaceId,
        source: semantic_vocabulary::PlaceId,
        source_access: StructuralAccess,
        source_path: HandleSpan<StructuralPathSegment>,
    },
    /// A leaf of an already retained value in this plan's ordered roster.
    /// The path selects its captured descriptor, never a source expression to
    /// evaluate again. An empty path retains the ordinary borrowed operand.
    RetainedValue {
        live_value: u32,
        path: HandleSpan<StructuralPathSegment>,
    },
    /// The output leaf established by this operation's normal completion.
    CompletedOutput {
        call: OperationId,
        argument: StructuralArgument,
        path: HandleSpan<StructuralPathSegment>,
    },
    /// Contents observed at this earlier call's entry, before its writes.
    CallInput {
        call: OperationId,
        argument: StructuralArgument,
        path: HandleSpan<StructuralPathSegment>,
        stored: bool,
    },
    /// A nonspatial obligation; its input links contain the actual captures.
    InputDependencies,
    DescriptorTransfer,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TerminalSuspensionLoanBacking {
    #[default]
    Unresolved,
    /// Canonical live-value roster index of the retained owner. Its enclosing
    /// activation lifetime covers the incomplete call, never a descriptor slot
    /// standing in for an incoming referent.
    RetainedOwner {
        live_value: u32,
    },
    /// The call's incoming referent namespace. The argument is an ingress
    /// coordinate, not proof that its descriptor owns or pins that storage.
    IncomingReferent {
        argument: StructuralArgument,
    },
    CompletedOutput {
        loan: Handle<TerminalSuspensionLoan>,
    },
    InputDependencies,
    /// Backing observed by this row's original CallInput capture.
    CapturedInput,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalSuspensionValueLoans {
    #[default]
    Unresolved,
    ViewFree,
    Retained {
        links: HandleSpan<Handle<TerminalSuspensionLoan>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionCallTarget {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
    /// The public slot owns the shape; the call's captured selector selects its row.
    Dynamic {
        interface: u32,
        slot: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSuspensionLiveValue {
    pub place: TerminalSuspensionPlace,
    pub value_type: TerminalSuspensionValueType,
    pub storage: TerminalSuspensionStorage,
    /// Complete live Terminal claims attached to this exact place.
    pub claim_count: u32,
    pub claims: Vec<ClaimId>,
    pub effective: CarryPolicy,
    pub loans: TerminalSuspensionValueLoans,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionPlace {
    Scalar(ValueId),
    /// Retain the ordinary storage operand, including its access. A physical
    /// pointer and path do not distinguish ownership from a loan; this row
    /// records the existing authority without establishing backing or pinning.
    Structural(StructuralArgument),
}

impl TerminalSuspensionPlace {
    /// Scalar identities are retained uses even when they occur only in a
    /// projected storage coordinate rather than as a carried scalar value.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) {
        match self {
            Self::Scalar(value) => visit(*value),
            Self::Structural(argument) => {
                for segment in &argument.path {
                    if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        visit(*index);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionValueType {
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionStorage {
    Parameter,
    Local,
    CallArgument,
}

pub fn suspension_frontier_commitment(
    identities: &TerminalIdentityTable,
    plan: &TerminalSuspensionCallPlan,
) -> [u8; 32] {
    let mut hash =
        crate::digest_encoder::DomainDigestEncoder::new(b"terminal-suspension-frontier-v9\0");
    hash.raw(&plan.operation.get().to_le_bytes());
    hash.raw(&plan.crossing.get().to_le_bytes());
    match plan.target {
        TerminalSuspensionCallTarget::Machine(machine) => {
            hash.byte(1);
            hash.raw(&machine.get().to_le_bytes());
        }
        TerminalSuspensionCallTarget::Boundary(boundary) => {
            hash.byte(2);
            hash.raw(&boundary.get().to_le_bytes());
        }
        TerminalSuspensionCallTarget::Dynamic { interface, slot } => {
            hash.byte(5);
            hash.raw(&interface.to_le_bytes());
            hash.raw(&slot.to_le_bytes());
        }
    }
    hash_carry_policy(&mut hash, plan.effective);
    hash.raw(&plan.live_value_count.to_le_bytes());
    hash.u64(plan.live_values.len() as u64);
    for live in &plan.live_values {
        match &live.place {
            TerminalSuspensionPlace::Scalar(value) => {
                hash.byte(1);
                hash.raw(&value.get().to_le_bytes());
            }
            TerminalSuspensionPlace::Structural(argument) => {
                hash.byte(2);
                hash.raw(&argument.place.get().to_le_bytes());
                hash_access(&mut hash, argument.access);
                hash.u64(argument.path.len() as u64);
                for segment in &argument.path {
                    hash_loan_path_segment(identities, &mut hash, segment);
                }
            }
        }
        match live.value_type {
            TerminalSuspensionValueType::Scalar(scalar) => {
                hash.byte(1);
                hash_scalar_type(&mut hash, scalar);
            }
            TerminalSuspensionValueType::Structural(structural) => {
                hash.byte(2);
                hash.raw(&structural.get().to_le_bytes());
            }
        }
        hash.byte(match live.storage {
            TerminalSuspensionStorage::Parameter => 2,
            TerminalSuspensionStorage::Local => 3,
            TerminalSuspensionStorage::CallArgument => 4,
        });
        hash.raw(&live.claim_count.to_le_bytes());
        hash.u64(live.claims.len() as u64);
        for claim in &live.claims {
            hash.raw(&claim.get().to_le_bytes());
        }
        hash_carry_policy(&mut hash, live.effective);
        match live.loans {
            TerminalSuspensionValueLoans::Unresolved => hash.byte(0),
            TerminalSuspensionValueLoans::ViewFree => hash.byte(1),
            TerminalSuspensionValueLoans::Retained { links } => {
                hash.byte(2);
                hash_span(&mut hash, links);
            }
        }
    }
    hash_loans(identities, &mut hash, &plan.loans);
    hash.finish()
}

impl TerminalSuspensionCallPlan {
    /// Captured selectors are uses of their formation-time values even when
    /// the later call no longer names those values in its ordinary operands.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) {
        for live in &self.live_values {
            live.place.visit_value_ids(&mut visit);
        }
        for (_, path) in self.loans.paths.iter() {
            if let StructuralPathSegment::RuntimeIndex { index, .. } = path {
                visit(*index);
            }
        }
        for (_, loan) in self.loans.occurrences.iter() {
            if let TerminalSuspensionLoanCapture::CompletedOutput { argument, .. }
            | TerminalSuspensionLoanCapture::CallInput { argument, .. } = &loan.capture
            {
                for segment in &argument.path {
                    if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        visit(*index);
                    }
                }
            }
            if let TerminalSuspensionLoanBacking::IncomingReferent { argument } = &loan.backing {
                for segment in &argument.path {
                    if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        visit(*index);
                    }
                }
            }
        }
    }
}

fn hash_handle<T>(hash: &mut crate::digest_encoder::DomainDigestEncoder, handle: Handle<T>) {
    hash.raw(&handle.arena_index().to_le_bytes());
    hash.raw(&handle.generation().to_le_bytes());
}

fn hash_span<T>(hash: &mut crate::digest_encoder::DomainDigestEncoder, span: HandleSpan<T>) {
    hash_handle(hash, span.start());
    hash.raw(&span.count().to_le_bytes());
}

fn hash_loans(
    identities: &TerminalIdentityTable,
    hash: &mut crate::digest_encoder::DomainDigestEncoder,
    loans: &TerminalSuspensionLoans,
) {
    hash.u64(loans.paths.len() as u64);
    for (_, segment) in loans.paths.iter() {
        hash_loan_path_segment(identities, hash, segment);
    }
    hash.u64(loans.value_links.len() as u64);
    for (_, link) in loans.value_links.iter() {
        hash_handle(hash, *link);
    }
    hash.u64(loans.supplier_links.len() as u64);
    for (_, supplier) in loans.supplier_links.iter() {
        hash_handle(hash, *supplier);
    }
    hash.u64(loans.input_links.len() as u64);
    for (_, input) in loans.input_links.iter() {
        hash_handle(hash, *input);
    }
    hash.u64(loans.occurrences.len() as u64);
    for (_, loan) in loans.occurrences.iter() {
        match &loan.capture {
            TerminalSuspensionLoanCapture::Unresolved => hash.byte(0),
            TerminalSuspensionLoanCapture::Reference {
                formation,
                carrier,
                source,
                source_access,
                source_path,
            } => {
                hash.byte(1);
                hash.raw(&formation.get().to_le_bytes());
                hash.raw(&carrier.get().to_le_bytes());
                hash.raw(&source.get().to_le_bytes());
                hash_access(hash, *source_access);
                hash_span(hash, *source_path);
            }
            TerminalSuspensionLoanCapture::RetainedValue { live_value, path } => {
                hash.byte(2);
                hash.raw(&live_value.to_le_bytes());
                hash_span(hash, *path);
            }
            TerminalSuspensionLoanCapture::CompletedOutput {
                call,
                argument,
                path,
            } => {
                hash.byte(3);
                hash.raw(&call.get().to_le_bytes());
                hash_argument(identities, hash, argument);
                hash_span(hash, *path);
            }
            TerminalSuspensionLoanCapture::CallInput {
                call,
                argument,
                path,
                stored,
            } => {
                hash.byte(4);
                hash.raw(&call.get().to_le_bytes());
                hash_argument(identities, hash, argument);
                hash_span(hash, *path);
                hash.byte(u8::from(*stored));
            }
            TerminalSuspensionLoanCapture::InputDependencies => hash.byte(5),
            TerminalSuspensionLoanCapture::DescriptorTransfer => hash.byte(6),
        }
        match &loan.backing {
            TerminalSuspensionLoanBacking::Unresolved => hash.byte(0),
            TerminalSuspensionLoanBacking::RetainedOwner { live_value } => {
                hash.byte(1);
                hash.raw(&live_value.to_le_bytes());
            }
            TerminalSuspensionLoanBacking::IncomingReferent { argument } => {
                hash.byte(2);
                hash.raw(&argument.place.get().to_le_bytes());
                hash_access(hash, argument.access);
                hash.u64(argument.path.len() as u64);
                for segment in &argument.path {
                    hash_loan_path_segment(identities, hash, segment);
                }
            }
            TerminalSuspensionLoanBacking::CompletedOutput { loan } => {
                hash.byte(3);
                hash_handle(hash, *loan);
            }
            TerminalSuspensionLoanBacking::InputDependencies => hash.byte(4),
            TerminalSuspensionLoanBacking::CapturedInput => hash.byte(5),
        }
        hash.byte(match loan.resource {
            TerminalSuspensionLoanResource::Unresolved => 0,
            TerminalSuspensionLoanResource::DirectRoot => 1,
            TerminalSuspensionLoanResource::Reborrow => 2,
            TerminalSuspensionLoanResource::IncomingReference => 3,
            TerminalSuspensionLoanResource::CompletedOutput => 4,
            TerminalSuspensionLoanResource::LifetimeDependency => 5,
            TerminalSuspensionLoanResource::OutputInput => 6,
            TerminalSuspensionLoanResource::DescriptorTransfer => 7,
        });
        hash_handle(hash, loan.parent);
        hash_span(hash, loan.suppliers);
        hash_span(hash, loan.inputs);
        hash_access(hash, loan.access);
        hash.byte(u8::from(loan.active_at_call));
    }
}

fn hash_access(hash: &mut crate::digest_encoder::DomainDigestEncoder, access: StructuralAccess) {
    hash.byte(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

fn hash_loan_path_segment(
    identities: &TerminalIdentityTable,
    hash: &mut crate::digest_encoder::DomainDigestEncoder,
    segment: &StructuralPathSegment,
) {
    match segment {
        StructuralPathSegment::AllElements => hash.byte(10),
        StructuralPathSegment::Case(case) => {
            hash.byte(7);
            hash.raw(&case.get().to_le_bytes());
        }
        StructuralPathSegment::Field(field) => {
            hash.byte(1);
            let field = identities.text(*field).unwrap_or_default();
            hash.u64(field.len() as u64);
            hash.raw(field.as_bytes());
        }
        StructuralPathSegment::FixedIndex(index) => {
            hash.byte(2);
            hash.raw(&index.to_le_bytes());
        }
        StructuralPathSegment::Referent => hash.byte(3),
        StructuralPathSegment::FixedRange { start, end } => {
            hash.byte(4);
            hash.raw(&start.to_le_bytes());
            hash.raw(&end.to_le_bytes());
        }
        StructuralPathSegment::RuntimeIndex { index, obligation } => {
            hash.byte(5);
            hash.raw(&index.get().to_le_bytes());
            hash.raw(&obligation.get().to_le_bytes());
        }
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            hash.byte(8);
            hash.raw(&structural_type.get().to_le_bytes());
            hash.byte(u8::from(stored_scalar.is_some()));
            if let Some(scalar) = stored_scalar {
                hash_scalar_type(hash, *scalar);
            }
        }
    }
}

fn hash_scalar_type(hash: &mut crate::digest_encoder::DomainDigestEncoder, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => hash.byte(1),
        ScalarType::Integer(integer) => {
            hash.byte(2);
            hash.byte(match integer.carrier() {
                IntegerCarrier::Fixed => 1,
                IntegerCarrier::Address => 2,
            });
            hash.byte(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            hash.raw(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            hash.byte(3);
            hash.byte(match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}

fn hash_carry_policy(hash: &mut crate::digest_encoder::DomainDigestEncoder, policy: CarryPolicy) {
    hash.byte(u8::from(
        policy.suspension == language_semantics::CarrySuspension::Allowed,
    ));
    hash.byte(u8::from(policy.cpu == language_semantics::CarryCpu::Any));
    hash.byte(u8::from(
        policy.host_thread == language_semantics::CarryHostThread::Any,
    ));
    hash.byte(u8::from(
        policy.address == language_semantics::CarryAddress::Movable,
    ));
}

fn hash_argument(
    identities: &TerminalIdentityTable,
    hash: &mut crate::digest_encoder::DomainDigestEncoder,
    argument: &StructuralArgument,
) {
    hash.raw(&argument.place.get().to_le_bytes());
    hash_access(hash, argument.access);
    hash.u64(argument.path.len() as u64);
    for segment in &argument.path {
        hash_loan_path_segment(identities, hash, segment);
    }
}
