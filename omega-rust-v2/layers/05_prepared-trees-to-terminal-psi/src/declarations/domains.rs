use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{Site, TypedTree, ValueType};

use crate::declarations::contracts::{ClauseOwner, lower_clause};
use crate::{ContractClause, LoweringError, LoweringTypes, site_at};

use crate::lowering::pending::IssuersPending;
use crate::{LoweredDomain, LoweredDomainAlias};
/// A policy execution cannot depend on the issuance answers it is computing.
/// Its declarations must also be closed: a child without its parent or an
/// alias without a constituent would name a domain absent from the unit. Use
/// the checked identities, irrespective of declaration order or spelling.
pub(crate) fn policy_unit_exclusions(typed: &omega_typed::TypedStage) -> std::collections::HashSet<&omega_resolved::DeclarationId> {
    let mut dependents = std::collections::HashMap::<_, Vec<_>>::new();
    for (domain, carrier) in &typed.domain_carriers {
        let members = typed.domain_alias_members.get(domain).into_iter().flatten();
        for prerequisite in carrier.domains().iter().chain(members) {
            dependents.entry(&prerequisite.declaration).or_default().push(domain);
        }
    }
    let mut excluded = std::collections::HashSet::new();
    let mut pending: Vec<_> = typed.access_policies.iter().map(|selection| &selection.domain).collect();
    while let Some(domain) = pending.pop() {
        if excluded.insert(domain) && let Some(children) = dependents.remove(domain) {
            pending.extend(children);
        }
    }
    excluded
}

pub(crate) enum DomainDeclaration<'a> {
    /// Lowering decides no issuer, so the row states none. `policy` is the
    /// selection whose executed plan decides it. Without one the domain has no
    /// access plan and anyone may issue it.
    Named {
        row: LoweredDomain<crate::TypeId, IssuersPending>,
        policy: Option<&'a omega_typed::AccessPolicySelection>,
    },
    Alias(LoweredDomainAlias),
}

/// The published row of a domain that selects no access policy: it has no
/// plan, so nothing restricts who may issue it.
pub(crate) fn unrestricted(row: LoweredDomain<crate::TypeId, IssuersPending>) -> LoweredDomain {
    // Every field but the issuer is the row as lowered.
    let LoweredDomain {
        parameters,
        index_binders,
        declaration,
        carrier,
        cleanup_target,
        public,
        requires,
        issuers: IssuersPending,
        site,
    } = row;
    LoweredDomain {
        parameters,
        index_binders,
        declaration,
        carrier,
        cleanup_target,
        public,
        requires,
        issuers: omega_psi::DomainIssuers::Unrestricted,
        site,
    }
}

/// What lowering does with a domain that selects an access policy.
#[derive(Clone, Copy)]
pub(crate) enum SelectedPolicies {
    /// Refuse the package, with the reason the policy is unanswered.
    Refuse,
    /// Lower the row without an issuer and leave the decision to the driver.
    Defer,
}

enum DomainDefinition {
    Named(Vec<ContractClause>),
    Alias(Vec<omega_psi::DomainAtom>),
}

pub(crate) struct DomainSchema<'a> {
    pub declaration: &'a DeclarationId,
    pub carrier: &'a ValueType,
    pub site: Site,
    public: bool,
    /// The access policy this domain selects. Its executed plan says who may
    /// introduce the qualification, and lowering does not have that plan.
    pub(crate) policy: Option<&'a omega_typed::AccessPolicySelection>,
    parameters: Vec<crate::TemplateBinder>,
    index_binders: Vec<usize>,
    definition: DomainDefinition,
}

impl<'a> DomainSchema<'a> {
    pub(crate) fn predicates(&self) -> Option<&[ContractClause]> {
        match &self.definition {
            DomainDefinition::Named(clauses) if self.parameters.is_empty() => Some(clauses),
            DomainDefinition::Named(_) => None,
            DomainDefinition::Alias(_) => None,
        }
    }

    pub(crate) fn into_template(self) -> Result<LoweredDomain<crate::TypeExpression>, LoweringError> {
        let DomainDefinition::Named(requires) = self.definition else {
            return Err(LoweringError::new(&self.site, "carrier-generic domain alias needs an applied expansion"));
        };
        Ok(LoweredDomain {
            declaration: crate::identity::declaration(self.declaration, &self.site)?,
            carrier: super::type_expression::type_expression(self.carrier, &self.site)?,
            parameters: self.parameters,
            index_binders: self.index_binders,
            cleanup_target: None,
            public: self.public,
            requires,
            // `read_domain` refuses a carrier-generic domain that selects a policy.
            issuers: omega_psi::DomainIssuers::Unrestricted,
            site: self.site,
        })
    }

