//! A completed L3 stage records checked expressions, selected declarations and the obligations handed to later layers.

use crate::{
    AccessPolicySelection, BoundarySlot, CallableType, CleanupSelection, CollectionAccess, ConformanceRow, CrashRoute, DataProperties, GuardedCall,
    IndirectCall, InvocationTarget, LogicalHypothesis, MachineEffects, PackageConstruction, RankingWitness, Site, StaticIntegerRequest, TraitRefinement,
    TypedCall, TypedExpression, TypedInstruction, TypedMembership, TypedOperation, TypedSignature, TypingObligation, ValueType,
};
use num_bigint::BigInt;
use omega_resolver::DeclarationId;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct TypedStage {
    pub interface_contracts: Vec<crate::InterfaceContract>,
    pub mathematical_interfaces: Vec<(DeclarationId, ValueType, Vec<DeclarationId>)>,
    pub logical_hypotheses: Vec<LogicalHypothesis>,
    pub expressions: Vec<TypedExpression>,
    pub evaluated_indices: Vec<(StaticIntegerRequest, BigInt)>,
    pub constants: Vec<(DeclarationId, ValueType)>,
    pub signatures: Vec<TypedSignature>,
    pub callable_bounds: Vec<(DeclarationId, usize, CallableType)>,
    pub calls: Vec<TypedCall>,
    pub operations: Vec<TypedOperation>,
    pub instructions: Vec<TypedInstruction>,
    pub obligations: Vec<TypingObligation>,
    pub data_properties: Vec<(DeclarationId, DataProperties)>,
    pub packages: Vec<PackageConstruction>,
    pub collection_accesses: Vec<CollectionAccess>,
    pub cleanups: Vec<CleanupSelection>,
    pub access_policies: Vec<AccessPolicySelection>,
    pub ranking_witnesses: Vec<RankingWitness>,
    pub crash_routes: Vec<CrashRoute>,
    pub guarded_calls: Vec<GuardedCall>,
    pub boundary_calls: Vec<(Site, BoundarySlot)>,
    pub invocation_ceilings: Vec<(DeclarationId, Site, InvocationTarget)>,
    pub refinements: Vec<TraitRefinement>,
    pub indirect_calls: Vec<IndirectCall>,
    pub memberships: Vec<TypedMembership>,
    pub machine_effects: Vec<MachineEffects>,
    pub conformance_rows: Vec<ConformanceRow>,
    /// Source expressions whose exact integer value lands at a concrete destination.
    pub landings: Vec<TypedExpression>,
}
