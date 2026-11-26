//! Canonical post-inference machine identity and discharge materialization for
//! conformance witnesses.
//!
//! `prepare_discharge` finishes every module-wide inference pass before per-machine
//! proof checking runs. Two conformance witnesses often share structurally
//! identical helpers — or identical witness bodies — whose module-local
//! declarations (`MachineId`, `StructuralTypeId`, `StructuralDomainId`,
//! `ServiceId`, `BoundaryMachineId`, `TerminalIdentityIndex`, and `CycleComponentId`)
//! carry different dense numbers.
//!
//! When a witness machine stays within the conservative portable subset here,
//! `canonicalize_witness_machine` alpha-renames every reachable module-local ID
//! in deterministic depth-first order while preserving machine-local IDs
//! (`BlockId`, `EdgeId`, `OperationId`, `ValueId`, `PlaceId`, `ClaimId`,
//! `ObligationId`, `ExactPlaceId`, `WriteId`, `InvocationId`) literally.
//! Representatives run `check_single_machine` once per equivalence class, and
//! duplicates receive remapped verdicts, knowledge rows, propositions, and
//! fresh `branch_warnings::Deferred` contexts using the inverse bijection.
//!
//! Any unsupported construct or any representative refusal / target condition /
//! remapping failure returns `None` or `Err(())` cleanly so the caller runs
//! `check_single_machine` directly and preserves exact local diagnostics.

use std::cmp::Ordering;

use arena::NumericId;
use language_core::BindingRelevance;
use proof_admission::{AcceptedFact, AcceptedFactRoute, CertificateEnvelope, ProofNode, ProofRule};
use semantic_vocabulary::{
    AuthoredFieldStorage, BoundaryMachineId, CanonicalStructuralPathSegment, ContractId,
    CrashCause, CycleComponentId, DomainSemanticId, EdgeId, IntegerMathLiteral, IntegerMathTerm,
    IntegerType, IntegerWidth, InvocationSite, MachineId, PlaceId, Proposition, PropositionContext,
    ScalarApplicationId, ScalarDomainId, ScalarQualificationSetId, ScalarReadProjection,
    ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralCaseSubject, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, Termination, WidthParameter,
};
use terminal_check_tables::bindings::{Content as WriteContent, PlaceOrigin, PlaceRow, WriteRow};
use terminal_check_tables::edges::{
    Actual, Callee, Destination, Disposition, EdgeRow, Fate, InvocationRow,
};
use terminal_check_tables::obligations::{
    Lender, MembershipSubject, ObligationKind, ObligationRow,
};
use terminal_check_tables::values::{Justification, KnowledgeRow, ValueOrigin, ValueRow};
use terminal_check_tables::{MachineTables, ProgramPoint, Refusal, RefusalSite};
use terminal_psi::{
    ArrayExtent, Block, BoundaryMachineDeclaration, BoundaryMachineResult, ByteSequenceCarrier,
    CallAcknowledgement, CallFormation, CallOrigin, CallSite, CheckedBoundaryCallable,
    CheckedBoundaryExecution, CheckedMachineCapture, CheckedMachineDeclaration,
    CheckedMachineSpecialization, ClosedReachApplication, ClosedReachCall, ClosedReachCapture,
    ClosedReachParameter, CompletionReceipt, ContractClause, ElementViewRegion, EntryClaim,
    ExecutionSite, FloatResultPolicy, LayoutMeasurement, MachineContract,
    NominalDeclarationIdentity, Operation, OperationKind, OperationResult, PublishedWriteFrame,
    RecordFieldValue, ReferenceStorageView, ResolvedStructuralPathSegment,
    ResultQualificationEstablishment, RuntimeDomainMembership, ScalarApplication,
    ScalarBlockInvariant, ScalarDomainDeclaration, ScalarQualificationSet, ServiceDeclaration,
    StaticRequirementDispatch, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralCaseDeclaration, StructuralDomainAccessPlan, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathQualification, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralRecordStorage, StructuralReferentIdentity, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TargetMachineCallable,
    TerminalAffineCleanupAction, TerminalIdentityIndex, TerminalIdentityTable,
    TerminalIntegerRankValue, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalNaturalCycle, TerminalRankedScc, Terminator,
};

use crate::graph::Graph;
use crate::machine_inputs::MachineInputs;
use crate::{
    Discharged, ElaborationView, Evidence, FiniteIntegerRanking, MachineDischarge, ModuleReplays,
    StorageUse, WindowUse, branch_warnings, equality, laws, minted,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalMachineKey {
    pub(crate) hash: u64,
    pub(crate) bytes: Vec<u8>,
}

impl Ord for CanonicalMachineKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash
            .cmp(&other.hash)
            .then_with(|| self.bytes.cmp(&other.bytes))
    }
}

impl PartialOrd for CanonicalMachineKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Bijection<Id: Copy + Eq> {
    forward_dense: Vec<u32>,
    forward_sparse: Vec<(u64, u32)>,
    reverse: Vec<Id>,
}

impl<Id: Copy + Eq> Default for Bijection<Id> {
    fn default() -> Self {
        Self {
            forward_dense: Vec::new(),
            forward_sparse: Vec::new(),
            reverse: Vec::new(),
        }
    }
}

const DENSE_BIJECTION_LIMIT: usize = 16_384;

impl<Id: NumericId> Bijection<Id> {
    fn intern(&mut self, id: Id) -> (u32, bool) {
        let raw = id.get();
        if let Ok(index) = usize::try_from(raw)
            && index < DENSE_BIJECTION_LIMIT
        {
            if let Some(&existing) = self.forward_dense.get(index)
                && existing != 0
            {
                return (existing, false);
            }
            if self.forward_dense.len() <= index {
                self.forward_dense.resize(index + 1, 0);
            }
            self.reverse.push(id);
            let canonical = u32::try_from(self.reverse.len()).expect("canonical id fits in u32");
            self.forward_dense[index] = canonical;
            return (canonical, true);
        }
        if let Some((_, existing)) = self.forward_sparse.iter().find(|(k, _)| *k == raw) {
            return (*existing, false);
        }
        self.reverse.push(id);
        let canonical = u32::try_from(self.reverse.len()).expect("canonical id fits in u32");
        self.forward_sparse.push((raw, canonical));
        (canonical, true)
    }

    fn forward(&self, id: Id) -> Option<u32> {
        let raw = id.get();
        if let Ok(index) = usize::try_from(raw)
            && index < DENSE_BIJECTION_LIMIT
        {
            return self.forward_dense.get(index).copied().filter(|&v| v != 0);
        }
        self.forward_sparse
            .iter()
            .find_map(|(k, v)| (*k == raw).then_some(*v))
    }

    fn inverse(&self, canonical: u32) -> Option<Id> {
        let index = usize::try_from(canonical.checked_sub(1)?).ok()?;
        self.reverse.get(index).copied()
    }
}

#[derive(Default)]
struct ScalarSetMap {
    forward: Vec<(ScalarQualificationSetId, u32)>,
}

impl ScalarSetMap {
    fn intern(&mut self, id: ScalarQualificationSetId) -> (u32, bool) {
        if id == ScalarQualificationSetId::ZERO {
            return (0, false);
        }
        if let Some((_, existing)) = self.forward.iter().find(|(k, _)| *k == id) {
            return (*existing, false);
        }
        let canonical = u32::try_from(self.forward.len() + 1).expect("scalar set id fits in u32");
        self.forward.push((id, canonical));
        (canonical, true)
    }
}

#[derive(Default)]
struct IdentMap {
    dense: Vec<u32>,
    texts: Vec<String>,
}

impl IdentMap {
    fn intern(
        &mut self,
        identities: &TerminalIdentityTable,
        index: TerminalIdentityIndex,
    ) -> Result<(u32, bool), ()> {
        if index == TerminalIdentityIndex::ABSENT {
            return Ok((0, false));
        }
        if identities.is_invocation_local(index) {
            return Err(());
        }
        let raw = index.0 as usize;
        if let Some(&canonical) = self.dense.get(raw)
            && canonical != 0
        {
            return Ok((canonical, false));
        }
        let text = identities.text(index).ok_or(())?;
        let (canonical, is_new) = match self.texts.iter().position(|existing| existing == text) {
            Some(pos) => (u32::try_from(pos + 1).map_err(|_| ())?, false),
            None => {
                self.texts.push(text.to_owned());
                (u32::try_from(self.texts.len()).map_err(|_| ())?, true)
            }
        };
        if self.dense.len() <= raw {
            self.dense.resize(raw + 1, 0);
        }
        self.dense[raw] = canonical;
        Ok((canonical, is_new))
    }

    fn text_of(&self, canonical: u32) -> Option<&str> {
        let index = usize::try_from(canonical.checked_sub(1)?).ok()?;
        self.texts.get(index).map(String::as_str)
    }
}

#[derive(Default)]
struct CycleMap {
    forward: Vec<(CycleComponentId, u32)>,
    reverse: Vec<CycleComponentId>,
}

impl CycleMap {
    fn intern(&mut self, id: CycleComponentId) -> (u32, bool) {
        if let Some((_, canonical)) = self.forward.iter().find(|(existing, _)| *existing == id) {
            return (*canonical, false);
        }
        self.reverse.push(id);
        let canonical = u32::try_from(self.reverse.len()).expect("cycle id fits in u32");
        self.forward.push((id, canonical));
        (canonical, true)
    }

    fn forward(&self, id: CycleComponentId) -> Option<u32> {
        self.forward
            .iter()
            .find_map(|(existing, canonical)| (*existing == id).then_some(*canonical))
    }

    fn inverse(&self, canonical: u32) -> Option<CycleComponentId> {
        let index = usize::try_from(canonical.checked_sub(1)?).ok()?;
        self.reverse.get(index).copied()
    }
}

#[derive(Default)]
pub(crate) struct CanonicalIdMaps {
    machines: Bijection<MachineId>,
    structural_types: Bijection<StructuralTypeId>,
    structural_fields: Bijection<StructuralFieldId>,
    structural_cases: Bijection<StructuralCaseId>,
    structural_domains: Bijection<StructuralDomainId>,
    scalar_domains: Bijection<ScalarDomainId>,
    scalar_sets: ScalarSetMap,
    domain_semantics: Bijection<DomainSemanticId>,
    services: Bijection<ServiceId>,
    boundaries: Bijection<BoundaryMachineId>,
    contracts: Bijection<ContractId>,
    scalar_applications: Bijection<ScalarApplicationId>,
    identities: IdentMap,
    cycles: CycleMap,
}

pub(crate) struct CanonicalizedMachine {
    pub(crate) key: CanonicalMachineKey,
    maps: CanonicalIdMaps,
}

struct KeyWriter {
    hash: u64,
    bytes: Vec<u8>,
}

impl Default for KeyWriter {
    fn default() -> Self {
        Self {
            hash: 0xcbf2_9ce4_8422_2325,
            bytes: Vec::with_capacity(512),
        }
    }
}

