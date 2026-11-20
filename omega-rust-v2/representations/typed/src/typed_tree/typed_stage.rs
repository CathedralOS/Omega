//! A completed L3 stage records checked expressions, selected declarations and the obligations handed to later layers.

use crate::{
    AccessPolicySelection, BoundarySlot, CallableType, CleanupSelection, CollectionAccess, ConformanceRow, CrashRoute, CrashRouteRef, DataProperties,
    GuardedCall, GuardedInvocationRef, IndirectCall, InvocationTarget, LogicalHypothesis, MachineEffects, PackageConstruction, RankingWitness, Site,
    StaticValue, StaticValueRequest, TraitRefinement, TypedCall, TypedExpression, TypedInstruction, TypedMembership, TypedNumericLanding, TypedOperation,
    TypedSignature, TypingObligation, ValueType,
};
use omega_resolved::{DeclarationId, TreeNodeId};
use std::sync::Arc;

/// A stored field or requirement projection in a record's declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedField {
    pub declaration: DeclarationId,
    pub site: Site,
    pub ty: ValueType,
}
impl TypedField {
    pub fn name(&self) -> &str {
        self.declaration.path.rsplit([':', '\\']).next().unwrap()
    }
}

/// A construction selects one data declaration and, for a sum, one exact case.
/// Initializers stay in evaluation order, with each field's declaration-order slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedConstruction {
    pub owner: DeclarationId,
    pub case: Option<DeclarationId>,
    pub carrier: ValueType,
    pub initializers: Vec<FieldInitializer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldInitializer {
    pub position: usize,
    pub expression: usize,
}

/// Field carriers of a concrete data application, in declaration order. Every
/// case is present so preparation can check the complete type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticDataShape {
    pub fields: Vec<ValueType>,
    pub cases: Vec<(DeclarationId, Vec<ValueType>)>,
    /// Every construction, including an implicit zero, owes the declaration's predicate.
    pub requires_establishment: bool,
}

