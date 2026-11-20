use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{DataProperties, Site, TypedField, TypedTree, TypedTrees, ValueType};

use crate::declarations::contracts::{ClauseOwner, lower_clause};
use crate::{ContractClause, LoweringError, LoweringTypes, site_at};

use crate::{AliasExpansion, AliasTarget, DataId, LoweredAlias, LoweredCase, LoweredData, LoweredField};
pub(crate) enum DataDeclaration<'a> {
    Schema(DataSchema<'a>),
    Alias(PendingAlias<'a>),
}

pub(crate) enum PendingAlias<'a> {
    /// A named edge or checked expansion needing no package type row.
    Lowered(LoweredAlias),
    Structural {
        declaration: &'a DeclarationId,
        carrier: &'a ValueType,
        public: bool,
        site: Site,
    },
}

impl PendingAlias<'_> {
    pub(crate) fn lower(
        self,
        types: &LoweringTypes,
    ) -> Result<LoweredAlias, LoweringError> {
        match self {
            Self::Lowered(alias) => Ok(alias),
            Self::Structural {
                declaration,
                carrier,
                public,
                site,
            } => {
                let target = types
                    .id_for(carrier)
                    .map_err(|error| LoweringError::new(&site, error.to_string()))?;
                Ok(LoweredAlias {
                    declaration: crate::identity::declaration(declaration, &site)?,
                    target: AliasTarget::Structural(target),
                    public,
                    site,
                })
            }
        }
    }
}

pub(crate) struct DataSchema<'a> {
    forest: &'a TypedTrees<'a>,
    pub carrier: ValueType,
    pub declaration: &'a DeclarationId,
    pub site: Site,
    public: bool,
    properties: DataProperties,
    lifetime_count: usize,
    pub value_indices: Vec<&'a ValueType>,
    requires: Vec<ContractClause>,
    fields: &'a [TypedField],
    cases: Vec<(&'a DeclarationId, Site, &'a [TypedField])>,
}

