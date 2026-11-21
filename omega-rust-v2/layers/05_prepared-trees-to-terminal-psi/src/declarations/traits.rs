use super::{
    contracts::{ClauseOwner, lower_clause},
    domains::DomainSchema,
    machine::append_domain_predicates,
};
use crate::{ContractClause, LoweredRequirement, LoweredTrait, LoweringError, LoweringTypes, PredicateStep, site_at};
use omega_prepared::PreparedTrees;
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{Site, TypedSignature, TypedTree, ValueType};
use std::collections::HashMap;

pub(crate) struct TraitSchema<'a> {
    pub declaration: &'a DeclarationId,
    public: bool,
    boundary: bool,
    restriction: Option<crate::TraitRestriction>,
    site: Site,
    parameters: Vec<crate::TemplateBinder>,
    dependent_requirements: Vec<crate::TemplateRequirement>,
    requirements: Vec<RequirementSchema<'a>>,
}

struct RequirementSchema<'a> {
    signature: &'a TypedSignature,
    indices: Vec<&'a ValueType>,
    requires: Vec<ContractClause>,
    ensures: Vec<ContractClause>,
    crash_routes: Vec<crate::CrashRoute>,
}

pub(crate) fn read_trait<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a TypedTree<'_>,
    node: usize,
    signatures: &HashMap<&DeclarationId, &'a TypedSignature>,
) -> Result<TraitSchema<'a>, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let declaration = resolved
        .declaration_at(node)
        .ok_or_else(|| LoweringError::new(&site, "trait has no resolved identity"))?;
    let mut requirements = Vec::new();
    let (parameters, witnesses) = super::type_expression::template_binders(prepared.typed().generic_parameters.get(declaration), &site)?;
    if witnesses != 0
        || !parameters
            .iter()
            .all(|b| matches!(b, crate::TemplateBinder::Type { .. }))
    {
        return Err(LoweringError::new(&site, "unsupported generic trait binder"));
    }
    if let Some(base) = prepared.typed().refinement_bases.get(declaration) {
        if !parameters.is_empty() || !base.trait_arguments.is_empty() || file.nodes[node].has_boundary_modifier() {
            return Err(LoweringError::new(&site, "restricted trait needs an ordinary unapplied base"));
        }
        let [parent, clauses @ ..] = file.nodes[node].children.as_slice() else {
            return Err(LoweringError::new(&site, "restricted trait lacks its base"));
        };
        if !matches!(resolved.symbol(*parent), Some(omega_resolved::Symbol::Declaration(id)) if id == &base.declaration) {
            return Err(LoweringError::new(&site_at(tree, *parent), "restricted trait base disagrees with its resolved declaration"));
        }
        let mut all_members = Vec::new();
        for &child in clauses {
            let clause = site_at(tree, child);
            let checked = prepared.typed().refinements.iter()
                .find(|r| r.schema == *declaration && r.site == clause && file.nodes[child].kind == NodeKind::Restriction)
                .ok_or_else(|| LoweringError::new(&clause, "restriction lacks its checked wildcard clause"))?;
            let axes = &file.nodes[child].children;
            if axes.iter().any(|&n| match file.nodes[n].kind {
                NodeKind::Blocks | NodeKind::Suspends => file.nodes[n].pause_flag() != Some(omega_syntax::PauseFlag::Explicit(false)),
                NodeKind::Reaches | NodeKind::Terminates => false,
                _ => true,
            }) || axes.iter().filter(|&&n| file.nodes[n].kind == NodeKind::Reaches).count() > 1 {
                return Err(LoweringError::new(&clause, "unsupported wildcard contract refinement"));
            }
            all_members.push(crate::MemberRestriction {
                prohibits_block: checked.prohibits_block,
                prohibits_suspend: checked.prohibits_suspend,
                reach: checked.reach.as_deref().map(|r| super::members::reach(r, Clone::clone, &clause)).transpose()?,
                requires_termination: checked.terminates,
            });
        }
        return Ok(TraitSchema {
            declaration,
            public: resolved.is_exported(node),
            boundary: false,
            restriction: Some(crate::TraitRestriction {
                base: crate::TemplateRequirementSlot { declaration: crate::identity::declaration(&base.declaration, &site)?, arguments: Vec::new() },
                all_members,
            }),
            site,
            parameters,
            dependent_requirements: Vec::new(),
            requirements: Vec::new(),
        });
    }
    let mut dependent_requirements = Vec::new();
    for &child in &file.nodes[node].children {
        if file.nodes[child].kind == NodeKind::GenericParams && file.generic_parameters(node).count() == parameters.len() {
            continue;
        }
        if file.nodes[child].kind != NodeKind::Machine {
            return Err(LoweringError::new(
                &site_at(tree, child),
                "trait binders, inheritance and restrictions require an applied trait interface",
            ));
        }
        let signature = resolved
            .declaration_at(child)
            .and_then(|id| signatures.get(id))
            .copied()
            .filter(|sig| sig.node == file.node_id(child))
            .ok_or_else(|| LoweringError::new(&site_at(tree, child), "trait member has no matching typed requirement"))?;
        // A type or const binder makes the requirement a template. Runtime value
        // indices alone keep the closed requirement form below.
        if file
            .generic_parameters(child)
            .any(|(_, _, kind)| kind != NodeKind::KindedParam)
        {
            dependent_requirements.push(generic_requirement(prepared, tree, child, signature)?);
            continue;
        }
        if !parameters.is_empty() {
            if file.generic_parameters(child).next().is_some() || file.child(child, NodeKind::Body).is_some() {
                return Err(LoweringError::new(
                    &signature.site,
                    "generic trait member requires a bodyless, unapplied requirement",
                ));
            }
            dependent_requirements.push(crate::TemplateRequirement {
                declaration: crate::identity::declaration(&signature.machine, &signature.site)?,
                parameters: Vec::new(),
                signature: super::machine_templates::signature(prepared, tree, signature)?,
                declared_pause: signature.pause,
            });
            continue;
        }
        let mut indices = Vec::new();
        for (index, (_, binder, kind)) in file.generic_parameters(child).enumerate() {
            let bound = prepared
                .typed()
                .generic_parameters
                .get(&signature.machine)
                .and_then(|g| g.parameters.get(index));
            let carrier = bound
                .filter(|b| b.kind == kind && kind == NodeKind::KindedParam && b.multiplicity.is_none())
                .and_then(|b| b.carrier.as_deref())
                .filter(|ty| matches!(ty, ValueType::Integer { width: Some(1..), .. } | ValueType::Bool))
                .ok_or_else(|| LoweringError::new(&site_at(tree, binder), "trait requirement needs a supported runtime value-index carrier"))?;
            indices.push(carrier);
        }
        let owner = ClauseOwner::Requirement(signature, &indices);
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        for &part in &file.nodes[child].children {
            match file.nodes[part].kind {
                NodeKind::Path | NodeKind::Params | NodeKind::Result | NodeKind::GenericParams | NodeKind::Reaches | NodeKind::Blocks | NodeKind::Suspends => {}
                // Routes, including guards over the requirement formals, are lowered below.
                NodeKind::Crashes => {}
                NodeKind::Terminates if file.nodes[part].children.is_empty() => {}
                NodeKind::Requires => requires.push(lower_clause(prepared, &crate::lowering::applied_tree::LoweringTree::plain(tree), owner, part)?),
                NodeKind::Ensures => ensures.push(lower_clause(prepared, &crate::lowering::applied_tree::LoweringTree::plain(tree), owner, part)?),
                _ => return Err(LoweringError::new(&site_at(tree, part), "unsupported trait requirement clause")),
            }
        }
        if signature.requires == requires.is_empty()
            || signature.has_guarantee == ensures.is_empty()
            || signature.operator.is_some()
            || signature.satisfies.is_some()
            || !signature.multiplicity_bounds.is_empty()
        {
            return Err(LoweringError::new(&signature.site, "trait requirement needs additional signature evidence"));
        }
        let crash_routes = super::crash_routes::lower_routes(
            prepared,
            &crate::lowering::applied_tree::LoweringTree::plain(tree),
            owner,
            child,
            &signature.site,
            signature.crash_ceiling.as_deref(),
        )?;
        requirements.push(RequirementSchema {
            signature,
            indices,
            requires,
            ensures,
            crash_routes,
        });
    }
    Ok(TraitSchema {
        declaration,
        site,
        public: resolved.is_exported(node),
        boundary: file.nodes[node].has_boundary_modifier(),
        restriction: None,
        parameters,
        dependent_requirements,
        requirements,
    })
}

