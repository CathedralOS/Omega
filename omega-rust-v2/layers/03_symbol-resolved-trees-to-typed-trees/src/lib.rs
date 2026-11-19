//! L3 types one resolved Omega stage. It checks declarations across the admitted files,
//! selects calls and operators, and records the facts later layers must prove.
//!
//! Start with `typing/type_stage.rs` for pass order. `declarations/` indexes the
//! names supplied by L2. `type_system/` interprets annotations and specializes types.
//! `expressions/` checks machine bodies. `contracts/` composes effects and evidence
//! across declarations. `evaluation/` forms static indices and checks closed results.
//! `toolchain_declarations/` admits the toolchain declarations used by the source command.
//!
//! `typed_tree/typed_stage.rs` describes the result and links its component types.
//! The language rules and their executable examples are in the crate's CONTRACT.md.
#![forbid(unsafe_code)]

mod contracts;
mod declarations;
mod errors;
mod evaluation;
mod expressions;
mod toolchain_declarations;
mod type_system;
mod typed_tree;
mod typing;

pub use errors::type_error::TypeError;
pub use evaluation::integer_arithmetic::{IntegerOperationError, evaluate_integer_operation};
pub use evaluation::integer_index::IntegerIndex;
pub use evaluation::specialized_index_value::SpecializedIndexValue;
pub use evaluation::static_template::StaticTemplate;
pub use evaluation::static_value::StaticValue;
pub use evaluation::static_value_request::StaticValueRequest;
pub use omega_toolchain::{SourcePackage, resolve_source_package};
pub use toolchain_declarations::core_interface::standard_library;
pub use toolchain_declarations::source_package::{type_source_package, type_source_package_for_preparation};
pub use typed_tree::access::Access;
pub use typed_tree::calls::call_use::CallUse;
pub use typed_tree::calls::callable_type::CallableType;
pub use typed_tree::calls::conformance_row::ConformanceRow;
pub use typed_tree::calls::indirect_call::IndirectCall;
pub use typed_tree::calls::requirement_slot::RequirementSlot;
pub use typed_tree::calls::typed_call::{AppliedOperand, TypedCall};
pub use typed_tree::calls::typed_signature::TypedSignature;
pub use typed_tree::domain_application::DomainApplication;
pub use typed_tree::effects::crash_cause::CrashCause;
pub use typed_tree::effects::crash_route::{CrashRoute, CrashRouteRef};
pub use typed_tree::effects::guarded_call::{GuardedCall, GuardedInvocationRef};
pub use typed_tree::effects::invocation_target::InvocationTarget;
pub use typed_tree::effects::machine_effects::MachineEffects;
pub use typed_tree::effects::pause_effects::{DeclaredPauseBounds, PauseEffects};
pub use typed_tree::effects::service_application::ServiceApplication;
pub use typed_tree::proofs::logical_hypothesis::LogicalHypothesis;
pub use typed_tree::proofs::obligation_kind::ObligationKind;
pub use typed_tree::proofs::ranking_witness::RankingWitness;
pub use typed_tree::proofs::trait_refinement::TraitRefinement;
pub use typed_tree::proofs::typed_membership::TypedMembership;
pub use typed_tree::proofs::typing_obligation::TypingObligation;
pub use typed_tree::site::Site;
pub use typed_tree::storage::access_policy_selection::AccessPolicySelection;
pub use typed_tree::storage::carry::Carry;
pub use typed_tree::storage::cleanup_selection::CleanupSelection;
pub use typed_tree::storage::collection_access::CollectionAccess;
pub use typed_tree::storage::data_properties::DataProperties;
pub use typed_tree::storage::multiplicity::Multiplicity;
pub use typed_tree::storage::opening::{Opening, PlaceStep, Projection};
pub use typed_tree::storage::package_construction::PackageConstruction;
pub use typed_tree::typed_expression::{TypedExpression, TypedNumericLanding};
pub use typed_tree::typed_instruction::TypedInstruction;
pub use typed_tree::typed_operation::TypedOperation;
pub use typed_tree::typed_stage::{FieldInitializer, StaticDataShape, TypedConstant, TypedConstruction, TypedField, TypedStage};
pub use typed_tree::typed_trees::{NodeSemantics, TypedTree, TypedTrees};
pub use typed_tree::value_type::ValueType;
pub use typing::assembly_types::AssemblyTypes;
pub use typing::boundary_slot::BoundarySlot;
pub use typing::entry_points::{type_stage, type_stage_for_target, type_stage_in_environment};
pub use typing::ranking_view::RankingView;
pub use typing::slot_direction::SlotDirection;
pub use typing::typing_environment::TypingEnvironment;
pub use typing::typing_progress::{PendingTyping, StaticValueDemand, TypingProgress};
