#![forbid(unsafe_code)]

//! Durable target-neutral facts and their current storage.
//!
//! Start at `fact_plan`: a `FactPlan` holds the facts checking established for
//! one program, organized as contexts, places, evidence, and write frames, and
//! the resolution helpers that map a place back to its member symbol and
//! render its canonical label (`self.items[0]`). Later stages read facts by
//! handle; none of them re-derives a fact from source.

pub mod fact_plan;

pub use fact_plan::contexts::view::FactContextView;
pub use fact_plan::contexts::{
    BooleanFact, DomainMembershipFact, FactContext, FactContextGroup, FactContexts, FactRef,
    ProgramPoint, SymbolFactSet, TypeConstraintFact, view,
};
pub use fact_plan::evidence::{
    ContractFactKind, DataDefinitionFactDependency, DataDefinitionFactRecord,
    DomainDefinitionFactDependency, DomainDefinitionFactRecord, Fact, FactOrigin, FactPayload,
    InstantiatedExpression, IntegerRange, ProofObligationKind, QualificationCorrespondence,
    QualificationEvidence, QualificationPayloadIdentity, ScalarValue,
};
pub use fact_plan::places::resolution::{
    canonical_place_label_from_parts, effective_member_symbol, payload_variant_for_field,
    resolve_place_member_symbol,
};
pub use fact_plan::places::write_frame::{NormalizedWriteFrame, WriteFrameCompleteness};
pub use fact_plan::places::{FactPlace, Place, PlaceRoot, PlaceSegment, resolution, write_frame};
pub use fact_plan::{
    FactContextHandle, FactHandle, FactPlan, FactRefHandle, PlaceHandle, PlaceSegmentHandle,
    QualificationCorrespondenceHandle, contexts, evidence, places,
};