/// Publish the requirement's own binders with its open interface. Default
/// bodies, private witnesses and binders without a checked kind are refused.
fn generic_requirement(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    child: usize,
    signature: &TypedSignature,
) -> Result<crate::TemplateRequirement, LoweringError> {
    let file = tree.resolved().file;
    if file.child(child, NodeKind::Body).is_some() {
        return Err(LoweringError::new(&signature.site, "generic trait member requires a bodyless requirement"));
    }
    let checked = prepared.typed().generic_parameters.get(&signature.machine);
    let (parameters, witnesses) = super::type_expression::template_binders(checked, &signature.site)?;
    if witnesses != 0 || checked.is_some_and(|p| !p.equations.is_empty()) || parameters.len() != file.generic_parameters(child).count() {
        return Err(LoweringError::new(&signature.site, "generic requirement needs exactly its checked binders"));
    }
    Ok(crate::TemplateRequirement {
        declaration: crate::identity::declaration(&signature.machine, &signature.site)?,
        parameters,
        signature: super::machine_templates::signature(prepared, tree, signature)?,
        declared_pause: signature.pause,
    })
}

impl<'a> TraitSchema<'a> {
    /// Bodyless ordinary requirements available to calls through a supplied record.
    pub(super) fn unapplied_members(&self) -> impl Iterator<Item = &'a DeclarationId> {
        self.requirements.iter().filter(|r| !self.boundary && self.parameters.is_empty() && r.indices.is_empty()).map(|r| &r.signature.machine)
    }

    pub(crate) fn collect_carriers(
        &self,
        carriers: &mut Vec<(&'a ValueType, Site)>,
    ) {
        for r in &self.requirements {
            let sig = r.signature;
            carriers.extend(r.indices.iter().map(|ty| (*ty, sig.site.clone())));
            for (span, ty) in &sig.parameters {
                let mut site = sig.site.clone();
                site.span = *span;
                carriers.push((ty, site));
            }
            if sig.result != ValueType::NoResult {
                carriers.push((&sig.result, sig.site.clone()));
            }
        }
    }

    pub(crate) fn lower(
        self,
        types: &LoweringTypes,
        domains: &[DomainSchema<'_>],
    ) -> Result<LoweredTrait, LoweringError> {
        let mut requirements = Vec::with_capacity(self.requirements.len());
        for mut r in self.requirements {
            let sig = r.signature;
            let id = |ty: &ValueType| {
                types
                    .id_for(ty)
                    .map_err(|e| LoweringError::new(&sig.site, e.to_string()))
            };
            let value_indices = r.indices.into_iter().map(id).collect::<Result<Vec<_>, _>>()?;
            let parameter_types = sig
                .parameters
                .iter()
                .map(|(_, ty)| {
                    types
                        .parameter_type(ty)
                        .map_err(|e| LoweringError::new(&sig.site, e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result_type = (sig.result != ValueType::NoResult)
                .then(|| id(&sig.result))
                .transpose()?;
            for (index, (_, ty)) in sig.parameters.iter().enumerate() {
                append_domain_predicates(&mut r.requires, ty, PredicateStep::Formal(index), domains)?;
            }
            append_domain_predicates(&mut r.ensures, &sig.result, PredicateStep::Result, domains)?;
            let reach = sig
                .reach
                .as_deref()
                .map(|row| super::members::reach(row, Clone::clone, &sig.site))
                .transpose()?;
            requirements.push(LoweredRequirement {
                declaration: crate::identity::declaration(&sig.machine, &sig.site)?,
                family: super::finite_family::declared_family(value_indices.len(), &r.requires),
                value_indices,
                parameter_types,
                result_type,
                requires: r.requires,
                ensures: r.ensures,
                reach,
                crash_ceiling: sig.crash_ceiling.clone(),
                crash_routes: r.crash_routes,
                pause: sig.pause,
                terminates: sig.terminates,
                site: sig.site.clone(),
            });
        }
        Ok(LoweredTrait {
            declaration: crate::identity::declaration(self.declaration, &self.site)?,
            public: self.public,
            boundary: self.boundary,
            restriction: self.restriction,
            parameters: self.parameters,
            dependent_requirements: self.dependent_requirements,
            requirements,
            site: self.site,
        })
    }
}
