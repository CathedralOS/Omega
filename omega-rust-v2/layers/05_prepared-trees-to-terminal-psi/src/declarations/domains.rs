use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{Site, TypedTree, ValueType};

use crate::declarations::contracts::{ClauseOwner, lower_clause};
use crate::{ContractClause, LoweringError, LoweringTypes, site_at};

use crate::{LoweredDomain, LoweredDomainAlias};
pub(crate) enum DomainDeclaration {
    Named(LoweredDomain),
    Alias(LoweredDomainAlias),
}

enum DomainDefinition {
    Named(Vec<ContractClause>),
    Alias(Vec<DeclarationId>),
}

pub(crate) struct DomainSchema<'a> {
    pub declaration: &'a DeclarationId,
    pub carrier: &'a ValueType,
    pub site: Site,
    public: bool,
    parameters: Vec<crate::TemplateBinder>,
    definition: DomainDefinition,
}

impl DomainSchema<'_> {
    pub(crate) fn predicates(&self) -> Option<&[ContractClause]> {
        match &self.definition {
            DomainDefinition::Named(clauses) if self.parameters.is_empty() => Some(clauses),
            DomainDefinition::Named(_) => None,
            DomainDefinition::Alias(_) => None,
        }
    }

    pub(crate) fn lower(
        self,
        types: &LoweringTypes,
    ) -> Result<DomainDeclaration, LoweringError> {
        let carrier = types
            .id_for(self.carrier)
            .map_err(|error| LoweringError::new(&self.site, error.to_string()))?;
        let declaration = crate::identity::declaration(self.declaration, &self.site)?;
        let public = self.public;
        let site = self.site;
        Ok(match self.definition {
            DomainDefinition::Named(requires) => DomainDeclaration::Named(LoweredDomain {
                parameters: self.parameters,
                declaration,
                carrier,
                public,
                requires,
                site,
            }),
            DomainDefinition::Alias(members) => DomainDeclaration::Alias(LoweredDomainAlias {
                declaration,
                carrier,
                public,
                members: members
                    .iter()
                    .map(|id| crate::identity::declaration(id, &site))
                    .collect::<Result<_, _>>()?,
                site,
            }),
        })
    }
}

pub(crate) fn read_domain<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a TypedTree<'_>,
    node: usize,
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
    let head = file
        .child(node, NodeKind::PathType)
        .ok_or_else(|| LoweringError::new(&site, "domain has no typed head"))?;
    if matches!(resolved.symbol(head), Some(Symbol::Declaration(parent)) if parent.kind == NodeKind::Domain) || !carrier.domains().is_empty() {
        return Err(LoweringError::new(&site_at(tree, head), "inherited domain requires its parent qualification"));
    }
    if crate::lowering::types::scalar_carrier(carrier).is_err() {
        return Err(LoweringError::new(&site_at(tree, head), "domain carrier requires a fixed scalar"));
    }
    let public = resolved.exports().any(|export| &export.id == declaration);
    if let Some(members) = prepared.typed().domain_alias_members.get(declaration) {
        if !parameters.is_empty() || members.iter().any(|member| !member.arguments.is_empty()) {
            return Err(LoweringError::new(&site, "domain alias requires indexed qualification lowering"));
        }
        return Ok(DomainSchema {
            declaration,
            carrier,
            site,
            public,
            parameters,
            definition: DomainDefinition::Alias(
                members
                    .iter()
                    .map(|member| member.declaration.clone())
                    .collect(),
            ),
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
            NodeKind::PathType => return Err(LoweringError::new(&site_at(tree, member), "domain alias has no typed expansion")),
            kind => return Err(LoweringError::new(&site_at(tree, member), format!("unsupported domain member: {kind:?}"))),
        }
    }
    Ok(DomainSchema {
        declaration,
        carrier,
        public,
        parameters,
        definition: DomainDefinition::Named(requires),
        site,
    })
}
