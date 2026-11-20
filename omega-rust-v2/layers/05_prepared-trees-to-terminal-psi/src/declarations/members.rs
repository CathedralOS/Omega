//! Preserve the applied requirement separately from the body or boundary that realizes it.
use super::type_expression::type_expression;
use crate::lowering::applications::MachineInstance;
use crate::machines::calls::Callees;
use crate::{
    BoundaryDirection, ConformanceMember, LoweredConformance, LoweredTrait, LoweringError, LoweringTypes, MemberCallSelection, MemberRealization,
    TemplateRequirementSlot, site_at,
};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, Site, TypedCall, TypedSignature, TypedTree, TypedTrees, ValueType};
use std::collections::HashMap;

fn slot(
    id: &DeclarationId,
    arguments: &[ValueType],
    site: &Site,
) -> Result<TemplateRequirementSlot, LoweringError> {
    Ok(TemplateRequirementSlot {
        declaration: crate::identity::declaration(id, site)?,
        arguments: arguments
            .iter()
            .map(|v| type_expression(v, site))
            .collect::<Result<_, _>>()?,
    })
}

pub(crate) fn reach(
    services: &[omega_typed::ServiceApplication],
    instantiate: impl Fn(&ValueType) -> ValueType,
    site: &Site,
) -> Result<Vec<TemplateRequirementSlot>, LoweringError> {
    let mut closed = std::collections::HashSet::with_capacity(services.len());
    let mut rows = Vec::with_capacity(services.len());
    for service in services {
        let arguments: Vec<_> = service.arguments.iter().map(&instantiate).collect();
        let applied = (&service.service, arguments);
        if !closed.contains(&applied) {
            rows.push(slot(applied.0, &applied.1, site)?);
            closed.insert(applied);
        }
    }
    Ok(rows)
}

pub(crate) fn evidence_argument(
    typed: &TypedTrees<'_>,
    instance: &MachineInstance<'_>,
    index: usize,
    selected: &DeclarationId,
) -> Result<crate::StaticArgument, LoweringError> {
    let site = &instance.signature.site;
    let bound = typed
        .generic_parameters
        .get(instance.application.machine.as_ref())
        .and_then(|g| g.parameters.get(index))
        .filter(|p| p.kind == NodeKind::ConstParam)
        .and_then(|p| p.carrier.as_deref())
        .map(|v| instance.application.instantiate(v));
    let Some(ValueType::Evidence(bound)) = bound else {
        return Err(LoweringError::new(site, "selected conformance lacks its declared evidence bound"));
    };
    if !typed.conformance_targets.contains_key(selected) {
        return Err(LoweringError::new(site, "selected conformance lacks its checked trait application"));
    }
    Ok(crate::StaticArgument::Conformance {
        declaration: crate::identity::declaration(selected, site)?,
        bound: slot(&bound.declaration, &bound.trait_arguments, site)?,
    })
}