impl KeyWriter {
    fn push_bytes(&mut self, slice: &[u8]) {
        for &byte in slice {
            self.hash ^= u64::from(byte);
            self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.bytes.extend_from_slice(slice);
    }

    fn u8(&mut self, value: u8) {
        self.push_bytes(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.push_bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.push_bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.push_bytes(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.push_bytes(&value.to_le_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.push_bytes(&value.to_le_bytes());
    }

    fn id64<Id: NumericId>(&mut self, id: Id) {
        self.u64(id.get());
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn str(&mut self, value: &str) {
        self.usize(value.len());
        self.push_bytes(value.as_bytes());
    }

    fn finish(self) -> CanonicalMachineKey {
        CanonicalMachineKey {
            hash: self.hash,
            bytes: self.bytes,
        }
    }
}

#[derive(Default)]
struct PendingQueues {
    machines: Vec<MachineId>,
    structural_types: Vec<StructuralTypeId>,
    structural_domains: Vec<StructuralDomainId>,
    scalar_domains: Vec<ScalarDomainId>,
    scalar_sets: Vec<ScalarQualificationSetId>,
    services: Vec<ServiceId>,
    boundaries: Vec<BoundaryMachineId>,
    scalar_applications: Vec<ScalarApplicationId>,
}

impl PendingQueues {
    fn is_empty(&self) -> bool {
        self.machines.is_empty()
            && self.structural_types.is_empty()
            && self.structural_domains.is_empty()
            && self.scalar_domains.is_empty()
            && self.scalar_sets.is_empty()
            && self.services.is_empty()
            && self.boundaries.is_empty()
            && self.scalar_applications.is_empty()
    }
}

struct CanonicalEncoder<'a> {
    view: &'a ElaborationView<'a>,
    module: &'a TerminalModule,
    maps: CanonicalIdMaps,
    pending: PendingQueues,
    writer: KeyWriter,
}

impl<'a> CanonicalEncoder<'a> {
    fn new(view: &'a ElaborationView<'a>) -> Self {
        Self {
            view,
            module: view.module,
            maps: CanonicalIdMaps::default(),
            pending: PendingQueues::default(),
            writer: KeyWriter::default(),
        }
    }

    fn machine(&mut self, id: MachineId) {
        let (canonical, is_new) = self.maps.machines.intern(id);
        if is_new {
            self.pending.machines.push(id);
        }
        self.writer.u32(canonical);
    }

    fn structural_type(&mut self, id: StructuralTypeId) {
        let (canonical, is_new) = self.maps.structural_types.intern(id);
        if is_new {
            self.pending.structural_types.push(id);
        }
        self.writer.u32(canonical);
    }

    fn structural_field(&mut self, id: StructuralFieldId) {
        let (canonical, _) = self.maps.structural_fields.intern(id);
        self.writer.u32(canonical);
    }

    fn structural_case(&mut self, id: StructuralCaseId) {
        let (canonical, _) = self.maps.structural_cases.intern(id);
        self.writer.u32(canonical);
    }

    fn structural_domain(&mut self, id: StructuralDomainId) {
        let (canonical, is_new) = self.maps.structural_domains.intern(id);
        if is_new {
            self.pending.structural_domains.push(id);
        }
        self.writer.u32(canonical);
    }

    fn scalar_domain(&mut self, id: ScalarDomainId) {
        let (canonical, is_new) = self.maps.scalar_domains.intern(id);
        if is_new {
            self.pending.scalar_domains.push(id);
        }
        self.writer.u32(canonical);
    }

    fn scalar_set(&mut self, id: ScalarQualificationSetId) {
        let (canonical, is_new) = self.maps.scalar_sets.intern(id);
        if is_new {
            self.pending.scalar_sets.push(id);
        }
        self.writer.u32(canonical);
    }

    fn domain_semantic(&mut self, id: DomainSemanticId) {
        let (canonical, _) = self.maps.domain_semantics.intern(id);
        self.writer.u32(canonical);
    }

    fn service(&mut self, id: ServiceId) {
        let (canonical, is_new) = self.maps.services.intern(id);
        if is_new {
            self.pending.services.push(id);
        }
        self.writer.u32(canonical);
    }

    fn boundary(&mut self, id: BoundaryMachineId) {
        let (canonical, is_new) = self.maps.boundaries.intern(id);
        if is_new {
            self.pending.boundaries.push(id);
        }
        self.writer.u32(canonical);
    }

    fn contract(&mut self, id: ContractId) {
        let (canonical, _) = self.maps.contracts.intern(id);
        self.writer.u32(canonical);
    }

    fn scalar_application(&mut self, id: ScalarApplicationId) {
        let (canonical, is_new) = self.maps.scalar_applications.intern(id);
        if is_new {
            self.pending.scalar_applications.push(id);
        }
        self.writer.u32(canonical);
    }

    fn identity(&mut self, idx: TerminalIdentityIndex) -> Result<(), ()> {
        let (canonical, is_new) = self.maps.identities.intern(&self.module.identities, idx)?;
        self.writer.u32(canonical);
        if is_new && let Some(text) = self.maps.identities.text_of(canonical) {
            let owned = text.to_owned();
            self.writer.str(&owned);
        }
        Ok(())
    }

    fn target_callable(&mut self, target: TargetMachineCallable) {
        match target {
            TargetMachineCallable::Machine(id) => {
                self.writer.u8(0);
                self.machine(id);
            }
            TargetMachineCallable::Boundary(id) => {
                self.writer.u8(1);
                self.boundary(id);
            }
        }
    }

    fn option<T>(
        &mut self,
        value: Option<T>,
        mut encode: impl FnMut(&mut Self, T) -> Result<(), ()>,
    ) -> Result<(), ()> {
        match value {
            None => self.writer.bool(false),
            Some(inner) => {
                self.writer.bool(true);
                encode(self, inner)?;
            }
        }
        Ok(())
    }

    fn slice<T>(
        &mut self,
        items: &[T],
        mut encode: impl FnMut(&mut Self, &T) -> Result<(), ()>,
    ) -> Result<(), ()> {
        self.writer.usize(items.len());
        for item in items {
            encode(self, item)?;
        }
        Ok(())
    }

    fn integer_type(&mut self, integer: IntegerType) {
        self.writer.u8(integer.sign() as u8);
        self.writer.bool(integer.is_address());
        match integer.width() {
            IntegerWidth::Bits(bits) => {
                self.writer.u8(0);
                self.writer.u16(bits);
            }
            IntegerWidth::Parameter(parameter) => {
                self.writer.u8(1);
                self.machine(parameter.owner);
                self.writer.id64(parameter.value);
            }
        }
    }

    fn integer_value(&mut self, value: semantic_vocabulary::IntegerValue) {
        match value {
            semantic_vocabulary::IntegerValue::Unsigned(v) => {
                self.writer.u8(0);
                self.writer.u128(v);
            }
            semantic_vocabulary::IntegerValue::Signed(v) => {
                self.writer.u8(1);
                self.writer.i128(v);
            }
        }
    }

    fn scalar_type(&mut self, ty: ScalarType) -> Result<(), ()> {
        match ty {
            ScalarType::Boolean => self.writer.u8(0),
            ScalarType::Integer(int) => {
                self.writer.u8(1);
                self.integer_type(int);
            }
            ScalarType::IeeeFloat(format) => {
                self.writer.u8(2);
                self.writer.u8(format as u8);
            }
            ScalarType::Callable(_) => return Err(()),
        }
        Ok(())
    }

    fn qualified_scalar_type(
        &mut self,
        ty: semantic_vocabulary::QualifiedScalarType,
    ) -> Result<(), ()> {
        self.scalar_type(ty.scalar_type)?;
        self.scalar_set(ty.qualifications);
        Ok(())
    }

    fn multiplicity(&mut self, mult: StructuralMultiplicity) {
        self.writer.u8(match mult {
            StructuralMultiplicity::Unrestricted => 0,
            StructuralMultiplicity::Affine => 1,
            StructuralMultiplicity::Linear => 2,
        });
    }

    fn access(&mut self, access: StructuralAccess) {
        self.writer.u8(match access {
            StructuralAccess::Owned => 0,
            StructuralAccess::SharedBorrow => 1,
            StructuralAccess::MutableBorrow => 2,
            StructuralAccess::WriteOnlyBorrow => 3,
        });
    }

    fn canonical_segment(&mut self, seg: CanonicalStructuralPathSegment) {
        match seg {
            CanonicalStructuralPathSegment::Field(field) => {
                self.writer.u8(0);
                self.structural_field(field);
            }
            CanonicalStructuralPathSegment::FixedIndex(idx) => {
                self.writer.u8(1);
                self.writer.u64(idx);
            }
            CanonicalStructuralPathSegment::Case(case) => {
                self.writer.u8(2);
                self.structural_case(case);
            }
            CanonicalStructuralPathSegment::Referent => {
                self.writer.u8(3);
            }
        }
    }

    fn resolved_segment(&mut self, seg: ResolvedStructuralPathSegment) -> Result<(), ()> {
        match seg {
            ResolvedStructuralPathSegment::Field(field) => {
                self.writer.u8(0);
                self.structural_field(field);
            }
            ResolvedStructuralPathSegment::Case(case) => {
                self.writer.u8(1);
                self.structural_case(case);
            }
            ResolvedStructuralPathSegment::FixedIndex(idx) => {
                self.writer.u8(2);
                self.writer.u64(idx);
            }
            ResolvedStructuralPathSegment::RuntimeIndex { selector } => {
                self.writer.u8(3);
                self.writer.id64(selector);
            }
            ResolvedStructuralPathSegment::Extent => self.writer.u8(4),
            ResolvedStructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                self.writer.u8(5);
                self.structural_type(structural_type);
                self.option(stored_scalar, |s, st| s.scalar_type(st))?;
            }
            ResolvedStructuralPathSegment::Referent => self.writer.u8(6),
            ResolvedStructuralPathSegment::UnknownContents => self.writer.u8(7),
        }
        Ok(())
    }

    fn structural_path_segment(&mut self, seg: &StructuralPathSegment) -> Result<(), ()> {
        match seg {
            StructuralPathSegment::AllElements => self.writer.u8(0),
            StructuralPathSegment::Case(case) => {
                self.writer.u8(1);
                self.structural_case(*case);
            }
            StructuralPathSegment::Field(identity) => {
                self.writer.u8(2);
                self.identity(*identity)?;
            }
            StructuralPathSegment::FixedIndex(idx) => {
                self.writer.u8(3);
                self.writer.u64(*idx);
            }
            StructuralPathSegment::FixedRange { start, end } => {
                self.writer.u8(4);
                self.writer.u64(*start);
                self.writer.u64(*end);
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                self.writer.u8(5);
                self.writer.id64(*index);
                self.writer.id64(*obligation);
            }
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                self.writer.u8(6);
                self.structural_type(*structural_type);
                self.option(*stored_scalar, |s, st| s.scalar_type(st))?;
            }
            StructuralPathSegment::Referent => self.writer.u8(7),
        }
        Ok(())
    }

    fn scalar_term(&mut self, term: &ScalarTerm) -> Result<(), ()> {
        match term {
            ScalarTerm::DeclarationApplication { .. } => return Err(()),
            ScalarTerm::Application {
                application,
                scalar_type,
            } => {
                self.writer.u8(1);
                self.scalar_application(*application);
                self.scalar_type(*scalar_type)?;
            }
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                self.writer.u8(2);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for seg in path {
                    match seg {
                        ScalarReadProjection::Referent => self.writer.u8(0),
                        ScalarReadProjection::Extent => self.writer.u8(1),
                        ScalarReadProjection::Static(cseg) => {
                            self.writer.u8(2);
                            self.canonical_segment(*cseg);
                        }
                        ScalarReadProjection::Index(inner) => {
                            self.writer.u8(3);
                            self.scalar_term(inner)?;
                        }
                    }
                }
                self.scalar_type(*scalar_type)?;
            }
            ScalarTerm::IeeeFloatLiteral(val) => {
                self.writer.u8(3);
                self.ieee_float_value(*val);
            }
            ScalarTerm::Value { id, scalar_type } => {
                self.writer.u8(4);
                self.writer.id64(*id);
                self.scalar_type(*scalar_type)?;
            }
            ScalarTerm::BooleanField {
                version,
                root,
                path,
            } => {
                self.writer.u8(5);
                self.content_place_version(*version);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
            }
            ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            } => {
                self.writer.u8(6);
                self.content_place_version(*version);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
                self.integer_type(*scalar_type);
            }
            ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            } => {
                self.writer.u8(7);
                self.content_place_version(*version);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
                self.writer.u8(*format as u8);
            }
            ScalarTerm::ViewExtent {
                root,
                path,
                scalar_type,
            } => {
                self.writer.u8(8);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
                self.integer_type(*scalar_type);
            }
            ScalarTerm::Boolean(b) => {
                self.writer.u8(9);
                self.writer.bool(*b);
            }
            ScalarTerm::BooleanNot { operand } => {
                self.writer.u8(10);
                self.scalar_term(operand)?;
            }
            ScalarTerm::BooleanEqual { left, right } => {
                self.writer.u8(11);
                self.scalar_term(left)?;
                self.scalar_term(right)?;
            }
            ScalarTerm::IntegerEqual {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::IntegerLessThan {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                self.writer.u8(scalar_binary_tag(term));
                self.integer_type(*scalar_type);
                self.scalar_term(left)?;
                self.scalar_term(right)?;
            }
            ScalarTerm::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                self.writer.u8(30);
                self.integer_type(*scalar_type);
                self.scalar_term(operand)?;
            }
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                self.writer.u8(31);
                self.integer_type(*source_type);
                self.integer_type(*target_type);
                self.scalar_term(operand)?;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                self.writer.u8(32);
                self.integer_type(*source_type);
                self.integer_type(*target_type);
                self.scalar_term(operand)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                let tag = match term {
                    ScalarTerm::WrappingIntegerShiftLeft { .. } => 33,
                    ScalarTerm::WrappingIntegerShiftRight { .. } => 34,
                    ScalarTerm::ExactIntegerShiftLeft { .. } => 35,
                    _ => 36,
                };
                self.writer.u8(tag);
                self.integer_type(*value_type);
                self.integer_type(*count_type);
                self.scalar_term(value)?;
                self.scalar_term(count)?;
            }
            ScalarTerm::Integer { scalar_type, value } => {
                self.writer.u8(37);
                self.integer_type(*scalar_type);
                self.integer_value(*value);
            }
            ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                self.writer.u8(38);
                self.writer.u8(*predicate as u8);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
            }
        }
        Ok(())
    }

    fn ieee_float_value(&mut self, val: semantic_vocabulary::IeeeFloatValue) {
        match val {
            semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                self.writer.u8(0);
                self.writer.u32(bits);
            }
            semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                self.writer.u8(1);
                self.writer.u64(bits);
            }
        }
    }

    fn content_place_version(&mut self, version: semantic_vocabulary::ContentPlaceVersion) {
        self.writer.u8(match version {
            semantic_vocabulary::ContentPlaceVersion::Entry => 0,
            semantic_vocabulary::ContentPlaceVersion::Current => 1,
        });
    }

    fn integer_math_term(&mut self, term: &IntegerMathTerm) -> Result<(), ()> {
        match term {
            IntegerMathTerm::IntegerLiteral(lit) => {
                self.writer.u8(0);
                let lit: IntegerMathLiteral = *lit;
                self.writer.bool(lit.negative());
                self.writer.u128(lit.magnitude());
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                self.writer.u8(1);
                self.integer_type(*source_type);
                self.writer.id64(*value);
            }
            IntegerMathTerm::Embed(scalar) => {
                self.writer.u8(2);
                self.scalar_term(scalar)?;
            }
            IntegerMathTerm::Add(l, r)
            | IntegerMathTerm::Subtract(l, r)
            | IntegerMathTerm::Multiply(l, r) => {
                let tag = match term {
                    IntegerMathTerm::Add(..) => 3,
                    IntegerMathTerm::Subtract(..) => 4,
                    _ => 5,
                };
                self.writer.u8(tag);
                self.integer_math_term(l)?;
                self.integer_math_term(r)?;
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                self.writer.u8(6);
                self.integer_math_term(value)?;
                self.integer_math_term(count)?;
            }
        }
        Ok(())
    }

    fn structural_case_subject(&mut self, subject: &StructuralCaseSubject) {
        self.writer.id64(subject.root());
        self.writer.usize(subject.path().len());
        for &seg in subject.path() {
            self.canonical_segment(seg);
        }
    }

    fn proposition(&mut self, proposition: &Proposition) -> Result<(), ()> {
        match proposition {
            Proposition::Truth => self.writer.u8(0),
            Proposition::Falsehood => self.writer.u8(1),
            Proposition::Atom(id) => {
                self.writer.u8(2);
                self.writer.id64(*id);
            }
            Proposition::Equal(l, r)
            | Proposition::LessThan(l, r)
            | Proposition::LessOrEqual(l, r) => {
                let tag = match proposition {
                    Proposition::Equal(..) => 3,
                    Proposition::LessThan(..) => 4,
                    _ => 5,
                };
                self.writer.u8(tag);
                self.scalar_term(l)?;
                self.scalar_term(r)?;
            }
            Proposition::IntegerMathEqual(l, r)
            | Proposition::IntegerMathLessThan(l, r)
            | Proposition::IntegerMathLessOrEqual(l, r) => {
                let tag = match proposition {
                    Proposition::IntegerMathEqual(..) => 6,
                    Proposition::IntegerMathLessThan(..) => 7,
                    _ => 8,
                };
                self.writer.u8(tag);
                self.integer_math_term(l)?;
                self.integer_math_term(r)?;
            }
            Proposition::IeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                self.writer.u8(9);
                self.writer.u8(*kind as u8);
                self.writer.u8(*format as u8);
                self.writer.id64(left.root());
                self.writer.usize(left.path().len());
                for &seg in left.path() {
                    self.canonical_segment(seg);
                }
                self.writer.id64(right.root());
                self.writer.usize(right.path().len());
                for &seg in right.path() {
                    self.canonical_segment(seg);
                }
            }
            Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                self.writer.u8(10);
                self.writer.u8(*kind as u8);
                self.writer.u8(*format as u8);
                self.scalar_term(left)?;
                self.scalar_term(right)?;
            }
            Proposition::StructuralCaseMembership { subject, case } => {
                self.writer.u8(11);
                self.structural_case_subject(subject);
                self.structural_case(*case);
            }
            Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                self.writer
                    .u8(if matches!(proposition, Proposition::Conjunction(_)) {
                        12
                    } else {
                        13
                    });
                self.slice(parts, |s, part| s.proposition(part))?;
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.writer.u8(14);
                self.proposition(premise)?;
                self.proposition(conclusion)?;
            }
            Proposition::ContentConservation(_) | Proposition::FloatIntegerConversion(_) => {
                return Err(());
            }
            Proposition::IeeeFloatIntegerExact { source, target } => {
                self.writer.u8(17);
                self.scalar_term(source)?;
                self.integer_type(*target);
            }
            Proposition::IntegerIeeeFloatExact { source, target } => {
                self.writer.u8(18);
                self.scalar_term(source)?;
                self.writer.u8(*target as u8);
            }
            Proposition::DeclarationIdentity(clause) => {
                if !clause.is_portable() {
                    return Err(());
                }
                self.writer.u8(19);
                self.writer.u32(clause.left);
                self.writer.u32(clause.right);
                self.writer.usize(clause.values.len());
                for value in &clause.values {
                    match value.carrier {
                        semantic_vocabulary::ClauseCarrier::Parameter(p) => {
                            self.writer.u8(0);
                            self.writer.u32(p);
                        }
                        semantic_vocabulary::ClauseCarrier::Scalar(st) => {
                            self.writer.u8(1);
                            self.scalar_type(st)?;
                        }
                        semantic_vocabulary::ClauseCarrier::Structural(st) => {
                            self.writer.u8(2);
                            self.structural_type(st);
                        }
                        semantic_vocabulary::ClauseCarrier::FloatMeaning(fmt) => {
                            self.writer.u8(3);
                            self.writer.u8(fmt as u8);
                        }
                    }
                    match &value.operand {
                        semantic_vocabulary::ClauseOperand::Scalar(term) => {
                            self.writer.u8(0);
                            self.scalar_term(term)?;
                        }
                        semantic_vocabulary::ClauseOperand::Input(subject) => {
                            self.writer.u8(1);
                            self.structural_case_subject(subject);
                        }
                        semantic_vocabulary::ClauseOperand::Formal(_)
                        | semantic_vocabulary::ClauseOperand::Application { .. }
                        | semantic_vocabulary::ClauseOperand::FloatMeaning(_) => return Err(()),
                    }
                }
            }
        }
        Ok(())
    }

    fn structural_argument(&mut self, arg: &StructuralArgument) -> Result<(), ()> {
        self.writer.id64(arg.place);
        self.slice(&arg.path, Self::structural_path_segment)
    }

    fn path_qualification(&mut self, qual: &StructuralPathQualification) -> Result<(), ()> {
        self.slice(&qual.path, Self::structural_path_segment)?;
        self.structural_domain(qual.domain);
        Ok(())
    }

    fn proof_term(&mut self, _term: &semantic_vocabulary::ProofTerm) -> Result<(), ()> {
        Err(())
    }

    fn nominal_identity(&mut self, id: &NominalDeclarationIdentity) -> Result<(), ()> {
        self.identity(id.identity())
    }

    fn checked_declaration(&mut self, decl: &CheckedMachineDeclaration) -> Result<(), ()> {
        self.nominal_identity(&decl.machine)?;
        self.nominal_identity(&decl.state)?;
        self.identity(decl.semantic_declaration)?;
        self.slice(&decl.parameter_types, |s, idx| s.identity(*idx))?;
        self.identity(decl.callable)?;
        self.identity(decl.result_type)?;
        self.writer.push_bytes(&decl.contract);
        self.writer.bool(decl.published_may_suspend);
        self.writer.bool(decl.published_may_block);
        self.option(decl.specialization.as_ref(), |s, spec| {
            s.checked_specialization(spec)
        })
    }

    fn checked_specialization(&mut self, spec: &CheckedMachineSpecialization) -> Result<(), ()> {
        self.option(spec.capture.as_ref(), |s, cap: &CheckedMachineCapture| {
            s.identity(cap.template_identity)?;
            s.writer.push_bytes(&cap.template_commitment);
            s.writer.push_bytes(&cap.specialization_commitment);
            Ok(())
        })?;
        self.nominal_identity(&spec.template)?;
        self.nominal_identity(&spec.template_entry)?;
        self.identity(spec.template_callable)?;
        self.identity(spec.template_semantic_declaration)?;
        self.writer.push_bytes(&spec.commitment);
        Ok(())
    }

    fn establishment_route(&mut self, route: &StructuralEstablishmentRoute) -> Result<(), ()> {
        match route {
            StructuralEstablishmentRoute::CheckedRequirement {
                declaration,
                requirement_identity,
            } => {
                self.writer.u8(0);
                self.nominal_identity(declaration)?;
                self.identity(*requirement_identity)?;
            }
            StructuralEstablishmentRoute::ExactMachine {
                declaration,
                machine_identity,
            } => {
                self.writer.u8(1);
                self.nominal_identity(declaration)?;
                self.identity(*machine_identity)?;
            }
            StructuralEstablishmentRoute::BoundaryRequirement {
                declaration,
                requirement_identity,
            } => {
                self.writer.u8(2);
                self.nominal_identity(declaration)?;
                self.identity(*requirement_identity)?;
            }
        }
        Ok(())
    }

    fn crash_cause(&mut self, cause: CrashCause) {
        self.writer.u8(match cause {
            CrashCause::Trap => 0,
            CrashCause::Abort => 1,
        });
    }

    fn crash_bucket(&mut self, bucket: &terminal_psi::CrashRouteBucket) -> Result<(), ()> {
        self.crash_cause(bucket.cause);
        self.slice(&bucket.alternatives, |s, alt| match alt {
            terminal_psi::CrashRouteGuard::Truth => {
                s.writer.u8(0);
                Ok(())
            }
            terminal_psi::CrashRouteGuard::Predicate(term) => {
                s.writer.u8(1);
                s.proposition(term.proposition())
            }
        })
    }

    fn runtime_membership(&mut self, mem: &RuntimeDomainMembership) -> Result<(), ()> {
        match mem {
            RuntimeDomainMembership::Scalar { subject, domain } => {
                self.writer.u8(0);
                self.scalar_term(subject)?;
                self.scalar_domain(*domain);
            }
            RuntimeDomainMembership::Structural {
                root,
                path,
                version,
                domain,
            } => {
                self.writer.u8(1);
                self.writer.id64(*root);
                self.writer.usize(path.len());
                for &seg in path {
                    self.canonical_segment(seg);
                }
                self.content_place_version(*version);
                self.structural_domain(*domain);
            }
        }
        Ok(())
    }

    fn machine_result(&mut self, result: &TerminalMachineResult) -> Result<(), ()> {
        match result {
            TerminalMachineResult::Unit => self.writer.u8(0),
            TerminalMachineResult::Scalar(decl) => {
                self.writer.u8(1);
                self.writer.id64(decl.id);
                self.qualified_scalar_type(decl.value_type())?;
            }
            TerminalMachineResult::Structural(res) => {
                self.writer.u8(2);
                self.structural_result_decl(res)?;
            }
        }
        Ok(())
    }

    fn structural_result_decl(&mut self, res: &StructuralResultDeclaration) -> Result<(), ()> {
        self.writer.id64(res.place);
        self.structural_type(res.structural_type);
        self.multiplicity(res.multiplicity);
        self.slice(&res.qualifications, |s, &dom| {
            s.structural_domain(dom);
            Ok(())
        })?;
        self.slice(&res.projected_qualifications, Self::path_qualification)?;
        if !res.reference_sources.is_empty() {
            return Err(());
        }
        Ok(())
    }

    fn machine_contract(&mut self, contract: &MachineContract) -> Result<(), ()> {
        self.contract(contract.id);
        self.option(contract.selected_application.as_ref(), |s, dispatch| {
            s.static_requirement_dispatch(dispatch)
        })?;
        self.slice(&contract.parameters, |s, v| {
            s.writer.id64(v.id);
            s.qualified_scalar_type(v.value_type())
        })?;
        self.slice(&contract.parameter_order, |s, kind| {
            s.writer.u8(match kind {
                terminal_psi::BoundaryParameterKind::Scalar => 0,
                terminal_psi::BoundaryParameterKind::Structural => 1,
            });
            Ok(())
        })?;
        self.slice(&contract.structural_parameters, |s, p| {
            s.structural_parameter(p)
        })?;
        self.machine_result(&contract.result)?;
        if contract
            .result_lifetimes
            .as_ref()
            .is_some_and(|lifetimes| !lifetimes.is_empty())
        {
            return Err(());
        }
        self.writer.bool(contract.result_lifetimes.is_some());
        self.slice(&contract.entry_claims, Self::entry_claim)?;
        self.option(contract.declared_service_reach.as_ref(), |s, services| {
            s.slice(services, |s, &id| {
                s.service(id);
                Ok(())
            })
        })?;
        self.option(
            contract.declared_synchronous_entries.as_ref(),
            |s, services| {
                s.slice(services, |s, &id| {
                    s.service(id);
                    Ok(())
                })
            },
        )?;
        self.option(contract.declared_crash_routes.as_ref(), |s, buckets| {
            s.slice(buckets, Self::crash_bucket)
        })?;
        self.option(contract.declared_suspension, |s, v| {
            s.writer.bool(v);
            Ok(())
        })?;
        self.option(contract.declared_blocking, |s, v| {
            s.writer.bool(v);
            Ok(())
        })?;
        self.writer.bool(contract.published_may_suspend);
        self.writer.bool(contract.published_may_block);
        self.option(contract.closed_reach_application.as_ref(), |s, app| {
            s.closed_reach_application(app)
        })?;
        self.slice(&contract.published_service_ceiling, |s, &id| {
            s.service(id);
            Ok(())
        })?;
        self.slice(&contract.published_synchronous_entries, |s, &id| {
            s.service(id);
            Ok(())
        })?;
        self.option(
            contract.published_writes.as_ref(),
            |s, writes: &Vec<(u32, Vec<StructuralFieldId>)>| {
                s.slice(writes, |s, (ordinal, fields)| {
                    s.writer.u32(*ordinal);
                    s.slice(fields, |s, &f| {
                        s.structural_field(f);
                        Ok(())
                    })
                })
            },
        )?;
        let _: &PublishedWriteFrame = &contract.published_writes;
        self.writer.u8(match contract.termination {
            Termination::Terminates => 0,
            Termination::NoGuarantee => 1,
        });
        if !contract.content_entry_claims.is_empty() {
            return Err(());
        }
        self.slice(&contract.crash_routes, Self::crash_bucket)?;
        self.slice(&contract.erased_scalar_formals, |s, v| {
            s.writer.id64(v.id);
            s.qualified_scalar_type(v.value_type())
        })?;
        if !contract.erased_proof_formals.is_empty() {
            return Err(());
        }
        self.slice(&contract.membership_ensures, Self::runtime_membership)?;
        self.slice(&contract.requires, Self::proposition)?;
        self.slice(&contract.source_checked_requires, Self::proposition)?;
        self.slice(&contract.ensures, |s, clause: &ContractClause| {
            s.writer.id64(clause.obligation);
            s.proposition(&clause.proposition)
        })?;
        let _ = core::mem::size_of::<OutcomeSpecificEnsureCheck>();
        if !contract.outcome_specific_ensures.is_empty() {
            return Err(());
        }
        Ok(())
    }

    fn structural_parameter(&mut self, p: &StructuralParameterDeclaration) -> Result<(), ()> {
        self.writer.u32(p.position);
        self.writer.id64(p.place);
        self.structural_type(p.structural_type);
        self.multiplicity(p.multiplicity);
        self.access(p.access);
        self.slice(&p.qualifications, |s, &dom| {
            s.structural_domain(dom);
            Ok(())
        })?;
        self.slice(&p.projected_qualifications, Self::path_qualification)?;
        self.slice(&p.additional_input_qualifications, Self::path_qualification)?;
        Ok(())
    }

    fn entry_claim(&mut self, claim: &EntryClaim) -> Result<(), ()> {
        self.writer.id64(claim.claim);
        self.writer.id64(claim.input);
        self.slice(&claim.path, Self::structural_path_segment)
    }

    fn static_requirement_dispatch(&mut self, d: &StaticRequirementDispatch) -> Result<(), ()> {
        self.writer
            .u64(d.conformance_application_report_fingerprint);
        self.writer
            .push_bytes(&d.conformance_application_commitment.as_bytes());
        self.identity(d.public_requirement_identity)?;
        self.identity(d.declaring_trait_identity)?;
        self.identity(d.requirement_identity)?;
        self.identity(d.realization_identity)?;
        self.identity(d.realization_callable_identity)?;
        self.machine(d.realization);
        Ok(())
    }

    fn closed_reach_parameter(&mut self, param: &ClosedReachParameter) -> Result<(), ()> {
        match param {
            ClosedReachParameter::Type { argument } => {
                self.writer.u8(0);
                self.writer.str(argument);
            }
            ClosedReachParameter::Const { .. } => {
                return Err(());
            }
            ClosedReachParameter::Proposition { argument } => {
                self.writer.u8(2);
                self.writer.str(argument);
            }
            ClosedReachParameter::Machine(binding) => {
                self.writer.u8(3);
                self.option(binding.nominal_requirement.as_deref(), |s, name| {
                    s.writer.str(name);
                    Ok(())
                })?;
                self.writer.usize(binding.requirement_parameter_kinds.len());
                self.writer.push_bytes(&binding.requirement_parameter_kinds);
                self.slice(&binding.requirement_arguments, |s, &idx| s.identity(idx))?;
                self.option(binding.upper_bound.as_ref(), |s, list| {
                    s.slice(list, |s, &id| {
                        s.service(id);
                        Ok(())
                    })
                })?;
                self.writer.bool(binding.upper_bound_may_suspend);
                self.writer.bool(binding.upper_bound_may_block);
                self.writer.bool(binding.upper_bound_crashes_none);
                self.writer.bool(binding.selected_may_suspend);
                self.writer.bool(binding.selected_may_block);
                self.writer.bool(binding.selected_may_crash);
                self.identity(binding.selected_identity)?;
                self.writer
                    .push_bytes(&binding.selected_contract_commitment);
                self.slice(&binding.selected_reach, |s, &id| {
                    s.service(id);
                    Ok(())
                })?;
                self.option(binding.callee, |s, callee| {
                    s.machine(callee);
                    Ok(())
                })?;
                self.option(binding.schema.as_ref(), |s, schema| {
                    s.identity(schema.template_identity)?;
                    s.writer.push_bytes(&schema.template_commitment);
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    fn closed_reach_application(&mut self, app: &ClosedReachApplication) -> Result<(), ()> {
        self.identity(app.template_identity)?;
        self.writer.push_bytes(&app.template_base_commitment);
        self.writer.push_bytes(&app.template_commitment);
        self.writer.push_bytes(&app.specialization_commitment);
        self.writer.usize(app.parameter_kinds.len());
        self.writer.push_bytes(&app.parameter_kinds);
        self.slice(&app.telescope, Self::closed_reach_parameter)?;
        self.option(app.capture.as_ref(), |s, cap: &ClosedReachCapture| {
            s.identity(cap.template_identity)?;
            s.writer.push_bytes(&cap.template_base_commitment);
            s.writer.push_bytes(&cap.template_commitment);
            s.writer.push_bytes(&cap.specialization_commitment);
            s.writer.usize(cap.parameter_kinds.len());
            s.writer.push_bytes(&cap.parameter_kinds);
            s.slice(&cap.telescope, Self::closed_reach_parameter)
        })?;
        self.slice(&app.fixed, |s, &id| {
            s.service(id);
            Ok(())
        })?;
        self.slice(&app.dependencies, |s, &dep| {
            s.writer.u32(dep);
            Ok(())
        })?;
        self.slice(&app.calls, |s, call: &ClosedReachCall| {
            s.invocation_site(call.site);
            s.writer.u32(call.binder);
            s.option(call.application.as_ref(), |s, inner| {
                s.machine(inner.callee);
                s.writer.push_bytes(&inner.specialization_commitment);
                if !inner.arguments.is_empty() {
                    return Err(());
                }
                Ok(())
            })
        })
    }

    fn invocation_site(&mut self, site: InvocationSite) {
        match site {
            InvocationSite::Operation(op) => {
                self.writer.u8(0);
                self.writer.id64(op);
            }
            InvocationSite::ExternalCompletion(edge) => {
                self.writer.u8(1);
                self.writer.id64(edge);
            }
            InvocationSite::InvokeNoNormal(edge) => {
                self.writer.u8(2);
                self.writer.id64(edge);
            }
            InvocationSite::Cleanup {
                edge,
                action_ordinal,
            } => {
                self.writer.u8(3);
                self.writer.id64(edge);
                self.writer.u32(action_ordinal);
            }
        }
    }

    fn encode_machine_body(&mut self, id: MachineId) -> Result<(), ()> {
        let lowered = crate::terminal_machine(&self.module.machines, id, id).map_err(|_| ())?;
        if lowered.assembly_target_requirement != Default::default() {
            return Err(());
        }
        // Machine roles & parameter bindings from ElaborationView.
        self.writer.bool(self.view.requirements.contains(&id));
        self.writer.bool(self.view.applications.contains(&id));
        self.writer.bool(self.view.local_results.contains(&id));
        self.writer
            .usize(self.view.value_parameters.get(id).copied().unwrap_or(0));
        match self.view.value_arguments.get(id) {
            None => self.writer.bool(false),
            Some(args) => {
                self.writer.bool(true);
                self.slice(args, Self::scalar_term)?;
            }
        }
        // Scalar block invariants belonging to this machine.
        let matching_invariants: Vec<&ScalarBlockInvariant> = self
            .module
            .proofs
            .scalar_block_invariants
            .iter()
            .filter(|inv| inv.machine == id)
            .collect();
        self.writer.usize(matching_invariants.len());
        for inv in matching_invariants {
            self.writer.id64(inv.header);
            self.proposition(&inv.predicate)?;
            self.writer.usize(inv.arrivals.len());
            for arrival in &inv.arrivals {
                self.writer.id64(arrival.edge);
                self.writer.id64(arrival.obligation);
            }
        }
        // Nominal call arguments whose caller is this machine.
        let matching_nominal: Vec<_> = self
            .view
            .nominal_calls
            .iter()
            .filter(|call| call.caller == id)
            .collect();
        self.writer.usize(matching_nominal.len());
        for call in matching_nominal {
            self.machine(call.caller);
            self.writer.id64(call.operation);
            self.machine(call.callee);
            self.writer.u32(call.argument);
            self.structural_type(call.actual);
            self.structural_type(call.expected);
            self.structural_type(call.actual_constructor);
            self.structural_type(call.expected_constructor);
            self.identity(call.declaration)?;
            self.identity(call.actual_application)?;
            self.identity(call.expected_application)?;
            self.writer.usize(call.const_arguments.len());
            for arg in &call.const_arguments {
                self.writer.u32(arg.parameter);
                self.scalar_term(&arg.actual)?;
            }
        }
        // TerminalMachine header and body.
        self.slice(&lowered.call_formations, |s, cf: &CallFormation| {
            match cf.site {
                CallSite::Operation(op) => {
                    s.writer.u8(0);
                    s.writer.id64(op);
                }
                CallSite::Edge(edge) => {
                    s.writer.u8(1);
                    s.writer.id64(edge);
                }
            }
            s.call_acknowledgement(cf.acknowledgement);
            s.option(cf.tail_callee, |s, callee| {
                s.machine(callee);
                Ok(())
            })
        })?;
        self.option(lowered.attachment, |s, ty| {
            s.structural_type(ty);
            Ok(())
        })?;
        self.option(lowered.ranked_scc.as_ref(), |s, scc| {
            match scc {
                TerminalRankedScc::Natural(cycles) => {
                    s.writer.usize(cycles.len());
                    for cycle in cycles {
                        let comp = terminal_semantics::control_graph::control_cycle_identity(
                            lowered, cycle,
                        );
                        let (canon_comp, _) = s.maps.cycles.intern(comp);
                        s.writer.u32(canon_comp);
                        s.encode_natural_cycle(cycle)?;
                    }
                }
            }
            Ok(())
        })?;
        if !lowered.content_identity_reshuffles.is_empty()
            || !lowered.content_partition_compositions.is_empty()
        {
            return Err(());
        }
        self.slice(
            &lowered.structural_places,
            |s, p: &StructuralPlaceDeclaration| {
                s.writer.id64(p.id);
                s.structural_place_kind(p.kind);
                Ok(())
            },
        )?;
        self.slice(&lowered.block_entry_claims, Self::entry_claim)?;
        if !lowered.loan_completions.is_empty() {
            return Err(());
        }
        self.writer.id64(lowered.entry);
        self.slice(&lowered.blocks, Self::block)?;
        self.machine_contract(&lowered.contract)?;
        Ok(())
    }

    fn call_acknowledgement(&mut self, ack: CallAcknowledgement) {
        self.writer.u8(match ack.origin {
            CallOrigin::Authored => 0,
            CallOrigin::CompilerGenerated => 1,
        });
        self.writer.bool(ack.acknowledges_suspend);
        self.writer.bool(ack.acknowledges_block);
        self.writer.bool(ack.suspension_position);
    }

    fn structural_place_kind(&mut self, kind: StructuralPlaceKind) {
        match kind {
            StructuralPlaceKind::Parameter { position } => {
                self.writer.u8(0);
                self.writer.u32(position);
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                self.writer.u8(1);
                self.writer.id64(block);
                self.writer.u32(position);
            }
            StructuralPlaceKind::Result => self.writer.u8(2),
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                self.writer.u8(3);
                self.writer.id64(producer);
                self.structural_type(structural_type);
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => {
                self.writer.u8(4);
                self.writer.u32(declaration_ordinal);
                self.structural_type(structural_type);
            }
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => {
                self.writer.u8(5);
                self.structural_type(attachment);
                self.structural_field(field);
                self.boundary(boundary);
            }
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction,
            } => {
                self.writer.u8(6);
                self.writer.u32(declaration_ordinal);
                self.structural_type(structural_type);
                self.writer.bool(construction.is_some());
                if let Some(el) = construction {
                    self.structural_type(el.root_structural_type);
                    self.writer.u64(el.index);
                }
            }
        }
    }

    fn encode_natural_cycle(&mut self, cycle: &TerminalNaturalCycle) -> Result<(), ()> {
        self.writer.usize(cycle.ranks.len());
        for rank in &cycle.ranks {
            self.writer.id64(rank.block);
            let vals = cycle.observations.span(rank.values).ok_or(())?;
            self.writer.usize(vals.len());
            for v in vals {
                self.integer_type(v.integer_type);
                self.writer.id64(v.value);
            }
        }
        self.writer.usize(cycle.edges.len());
        for edge in &cycle.edges {
            self.writer.id64(edge.edge);
            self.writer.id64(edge.source);
            self.writer.id64(edge.target);
            self.writer.u8(edge.comparison as u8);
            let vals = cycle.observations.span(edge.successor_ranks).ok_or(())?;
            self.writer.usize(vals.len());
            for v in vals {
                self.integer_type(v.integer_type);
                self.writer.id64(v.value);
            }
        }
        Ok(())
    }

    fn block(&mut self, block: &Block) -> Result<(), ()> {
        self.writer.id64(block.id);
        self.slice(&block.parameters, |s, v| {
            s.writer.id64(v.id);
            s.qualified_scalar_type(v.value_type())
        })?;
        self.slice(&block.structural_parameters, Self::structural_parameter)?;
        self.slice(&block.operations, Self::operation)?;
        self.terminator(&block.terminator)
    }

    fn operation(&mut self, op: &Operation) -> Result<(), ()> {
        if op.static_reach_binding.is_some() || op.suspension_crossing.is_some() {
            return Err(());
        }
        self.option(op.required_contract, |s, c| {
            s.contract(c);
            Ok(())
        })?;
        self.writer.id64(op.id);
        match &op.result {
            OperationResult::Unit => self.writer.u8(0),
            OperationResult::Scalar(decl) => {
                self.writer.u8(1);
                self.writer.id64(decl.id);
                self.qualified_scalar_type(decl.value_type())?;
            }
            OperationResult::Structural(res) => {
                self.writer.u8(2);
                self.structural_operation_result(res)?;
            }
        }
        self.operation_kind(&op.kind)
    }

    fn structural_operation_result(&mut self, res: &StructuralOperationResult) -> Result<(), ()> {
        self.writer.id64(res.place);
        self.structural_type(res.structural_type);
        self.multiplicity(res.multiplicity);
        self.slice(&res.qualifications, |s, &dom| {
            s.structural_domain(dom);
            Ok(())
        })?;
        self.slice(&res.projected_qualifications, Self::path_qualification)?;
        self.slice(
            &res.additional_input_qualifications,
            Self::path_qualification,
        )?;
        self.slice(
            &res.qualification_establishments,
            |s, est: &ResultQualificationEstablishment| {
                s.structural_domain(est.domain);
                s.writer.u32(est.route);
                Ok(())
            },
        )?;
        self.slice(&res.claims, |s, binding| {
            s.writer.id64(binding.claim);
            s.slice(&binding.path, Self::structural_path_segment)
        })
    }

    fn operation_kind(&mut self, kind: &OperationKind) -> Result<(), ()> {
        match kind {
            OperationKind::CallDynamic { .. }
            | OperationKind::SelectImplementation { .. }
            | OperationKind::SelectMachine { .. }
            | OperationKind::CallMachine { .. } => return Err(()),
            OperationKind::Call {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                requirement_obligations,
                crash_continuations,
            } => {
                self.writer.u8(1);
                self.machine(*callee);
                self.encode_call_tail(
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    &[],
                    &[],
                    requirement_obligations,
                    crash_continuations,
                )?;
            }
            OperationKind::CallUnit {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                self.writer.u8(2);
                self.machine(*callee);
                self.encode_call_tail(
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                )?;
            }
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                self.writer.u8(3);
                self.machine(*callee);
                self.encode_call_tail(
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                )?;
            }
            OperationKind::CallStructural {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            } => {
                self.writer.u8(4);
                self.machine(*callee);
                self.encode_call_tail(
                    &[],
                    &[],
                    &[],
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                )?;
                if !selected_evidence.is_empty() {
                    return Err(());
                }
            }
            OperationKind::CallStructuralWithScalarArguments {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            } => {
                self.writer.u8(5);
                self.machine(*callee);
                self.encode_call_tail(
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                )?;
                if !selected_evidence.is_empty() {
                    return Err(());
                }
            }
            OperationKind::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                completion_receipts,
                requirement_obligations,
            } => {
                self.writer.u8(6);
                self.boundary(*boundary);
                self.slice(arguments, |s, &v| {
                    s.writer.id64(v);
                    Ok(())
                })?;
                self.slice(structural_arguments, Self::structural_argument)?;
                self.slice(completion_receipts, |s, receipt: &CompletionReceipt| {
                    s.writer.id64(receipt.claim);
                    s.writer.u32(receipt.argument_index);
                    s.slice(&receipt.path, Self::structural_path_segment)
                })?;
                self.slice(requirement_obligations, |s, &ob| {
                    s.writer.id64(ob);
                    Ok(())
                })?;
            }
            OperationKind::ReadScalarApplication { application } => {
                self.writer.u8(7);
                self.scalar_application(*application);
            }
            OperationKind::EstablishNativeCodeLayout { machine } => {
                self.writer.u8(8);
                self.machine(*machine);
            }
            OperationKind::EstablishByteSequenceLiteral {
                destination,
                bytes,
                qualifications,
            } => {
                self.writer.u8(9);
                self.writer.id64(*destination);
                self.writer.usize(bytes.len());
                self.writer.push_bytes(bytes);
                self.slice(qualifications, |s, &dom| {
                    s.structural_domain(dom);
                    Ok(())
                })?;
            }
            OperationKind::EstablishElementView {
                destination,
                source,
                element,
                region,
            }
            | OperationKind::EstablishMutableElementView {
                destination,
                source,
                element,
                region,
            } => {
                let tag = if matches!(kind, OperationKind::EstablishElementView { .. }) {
                    10
                } else {
                    11
                };
                self.writer.u8(tag);
                self.writer.id64(*destination);
                self.structural_argument(source)?;
                self.structural_type(*element);
                match region {
                    ElementViewRegion::WholeCollection => self.writer.u8(0),
                    ElementViewRegion::WholeRepresentation {
                        source_length,
                        obligation,
                    } => {
                        self.writer.u8(1);
                        self.writer.id64(*source_length);
                        self.writer.id64(*obligation);
                    }
                    ElementViewRegion::RepresentationBytes {
                        start,
                        end,
                        obligation,
                    } => {
                        self.writer.u8(2);
                        self.writer.id64(*start);
                        self.writer.id64(*end);
                        self.writer.id64(*obligation);
                    }
                }
            }
            OperationKind::EstablishActivationClaim {
                bound,
                committed,
                backing_type,
                established_case,
                rejected_case,
            } => {
                self.writer.u8(12);
                self.writer.u64(*bound);
                self.writer.id64(*committed);
                self.structural_type(*backing_type);
                self.structural_case(*established_case);
                self.structural_case(*rejected_case);
            }
            other => {
                self.encode_remaining_operation_kind(other)?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_call_tail(
        &mut self,
        arguments: &[semantic_vocabulary::ValueId],
        erased_arguments: &[ScalarTerm],
        erased_proof_arguments: &[semantic_vocabulary::ProofTerm],
        structural_arguments: &[StructuralArgument],
        claim_transfers: &[terminal_psi::ClaimTransfer],
        requirement_obligations: &[semantic_vocabulary::ObligationId],
        crash_continuations: &[terminal_psi::CrashRouteBucket],
    ) -> Result<(), ()> {
        self.slice(arguments, |s, &v| {
            s.writer.id64(v);
            Ok(())
        })?;
        self.slice(erased_arguments, Self::scalar_term)?;
        self.slice(erased_proof_arguments, Self::proof_term)?;
        self.slice(structural_arguments, Self::structural_argument)?;
        self.slice(claim_transfers, |s, ct| {
            s.writer.id64(ct.claim);
            s.writer.u32(ct.argument_index);
            s.slice(&ct.path, Self::structural_path_segment)
        })?;
        self.slice(requirement_obligations, |s, &ob| {
            s.writer.id64(ob);
            Ok(())
        })?;
        self.slice(crash_continuations, Self::crash_bucket)
    }

    fn encode_remaining_operation_kind(&mut self, kind: &OperationKind) -> Result<(), ()> {
        match kind {
            OperationKind::IntegerConstant { value } => {
                self.writer.u8(20);
                self.integer_value(*value);
            }
            OperationKind::BooleanConstant { value } => {
                self.writer.u8(21);
                self.writer.bool(*value);
            }
            OperationKind::IeeeFloatConstant { value } => {
                self.writer.u8(22);
                self.ieee_float_value(*value);
            }
            OperationKind::FloatResultPolicy { policy } => {
                self.writer.u8(23);
                match policy {
                    FloatResultPolicy::Trapping { raw_result } => {
                        self.writer.u8(0);
                        self.writer.id64(*raw_result);
                    }
                    FloatResultPolicy::Saturating {
                        raw_result,
                        eligible,
                    } => {
                        self.writer.u8(1);
                        self.writer.id64(*raw_result);
                        self.writer.id64(*eligible);
                    }
                }
            }
            OperationKind::BooleanEqual { left, right }
            | OperationKind::IntegerEqual { left, right }
            | OperationKind::IntegerLessThan { left, right }
            | OperationKind::IntegerLessOrEqual { left, right }
            | OperationKind::IntegerBitwiseAnd { left, right }
            | OperationKind::IntegerBitwiseOr { left, right }
            | OperationKind::IntegerBitwiseXor { left, right }
            | OperationKind::WrappingIntegerAdd { left, right }
            | OperationKind::SaturatingIntegerAdd { left, right }
            | OperationKind::WrappingIntegerSubtract { left, right }
            | OperationKind::SaturatingIntegerSubtract { left, right }
            | OperationKind::WrappingIntegerMultiply { left, right }
            | OperationKind::SaturatingIntegerMultiply { left, right } => {
                self.writer.u8(24);
                self.writer.u8(simple_binary_op_tag(kind));
                self.writer.id64(*left);
                self.writer.id64(*right);
            }
            OperationKind::WrappingIntegerShiftLeft { value, count }
            | OperationKind::WrappingIntegerShiftRight { value, count } => {
                self.writer.u8(
                    if matches!(kind, OperationKind::WrappingIntegerShiftLeft { .. }) {
                        25
                    } else {
                        26
                    },
                );
                self.writer.id64(*value);
                self.writer.id64(*count);
            }
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            }
            | OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => {
                self.writer.u8(
                    if matches!(kind, OperationKind::ExactIntegerShiftLeft { .. }) {
                        27
                    } else {
                        28
                    },
                );
                self.writer.id64(*value);
                self.writer.id64(*count);
                self.writer.id64(*obligation);
            }
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            }
            | OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            }
            | OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            }
            | OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                self.writer.u8(29);
                self.writer.u8(checked_binary_op_tag(kind));
                self.writer.id64(*left);
                self.writer.id64(*right);
                self.writer.id64(*obligation);
            }
            OperationKind::IntegerBitwiseNot { operand }
            | OperationKind::IntegerWiden { operand } => {
                self.writer
                    .u8(if matches!(kind, OperationKind::IntegerBitwiseNot { .. }) {
                        30
                    } else {
                        31
                    });
                self.writer.id64(*operand);
            }
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => {
                self.writer.u8(32);
                self.writer.id64(*operand);
                self.writer.id64(*obligation);
            }
            OperationKind::EstablishZeroedStructural {} => self.writer.u8(33),
            OperationKind::EstablishStructuralLocal {} => self.writer.u8(34),
            OperationKind::EstablishTrivialAffineLocal { destination } => {
                self.writer.u8(35);
                self.writer.id64(*destination);
            }
            OperationKind::EstablishPrimitiveLocal {
                value,
                range_obligation,
            } => {
                self.writer.u8(36);
                self.writer.id64(*value);
                self.option(*range_obligation, |s, ob| {
                    s.writer.id64(ob);
                    Ok(())
                })?;
            }
            OperationKind::EstablishRecord { fields } => {
                self.writer.u8(37);
                self.slice(fields, |s, init| {
                    s.structural_field(init.field);
                    match &init.value {
                        RecordFieldValue::Scalar {
                            value,
                            range_obligation,
                        } => {
                            s.writer.u8(0);
                            s.writer.id64(*value);
                            s.option(*range_obligation, |s, ob| {
                                s.writer.id64(ob);
                                Ok(())
                            })?;
                        }
                        RecordFieldValue::Structural(arg) => {
                            s.writer.u8(1);
                            s.structural_argument(arg)?;
                        }
                    }
                    Ok(())
                })?;
            }
            OperationKind::EstablishScalarArray { elements } => {
                self.writer.u8(38);
                self.slice(elements, |s, &v| {
                    s.writer.id64(v);
                    Ok(())
                })?;
            }
            OperationKind::EstablishStructuralArray { elements } => {
                self.writer.u8(39);
                self.slice(elements, Self::structural_argument)?;
            }
            OperationKind::EstablishReference { source, storage } => {
                self.writer.u8(40);
                self.structural_argument(source)?;
                self.option(storage.as_ref(), |s, st: &ReferenceStorageView| {
                    s.writer.id64(st.byte_offset);
                    s.option(st.stored_scalar, |s, scalar| s.scalar_type(scalar))
                })?;
            }
            OperationKind::ReleaseReference { source, path } => {
                self.writer.u8(41);
                self.writer.id64(*source);
                self.slice(path, Self::structural_path_segment)?;
            }
            OperationKind::ByteSequenceLength { source } => {
                self.writer.u8(42);
                self.structural_argument(source)?;
            }
            OperationKind::ElementViewLength { source } => {
                self.writer.u8(43);
                self.structural_argument(source)?;
            }
            _ => return Err(()),
        }
        Ok(())
    }

    fn terminator(&mut self, terminator: &Terminator) -> Result<(), ()> {
        match terminator {
            Terminator::PendingNormalCompletion {
                call,
                scalars,
                storage,
            } => {
                self.writer.u8(0);
                self.writer.id64(*call);
                self.slice(scalars, |s, &v| {
                    s.writer.id64(v);
                    Ok(())
                })?;
                self.slice(storage, |s, &p| {
                    s.writer.id64(p);
                    Ok(())
                })?;
            }
            Terminator::EliminateEmpty { edge, source, path } => {
                self.writer.u8(1);
                self.writer.id64(*edge);
                self.writer.id64(*source);
                self.slice(path, Self::structural_path_segment)?;
            }
            Terminator::InvokeNoNormal {
                edge,
                static_reach_binding,
                required_contract,
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                if static_reach_binding.is_some() {
                    return Err(());
                }
                self.writer.u8(2);
                self.writer.id64(*edge);
                self.option(*required_contract, |s, c| {
                    s.contract(c);
                    Ok(())
                })?;
                self.machine(*callee);
                self.encode_call_tail(
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                )?;
            }
            Terminator::ExternalCompletion {
                edge,
                boundary,
                kind,
                arguments,
                structural_arguments,
                requirement_obligations,
                frontier_lower_bound,
            } => {
                self.writer.u8(3);
                self.writer.id64(*edge);
                self.boundary(*boundary);
                self.writer.u8(match kind {
                    semantic_vocabulary::ExternalCompletionKind::ProcessExit => 0,
                });
                self.slice(arguments, |s, &v| {
                    s.writer.id64(v);
                    Ok(())
                })?;
                self.slice(structural_arguments, Self::structural_argument)?;
                self.slice(requirement_obligations, |s, &ob| {
                    s.writer.id64(ob);
                    Ok(())
                })?;
                self.slice(frontier_lower_bound, |s, &c| {
                    s.writer.id64(c);
                    Ok(())
                })?;
            }
            Terminator::Jump {
                edge,
                target,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                cleanup_actions,
            } => {
                self.writer.u8(4);
                self.writer.id64(*edge);
                self.writer.id64(*target);
                self.slice(arguments, |s, &v| {
                    s.writer.id64(v);
                    Ok(())
                })?;
                self.slice(erased_arguments, Self::scalar_term)?;
                self.slice(erased_proof_arguments, Self::proof_term)?;
                self.slice(structural_arguments, Self::structural_argument)?;
                self.slice(cleanup_actions, Self::cleanup_action)?;
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                self.writer.u8(5);
                self.writer.id64(*condition);
                self.successor_edge(when_true)?;
                self.successor_edge(when_false)?;
            }
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                self.writer.u8(6);
                self.writer.id64(*source);
                self.slice(path, Self::structural_path_segment)?;
                self.slice(cases, |s, case_edge| {
                    s.writer.id64(case_edge.edge);
                    s.writer.id64(case_edge.target);
                    s.structural_case(case_edge.case);
                    s.slice(&case_edge.payload_fields, |s, &f| {
                        s.structural_field(f);
                        Ok(())
                    })?;
                    s.slice(&case_edge.structural_arguments, Self::structural_argument)?;
                    s.slice(&case_edge.cleanup_actions, Self::cleanup_action)
                })?;
            }
            Terminator::Return {
                edge,
                value,
                cleanup_actions,
            } => {
                self.writer.u8(7);
                self.writer.id64(*edge);
                self.writer.id64(*value);
                self.slice(cleanup_actions, Self::cleanup_action)?;
            }
            Terminator::ReturnUnit {
                edge,
                cleanup_actions,
            } => {
                self.writer.u8(8);
                self.writer.id64(*edge);
                self.slice(cleanup_actions, Self::cleanup_action)?;
            }
            Terminator::ReturnStructural {
                edge,
                source,
                returned_claims,
                cleanup_actions,
            } => {
                self.writer.u8(9);
                self.writer.id64(*edge);
                self.writer.id64(*source);
                self.slice(returned_claims, |s, claim| {
                    s.writer.id64(claim.claim);
                    s.slice(&claim.path, Self::structural_path_segment)
                })?;
                self.slice(cleanup_actions, Self::cleanup_action)?;
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                self.writer.u8(10);
                self.writer.id64(*edge);
                self.crash_cause(*cause);
                self.slice(site_guard, |s, term| s.proposition(term.proposition()))?;
                self.slice(frontier_lower_bound, |s, &c| {
                    s.writer.id64(c);
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    fn successor_edge(&mut self, edge: &terminal_psi::SuccessorEdge) -> Result<(), ()> {
        self.writer.id64(edge.edge);
        self.writer.id64(edge.target);
        self.slice(&edge.arguments, |s, &v| {
            s.writer.id64(v);
            Ok(())
        })?;
        self.slice(&edge.erased_arguments, Self::scalar_term)?;
        self.slice(&edge.erased_proof_arguments, Self::proof_term)?;
        self.slice(&edge.structural_arguments, Self::structural_argument)?;
        self.slice(&edge.cleanup_actions, Self::cleanup_action)
    }

    fn cleanup_action(&mut self, action: &TerminalAffineCleanupAction) -> Result<(), ()> {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                self.writer.u8(0);
                self.writer.id64(*place);
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                let discard: &StructuralAffineDiscard = discard;
                self.writer.u8(1);
                self.writer.id64(discard.place);
                self.slice(&discard.path, Self::structural_path_segment)?;
                self.structural_type(discard.structural_type);
            }
            TerminalAffineCleanupAction::InvokeOwned(cleanup) => {
                self.writer.u8(2);
                self.structural_argument(&cleanup.subject)?;
                self.structural_type(cleanup.structural_type);
                self.machine(cleanup.cleanup_machine);
                self.slice(&cleanup.claim_transfers, |s, ct| {
                    s.writer.id64(ct.claim);
                    s.writer.u32(ct.argument_index);
                    s.slice(&ct.path, Self::structural_path_segment)
                })?;
                self.slice(&cleanup.requirement_obligations, |s, &ob| {
                    s.writer.id64(ob);
                    Ok(())
                })?;
            }
            TerminalAffineCleanupAction::RetireHome(place) => {
                self.writer.u8(3);
                self.writer.id64(*place);
            }
        }
        Ok(())
    }

    fn root_tables(
        &mut self,
        tables: &MachineTables,
        inputs: &MachineInputs<'_>,
        verified_borrow_ends: Option<&[(EdgeId, PlaceId)]>,
    ) -> Result<(), ()> {
        // values
        self.writer.usize(tables.values.len());
        for (_, row) in tables.values.iter() {
            let row: &ValueRow = row;
            self.scalar_type(row.scalar_type)?;
            self.option(row.rigid, |s, r| {
                s.writer.u32(r.0);
                Ok(())
            })?;
            match row.origin {
                ValueOrigin::MachineParameter { position } => {
                    self.writer.u8(0);
                    self.writer.u32(position);
                }
                ValueOrigin::MachineResult => self.writer.u8(1),
                ValueOrigin::BlockParameter { block, position } => {
                    self.writer.u8(2);
                    self.writer.id64(block);
                    self.writer.u32(position);
                }
                ValueOrigin::Operation(op) => {
                    self.writer.u8(3);
                    self.writer.id64(op);
                }
                ValueOrigin::StorageContent { place, after } => {
                    self.writer.u8(4);
                    self.writer.id64(place);
                    self.option(after, |s, op| {
                        s.writer.id64(op);
                        Ok(())
                    })?;
                }
                ValueOrigin::StorageCase { place, case, after } => {
                    self.writer.u8(5);
                    self.writer.id64(place);
                    self.structural_case(case);
                    self.option(after, |s, op| {
                        s.writer.id64(op);
                        Ok(())
                    })?;
                }
            }
        }
        // places
        self.writer.usize(tables.places.len());
        for (_, place) in tables.places.iter() {
            let place: &PlaceRow = place;
            match place.origin {
                PlaceOrigin::Root(root) => {
                    self.writer.u8(0);
                    self.writer.id64(root);
                }
                PlaceOrigin::Projection { parent, segment } => {
                    self.writer.u8(1);
                    self.writer.id64(parent);
                    self.resolved_segment(segment)?;
                }
            }
            self.slice(&place.children, |s, &(seg, child)| {
                s.resolved_segment(seg)?;
                s.writer.id64(child);
                Ok(())
            })?;
            self.slice(&place.writes, |s, &w| {
                s.writer.id64(w);
                Ok(())
            })?;
        }
        // roots
        self.writer.usize(tables.roots.len());
        for (_, &exact) in tables.roots.iter() {
            self.writer.id64(exact);
        }
        // writes
        self.writer.usize(tables.writes.len());
        for (_, write) in tables.writes.iter() {
            self.encode_write_row(write);
        }
        // obligations
        self.writer.usize(tables.obligations.len());
        for (_, row) in tables.obligations.iter() {
            self.encode_obligation_row(row)?;
        }
        // edges
        self.writer.usize(tables.edges.len());
        for (_, edge) in tables.edges.iter() {
            self.encode_edge_row(edge);
        }
        // invocations
        self.writer.usize(tables.invocations.len());
        for (_, inv) in tables.invocations.iter() {
            self.encode_invocation_row(inv)?;
        }
        // inputs.branches (only BlockId + decided_in matter for unreached/reachability)
        self.writer.usize(inputs.branches.len());
        for branch in inputs.branches {
            self.writer.id64(branch.block);
            self.writer.id64(branch.decided_in);
        }
        // verified_borrow_ends
        self.option(verified_borrow_ends, |s, pairs| {
            s.slice(pairs, |s, &(edge, place)| {
                s.writer.id64(edge);
                s.writer.id64(place);
                Ok(())
            })
        })
    }

    fn encode_write_row(&mut self, write: &WriteRow) {
        self.encode_program_point(write.at);
        self.writer.id64(write.place);
        match write.content {
            WriteContent::Value(v) => {
                self.writer.u8(0);
                self.writer.id64(v);
            }
            WriteContent::Overwritten => self.writer.u8(1),
            WriteContent::Vacated => self.writer.u8(2),
        }
    }

    fn encode_program_point(&mut self, point: ProgramPoint) {
        match point {
            ProgramPoint::BlockEntry(b) => {
                self.writer.u8(0);
                self.writer.id64(b);
            }
            ProgramPoint::AfterOperation(op) => {
                self.writer.u8(1);
                self.writer.id64(op);
            }
            ProgramPoint::Edge(e) => {
                self.writer.u8(2);
                self.writer.id64(e);
            }
            ProgramPoint::BeforeCleanup {
                edge,
                action_ordinal,
            } => {
                self.writer.u8(3);
                self.writer.id64(edge);
                self.writer.u32(action_ordinal);
            }
            ProgramPoint::AfterCleanup {
                edge,
                action_ordinal,
            } => {
                self.writer.u8(4);
                self.writer.id64(edge);
                self.writer.u32(action_ordinal);
            }
        }
    }

    fn encode_obligation_row(&mut self, row: &ObligationRow) -> Result<(), ()> {
        self.encode_program_point(row.created_at);
        match row.kind {
            ObligationKind::Claim {
                established_at,
                multiplicity,
                frontier,
            } => {
                self.writer.u8(0);
                self.writer.id64(established_at);
                self.writer.u8(match multiplicity {
                    language_core::Multiplicity::Unrestricted => 0,
                    language_core::Multiplicity::Affine => 1,
                    language_core::Multiplicity::Linear => 2,
                });
                self.writer.id64(frontier);
            }
            ObligationKind::Loan {
                captured,
                access,
                lender,
            } => {
                self.writer.u8(1);
                self.writer.id64(captured);
                self.writer.u8(match access {
                    language_core::ReferenceAccess::Shared => 0,
                    language_core::ReferenceAccess::Mutable => 1,
                    language_core::ReferenceAccess::WriteOnly => 2,
                });
                match lender {
                    Lender::Owner => self.writer.u8(0),
                    Lender::Loan(ob) => {
                        self.writer.u8(1);
                        self.writer.id64(ob);
                    }
                    Lender::Selected => self.writer.u8(2),
                    Lender::Incoming => self.writer.u8(3),
                    Lender::Provider => self.writer.u8(4),
                }
            }
            ObligationKind::RestorationWindow { hole, loan } => {
                self.writer.u8(2);
                self.writer.id64(hole);
                self.writer.id64(loan);
            }
            ObligationKind::DomainDebt { place, domain } => {
                self.writer.u8(3);
                self.writer.id64(place);
                self.domain_semantic(domain);
            }
            ObligationKind::EstablishmentDebt { place } => {
                self.writer.u8(4);
                self.writer.id64(place);
            }
            ObligationKind::ExecutableMembership { subject } => {
                self.writer.u8(5);
                match subject {
                    MembershipSubject::Scalar { value, domain } => {
                        self.writer.u8(0);
                        self.writer.id64(value);
                        self.scalar_domain(domain);
                    }
                    MembershipSubject::Structural { place, domain } => {
                        self.writer.u8(1);
                        self.writer.id64(place);
                        self.structural_domain(domain);
                    }
                }
            }
            ObligationKind::IntegerWidthPositive { parameter } => {
                self.writer.u8(6);
                self.writer.id64(parameter);
            }
            ObligationKind::TheoremRequirement { .. } => return Err(()),
            ObligationKind::CallRequirement { invocation, clause } => {
                self.writer.u8(8);
                self.writer.id64(invocation);
                self.writer.u32(clause);
            }
            ObligationKind::OperationSafety { site } => {
                self.writer.u8(9);
                self.encode_execution_site(site);
            }
            ObligationKind::DataDomain { operation, clause } => {
                self.writer.u8(10);
                self.writer.id64(operation);
                self.writer.u32(clause);
            }
            ObligationKind::ArrivalInvariant { edge } => {
                self.writer.u8(11);
                self.writer.id64(edge);
            }
            ObligationKind::NormalGuarantee {
                exit,
                clause,
                published,
            } => {
                self.writer.u8(12);
                self.writer.id64(exit);
                self.writer.u32(clause);
                self.writer.id64(published);
            }
            ObligationKind::PublishedGuarantee { clause } => {
                self.writer.u8(13);
                self.writer.u32(clause);
            }
            ObligationKind::CrashCoverage { site, cause } => {
                self.writer.u8(14);
                self.encode_execution_site(site);
                self.crash_cause(cause);
            }
            ObligationKind::WellFoundedness { component } => {
                self.writer.u8(15);
                let canon = self.maps.cycles.forward(component).ok_or(())?;
                self.writer.u32(canon);
            }
            ObligationKind::RankComparison { component, edge } => {
                self.writer.u8(16);
                let canon = self.maps.cycles.forward(component).ok_or(())?;
                self.writer.u32(canon);
                self.writer.id64(edge);
            }
        }
        Ok(())
    }

    fn encode_execution_site(&mut self, site: ExecutionSite) {
        match site {
            ExecutionSite::Block(b) => {
                self.writer.u8(0);
                self.writer.id64(b);
            }
            ExecutionSite::Operation(op) => {
                self.writer.u8(1);
                self.writer.id64(op);
            }
        }
    }

    fn encode_edge_row(&mut self, edge: &EdgeRow) {
        self.writer.id64(edge.source);
        match edge.destination {
            Destination::Block(b) => {
                self.writer.u8(0);
                self.writer.id64(b);
            }
            Destination::Return => self.writer.u8(1),
            Destination::NoNormal => self.writer.u8(2),
        }
        self.writer.usize(edge.dispositions.len());
        for disp in &edge.dispositions {
            let disp: &Disposition = disp;
            self.writer.id64(disp.obligation);
            match disp.fate {
                Fate::Continues { home } => {
                    self.writer.u8(0);
                    self.writer.id64(home);
                }
                Fate::Consumed => self.writer.u8(1),
                Fate::CleanedUp => self.writer.u8(2),
                Fate::Discarded => self.writer.u8(3),
                Fate::Abandoned => self.writer.u8(4),
            }
        }
    }

    fn encode_invocation_row(&mut self, inv: &InvocationRow) -> Result<(), ()> {
        self.invocation_site(inv.site);
        match inv.callee {
            Callee::Declared(target) => {
                self.writer.u8(0);
                self.target_callable(target);
            }
            Callee::Selected { .. } => return Err(()),
        }
        self.option(inv.required_contract, |s, c| {
            s.contract(c);
            Ok(())
        })?;
        self.slice(&inv.actuals, |s, &actual: &Actual| {
            match actual {
                Actual::Value(v) => {
                    s.writer.u8(0);
                    s.writer.id64(v);
                }
                Actual::Place(p) => {
                    s.writer.u8(1);
                    s.writer.id64(p);
                }
            }
            Ok(())
        })?;
        self.slice(&inv.requirements, |s, &ob| {
            s.writer.id64(ob);
            Ok(())
        })
    }

    fn drain_pending(&mut self) -> Result<(), ()> {
        while !self.pending.is_empty() {
            if let Some(m) = self.pending.machines.pop() {
                self.writer.u8(0x10);
                self.encode_machine_body(m)?;
                continue;
            }
            if let Some(ty) = self.pending.structural_types.pop() {
                self.writer.u8(0x11);
                self.encode_structural_type(ty)?;
                continue;
            }
            if let Some(dom) = self.pending.structural_domains.pop() {
                self.writer.u8(0x12);
                self.encode_structural_domain(dom)?;
                continue;
            }
            if let Some(dom) = self.pending.scalar_domains.pop() {
                self.writer.u8(0x13);
                self.encode_scalar_domain(dom)?;
                continue;
            }
            if let Some(set_id) = self.pending.scalar_sets.pop() {
                self.writer.u8(0x14);
                self.encode_scalar_set(set_id)?;
                continue;
            }
            if let Some(service) = self.pending.services.pop() {
                self.writer.u8(0x15);
                self.encode_service(service)?;
                continue;
            }
            if let Some(boundary) = self.pending.boundaries.pop() {
                self.writer.u8(0x16);
                self.encode_boundary(boundary)?;
                continue;
            }
            if let Some(app) = self.pending.scalar_applications.pop() {
                self.writer.u8(0x17);
                self.encode_scalar_application(app)?;
            }
        }
        Ok(())
    }

    fn encode_structural_type(&mut self, id: StructuralTypeId) -> Result<(), ()> {
        let decl = self.module.structural_type(id);
        self.option(decl, |s, decl: &StructuralTypeDeclaration| {
            s.identity(decl.declaration_identity)?;
            s.option(decl.quotient_carrier, |s, q| {
                s.structural_type(q);
                Ok(())
            })?;
            s.identity(decl.identity)?;
            s.writer.bool(decl.owns_linear_claim);
            s.multiplicity(decl.multiplicity);
            s.writer.bool(decl.no_code_affine_cleanup);
            s.writer.bool(decl.descriptor_cleanup);
            s.option(decl.nominal_cleanup.as_ref(), |s, nom| {
                s.nominal_identity(&nom.declaration)?;
                s.machine(nom.cleanup_machine);
                Ok(())
            })?;
            s.encode_structural_shape(&decl.shape)?;
            s.option(decl.scalar_restriction.as_ref(), |s, fty| {
                s.encode_structural_field_type(fty)
            })?;
            s.slice(&decl.requires, Self::proposition)?;
            Ok(())
        })?;
        // Include Elaboration::domains for this type ID.
        match self.view.domains.get(id) {
            None => self.writer.bool(false),
            Some(clauses) => {
                self.writer.bool(true);
                self.slice(clauses, Self::proposition)?;
            }
        }
        // Include any matching structural_referents for this ID.
        let referents: Vec<&StructuralReferentIdentity> = self
            .module
            .types
            .structural_referents
            .iter()
            .filter(|r| r.id == id)
            .collect();
        self.writer.usize(referents.len());
        for r in referents {
            self.identity(r.identity)?;
            self.writer.bool(r.foreign_descriptor);
            self.writer.bool(r.abstract_opaque);
        }
        Ok(())
    }

    fn encode_byte_sequence_carrier(&mut self, carrier: ByteSequenceCarrier) {
        match carrier {
            ByteSequenceCarrier::BorrowedView { access } => {
                self.writer.u8(0);
                self.writer.bool(access.is_some());
                if let Some(a) = access {
                    self.access(a);
                }
            }
            ByteSequenceCarrier::BoundedView { access, capacity } => {
                self.writer.u8(1);
                self.writer.bool(access.is_some());
                if let Some(a) = access {
                    self.access(a);
                }
                self.writer.u64(capacity);
            }
            ByteSequenceCarrier::BoundedOwned { capacity } => {
                self.writer.u8(2);
                self.writer.u64(capacity);
            }
        }
    }

    fn encode_structural_shape(&mut self, shape: &StructuralTypeShape) -> Result<(), ()> {
        match shape {
            StructuralTypeShape::Opaque => self.writer.u8(0),
            StructuralTypeShape::Reference { referent, access } => {
                self.writer.u8(1);
                self.structural_type(*referent);
                self.access(*access);
            }
            StructuralTypeShape::PrimitiveScalar(st) => {
                self.writer.u8(2);
                self.scalar_type(*st)?;
            }
            StructuralTypeShape::ByteSequence(carrier) => {
                self.writer.u8(3);
                self.encode_byte_sequence_carrier(*carrier);
            }
            StructuralTypeShape::ElementView { element, access } => {
                self.writer.u8(4);
                self.structural_type(*element);
                self.option(*access, |s, a| {
                    s.access(a);
                    Ok(())
                })?;
            }
            StructuralTypeShape::Record { fields, storage } => {
                self.writer.u8(5);
                self.slice(fields, Self::encode_field)?;
                match *storage {
                    StructuralRecordStorage::Packed => self.writer.u8(0),
                    StructuralRecordStorage::AccessView(rs) => {
                        self.writer.u8(1);
                        self.writer.u64(rs.byte_size);
                        self.writer.u64(rs.byte_alignment);
                    }
                    StructuralRecordStorage::Accessor(rs) => {
                        self.writer.u8(2);
                        self.writer.u64(rs.byte_size);
                        self.writer.u64(rs.byte_alignment);
                    }
                    StructuralRecordStorage::Authored(rs) => {
                        self.writer.u8(3);
                        self.writer.u64(rs.byte_size);
                        self.writer.u64(rs.byte_alignment);
                    }
                    StructuralRecordStorage::TargetLayout(_) => return Err(()),
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                self.writer.u8(6);
                self.structural_type(*element);
                match length {
                    ArrayExtent::Literal(n) => {
                        self.writer.u8(0);
                        self.writer.u64(*n);
                    }
                    ArrayExtent::Application(app) => {
                        self.writer.u8(1);
                        self.scalar_application(*app);
                    }
                    ArrayExtent::Parameter { owner, value } => {
                        self.writer.u8(2);
                        self.machine(*owner);
                        self.writer.id64(*value);
                    }
                }
            }
            StructuralTypeShape::Sum { cases } => {
                self.writer.u8(7);
                self.slice(cases, Self::encode_case)?;
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                self.writer.u8(8);
                self.slice(fields, Self::encode_field)?;
                self.slice(cases, Self::encode_case)?;
            }
            StructuralTypeShape::AtomicCell(cell) => {
                self.writer.u8(9);
                self.encode_atomic_cell(*cell)?;
            }
            StructuralTypeShape::TypeParameter => self.writer.u8(10),
        }
        Ok(())
    }

    fn encode_atomic_cell(&mut self, cell: semantic_vocabulary::AtomicCellType) -> Result<(), ()> {
        match cell {
            semantic_vocabulary::AtomicCellType::Boolean => self.writer.u8(0),
            semantic_vocabulary::AtomicCellType::Integer(int) => {
                self.writer.u8(1);
                self.integer_type(int);
            }
            semantic_vocabulary::AtomicCellType::BoundedInteger(bounds) => {
                self.writer.u8(2);
                self.integer_type(bounds.integer_type());
                self.integer_value(bounds.minimum());
                self.integer_value(bounds.maximum());
            }
        }
        Ok(())
    }

    fn encode_field(&mut self, field: &StructuralFieldDeclaration) -> Result<(), ()> {
        self.identity(field.declaration_identity)?;
        self.option(field.inline_carrier, |s, ty| {
            s.structural_type(ty);
            Ok(())
        })?;
        self.structural_field(field.id);
        self.identity(field.identity)?;
        self.writer.u8(match field.relevance {
            BindingRelevance::Relevant => 0,
            BindingRelevance::Erased => 1,
        });
        self.encode_structural_field_type(&field.field_type)?;
        self.option(field.storage.as_ref(), |s, storage| match storage {
            AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar,
            } => {
                s.writer.u8(0);
                s.writer.u64(*byte_offset);
                s.option(*stored_scalar, |s, st| s.scalar_type(st))
            }
            AuthoredFieldStorage::Repeated {
                byte_offset,
                element_stride,
            } => {
                s.writer.u8(1);
                s.writer.u64(*byte_offset);
                s.writer.u64(*element_stride);
                Ok(())
            }
            AuthoredFieldStorage::Direct { byte_offset } => {
                s.writer.u8(2);
                s.writer.u64(*byte_offset);
                Ok(())
            }
            AuthoredFieldStorage::Fragmented(_) => Err(()),
        })?;
        Ok(())
    }

    fn encode_case(&mut self, case: &StructuralCaseDeclaration) -> Result<(), ()> {
        self.identity(case.declaration_identity)?;
        self.structural_case(case.id);
        self.identity(case.identity)?;
        self.slice(&case.fields, Self::encode_field)
    }

    fn encode_structural_field_type(&mut self, fty: &StructuralFieldType) -> Result<(), ()> {
        match fty {
            StructuralFieldType::Scalar(st) => {
                self.writer.u8(0);
                self.scalar_type(*st)?;
            }
            StructuralFieldType::Atomic(st) => {
                self.writer.u8(1);
                self.scalar_type(*st)?;
            }
            StructuralFieldType::AtomicBoundedInteger(bounds) => {
                self.writer.u8(2);
                self.integer_type(bounds.integer_type());
                self.integer_value(bounds.minimum());
                self.integer_value(bounds.maximum());
            }
            StructuralFieldType::BoundedInteger(bounds) => {
                self.writer.u8(3);
                self.integer_type(bounds.integer_type());
                self.integer_value(bounds.minimum());
                self.integer_value(bounds.maximum());
            }
            StructuralFieldType::RefinedScalar(_) => return Err(()),
            StructuralFieldType::IeeeFloat(fmt) => {
                self.writer.u8(4);
                self.writer.u8(*fmt as u8);
            }
            StructuralFieldType::ByteSequence(carrier) => {
                self.writer.u8(5);
                self.encode_byte_sequence_carrier(*carrier);
            }
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } => {
                self.writer.u8(6);
                self.encode_byte_sequence_carrier(*carrier);
                self.writer.u8(predicates.bits());
            }
            StructuralFieldType::Structural(ty) => {
                self.writer.u8(7);
                self.structural_type(*ty);
            }
            StructuralFieldType::Erased { type_identity } => {
                self.writer.u8(8);
                self.identity(*type_identity)?;
            }
        }
        Ok(())
    }

    fn encode_structural_domain(&mut self, id: StructuralDomainId) -> Result<(), ()> {
        let decl = self
            .module
            .types
            .structural_domains
            .iter()
            .find(|d| d.id == id);
        self.option(decl, |s, d: &StructuralDomainDeclaration| {
            if d.content_projection.is_some()
                || d.access_policy
                    .as_ref()
                    .is_some_and(|p: &StructuralDomainAccessPlan| {
                        p.issuance.is_some() || p.has_operation_rules()
                    })
            {
                return Err(());
            }
            s.option(d.required_memberships.as_ref(), |s, list| {
                s.slice(list, |s, &sem| {
                    s.domain_semantic(sem);
                    Ok(())
                })
            })?;
            s.writer.bool(d.complete_predicate_theory);
            s.option(d.predicate.as_ref(), |s, pred| {
                s.writer.id64(pred.subject);
                s.proposition(&pred.proposition)
            })?;
            s.domain_semantic(d.semantic_domain);
            s.identity(d.identity)?;
            s.structural_type(d.carrier);
            s.identity(d.subject_identity)?;
            s.option(d.byte_predicate, |s, bp| {
                s.writer.u8(bp as u8);
                Ok(())
            })?;
            s.option(d.establishment_routes.as_ref(), |s, routes| {
                s.slice(routes, Self::establishment_route)
            })
        })
    }

    fn encode_scalar_domain(&mut self, id: ScalarDomainId) -> Result<(), ()> {
        let decl = self
            .module
            .types
            .scalar_qualifications
            .domains
            .iter()
            .find(|d| d.id == id);
        self.option(decl, |s, d: &ScalarDomainDeclaration| {
            s.option(d.required_memberships.as_ref(), |s, list| {
                s.slice(list, |s, &sem| {
                    s.domain_semantic(sem);
                    Ok(())
                })
            })?;
            s.option(d.predicate.as_ref(), |s, pred| {
                s.writer.id64(pred.subject);
                s.proposition(&pred.proposition)
            })?;
            s.domain_semantic(d.semantic_domain);
            s.identity(d.identity)?;
            s.scalar_type(d.carrier)?;
            s.option(d.establishment_routes.as_ref(), |s, routes| {
                s.slice(routes, Self::establishment_route)
            })
        })
    }

    fn encode_scalar_set(&mut self, id: ScalarQualificationSetId) -> Result<(), ()> {
        let set = self
            .module
            .types
            .scalar_qualifications
            .sets
            .iter()
            .find(|s| s.id == id);
        self.option(set, |s, set: &ScalarQualificationSet| {
            s.slice(&set.domains, |s, &dom| {
                s.scalar_domain(dom);
                Ok(())
            })
        })
    }

    fn encode_service(&mut self, id: ServiceId) -> Result<(), ()> {
        let decl = self.module.interfaces.services.iter().find(|s| s.id == id);
        self.option(decl, |s, d: &ServiceDeclaration| {
            s.identity(d.identity)?;
            s.slice(&d.parents, |s, &parent| {
                s.service(parent);
                Ok(())
            })
        })
    }

    fn encode_boundary(&mut self, id: BoundaryMachineId) -> Result<(), ()> {
        let decl = self
            .module
            .interfaces
            .boundary_machines
            .iter()
            .find(|b| b.id == id);
        self.option(decl, |s, d: &BoundaryMachineDeclaration| {
            s.option(
                d.declaration.as_ref(),
                |s, callable: &CheckedBoundaryCallable| {
                    s.writer.push_bytes(&callable.service_declaration);
                    s.checked_declaration(&callable.source)?;
                    s.writer.u8(match callable.execution {
                        CheckedBoundaryExecution::Provider => 0,
                        CheckedBoundaryExecution::NumericPrimitive => 1,
                        CheckedBoundaryExecution::LayoutTargetFacts => 2,
                        _ => return Err(()),
                    });
                    if !callable.method_arguments.is_empty() {
                        return Err(());
                    }
                    Ok(())
                },
            )?;
            if d.numeric_intrinsic.is_some() {
                return Err(());
            }
            s.writer.u8(match d.role {
                terminal_psi::BoundaryDeclarationRole::Callable => 0,
                terminal_psi::BoundaryDeclarationRole::ContractEvidence => 1,
            });
            s.writer.u8(match d.kind {
                semantic_vocabulary::BoundaryRequirementKind::Machine => 0,
                semantic_vocabulary::BoundaryRequirementKind::Trait => 1,
                semantic_vocabulary::BoundaryRequirementKind::Operator => 2,
            });
            s.writer.u8(match d.completion {
                semantic_vocabulary::BoundaryCompletion::NormalReturn => 0,
                semantic_vocabulary::BoundaryCompletion::External(_) => 1,
            });
            s.writer.bool(d.unconditional_termination);
            s.identity(d.identity)?;
            s.nominal_identity(&d.owner)?;
            s.option(d.attachment, |s, ty| {
                s.structural_type(ty);
                Ok(())
            })?;
            s.slice(&d.parameter_order, |s, k| {
                s.writer.u8(match k {
                    terminal_psi::BoundaryParameterKind::Scalar => 0,
                    terminal_psi::BoundaryParameterKind::Structural => 1,
                });
                Ok(())
            })?;
            s.slice(&d.scalar_parameters, |s, &st| s.scalar_type(st))?;
            s.slice(&d.crash_routes, Self::crash_bucket)?;
            s.slice(&d.structural_parameters, Self::structural_parameter)?;
            match &d.result {
                BoundaryMachineResult::Unit => s.writer.u8(0),
                BoundaryMachineResult::Scalar(q) => {
                    s.writer.u8(1);
                    s.qualified_scalar_type(*q)?;
                }
                BoundaryMachineResult::Structural(res) => {
                    s.writer.u8(2);
                    s.structural_type(res.structural_type);
                    s.multiplicity(res.multiplicity);
                    s.slice(&res.qualifications, |s, &dom| {
                        s.structural_domain(dom);
                        Ok(())
                    })?;
                    s.slice(&res.projected_qualifications, Self::path_qualification)?;
                }
            }
            if !d.result_reference_regions.is_empty()
                || !d.reference_outputs.is_empty()
                || !d.program_local_root_introductions.is_empty()
                || !d.content_guarantees.is_empty()
                || d.result_lifetimes
                    .as_ref()
                    .is_some_and(|lifetimes| !lifetimes.is_empty())
            {
                return Err(());
            }
            s.writer.bool(d.result_lifetimes.is_some());
            s.slice(&d.requires, |s, req| {
                s.writer.u32(req.argument_index);
                s.slice(&req.path, Self::structural_path_segment)?;
                s.structural_domain(req.domain);
                Ok(())
            })?;
            s.slice(&d.normal_qualifications, |s, req| {
                s.writer.u32(req.argument_index);
                s.slice(&req.path, Self::structural_path_segment)?;
                s.structural_domain(req.domain);
                Ok(())
            })?;
            s.slice(&d.scalar_domain_requirements, |s, req| {
                s.writer.u32(req.argument_index);
                s.scalar_domain(req.domain);
                Ok(())
            })?;
            s.slice(&d.membership_ensures, Self::runtime_membership)?;
            s.slice(&d.scalar_requires, Self::proposition)?;
            s.slice(&d.scalar_ensures, Self::proposition)?;
            s.slice(&d.fixed_service_reach, |s, &id| {
                s.service(id);
                Ok(())
            })?;
            s.writer.bool(d.service_reach_is_installation_bound);
            s.option(d.owning_service, |s, id| {
                s.service(id);
                Ok(())
            })?;
            s.slice(&d.published_service_ceiling, |s, &id| {
                s.service(id);
                Ok(())
            })?;
            s.slice(&d.published_synchronous_entries, |s, &id| {
                s.service(id);
                Ok(())
            })
        })
    }

    fn encode_scalar_application(&mut self, id: ScalarApplicationId) -> Result<(), ()> {
        let decl = self.module.scalars.scalar_applications.declaration(id);
        self.option(decl, |s, app| {
            match app {
                ScalarApplication::Missing => s.writer.u8(0),
                ScalarApplication::LayoutObservation {
                    subject,
                    measurement,
                    semantics_version,
                    scalar_type,
                } => {
                    s.writer.u8(1);
                    s.structural_type(*subject);
                    s.writer.u8(match measurement {
                        LayoutMeasurement::Size => 0,
                        LayoutMeasurement::Alignment => 1,
                    });
                    s.writer.u32(*semantics_version);
                    s.scalar_type(*scalar_type)?;
                }
                ScalarApplication::Evaluate { entry, scalar_type } => {
                    s.writer.u8(2);
                    s.target_callable(*entry);
                    s.scalar_type(*scalar_type)?;
                }
            }
            Ok(())
        })
    }
}