    /// Whether the declaration authors any requirement, whether or not it has binders.
    pub(crate) fn has_predicates(&self) -> bool {
        matches!(&self.definition, DomainDefinition::Named(clauses) if !clauses.is_empty())
    }

    /// The authored clauses at one application of this domain. Each index
    /// binder reads the closed argument at that binder's head position, so
    /// `Bound<5>` of `Bound<N> requires self > N` states `self > 5`.
    /// `None` means some clause reads a binder the application does not close;
    /// the caller must refuse rather than publish a qualification without it.
    pub(crate) fn applied_predicates(
        &self,
        arguments: &[ValueType],
    ) -> Option<Vec<ContractClause>> {
        let DomainDefinition::Named(clauses) = &self.definition else {
            return None;
        };
        if arguments.is_empty() {
            return self.predicates().map(<[_]>::to_vec);
        }
        if arguments.len() != self.index_binders.len() {
            return None;
        }
        let closed = |slot: usize| {
            let position = self.index_binders.iter().position(|&binder| binder == slot)?;
            match &arguments[position] {
                ValueType::StaticInteger { value, .. } => Some(crate::PredicateStep::Integer(value.clone())),
                ValueType::StaticValue {
                    carrier,
                    value: omega_typed::StaticValue::Bool(value),
                } if **carrier == ValueType::Bool => Some(crate::PredicateStep::Boolean(*value)),
                _ => None,
            }
        };
        clauses
            .iter()
            .map(|clause| {
                let predicate = clause
                    .predicate
                    .iter()
                    .map(|step| match step {
                        crate::PredicateStep::ValueIndex(slot) => closed(*slot),
                        // Domain clauses own no mathematical terms; never substitute inside one.
                        crate::PredicateStep::MathematicalTerm(_) => None,
                        other => Some(other.clone()),
                    })
                    .collect::<Option<_>>()?;
                Some(ContractClause {
                    predicate,
                    site: clause.site.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn lower(
        self,
        types: &LoweringTypes,
    ) -> Result<DomainDeclaration<'a>, LoweringError> {
        let carrier = types
            .id_for(self.carrier)
            .map_err(|error| LoweringError::new(&self.site, error.to_string()))?;
        let declaration = crate::identity::declaration(self.declaration, &self.site)?;
        let public = self.public;
        let site = self.site;
        Ok(match self.definition {
            DomainDefinition::Named(requires) => DomainDeclaration::Named {
                row: LoweredDomain {
                    parameters: self.parameters,
                    index_binders: self.index_binders,
                    cleanup_target: types.domain_cleanup(self.declaration),
                    declaration,
                    carrier,
                    public,
                    requires,
                    issuers: IssuersPending,
                    site,
                },
                policy: self.policy,
            },
            DomainDefinition::Alias(members) => DomainDeclaration::Alias(LoweredDomainAlias {
                declaration,
                carrier,
                public,
                parameters: self.parameters,
                index_binders: self.index_binders,
                members,
                site,
            }),
        })
    }
}

/// Lower the actual selected definition while the domain still owes the result of
/// evaluating it. The resulting template names its callees; any external call
/// needs its own definition and proof before the policy can execute.
pub fn lower_selected_policy_definition(
    prepared: &PreparedTrees<'_>,
    selection: &omega_typed::AccessPolicySelection,
) -> Result<crate::SelectedPolicyDefinition, LoweringError> {
    let typed = prepared.typed();
    let site = &selection.site;
    if !typed.access_policies.contains(selection) {
        return Err(LoweringError::new(site, "access policy selection is absent from the prepared package"));
    }
    let trees: Vec<_> = typed.trees().collect();
    let (owner_tree, owner_node) = trees.iter().find_map(|tree| {
        tree.resolved().declaration_nodes().find(|(_, id)| *id == &selection.policy).map(|(node, _)| (tree, node))
    }).ok_or_else(|| LoweringError::new(site, "selected access policy needs its defining conformance"))?;
    let conformance = super::members::conformance(typed, owner_tree, owner_node, &mut Vec::new())?;
    let sig = typed.signatures.iter().find(|s| s.machine == selection.plan_machine)
        .ok_or_else(|| LoweringError::new(site, "selected access policy lacks its machine signature"))?;
    if !trees.get(sig.node.file).is_some_and(|tree| tree.resolved().declaration_at(sig.node.node) == Some(&sig.machine)) {
        return Err(LoweringError::new(site, "selected access policy needs its defining machine body"));
    }
    let plans = super::machine_templates::plan(prepared, &trees, &[sig], &[], &[])?;
    let [plan] = <[_; 1]>::try_from(plans)
        .map_err(|_| LoweringError::new(site, "selected access policy needs one machine template"))?;
    if !conformance.members.iter().any(|row| row.implementation == plan.declaration) {
        return Err(LoweringError::new(site, "selected access policy machine is absent from its conformance"));
    }
    Ok(crate::SelectedPolicyDefinition {
        domain: crate::identity::declaration(&selection.domain, site)?,
        conformance,
        arguments: selection.arguments.iter().map(|a| super::type_expression::declaration_term(a, site)).collect::<Result<_, _>>()?,
        plan,
        site: site.clone(),
    })
}

/// Why the domain of `selection` cannot be published while its policy has no
/// admitted answer. The reason is how far that policy has come in `prepared`.
/// [`crate::lower_package`] refuses the domain with this error, and a driver
/// that defers the decision reports the same one for a domain it cannot answer.
pub fn unanswered_policy_refusal(
    prepared: &PreparedTrees<'_>,
    selection: &omega_typed::AccessPolicySelection,
) -> LoweringError {
    let refuse = |reason: &str| LoweringError::new(&selection.site, reason);
    let executed = "domain access policy needs admitted execution of the selected plan for its semantic schema";
    let Some(demand) = prepared.pending_access_plan(&selection.domain) else {
        return refuse("selected domain access policy lacks its prepared invocation demand");
    };
    match (demand.schema, demand.pending_construction) {
        (omega_prepared::PolicySchemaInput::NeedsPackageOwner, _) => refuse("domain access policy needs the package owner for its semantic schema"),
        (omega_prepared::PolicySchemaInput::UnsupportedSubject, _) => refuse("domain access policy needs a semantic schema for this carrier"),
        (omega_prepared::PolicySchemaInput::Ready { .. }, Some(_)) if demand.construction_checked() => refuse(executed),
        (omega_prepared::PolicySchemaInput::Ready { .. }, Some(construction)) => match construction.carrier.nominal_parts() {
            Some((data, _)) => refuse(&format!(
                "domain access policy needs checked establishment of `{}` in `{}`",
                data.path, construction.machine.machine.path
            )),
            None => refuse("domain access policy construction lacks its data carrier"),
        },
        (omega_prepared::PolicySchemaInput::Ready { .. }, None) => refuse("domain access policy needs its evaluated plan for the selected semantic schema"),
    }
}

pub(crate) fn read_domain<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a TypedTree<'_>,
    node: usize,
    selected: SelectedPolicies,
) -> Result<DomainSchema<'a>, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let declaration = resolved
        .declaration_at(node)
        .ok_or_else(|| LoweringError::new(&site, "domain has no resolved identity"))?;
    let binders = prepared.typed().generic_parameters.get(declaration);
    let (parameters, witness_count) = super::type_expression::template_binders(binders, &site)?;
    if witness_count != 0 || binders.is_some_and(|p| !p.equations.is_empty()) || file.generic_parameters(node).count() != parameters.len() {
        return Err(LoweringError::new(&site, "domain requires its complete checked generic binders"));
    }
    let carrier = prepared
        .typed()
        .domain_carriers
        .get(declaration)
        .ok_or_else(|| LoweringError::new(&site, "domain has no typed carrier"))?;
    // Who may issue a domain that selects a policy is what an execution of that
    // policy returns. Lowering runs nothing, so it either refuses the domain or
    // lowers its row with no issuer.
    let policy = prepared
        .typed()
        .access_policies
        .iter()
        .find(|policy| &policy.domain == declaration);
    let alias = prepared.typed().domain_alias_members.get(declaration);
    if let Some(selection) = policy {
        if matches!(selected, SelectedPolicies::Refuse) {
            return Err(unanswered_policy_refusal(prepared, selection));
        }
        // Only a named domain on a closed carrier has a row that states
        // issuers. An alias row has no such field, and a cast never consults a
        // carrier-generic family, so a plan answered for either would be dropped.
        let unpublished = if alias.is_some() {
            Some("domain alias cannot publish an access policy: its qualification belongs to the domains it names")
        } else if matches!(carrier, ValueType::Parameter { .. }) {
            Some("carrier-generic domain cannot publish an access policy: no closed carrier takes its plan")
        } else {
            None
        };
        if let Some(reason) = unpublished {
            // Without a frozen invocation no plan can be answered, and that is the earlier fault.
            return Err(match prepared.policy_invocation(declaration) {
                Some(_) => LoweringError::new(&selection.site, reason),
                None => unanswered_policy_refusal(prepared, selection),
            });
        }
    }
    let head = file
        .child(node, NodeKind::PathType)
        .ok_or_else(|| LoweringError::new(&site, "domain has no typed head"))?;
    if matches!(resolved.symbol(head), Some(Symbol::Declaration(parent)) if parent.kind == NodeKind::Domain) && carrier.domains().is_empty() {
        return Err(LoweringError::new(&site_at(tree, head), "inherited domain requires its parent qualification"));
    }
    let public = resolved.exports().any(|export| &export.id == declaration);
    let index_binders = resolved
        .arguments_for(head, declaration)
        .iter()
        .map(|&arg| {
            let slot = match resolved.symbol(arg) {
                Some(Symbol::Binding { binder_node, .. }) => file
                    .generic_parameters(node)
                    .position(|(_, binder, _)| binder == *binder_node),
                _ => None,
            };
            slot.ok_or_else(|| LoweringError::new(&site_at(tree, arg), "domain index has no declaration binder"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(members) = alias {
        let atoms = members
            .iter()
            .map(|member| {
                Ok(omega_psi::DomainAtom {
                    declaration: crate::identity::declaration(&member.declaration, &site)?,
                    arguments: member
                        .arguments
                        .iter()
                        .map(|arg| super::type_expression::declaration_term(arg, &site))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, LoweringError>>()?;
        return Ok(DomainSchema {
            declaration,
            carrier,
            site,
            public,
            policy,
            parameters,
            index_binders,
            definition: DomainDefinition::Alias(atoms),
        });
    }
    let mut requires = Vec::new();
    let indices = binders
        .into_iter()
        .flat_map(|g| &g.parameters)
        .map(|p| p.carrier.as_deref())
        .collect::<Vec<_>>();
    for &member in &file.nodes[node].children {
        if member == head {
            continue;
        }
        match file.nodes[member].kind {
            NodeKind::GenericParams => {}
            // The clause selects the policy. The row carries that selection and no issuer.
            NodeKind::Access if policy.is_some() => {}
            NodeKind::Requires => requires.push(lower_clause(
                prepared,
                &crate::lowering::applied_tree::LoweringTree::plain(tree),
                if parameters.is_empty() {
                    ClauseOwner::Subject(carrier)
                } else {
                    ClauseOwner::Domain(node, carrier, &indices)
                },
                member,
            )?),
            NodeKind::DroppedBy if parameters.is_empty() && matches!(carrier, ValueType::Nominal(_)) => {
                let site = site_at(tree, member);
                let selection = prepared.typed().cleanups.iter().find(|s| s.owner == *declaration && s.site == site)
                    .ok_or_else(|| LoweringError::new(&site, "domain cleanup lacks its selected machine"))?;
                let mut rows = tree.semantics_at(member);
                if !matches!(rows.next(), Some(omega_typed::NodeSemantics::Obligation(o))
                        if matches!(&o.kind, omega_typed::ObligationKind::CleanupContract(id) if id == &selection.machine))
                    || rows.next().is_some()
                {
                    return Err(LoweringError::new(&site, "domain cleanup lacks its selected contract obligation"));
                }
            }
            NodeKind::PathType => return Err(LoweringError::new(&site_at(tree, member), "domain alias has no typed expansion")),
            kind => return Err(LoweringError::new(&site_at(tree, member), format!("unsupported domain member: {kind:?}"))),
        }
    }
    Ok(DomainSchema {
        declaration,
        carrier,
        public,
        policy,
        parameters,
        index_binders,
        definition: DomainDefinition::Named(requires),
        site,
    })
}
