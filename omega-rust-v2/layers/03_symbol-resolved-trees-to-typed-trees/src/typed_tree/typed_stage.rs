//! A completed L3 stage records checked expressions, selected declarations and the obligations handed to later layers.

use crate::{
    AccessPolicySelection, BoundarySlot, CallableType, CleanupSelection, CollectionAccess, ConformanceRow, CrashRoute, CrashRouteRef, DataProperties,
    GuardedCall, GuardedInvocationRef, IndirectCall, InvocationTarget, LogicalHypothesis, MachineEffects, PackageConstruction, RankingWitness, Site,
    StaticIntegerRequest, TraitRefinement, TypedCall, TypedExpression, TypedInstruction, TypedMembership, TypedOperation, TypedSignature, TypingObligation,
    ValueType,
};
use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use std::sync::Arc;

/// A stored field or requirement projection in a record's declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedField {
    pub declaration: DeclarationId,
    pub site: Site,
    pub ty: ValueType,
}
impl TypedField {
    pub(crate) fn name(&self) -> &str {
        self.declaration.path.rsplit("::").next().unwrap()
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct TypedStage {
    /// Typed definitions and predicates of the library admitted for this source stage.
    pub library: Option<&'static crate::TypedTrees<'static>>,
    pub mathematical_interfaces: Vec<(DeclarationId, ValueType, Vec<DeclarationId>)>,
    pub logical_hypotheses: Vec<LogicalHypothesis>,
    pub expressions: Vec<TypedExpression>,
    pub evaluated_indices: Vec<(StaticIntegerRequest, BigInt)>,
    pub constants: Vec<(DeclarationId, ValueType)>,
    /// Evaluated receivers, projections, and aliases introduced by borrowed patterns.
    pub(crate) place_steps: std::collections::BTreeMap<omega_resolver::TreeNodeId, crate::PlaceStep>,
    pub record_fields: std::collections::HashMap<DeclarationId, Arc<Vec<TypedField>>>,
    pub signatures: Vec<Arc<TypedSignature>>,
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
    pub machine_effects: Vec<Arc<MachineEffects>>,
    pub conformance_rows: Vec<ConformanceRow>,
    /// Source expressions whose exact integer value lands at a concrete destination.
    pub landings: Vec<TypedExpression>,
}

impl TypedStage {
    pub fn crash_route(
        &self,
        reference: CrashRouteRef,
    ) -> Option<&CrashRoute> {
        match reference {
            CrashRouteRef::Local(index) => self.crash_routes.get(index),
            CrashRouteRef::Library(index) => self.library?.crash_routes.get(index),
        }
    }

    pub fn guarded_invocation(
        &self,
        reference: GuardedInvocationRef,
    ) -> Option<&IndirectCall> {
        match reference {
            GuardedInvocationRef::Local(index) => self.indirect_calls.get(index),
            GuardedInvocationRef::Library(index) => self.library?.indirect_calls.get(index),
        }
    }

    /// The evaluated root and its projections in selection order. Each reference
    /// crossing identifies the loan supplying the following storage.
    pub fn place_path(
        &self,
        mut node: omega_resolver::TreeNodeId,
    ) -> (omega_resolver::TreeNodeId, Vec<&crate::PlaceStep>) {
        let mut steps = Vec::new();
        while let Some(step) = self.place_steps.get(&node) {
            if step.projection.is_some() {
                steps.push(step);
            }
            node = step.base;
        }
        steps.reverse();
        (node, steps)
    }
}