type OutcomeSpecificEnsureCheck = terminal_psi::OutcomeSpecificEnsure;

fn scalar_binary_tag(term: &ScalarTerm) -> u8 {
    match term {
        ScalarTerm::IntegerEqual { .. } => 12,
        ScalarTerm::IntegerLessThan { .. } => 13,
        ScalarTerm::IntegerLessOrEqual { .. } => 14,
        ScalarTerm::IntegerBitwiseAnd { .. } => 15,
        ScalarTerm::IntegerBitwiseOr { .. } => 16,
        ScalarTerm::IntegerBitwiseXor { .. } => 17,
        ScalarTerm::ExactIntegerAdd { .. } => 18,
        ScalarTerm::ExactIntegerSubtract { .. } => 19,
        ScalarTerm::ExactIntegerMultiply { .. } => 20,
        ScalarTerm::ExactIntegerDivide { .. } => 21,
        ScalarTerm::ExactIntegerRemainder { .. } => 22,
        ScalarTerm::WrappingIntegerDivide { .. } => 23,
        ScalarTerm::WrappingIntegerRemainder { .. } => 24,
        ScalarTerm::SaturatingIntegerDivide { .. } => 25,
        ScalarTerm::SaturatingIntegerRemainder { .. } => 26,
        ScalarTerm::WrappingIntegerAdd { .. } => 27,
        ScalarTerm::SaturatingIntegerAdd { .. } => 28,
        ScalarTerm::WrappingIntegerSubtract { .. } => 29,
        ScalarTerm::SaturatingIntegerSubtract { .. } => 39,
        ScalarTerm::WrappingIntegerMultiply { .. } => 40,
        ScalarTerm::SaturatingIntegerMultiply { .. } => 41,
        _ => unreachable!(),
    }
}