impl<'a> DataSchema<'a> {
    pub(crate) fn fields(&self) -> impl Iterator<Item = &'a TypedField> {
        self.fields
            .iter()
            .chain(self.cases.iter().flat_map(|(_, _, fields)| *fields))
    }

    pub(crate) fn field_carrier(
        &self,
        field: &'a TypedField,
    ) -> Result<(&'a ValueType, Option<usize>), LoweringError> {
        stored_field(self.declaration, self.lifetime_count, field)
    }

    pub(crate) fn lower(
        self,
        id: DataId,
        types: &LoweringTypes,
    ) -> Result<LoweredData, LoweringError> {
        let typed=self.forest;
        let lower_fields = |fields: &'a [TypedField]| {
            fields
                .iter()
                .map(|field| {
                    let (storage, lifetime) = stored_field(self.declaration, self.lifetime_count, field)?;
                    let carrier = types
                        .id_for(storage)
                        .map_err(|error| LoweringError::new(&field.site, error.to_string()))?;
                    Ok(LoweredField {
                        lifetime,
                        declaration: crate::identity::declaration(&field.declaration, &field.site)?,
                        carrier,
                        site: field.site.clone(),
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        };
        let carrier = types
            .id_for(&self.carrier)
            .map_err(|error| LoweringError::new(&self.site, error.to_string()))?;
        Ok(LoweredData {
            id,
            type_arguments: Vec::new(),
            cleanup_target: None,
            declaration: crate::identity::declaration(self.declaration, &self.site)?,
            carrier,
            public: self.public,
            numbering: typed.schema_numbering.get(self.declaration).cloned(),
            multiplicity: self.properties.multiplicity,
            lifetime_count: self.lifetime_count,
            value_indices: self
                .value_indices
                .iter()
                .map(|ty| {
                    types
                        .id_for(ty)
                        .map_err(|e| LoweringError::new(&self.site, e.to_string()))
                })
                .collect::<Result<_, _>>()?,
            carry: self.properties.carry,
            requires: self.requires,
            fields: lower_fields(self.fields)?,
            cases: self
                .cases
                .into_iter()
                .map(|(declaration, site, fields)| {
                    Ok(LoweredCase {
                        numbering: typed.schema_numbering.get(declaration).cloned(),
                        declaration: crate::identity::declaration(declaration, &site)?,
                        fields: lower_fields(fields)?,
                        site,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            site: self.site,
        })
    }
}

pub(crate) fn read_data<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a TypedTree<'_>,
    node: usize,
    nested: &mut Vec<usize>,
) -> Result<DataDeclaration<'a>, LoweringError> {
    let typed = tree.forest();
    let resolved = tree.resolved();
    let file = resolved.file;
    let refuse = |at, message| LoweringError::new(&site_at(tree, at), message);
    let declaration = resolved
        .declaration_at(node)
        .ok_or_else(|| refuse(node, "data has no resolved identity"))?;
    let public = resolved.exports().any(|export| &export.id == declaration);
    if let Some(carrier) = typed.type_alias_targets.get(declaration) {
        return read_alias(prepared, tree, node, declaration, carrier, public).map(DataDeclaration::Alias);
    }
    let carrier = ValueType::Nominal(Box::new(declaration.clone()));
    let mut requires = Vec::new();
    let mut field_nodes = Vec::new();
    let mut cases = Vec::new();
    let mut has_attributes = false;
    let mut lifetime_count = 0;
    let mut value_indices = Vec::new();
    for &member in &file.nodes[node].children {
        match file.nodes[member].kind {
            NodeKind::Attributes => has_attributes = true,
            NodeKind::Field => {
                field_members(tree, member, nested)?;
                field_nodes.push(member);
            }
            NodeKind::Case => {
                let case = resolved
                    .declaration_at(member)
                    .ok_or_else(|| refuse(member, "case has no resolved identity"))?;
                let mut payload = Vec::new();
                for &part in &file.nodes[member].children {
                    match file.nodes[part].kind {
                        NodeKind::CaseField => {
                            field_members(tree, part, nested)?;
                            payload.push(part);
                        }
                        NodeKind::StableNumber | NodeKind::RetiredNumber => {}
                        _ => return Err(refuse(part, "case storage attributes require data layout lowering")),
                    }
                }
                let fields = checked_fields(typed, tree, member, case, &payload)?;
                cases.push((case, site_at(tree, member), fields));
            }
            NodeKind::RetiredNumber => {}
            NodeKind::Data => nested.push(member),
            NodeKind::GenericParams => {
                let unsupported = || refuse(member, "generic data requires a closed application before lowering");
                let binders = typed
                    .generic_parameters
                    .get(declaration)
                    .filter(|g| g.witness_count == 0 && g.equations.is_empty())
                    .ok_or_else(unsupported)?;
                if binders
                    .parameters
                    .iter()
                    .all(|p| p.kind == NodeKind::LifetimeParam)
                {
                    lifetime_count = binders.parameters.len();
                } else {
                    for parameter in &binders.parameters {
                        let ty = parameter
                            .carrier
                            .as_deref()
                            .filter(|ty| {
                                parameter.kind == NodeKind::KindedParam
                                    && parameter.multiplicity.is_none()
                                    && matches!(ty, ValueType::Integer { width: Some(1..), .. } | ValueType::Bool)
                            })
                            .ok_or_else(unsupported)?;
                        value_indices.push(ty);
                    }
                }
            }
            NodeKind::Requires => requires.push(lower_clause(
                prepared,
                &crate::lowering::applied_tree::LoweringTree::plain(tree),
                if value_indices.is_empty() {
                    ClauseOwner::Subject(&carrier)
                } else {
                    ClauseOwner::IndexedData(node, declaration, &value_indices)
                },
                member,
            )?),
            NodeKind::DroppedBy => {
                let site=site_at(tree,member);
                let selection=typed.cleanups.iter().find(|s|s.site==site && s.owner==*declaration).ok_or_else(||refuse(member,"data cleanup has no selected target"))?;
                let mut rows=tree.semantics_at(member);
                if !matches!(rows.next(),Some(omega_typed::NodeSemantics::Obligation(o)) if matches!(&o.kind,omega_typed::ObligationKind::CleanupContract(id) if id==&selection.machine)) || rows.next().is_some(){
                    return Err(refuse(member,"data cleanup lacks its contract obligation"));
                }
            },
            _ => return Err(refuse(member, "data alias or unsupported member requires declaration lowering")),
        }
    }
    let properties = typed
        .data_properties
        .iter()
        .find(|(id, _)| id == declaration)
        .map(|(_, properties)| properties.clone());
    if has_attributes && properties.is_none() {
        return Err(refuse(node, "data has no typed multiplicity and carry attributes"));
    }
    let fields = checked_fields(typed, tree, node, declaration, &field_nodes)?;
    if !fields.is_empty() && !cases.is_empty() {
        return Err(refuse(node, "typed data mixes record fields and sum cases"));
    }
    Ok(DataDeclaration::Schema(DataSchema {
        forest:typed,
        carrier,
        declaration,
        site: site_at(tree, node),
        public,
        properties: properties.unwrap_or_default(),
        lifetime_count,
        value_indices,
        requires,
        fields,
        cases,
    }))
}

/// Keep one edge per closed bare alias so diagnostics and replay can recover the
/// original chain even though typed uses have the expanded carrier. Generic
/// aliases and applied targets hand off their exact binders and normalized
/// expansion instead; no layout or nominal identity is synthesized for them.
fn read_alias<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a TypedTree<'_>,
    node: usize,
    declaration: &'a DeclarationId,
    carrier: &'a ValueType,
    public: bool,
) -> Result<PendingAlias<'a>, LoweringError> {
    let typed = prepared.typed();
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let refuse = |at, message: &str| LoweringError::new(&site_at(tree, at), message);
    let mut generic = false;
    let mut rest = Vec::new();
    for &child in &file.nodes[node].children {
        if file.nodes[child].kind == NodeKind::GenericParams && !generic && rest.is_empty() {
            generic = true;
        } else {
            rest.push(child);
        }
    }
    let Some((&target, others)) = rest.split_last() else {
        return Err(refuse(node, "data alias has no target type"));
    };
    // Attributes or other members would give the alias an identity of its own.
    if !others.is_empty() {
        return Err(refuse(target, "data alias or unsupported member requires declaration lowering"));
    }
    let enclosing = typed
        .enclosing_data_parameters
        .get(declaration)
        .map_or(&[][..], Vec::as_slice);
    let kind = file.nodes[target].kind;
    if !generic && enclosing.is_empty() && kind == NodeKind::ArrayType {
        return Ok(PendingAlias::Structural {
            declaration,
            carrier,
            public,
            site,
        });
    }
    let head = match resolved.symbol(target) {
        Some(Symbol::Declaration(id)) if id.kind == NodeKind::Data => Some(crate::identity::declaration(id, &site_at(tree, target))?),
        _ => None,
    };
    if !generic && kind == NodeKind::PathType && file.nodes[target].children.is_empty() {
        let named = match (&head, resolved.symbol(target)) {
            (Some(id), _) => Some(AliasTarget::Declaration(id.clone())),
            (None, Some(Symbol::Builtin(name))) => Some(AliasTarget::Builtin(name.clone())),
            _ => None,
        };
        if let Some(target) = named {
            return Ok(PendingAlias::Lowered(LoweredAlias {
                declaration: crate::identity::declaration(declaration, &site)?,
                target,
                public,
                site,
            }));
        }
    }
    let binders = typed.generic_parameters.get(declaration);
    if binders.is_some_and(|b| b.witness_count != 0 || !b.equations.is_empty()) {
        return Err(refuse(node, "data alias admits only ordinary generic binders"));
    }
    let (parameters, _) = super::type_expression::template_binders(binders, &site)?;
    if generic == parameters.is_empty() || file.generic_parameters(node).count() != parameters.len() {
        return Err(refuse(node, "data alias has incomplete checked binders"));
    }
    Ok(PendingAlias::Lowered(LoweredAlias {
        declaration: crate::identity::declaration(declaration, &site)?,
        target: AliasTarget::Expansion(AliasExpansion {
            enclosing_parameters: enclosing
                .iter()
                .map(|id| crate::identity::declaration(id, &site))
                .collect::<Result<_, _>>()?,
            parameters,
            head,
            expansion: super::type_expression::declaration_term(carrier, &site_at(tree, target))?,
        }),
        public,
        site,
    }))
}

pub(super) fn field_members(
    tree: &TypedTree<'_>,
    field: usize,
    nested: &mut Vec<usize>,
) -> Result<(), LoweringError> {
    let file = tree.resolved().file;
    for &part in &file.nodes[field].children {
        match file.nodes[part].kind {
            NodeKind::Data => nested.push(part),
            NodeKind::Attributes => {
                return Err(LoweringError::new(
                    &site_at(tree, part),
                    "field storage attributes require data layout lowering",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn checked_fields<'a>(
    typed: &'a TypedTrees<'_>,
    tree: &TypedTree<'_>,
    node: usize,
    declaration: &DeclarationId,
    field_nodes: &[usize],
) -> Result<&'a [TypedField], LoweringError> {
    let fields = typed
        .record_fields
        .get(declaration)
        .ok_or_else(|| LoweringError::new(&site_at(tree, node), "data has no typed field roster"))?;
    if fields.len() != field_nodes.len()
        || fields
            .iter()
            .zip(field_nodes)
            .any(|(field, &field_node)| tree.resolved().declaration_at(field_node) != Some(&field.declaration))
    {
        return Err(LoweringError::new(&site_at(tree, node), "typed field roster disagrees with the declaration"));
    }
    Ok(fields)
}

fn stored_field<'a>(
    declaration: &DeclarationId,
    lifetime_count: usize,
    field: &'a TypedField,
) -> Result<(&'a ValueType, Option<usize>), LoweringError> {
    match &field.ty {
        ValueType::AtLifetime(reference, region) => match region.as_ref() {
            ValueType::Parameter { owner, index }
                if owner.as_ref() == declaration && *index < lifetime_count && matches!(reference.as_ref(), ValueType::Reference(..)) =>
            {
                Ok((reference, Some(*index)))
            }
            _ => Err(LoweringError::new(&field.site, "stored reference lifetime requires its owning region binder")),
        },
        ty => Ok((ty, None)),
    }
}