pub(crate) fn conformance(
    typed: &TypedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    nested: &mut Vec<usize>,
) -> Result<LoweredConformance, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let owner = resolved
        .declaration_at(node)
        .ok_or_else(|| LoweringError::new(&site, "conformance lacks its declared identity"))?;
    let implemented = typed
        .conformance_targets
        .get(owner)
        .ok_or_else(|| LoweringError::new(&site, "conformance lacks its checked trait application"))?;
    let mut members = Vec::new();
    for &child in &file.nodes[node].children {
        match file.nodes[child].kind {
            NodeKind::PathType => continue,
            NodeKind::Machine => nested.push(child),
            NodeKind::SelectionRow => {}
            _ => {
                return Err(LoweringError::new(
                    &site_at(tree, child),
                    "conformance requires additional binder or hypothesis evidence",
                ));
            }
        }
        let mut records = tree.semantics_at(child);
        let row = records
            .find_map(|r| if let NodeSemantics::RequirementBinding(r) = r { Some(r) } else { None })
            .filter(|r| r.conformance.as_ref() == Some(owner))
            .ok_or_else(|| LoweringError::new(&site_at(tree, child), "conformance member lacks its selected implementation"))?;
        if !tree.semantics_at(child).any(|r|matches!(r,NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::RefineRequirement{requirement,implementation} if requirement==&row.requirement && implementation==&row.implementation))){
            return Err(LoweringError::new(&row.site,"conformance member lacks its matching refinement obligation"));
        }
        members.push(ConformanceMember {
            requirement: slot(&row.requirement.declaration, &row.requirement.trait_arguments, &row.site)?,
            implementation: crate::identity::declaration(&row.implementation, &row.site)?,
            site: row.site.clone(),
        });
    }
    Ok(LoweredConformance {
        declaration: crate::identity::declaration(owner, &site)?,
        implemented: slot(&implemented.declaration, &implemented.trait_arguments, &site)?,
        members,
        public: resolved.is_exported(node),
        site,
    })
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub(crate) struct Key {
    owner: DeclarationId,
    requirement: DeclarationId,
    receiver: Vec<ValueType>,
    arguments: Vec<ValueType>,
    conformance: Option<DeclarationId>,
}
impl Key {
    pub(crate) fn from_call(call: &TypedCall) -> Option<Self> {
        let [(owner, receiver)] = call.enclosing_arguments.as_slice() else {
            return None;
        };
        if owner.kind != NodeKind::Trait {
            return None;
        }
        let conformance = match &call.evidence {
            None => None,
            Some(ValueType::Implementation(e)) => Some(*e.clone()),
            _ => return None,
        };
        Some(Self {
            owner: owner.clone(),
            requirement: call.machine.clone(),
            receiver: receiver.clone(),
            arguments: call.type_arguments.clone(),
            conformance,
        })
    }
    /// The requirement's own closed arguments, in binder order.
    pub(crate) fn static_arguments(&self) -> &[ValueType] {
        &self.arguments
    }
    /// The selected conformance's row for this applied requirement. A boundary selection has none.
    pub(crate) fn implementation<'t>(
        &self,
        typed: &'t TypedTrees<'_>,
        site: &Site,
    ) -> Result<Option<&'t DeclarationId>, LoweringError> {
        let Some(id) = &self.conformance else { return Ok(None) };
        typed
            .conformance_rows
            .iter()
            .find(|r| r.conformance.as_ref() == Some(id) && r.requirement.declaration == self.requirement && r.requirement.trait_arguments == self.receiver)
            .map(|r| Some(&r.implementation))
            .ok_or_else(|| LoweringError::new(site, "selected evidence has no row for the applied requirement"))
    }
    /// Close the requirement interface: trait binders from the receiver, then its own binders.
    fn close(
        &self,
        ty: &ValueType,
    ) -> ValueType {
        let applied = ty.substitute(&self.owner, &self.receiver);
        if self.arguments.is_empty() {
            applied
        } else {
            applied.substitute(&self.requirement, &self.arguments)
        }
    }
    /// L3 records the slot against the authored call. Close it in the caller's
    /// application and keep it only if it names this exact requirement application.
    fn boundary_direction(
        &self,
        typed: &TypedTrees<'_>,
        application: &omega_typed::MachineApplication,
        site: &Site,
    ) -> Result<Option<BoundaryDirection>, LoweringError> {
        let closed = |values: &[ValueType], expected: &[ValueType]| {
            values.len() == expected.len()
                && values
                    .iter()
                    .zip(expected)
                    .all(|(v, e)| &application.instantiate(v) == e)
        };
        let mut at_site = typed
            .boundary_calls
            .iter()
            .filter(|(s, _)| s == site)
            .map(|(_, slot)| slot)
            .peekable();
        if at_site.peek().is_none() {
            return Ok(None);
        }
        let mut matching = at_site.filter(|slot| {
            slot.requirement.declaration == self.requirement
                && closed(&slot.requirement.trait_arguments, &self.receiver)
                && closed(&slot.arguments, &self.arguments)
        });
        let (Some(slot), None) = (matching.next(), matching.next()) else {
            return Err(LoweringError::new(site, "boundary slot disagrees with the selected member application"));
        };
        Ok(Some(match slot.direction {
            omega_typed::SlotDirection::ProgramToProvider => BoundaryDirection::ProgramToProvider,
            omega_typed::SlotDirection::EnvironmentToProgram => BoundaryDirection::EnvironmentToProgram,
        }))
    }
}
struct Plan {
    key: Key,
    signature: TypedSignature,
    implementation: Option<DeclarationId>,
    direction: Option<BoundaryDirection>,
}
pub(crate) struct MemberPlans(Vec<Plan>);