fn simple_binary_op_tag(kind: &OperationKind) -> u8 {
    match kind {
        OperationKind::BooleanEqual { .. } => 0,
        OperationKind::IntegerEqual { .. } => 1,
        OperationKind::IntegerLessThan { .. } => 2,
        OperationKind::IntegerLessOrEqual { .. } => 3,
        OperationKind::IntegerBitwiseAnd { .. } => 4,
        OperationKind::IntegerBitwiseOr { .. } => 5,
        OperationKind::IntegerBitwiseXor { .. } => 6,
        OperationKind::WrappingIntegerAdd { .. } => 7,
        OperationKind::SaturatingIntegerAdd { .. } => 8,
        OperationKind::WrappingIntegerSubtract { .. } => 9,
        OperationKind::SaturatingIntegerSubtract { .. } => 10,
        OperationKind::WrappingIntegerMultiply { .. } => 11,
        OperationKind::SaturatingIntegerMultiply { .. } => 12,
        _ => unreachable!(),
    }
}

fn checked_binary_op_tag(kind: &OperationKind) -> u8 {
    match kind {
        OperationKind::ExactIntegerAdd { .. } => 0,
        OperationKind::ExactIntegerSubtract { .. } => 1,
        OperationKind::ExactIntegerMultiply { .. } => 2,
        OperationKind::ExactIntegerDivide { .. } => 3,
        OperationKind::ExactIntegerRemainder { .. } => 4,
        OperationKind::WrappingIntegerDivide { .. } => 5,
        OperationKind::WrappingIntegerRemainder { .. } => 6,
        OperationKind::SaturatingIntegerDivide { .. } => 7,
        OperationKind::SaturatingIntegerRemainder { .. } => 8,
        _ => unreachable!(),
    }
}