/// A local constant and the typed occurrence that initializes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedConstant {
    pub declaration: DeclarationId,
    pub initializer: StaticValueRequest,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct TypedStage {
    /// Typed definitions and predicates of the library admitted for this source stage.
    pub library: Option<&'static crate::TypedTrees<'static>>,
    /// Checked binders for each local generic declaration in declared order.
    pub generic_parameters: std::collections::BTreeMap<DeclarationId, crate::GenericParameters>,
    /// Enclosing declarations supplying the leading arguments of a nested data application, outermost first.
    pub enclosing_data_parameters: std::collections::BTreeMap<DeclarationId, Vec<DeclarationId>>,
    pub mathematical_interfaces: Vec<(DeclarationId, ValueType, Vec<DeclarationId>)>,
    pub logical_hypotheses: Vec<LogicalHypothesis>,
    /// Local bindings that own writable storage, keyed by their pattern.
    pub mutable_bindings: std::collections::BTreeSet<TreeNodeId>,
    pub expressions: Vec<TypedExpression>,
    pub evaluated_indices: Vec<(StaticValueRequest, StaticValue)>,
    pub constants: Vec<TypedConstant>,
    /// Lexical binders partition the flat argument tuple for a specialized static request.
    pub static_request_scopes: std::collections::BTreeMap<TreeNodeId, Vec<(DeclarationId, usize)>>,
    pub(crate) constant_references: Vec<(omega_resolved::TreeNodeId, DeclarationId)>,
    /// Evaluated receivers, projections, and aliases introduced by borrowed patterns.
    pub(crate) place_steps: std::collections::BTreeMap<omega_resolved::TreeNodeId, crate::PlaceStep>,
    /// Numbered record, sum and payload scopes keyed by their owner declaration.
    pub schema_numbering: std::collections::HashMap<DeclarationId, Arc<omega_program_contracts::SchemaNumbering>>,
    pub record_fields: std::collections::HashMap<DeclarationId, Arc<Vec<TypedField>>>,
    /// The declaring sum of each case, published once from the case
    /// declaration's syntactic owner. Consumers must not infer it from paths.
    pub case_owners: std::collections::HashMap<DeclarationId, DeclarationId>,
    /// The normalized subject type of each domain declared in this forest.
    pub domain_carriers: std::collections::HashMap<DeclarationId, ValueType>,
    /// The underlying type of a transparent alias in its defining binder scope.
    pub type_alias_targets: std::collections::HashMap<DeclarationId, ValueType>,
    /// The normalized constituents of each domain alias, including their arguments.
    pub domain_alias_members: std::collections::HashMap<DeclarationId, Vec<crate::DomainApplication>>,
    pub static_data_shapes: std::collections::HashMap<ValueType, StaticDataShape>,
    pub(crate) constructions: std::collections::BTreeMap<TreeNodeId, TypedConstruction>,
    pub(crate) owned_byte_literals: std::collections::BTreeMap<TreeNodeId, Arc<[u8]>>,
    pub signatures: Vec<Arc<TypedSignature>>,
    /// Authored callable alias signatures and their nodes in this forest.
    pub(crate) callable_origins: Vec<(Site, TreeNodeId)>,
    pub callable_bounds: Vec<(DeclarationId, usize, CallableType)>,
    pub calls: Vec<TypedCall>,
    pub operations: Vec<TypedOperation>,
    pub instructions: Vec<TypedInstruction>,
    pub assembly_regions: std::collections::BTreeMap<TreeNodeId, crate::TypedAssemblyRegion>,
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
    /// Execution calls of an exact canonical terminal requirement, in call order.
    pub external_completions: Vec<crate::ExternalCompletion>,
    pub invocation_ceilings: Vec<(DeclarationId, Site, InvocationTarget)>,
    pub refinements: Vec<TraitRefinement>,
    pub indirect_calls: Vec<IndirectCall>,
    pub memberships: Vec<TypedMembership>,
    /// A successful discriminator test suffices for the entire selected case pattern.
    pub complete_case_patterns: std::collections::BTreeMap<omega_resolved::TreeNodeId, DeclarationId>,
    pub machine_effects: Vec<Arc<MachineEffects>>,
    /// Applied trait selected by the declaration, including a trait with no member rows.
    pub conformance_targets: std::collections::BTreeMap<DeclarationId, crate::RequirementSlot>,
    pub conformance_rows: Vec<ConformanceRow>,
    /// Source expressions whose exact numeric value lands at a concrete destination.
    pub landings: Vec<TypedNumericLanding>,
}

impl TypedStage {
    pub fn record_callable_origin(
        &mut self,
        contract: Site,
        node: TreeNodeId,
    ) {
        self.callable_origins.push((contract, node));
    }

    pub fn record_constant_reference(
        &mut self,
        node: TreeNodeId,
        constant: DeclarationId,
    ) {
        self.constant_references.push((node, constant));
    }

    pub fn constant_references(&self) -> &[(TreeNodeId, DeclarationId)] {
        &self.constant_references
    }

    pub fn record_place_step(
        &mut self,
        node: TreeNodeId,
        step: crate::PlaceStep,
    ) {
        self.place_steps.insert(node, step);
    }

    pub fn construction_carriers(&self) -> impl Iterator<Item = &ValueType> {
        self.constructions
            .values()
            .map(|construction| &construction.carrier)
    }

    pub fn record_construction(
        &mut self,
        node: TreeNodeId,
        construction: TypedConstruction,
    ) {
        self.constructions.insert(node, construction);
    }

    pub fn record_owned_byte_literal(
        &mut self,
        node: TreeNodeId,
        bytes: Arc<[u8]>,
    ) {
        self.owned_byte_literals.insert(node, bytes);
    }
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
        mut node: omega_resolved::TreeNodeId,
    ) -> (omega_resolved::TreeNodeId, Vec<&crate::PlaceStep>) {
        let mut steps = Vec::new();
        while let Some(step) = self.place_steps.get(&node) {
            if step.through.is_some() || step.projection.is_some() {
                steps.push(step);
            }
            node = step.base;
        }
        steps.reverse();
        (node, steps)
    }
}