pub(crate) fn plan(
    typed: &TypedTrees<'_>,
    trees: &[TypedTree<'_>],
    instances: &[MachineInstance<'_>],
    signatures: &HashMap<&DeclarationId, &TypedSignature>,
) -> Result<MemberPlans, LoweringError> {
    let mut directions = HashMap::new();
    let mut plans = Vec::new();
    let mut by_caller: HashMap<_, Vec<_>> = HashMap::new();
    for call in &typed.calls {
        if call.use_site == CallUse::Execution
            && call
                .enclosing_arguments
                .iter()
                .any(|(id, _)| id.kind == NodeKind::Trait)
            && let Some(owner) = &call.caller
        {
            by_caller.entry(owner).or_default().push(call.node.node);
        }
    }
    for nodes in by_caller.values_mut() {
        nodes.sort_unstable();
    }
    for instance in instances {
        for &node in by_caller.get(&instance.signature.machine).into_iter().flatten() {
            let call = instance
                .tree
                .semantics_at(node)
                .find_map(|s| if let NodeSemantics::Call(c) = s { Some(c) } else { None })
                .ok_or_else(|| LoweringError::new(&instance.signature.site, "selected member occurrence lacks its typed call"))?;
            // A canonical external completion is a terminal transfer, not a member selection.
            if typed.external_completions.iter().any(|c| c.node == call.node) {
                continue;
            }
            let key = Key::from_call(call).ok_or_else(|| LoweringError::new(&call.site, "member call needs a closed requirement and conformance"))?;
            let direction = if key.conformance.is_none() {
                key.boundary_direction(typed, &instance.application, &call.site)?
            } else {
                None
            };
            if let Some(previous) = directions.insert(key.clone(), direction) {
                if previous != direction {
                    return Err(LoweringError::new(&call.site, "boundary occurrences disagree on the selected slot direction"));
                }
                continue;
            }
            let sig = *signatures
                .get(&key.requirement)
                .ok_or_else(|| LoweringError::new(&call.site, "member call needs its packaged requirement"))?;
            let file = trees[sig.node.file].resolved().file;
            if file.generic_parameters(sig.node.node).count() != key.arguments.len() {
                return Err(LoweringError::new(&call.site, "member call needs one static argument per requirement binder"));
            }
            if key
                .arguments
                .iter()
                .flat_map(ValueType::components)
                .any(|ty| matches!(ty, ValueType::Parameter { .. }))
            {
                return Err(LoweringError::new(&call.site, "member call needs closed static arguments"));
            }
            let mut signature = sig.clone();
            for (_, ty) in &mut signature.parameters {
                *ty = key.close(ty);
            }
            signature.result = key.close(&signature.result);
            let implementation = key.implementation(typed, &call.site)?.cloned();
            plans.push(Plan {
                key,
                signature,
                implementation,
                direction,
            });
        }
    }
    Ok(MemberPlans(plans))
}

impl MemberPlans {
    pub(crate) fn collect_carriers<'a>(
        &'a self,
        carriers: &mut Vec<(&'a ValueType, Site)>,
    ) {
        for p in &self.0 {
            let s = &p.signature;
            carriers.extend(s.parameters.iter().map(|(_, t)| (t, s.site.clone())));
            if s.result != ValueType::NoResult {
                carriers.push((&s.result, s.site.clone()));
            }
        }
    }
    pub(crate) fn lower<'a>(
        &'a self,
        types: &LoweringTypes,
        traits: &[LoweredTrait],
        callees: &mut Callees<'a>,
    ) -> Result<Vec<MemberCallSelection>, LoweringError> {
        let mut result = Vec::with_capacity(self.0.len());
        for p in &self.0 {
            let key = &p.key;
            let sig = &p.signature;
            let site = &sig.site;
            let owner = slot(&key.owner, &key.receiver, site)?;
            let requirement = slot(&key.requirement, &key.receiver, site)?;
            let declaration = traits
                .iter()
                .find(|t| t.declaration == owner.declaration)
                .ok_or_else(|| LoweringError::new(site, "call requires its packaged trait"))?;
            let substituted_domains = || sig.parameters.iter().any(|(_, p)| !p.domains().is_empty()) || !sig.result.domains().is_empty();
            let (count, arity) = if let Some(m) = declaration
                .requirements
                .iter()
                .find(|r| r.declaration == requirement.declaration)
            {
                (m.requires.len(), m.value_indices.len())
            } else if let Some(m) = declaration
                .dependent_requirements
                .iter()
                .find(|r| r.declaration == requirement.declaration)
            {
                if substituted_domains() {
                    return Err(LoweringError::new(site, "applied requirement needs its substituted domain obligations"));
                }
                (m.signature.requires.len(), m.parameters.len())
            } else {
                return Err(LoweringError::new(site, "call requires its exact packaged member"));
            };
            if arity != key.arguments.len() {
                return Err(LoweringError::new(
                    site,
                    "member call static arguments disagree with the packaged requirement binders",
                ));
            }
            let static_arguments = key
                .arguments
                .iter()
                .map(|argument| match argument {
                    ValueType::MachineSelection(_) | ValueType::Implementation(_) | ValueType::Callable(_) => Err(LoweringError::new(
                        site,
                        "member call callable or evidence argument requires its selection lowering",
                    )),
                    _ => super::type_expression::declaration_term(argument, site),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let target = match (&key.conformance, &p.implementation) {
                (None, None) if declaration.boundary => MemberRealization::Boundary { direction: p.direction },
                (Some(id), Some(machine)) => {
                    // The implementation's binders correspond to the requirement's by position.
                    let implementation = callees
                        .local
                        .get(machine)
                        .and_then(|entries| entries.get(key.arguments.as_slice()))
                        .ok_or_else(|| LoweringError::new(site, "selected conformance implementation needs its exact lowered machine"))?;
                    if callees.realized_pause(implementation.machine) != Default::default() {
                        return Err(LoweringError::new(site, "suspending conformance calls need their exact crossing frontier"));
                    }
                    // Refinement owns the implementation preconditions. The call owes the public requirement.
                    MemberRealization::Conformance {
                        declaration: crate::identity::declaration(id, site)?,
                        machine: implementation.machine,
                    }
                }
                _ => return Err(LoweringError::new(site, "ordinary trait call requires selected conformance evidence")),
            };
            let id_for = |v: &ValueType| {
                types
                    .id_for(v)
                    .map_err(|e| LoweringError::new(site, e.to_string()))
            };
            callees.members.insert(key.clone(), (result.len(), sig, count));
            result.push(MemberCallSelection {
                owner,
                requirement,
                static_arguments,
                parameters: sig
                    .parameters
                    .iter()
                    .map(|(_, v)| {
                        types
                            .parameter_type(v)
                            .map_err(|e| LoweringError::new(site, e.to_string()))
                    })
                    .collect::<Result<_, _>>()?,
                result: (sig.result != ValueType::NoResult)
                    .then(|| id_for(&sig.result))
                    .transpose()?,
                target,
            });
        }
        Ok(result)
    }
}