/// Canonicalize one conformance witness machine right after `prepare_discharge`
/// and `check_affine`. Returns `None` for any construct outside the conservative
/// portable subset so the caller runs `check_single_machine` directly.
#[allow(clippy::too_many_arguments)]
pub(crate) fn canonicalize_witness_machine<'m>(
    view: &ElaborationView<'m>,
    id: MachineId,
    tables: &MachineTables,
    inputs: &MachineInputs<'_>,
    verified_borrow_ends: Option<&[(EdgeId, PlaceId)]>,
    law_instances: &[MachineId],
    theorems: &[(MachineId, &laws::LawJudgment)],
    _summaries: &ModuleReplays<'_>,
) -> Option<CanonicalizedMachine> {
    let module = view.module;
    if inputs.requirement
        || inputs.carried.is_some()
        || !tables.knowledge.is_empty()
        || minted::domains_are_declared(id, &module.types.structural_types, view.domains).is_err()
        || !law_instances.is_empty()
        || !theorems.is_empty()
        || !module.proofs.indexed_theorems.is_empty()
        || !module.proofs.theorem_parameters.is_empty()
        || !module.proofs.theorem_applications.is_empty()
        || !module.proofs.theorem_citations.is_empty()
        || !module.proofs.operation_crash_contracts.is_empty()
        || !module.proofs.closed_conformance_applications.is_empty()
        || !module.types.quotients.representations.is_empty()
        || !module.types.quotients.operations.is_empty()
        || !module.types.quotients.adaptations.is_empty()
        || !module.types.structural_qualification_changes.is_empty()
        || !module.types.scalar_qualifications.fields.is_empty()
        || !module.types.scalar_qualifications.primitives.is_empty()
        || !module.types.scalar_qualifications.returns.is_empty()
        || !module.types.scalar_qualifications.coercions.is_empty()
        || !module
            .types
            .scalar_qualifications
            .float_entry_ranges
            .is_empty()
        || !module
            .types
            .scalar_qualifications
            .integer_entry_ranges
            .is_empty()
        || !module.interfaces.callback_applications.is_empty()
        || !module
            .interfaces
            .installed_interrupt_registrations
            .is_empty()
        || module.execution.dynamic_dispatch != Default::default()
        || module.execution.task_activation_demands != Default::default()
        || !module.execution.suspension_call_plans.is_empty()
        || !module.execution.suspension_call_sites.is_empty()
        || !module.execution.declaration_contracts.is_empty()
        || !module.access.access_view_inputs.is_empty()
        || !module.access.access_view_plans.is_empty()
        || !module.access.access_lifecycle_plans.is_empty()
        || !module.access.reborrow_restored_call_uses.is_empty()
        || !module.scalars.float_meaning_projections.is_empty()
        || !module.scalars.float_meaning_equalities.is_empty()
        || !module.scalars.scalar_predicate_definitions.is_empty()
    {
        return None;
    }

    let mut encoder = CanonicalEncoder::new(view);
    encoder.machine(id);
    encoder
        .root_tables(tables, inputs, verified_borrow_ends)
        .ok()?;
    encoder.drain_pending().ok()?;

    Some(CanonicalizedMachine {
        key: encoder.writer.finish(),
        maps: encoder.maps,
    })
}

/// Only wholly accepted, non-target-conditioned, non-law discharges are
/// materialized onto duplicate witness machines. Refusals and target conditions
/// fall back to direct checking so exact local diagnostics and spans remain
/// unchanged.
pub(crate) fn can_materialize_discharge(discharge: &MachineDischarge) -> bool {
    discharge.conflicts.is_empty()
        && discharge.uses.iter().all(|used| used.verdict.is_ok())
        && discharge
            .windows
            .iter()
            .all(|window| window.verdict.is_ok())
        && discharge.verdicts.iter().all(|(_, verdict)| match verdict {
            Ok(discharged) => !matches!(
                discharged,
                Discharged::TargetCondition(_) | Discharged::Law(_)
            ),
            Err(_) => false,
        })
}

pub(crate) struct MaterializedDuplicate {
    pub(crate) discharge: MachineDischarge,
    pub(crate) propositions: arena::IdTable<semantic_vocabulary::PropositionId, Proposition>,
    pub(crate) knowledge: arena::IdTable<terminal_check_tables::KnowledgeId, KnowledgeRow>,
}

struct Remapper<'a> {
    from: &'a CanonicalIdMaps,
    to: &'a CanonicalIdMaps,
}

impl<'a> Remapper<'a> {
    fn machine(&self, id: MachineId) -> Result<MachineId, ()> {
        self.from
            .machines
            .forward(id)
            .and_then(|c| self.to.machines.inverse(c))
            .ok_or(())
    }

    fn structural_type(&self, id: StructuralTypeId) -> Result<StructuralTypeId, ()> {
        self.from
            .structural_types
            .forward(id)
            .and_then(|c| self.to.structural_types.inverse(c))
            .ok_or(())
    }

    fn structural_field(&self, id: StructuralFieldId) -> Result<StructuralFieldId, ()> {
        self.from
            .structural_fields
            .forward(id)
            .and_then(|c| self.to.structural_fields.inverse(c))
            .ok_or(())
    }

    fn structural_case(&self, id: StructuralCaseId) -> Result<StructuralCaseId, ()> {
        self.from
            .structural_cases
            .forward(id)
            .and_then(|c| self.to.structural_cases.inverse(c))
            .ok_or(())
    }

    fn scalar_application(&self, id: ScalarApplicationId) -> Result<ScalarApplicationId, ()> {
        self.from
            .scalar_applications
            .forward(id)
            .and_then(|c| self.to.scalar_applications.inverse(c))
            .ok_or(())
    }

    fn cycle(&self, id: CycleComponentId) -> Result<CycleComponentId, ()> {
        self.from
            .cycles
            .forward(id)
            .and_then(|c| self.to.cycles.inverse(c))
            .ok_or(())
    }

    fn integer_type(&self, integer: IntegerType) -> Result<IntegerType, ()> {
        match integer.width() {
            IntegerWidth::Bits(_) => Ok(integer),
            IntegerWidth::Parameter(parameter) => Ok(IntegerType::parameterized(
                integer.sign(),
                WidthParameter {
                    owner: self.machine(parameter.owner)?,
                    value: parameter.value,
                },
            )),
        }
    }

    fn scalar_type(&self, ty: ScalarType) -> Result<ScalarType, ()> {
        match ty {
            ScalarType::Boolean => Ok(ScalarType::Boolean),
            ScalarType::Integer(int) => Ok(ScalarType::Integer(self.integer_type(int)?)),
            ScalarType::IeeeFloat(fmt) => Ok(ScalarType::IeeeFloat(fmt)),
            ScalarType::Callable(_) => Err(()),
        }
    }

    fn canonical_segment(
        &self,
        seg: CanonicalStructuralPathSegment,
    ) -> Result<CanonicalStructuralPathSegment, ()> {
        Ok(match seg {
            CanonicalStructuralPathSegment::Field(f) => {
                CanonicalStructuralPathSegment::Field(self.structural_field(f)?)
            }
            CanonicalStructuralPathSegment::Case(c) => {
                CanonicalStructuralPathSegment::Case(self.structural_case(c)?)
            }
            CanonicalStructuralPathSegment::FixedIndex(idx) => {
                CanonicalStructuralPathSegment::FixedIndex(idx)
            }
            CanonicalStructuralPathSegment::Referent => CanonicalStructuralPathSegment::Referent,
        })
    }

    fn scalar_term(&self, term: &ScalarTerm) -> Result<ScalarTerm, ()> {
        Ok(match term {
            ScalarTerm::DeclarationApplication { .. } => return Err(()),
            ScalarTerm::Application {
                application,
                scalar_type,
            } => ScalarTerm::Application {
                application: self.scalar_application(*application)?,
                scalar_type: self.scalar_type(*scalar_type)?,
            },
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                let mut remapped_path = Vec::with_capacity(path.len());
                for seg in path {
                    remapped_path.push(match seg {
                        ScalarReadProjection::Referent => ScalarReadProjection::Referent,
                        ScalarReadProjection::Extent => ScalarReadProjection::Extent,
                        ScalarReadProjection::Static(cseg) => {
                            ScalarReadProjection::Static(self.canonical_segment(*cseg)?)
                        }
                        ScalarReadProjection::Index(inner) => {
                            ScalarReadProjection::Index(Box::new(self.scalar_term(inner)?))
                        }
                    });
                }
                ScalarTerm::StructuralRead {
                    root: *root,
                    path: remapped_path,
                    scalar_type: self.scalar_type(*scalar_type)?,
                }
            }
            ScalarTerm::IeeeFloatLiteral(v) => ScalarTerm::IeeeFloatLiteral(*v),
            ScalarTerm::Value { id, scalar_type } => ScalarTerm::Value {
                id: *id,
                scalar_type: self.scalar_type(*scalar_type)?,
            },
            ScalarTerm::BooleanField {
                version,
                root,
                path,
            } => ScalarTerm::BooleanField {
                version: *version,
                root: *root,
                path: path
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            } => ScalarTerm::IntegerField {
                version: *version,
                root: *root,
                path: path
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?,
                scalar_type: self.integer_type(*scalar_type)?,
            },
            ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            } => ScalarTerm::IeeeFloatField {
                version: *version,
                root: *root,
                path: path
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?,
                format: *format,
            },
            ScalarTerm::ViewExtent {
                root,
                path,
                scalar_type,
            } => ScalarTerm::ViewExtent {
                root: *root,
                path: path
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?,
                scalar_type: self.integer_type(*scalar_type)?,
            },
            ScalarTerm::Boolean(b) => ScalarTerm::Boolean(*b),
            ScalarTerm::BooleanNot { operand } => ScalarTerm::BooleanNot {
                operand: Box::new(self.scalar_term(operand)?),
            },
            ScalarTerm::BooleanEqual { left, right } => ScalarTerm::BooleanEqual {
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => ScalarTerm::IntegerBitwiseNot {
                scalar_type: self.integer_type(*scalar_type)?,
                operand: Box::new(self.scalar_term(operand)?),
            },
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => ScalarTerm::IntegerWiden {
                source_type: self.integer_type(*source_type)?,
                target_type: self.integer_type(*target_type)?,
                operand: Box::new(self.scalar_term(operand)?),
            },
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => ScalarTerm::IntegerExactCast {
                source_type: self.integer_type(*source_type)?,
                target_type: self.integer_type(*target_type)?,
                operand: Box::new(self.scalar_term(operand)?),
            },
            ScalarTerm::Integer { scalar_type, value } => ScalarTerm::Integer {
                scalar_type: self.integer_type(*scalar_type)?,
                value: *value,
            },
            ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => ScalarTerm::ByteSequencePredicate {
                predicate: *predicate,
                root: *root,
                path: path
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            ScalarTerm::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => ScalarTerm::WrappingIntegerShiftLeft {
                value_type: self.integer_type(*value_type)?,
                count_type: self.integer_type(*count_type)?,
                value: Box::new(self.scalar_term(value)?),
                count: Box::new(self.scalar_term(count)?),
            },
            ScalarTerm::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => ScalarTerm::WrappingIntegerShiftRight {
                value_type: self.integer_type(*value_type)?,
                count_type: self.integer_type(*count_type)?,
                value: Box::new(self.scalar_term(value)?),
                count: Box::new(self.scalar_term(count)?),
            },
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } => ScalarTerm::ExactIntegerShiftLeft {
                value_type: self.integer_type(*value_type)?,
                count_type: self.integer_type(*count_type)?,
                value: Box::new(self.scalar_term(value)?),
                count: Box::new(self.scalar_term(count)?),
            },
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => ScalarTerm::ExactIntegerShiftRight {
                value_type: self.integer_type(*value_type)?,
                count_type: self.integer_type(*count_type)?,
                value: Box::new(self.scalar_term(value)?),
                count: Box::new(self.scalar_term(count)?),
            },
            ScalarTerm::IntegerEqual {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerEqual {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerLessThan {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerLessOrEqual {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerBitwiseAnd {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerBitwiseOr {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => ScalarTerm::IntegerBitwiseXor {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerAdd {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerSubtract {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerMultiply {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerDivide {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerRemainder {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => ScalarTerm::WrappingIntegerDivide {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => ScalarTerm::WrappingIntegerRemainder {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => ScalarTerm::SaturatingIntegerDivide {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => ScalarTerm::SaturatingIntegerRemainder {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::WrappingIntegerAdd {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::SaturatingIntegerAdd {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => ScalarTerm::WrappingIntegerSubtract {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => ScalarTerm::SaturatingIntegerSubtract {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => ScalarTerm::WrappingIntegerMultiply {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
            ScalarTerm::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => ScalarTerm::SaturatingIntegerMultiply {
                scalar_type: self.integer_type(*scalar_type)?,
                left: Box::new(self.scalar_term(left)?),
                right: Box::new(self.scalar_term(right)?),
            },
        })
    }

    fn integer_math_term(&self, term: &IntegerMathTerm) -> Result<IntegerMathTerm, ()> {
        Ok(match term {
            IntegerMathTerm::IntegerLiteral(lit) => IntegerMathTerm::IntegerLiteral(*lit),
            IntegerMathTerm::MathValue { source_type, value } => IntegerMathTerm::MathValue {
                source_type: self.integer_type(*source_type)?,
                value: *value,
            },
            IntegerMathTerm::Embed(scalar) => {
                IntegerMathTerm::Embed(Box::new(self.scalar_term(scalar)?))
            }
            IntegerMathTerm::Add(l, r) => IntegerMathTerm::Add(
                Box::new(self.integer_math_term(l)?),
                Box::new(self.integer_math_term(r)?),
            ),
            IntegerMathTerm::Subtract(l, r) => IntegerMathTerm::Subtract(
                Box::new(self.integer_math_term(l)?),
                Box::new(self.integer_math_term(r)?),
            ),
            IntegerMathTerm::Multiply(l, r) => IntegerMathTerm::Multiply(
                Box::new(self.integer_math_term(l)?),
                Box::new(self.integer_math_term(r)?),
            ),
            IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
                value: Box::new(self.integer_math_term(value)?),
                count: Box::new(self.integer_math_term(count)?),
            },
        })
    }

    fn structural_case_subject(
        &self,
        subject: &StructuralCaseSubject,
    ) -> Result<StructuralCaseSubject, ()> {
        Ok(StructuralCaseSubject::new(
            subject.root(),
            subject
                .path()
                .iter()
                .map(|&seg| self.canonical_segment(seg))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn clause_identity(
        &self,
        clause: &semantic_vocabulary::ClauseIdentity,
    ) -> Result<semantic_vocabulary::ClauseIdentity, ()> {
        if !clause.is_portable() {
            return Err(());
        }
        let mut values = Vec::with_capacity(clause.values.len());
        for value in &clause.values {
            let carrier = match value.carrier {
                semantic_vocabulary::ClauseCarrier::Parameter(p) => {
                    semantic_vocabulary::ClauseCarrier::Parameter(p)
                }
                semantic_vocabulary::ClauseCarrier::Scalar(st) => {
                    semantic_vocabulary::ClauseCarrier::Scalar(self.scalar_type(st)?)
                }
                semantic_vocabulary::ClauseCarrier::Structural(st) => {
                    semantic_vocabulary::ClauseCarrier::Structural(self.structural_type(st)?)
                }
                semantic_vocabulary::ClauseCarrier::FloatMeaning(fmt) => {
                    semantic_vocabulary::ClauseCarrier::FloatMeaning(fmt)
                }
            };
            let operand = match &value.operand {
                semantic_vocabulary::ClauseOperand::Scalar(term) => {
                    semantic_vocabulary::ClauseOperand::Scalar(self.scalar_term(term)?)
                }
                semantic_vocabulary::ClauseOperand::Input(subject) => {
                    semantic_vocabulary::ClauseOperand::Input(
                        self.structural_case_subject(subject)?,
                    )
                }
                semantic_vocabulary::ClauseOperand::Formal(_)
                | semantic_vocabulary::ClauseOperand::Application { .. }
                | semantic_vocabulary::ClauseOperand::FloatMeaning(_) => return Err(()),
            };
            values.push(semantic_vocabulary::ClauseValue { carrier, operand });
        }
        Ok(semantic_vocabulary::ClauseIdentity {
            values,
            left: clause.left,
            right: clause.right,
        })
    }

    fn proposition(&self, proposition: &Proposition) -> Result<Proposition, ()> {
        Ok(match proposition {
            Proposition::Truth => Proposition::Truth,
            Proposition::Falsehood => Proposition::Falsehood,
            Proposition::Atom(id) => Proposition::Atom(*id),
            Proposition::Equal(l, r) => {
                Proposition::Equal(self.scalar_term(l)?, self.scalar_term(r)?)
            }
            Proposition::LessThan(l, r) => {
                Proposition::LessThan(self.scalar_term(l)?, self.scalar_term(r)?)
            }
            Proposition::LessOrEqual(l, r) => {
                Proposition::LessOrEqual(self.scalar_term(l)?, self.scalar_term(r)?)
            }
            Proposition::IntegerMathEqual(l, r) => Proposition::IntegerMathEqual(
                self.integer_math_term(l)?,
                self.integer_math_term(r)?,
            ),
            Proposition::IntegerMathLessThan(l, r) => Proposition::IntegerMathLessThan(
                self.integer_math_term(l)?,
                self.integer_math_term(r)?,
            ),
            Proposition::IntegerMathLessOrEqual(l, r) => Proposition::IntegerMathLessOrEqual(
                self.integer_math_term(l)?,
                self.integer_math_term(r)?,
            ),
            Proposition::IeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => {
                let left_path = left
                    .path()
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?;
                let right_path = right
                    .path()
                    .iter()
                    .map(|&seg| self.canonical_segment(seg))
                    .collect::<Result<Vec<_>, _>>()?;
                Proposition::IeeeFloatComparison {
                    kind: *kind,
                    format: *format,
                    left: semantic_vocabulary::IeeeFloatStructuralField::new(
                        left.root(),
                        left_path,
                    )
                    .map_err(|_| ())?,
                    right: semantic_vocabulary::IeeeFloatStructuralField::new(
                        right.root(),
                        right_path,
                    )
                    .map_err(|_| ())?,
                }
            }
            Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            } => Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left: self.scalar_term(left)?,
                right: self.scalar_term(right)?,
            },
            Proposition::StructuralCaseMembership { subject, case } => {
                Proposition::StructuralCaseMembership {
                    subject: self.structural_case_subject(subject)?,
                    case: self.structural_case(*case)?,
                }
            }
            Proposition::Conjunction(parts) => Proposition::Conjunction(
                parts
                    .iter()
                    .map(|p| self.proposition(p))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Proposition::Disjunction(parts) => Proposition::Disjunction(
                parts
                    .iter()
                    .map(|p| self.proposition(p))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Proposition::Implication {
                premise,
                conclusion,
            } => Proposition::Implication {
                premise: Box::new(self.proposition(premise)?),
                conclusion: Box::new(self.proposition(conclusion)?),
            },
            Proposition::ContentConservation(_) | Proposition::FloatIntegerConversion(_) => {
                return Err(());
            }
            Proposition::IeeeFloatIntegerExact { source, target } => {
                Proposition::IeeeFloatIntegerExact {
                    source: self.scalar_term(source)?,
                    target: self.integer_type(*target)?,
                }
            }
            Proposition::IntegerIeeeFloatExact { source, target } => {
                Proposition::IntegerIeeeFloatExact {
                    source: self.scalar_term(source)?,
                    target: *target,
                }
            }
            Proposition::DeclarationIdentity(clause) => {
                Proposition::DeclarationIdentity(self.clause_identity(clause)?)
            }
        })
    }

    fn proof_node(&self, node: &ProofNode) -> Result<ProofNode, ()> {
        let conclusion = self.proposition(&node.conclusion)?;
        let rule = match &node.rule {
            ProofRule::IntegerFloatExact => ProofRule::IntegerFloatExact,
            ProofRule::FloatIntegerConversion { premise, computed } => {
                ProofRule::FloatIntegerConversion {
                    premise: Box::new(self.proof_node(premise)?),
                    computed: computed
                        .as_ref()
                        .map(|c| self.proof_node(c).map(Box::new))
                        .transpose()?,
                }
            }
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => ProofRule::ValueEqualityTransport {
                premise: Box::new(self.proof_node(premise)?),
                equalities: equalities
                    .iter()
                    .map(|e| self.proof_node(e))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            ProofRule::PredicateDenotation { premise } => ProofRule::PredicateDenotation {
                premise: Box::new(self.proof_node(premise)?),
            },
            ProofRule::Primitive(p) => ProofRule::Primitive(*p),
            ProofRule::SemanticAxiom { index } => ProofRule::SemanticAxiom { index: *index },
            ProofRule::Assumption { index } => ProofRule::Assumption { index: *index },
            ProofRule::ConjunctionIntroduction(nodes) => ProofRule::ConjunctionIntroduction(
                nodes
                    .iter()
                    .map(|n| self.proof_node(n))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ProofRule::ConjunctionElimination {
                conjunction,
                conjunct,
            } => ProofRule::ConjunctionElimination {
                conjunction: std::sync::Arc::new(self.proof_node(conjunction)?),
                conjunct: *conjunct,
            },
            ProofRule::DisjunctionIntroduction { disjunct, index } => {
                ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(self.proof_node(disjunct)?),
                    index: *index,
                }
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => ProofRule::DisjunctionElimination {
                disjunction: Box::new(self.proof_node(disjunction)?),
                branches: branches
                    .iter()
                    .map(|b| self.proof_node(b))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            ProofRule::ImplicationIntroduction { body } => ProofRule::ImplicationIntroduction {
                body: Box::new(self.proof_node(body)?),
            },
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => ProofRule::ImplicationElimination {
                implication: Box::new(self.proof_node(implication)?),
                premise: Box::new(self.proof_node(premise)?),
            },
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(self.proof_node(left_equals_middle)?),
                middle_equals_right: Box::new(self.proof_node(middle_equals_right)?),
            },
            ProofRule::EqualitySymmetry { equality } => ProofRule::EqualitySymmetry {
                equality: Box::new(self.proof_node(equality)?),
            },
            ProofRule::IntegerOrderWeakening { relation } => ProofRule::IntegerOrderWeakening {
                relation: Box::new(self.proof_node(relation)?),
            },
            ProofRule::IntegerOrderDiscreteness { relation } => {
                ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(self.proof_node(relation)?),
                }
            }
            ProofRule::IntegerMathOrderTranslation { relation, addend } => {
                ProofRule::IntegerMathOrderTranslation {
                    relation: Box::new(self.proof_node(relation)?),
                    addend: self.integer_math_term(addend)?,
                }
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                ProofRule::IntegerOrderAntisymmetry {
                    forward: Box::new(self.proof_node(forward)?),
                    backward: Box::new(self.proof_node(backward)?),
                }
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => ProofRule::IntegerSubtractOrder {
                difference: Box::new(self.proof_node(difference)?),
                positive: Box::new(self.proof_node(positive)?),
            },
            ProofRule::IntegerAddOrder { sum, positive } => ProofRule::IntegerAddOrder {
                sum: Box::new(self.proof_node(sum)?),
                positive: Box::new(self.proof_node(positive)?),
            },
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => ProofRule::IntegerSubtractAntitone {
                smaller: Box::new(self.proof_node(smaller)?),
                larger: Box::new(self.proof_node(larger)?),
                order: Box::new(self.proof_node(order)?),
            },
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(self.proof_node(left_less_or_equal_middle)?),
                middle_less_or_equal_right: Box::new(self.proof_node(middle_less_or_equal_right)?),
            },
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(self.proof_node(left_to_middle)?),
                middle_to_right: Box::new(self.proof_node(middle_to_right)?),
            },
            ProofRule::IntegerOrderSubstitution {
                relation,
                equality,
                endpoint,
            } => ProofRule::IntegerOrderSubstitution {
                relation: Box::new(self.proof_node(relation)?),
                equality: Box::new(self.proof_node(equality)?),
                endpoint: *endpoint,
            },
            ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } => ProofRule::IntegerAffineBound {
                root_bound: Box::new(self.proof_node(root_bound)?),
                witness: proof_admission::IntegerAffineWitness {
                    root: self.scalar_term(&witness.root)?,
                    target: self.scalar_term(&witness.target)?,
                    definition_axioms: witness.definition_axioms.clone(),
                    literal_axioms: witness.literal_axioms.clone(),
                },
            },
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactAddDefinitionBound {
                left_bound: Box::new(self.proof_node(left_bound)?),
                right_bound: Box::new(self.proof_node(right_bound)?),
                definition_axiom: *definition_axiom,
            },
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: Box::new(self.proof_node(left_bound)?),
                right_bound: Box::new(self.proof_node(right_bound)?),
                definition_axiom: *definition_axiom,
            },
            ProofRule::IntegerCastBound {
                root_bound,
                witness,
            } => ProofRule::IntegerCastBound {
                root_bound: Box::new(self.proof_node(root_bound)?),
                witness: proof_admission::IntegerCastChainWitness {
                    root: self.scalar_term(&witness.root)?,
                    target: self.scalar_term(&witness.target)?,
                    definition_axioms: witness.definition_axioms.clone(),
                },
            },
            ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                ProofRule::IntegerCorrelatedForbiddenRoots {
                    witness: proof_admission::IntegerCorrelatedForbiddenRootWitness {
                        dividend: proof_admission::CorrelatedAffineBranchWitness {
                            root: self.scalar_term(&witness.dividend.root)?,
                            target: self.scalar_term(&witness.dividend.target)?,
                            steps: witness.dividend.steps.clone(),
                        },
                        divisor: proof_admission::CorrelatedAffineBranchWitness {
                            root: self.scalar_term(&witness.divisor.root)?,
                            target: self.scalar_term(&witness.divisor.target)?,
                            steps: witness.divisor.steps.clone(),
                        },
                        definition_axiom_count: witness.definition_axiom_count,
                        lower_bound_axiom: witness.lower_bound_axiom,
                        upper_bound_axiom: witness.upper_bound_axiom,
                        conclusion: self.proposition(&witness.conclusion)?,
                    },
                }
            }
            ProofRule::IntegerPowerOfTwoOrder { order } => ProofRule::IntegerPowerOfTwoOrder {
                order: Box::new(self.proof_node(order)?),
            },
            ProofRule::IeeeFloatOrderChain(nodes) => ProofRule::IeeeFloatOrderChain(
                nodes
                    .iter()
                    .map(|n| self.proof_node(n))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ProofRule::FloatMeaningNormalization(nodes) => ProofRule::FloatMeaningNormalization(
                nodes
                    .iter()
                    .map(|n| self.proof_node(n))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(ProofNode { conclusion, rule })
    }

    fn evidence(&self, ev: &Evidence) -> Result<Evidence, ()> {
        Ok(Evidence {
            goal: self.proposition(&ev.goal)?,
            assumptions: ev.assumptions.clone(),
            axioms: ev.axioms.clone(),
            certificate: CertificateEnvelope {
                identity: ev.certificate.identity,
                proof_system_marker: ev.certificate.proof_system_marker,
                proof: self.proof_node(&ev.certificate.proof)?,
            },
            fact: AcceptedFact {
                obligation: ev.fact.obligation,
                proposition: self.proposition(&ev.fact.proposition)?,
                route: match &ev.fact.route {
                    AcceptedFactRoute::KernelDerived(p) => AcceptedFactRoute::KernelDerived(*p),
                    AcceptedFactRoute::CertificateDerived {
                        identity,
                        proof_system_marker,
                        acceptance,
                    } => AcceptedFactRoute::CertificateDerived {
                        identity: *identity,
                        proof_system_marker: *proof_system_marker,
                        acceptance: acceptance.clone(),
                    },
                    AcceptedFactRoute::Admitted(admitted) => AcceptedFactRoute::Admitted(*admitted),
                },
            },
        })
    }

    fn refusal_site(&self, site: RefusalSite) -> Result<RefusalSite, ()> {
        Ok(match site {
            RefusalSite::Machine(m, c) => RefusalSite::Machine(self.machine(m)?, c),
            RefusalSite::Declaration(d) => RefusalSite::Declaration(d),
            RefusalSite::Program => RefusalSite::Program,
        })
    }

    fn finite_integer_ranking(
        &self,
        ranking: &FiniteIntegerRanking,
    ) -> Result<FiniteIntegerRanking, ()> {
        let component = self.cycle(ranking.component)?;
        let carriers = ranking
            .carriers
            .iter()
            .map(|&int| self.integer_type(int))
            .collect::<Result<Vec<_>, _>>()?;
        let mut observations = arena::Arena::default();
        let mut ranks = Vec::with_capacity(ranking.bindings.ranks.len());
        for rank in &ranking.bindings.ranks {
            let span = ranking.bindings.observations.span(rank.values).ok_or(())?;
            let remapped = span
                .iter()
                .map(|v| {
                    Ok(TerminalIntegerRankValue {
                        integer_type: self.integer_type(v.integer_type)?,
                        value: v.value,
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?;
            let values = observations.insert_many(remapped);
            ranks.push(terminal_psi::TerminalBlockNaturalRank {
                block: rank.block,
                values,
            });
        }
        let mut edges = Vec::with_capacity(ranking.bindings.edges.len());
        for edge in &ranking.bindings.edges {
            let span = ranking
                .bindings
                .observations
                .span(edge.successor_ranks)
                .ok_or(())?;
            let remapped = span
                .iter()
                .map(|v| {
                    Ok(TerminalIntegerRankValue {
                        integer_type: self.integer_type(v.integer_type)?,
                        value: v.value,
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?;
            let successor_ranks = observations.insert_many(remapped);
            edges.push(terminal_psi::TerminalNaturalRankEdge {
                edge: edge.edge,
                source: edge.source,
                target: edge.target,
                successor_ranks,
                comparison: edge.comparison,
            });
        }
        Ok(FiniteIntegerRanking {
            component,
            carriers,
            bindings: TerminalNaturalCycle {
                observations,
                ranks,
                edges,
            },
        })
    }
}

/// Remap a checked representative's `MachineDischarge`, `propositions`, and
/// `knowledge` onto a duplicate witness machine, rebuilding a fresh
/// `branch_warnings::Deferred` from the duplicate's own lowered machine and
/// tables. Returns `Err(())` if any identity cannot be cleanly mapped so the
/// caller falls back to `check_single_machine`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_duplicate_materialization(
    rep: &CanonicalizedMachine,
    rep_discharge: &MachineDischarge,
    rep_tables: &MachineTables,
    dup: &CanonicalizedMachine,
    dup_id: MachineId,
    dup_lowered: &TerminalMachine,
    dup_tables: &MachineTables,
) -> Result<MaterializedDuplicate, ()> {
    if !can_materialize_discharge(rep_discharge) {
        return Err(());
    }
    let remapper = Remapper {
        from: &rep.maps,
        to: &dup.maps,
    };

    let mut propositions = arena::IdTable::default();
    for (_, prop) in rep_tables.propositions.iter() {
        propositions.push(remapper.proposition(prop)?);
    }

    let mut knowledge = arena::IdTable::default();
    for (_, row) in rep_tables.knowledge.iter() {
        let justification = match row.justification {
            Justification::ExcludedDispatchCase { edge, selected } => {
                Justification::ExcludedDispatchCase {
                    edge,
                    selected: remapper.structural_case(selected)?,
                }
            }
            other => other,
        };
        knowledge.push(KnowledgeRow {
            proposition: row.proposition,
            available_from: row.available_from,
            justification,
            premises: row.premises.clone(),
        });
    }

    let mut verdicts = arena::IdTable::default();
    for (_, verdict) in rep_discharge.verdicts.iter() {
        let discharged = verdict.as_ref().map_err(|_| ())?;
        let remapped = match discharged {
            Discharged::RetainedMembership => Discharged::RetainedMembership,
            Discharged::Preserved => Discharged::Preserved,
            Discharged::Lent => Discharged::Lent,
            Discharged::Identity(judgment) => Discharged::Identity(equality::Judgment {
                goal: remapper.clause_identity(&judgment.goal)?,
                citations: judgment.citations.clone(),
                decision: judgment.decision.clone(),
                certificate: judgment.certificate.clone(),
            }),
            Discharged::ConservedContent(claim) => Discharged::ConservedContent(*claim),
            Discharged::Certificate(ev) => Discharged::Certificate(remapper.evidence(ev)?),
            Discharged::Law(_) | Discharged::TargetCondition(_) => return Err(()),
            Discharged::EveryExit(obs) => Discharged::EveryExit(obs.clone()),
            Discharged::PendingNormalGuarantee(obs) => {
                Discharged::PendingNormalGuarantee(obs.clone())
            }
            Discharged::Window(refusal) => Discharged::Window(Box::new(Refusal {
                reason: refusal.reason,
                site: remapper.refusal_site(refusal.site)?,
                source_span: None,
                detail: refusal.detail.clone(),
            })),
            Discharged::FiniteIntegerRanking(ranking) => {
                Discharged::FiniteIntegerRanking(remapper.finite_integer_ranking(ranking)?)
            }
        };
        verdicts.push(Ok(remapped));
    }

    let mut uses = Vec::with_capacity(rep_discharge.uses.len());
    for used in &rep_discharge.uses {
        if used.verdict.is_err() {
            return Err(());
        }
        uses.push(StorageUse {
            at: used.at,
            verdict: Ok(()),
        });
    }

    let mut windows = Vec::with_capacity(rep_discharge.windows.len());
    for window in &rep_discharge.windows {
        let ev = window.verdict.as_ref().map_err(|_| ())?;
        windows.push(WindowUse {
            opened_by: window.opened_by,
            at: window.at,
            verdict: Ok(remapper.evidence(ev)?),
        });
    }

    let graph = Graph::new(dup_id, dup_lowered, dup_tables).map_err(|_| ())?;
    let types = dup_tables.values.map(|_, row| row.scalar_type);
    let context = PropositionContext::from_value_types_and_places(
        types
            .iter()
            .filter(|(value, _)| {
                dup_tables
                    .values
                    .get(*value)
                    .is_none_or(|row| row.rigid.is_none())
            })
            .map(|(value, scalar_type)| (value, *scalar_type)),
        dup_lowered
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind)),
    )
    .map_err(|_| ())?;
    let branch_warnings = branch_warnings::Deferred::new(graph, context);

    Ok(MaterializedDuplicate {
        discharge: MachineDischarge {
            verdicts,
            uses,
            windows,
            weakened_joins: rep_discharge.weakened_joins.clone(),
            conflicts: Vec::new(),
            branch_warnings,
        },
        propositions,
        knowledge,
    })
}
