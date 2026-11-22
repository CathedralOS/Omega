use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{DataProperties, Site, TypedField, TypedTree, TypedTrees, ValueType};

use crate::declarations::contracts::{ClauseOwner, lower_clause};
use crate::declarations::regions::{RegionData, Stored};
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
        asserted_multiplicity: Option<omega_typed::Multiplicity>,
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
                asserted_multiplicity,
                public,
                site,
            } => {
                let target = types
                    .id_for(carrier)
                    .map_err(|error| LoweringError::new(&site, error.to_string()))?;
                Ok(LoweredAlias {
                    declaration: crate::identity::declaration(declaration, &site)?,
                    target: AliasTarget::Structural(target),
                    asserted_multiplicity,
                    public,
                    site,
                })
            }
        }
    }
}

/// Where one field keeps the lifetime binders of its declaration: the binder
/// of a stored reference, and the binders that fill those of the record the
/// field stores or refers to through that reference.
#[derive(Default)]
pub(crate) struct FieldRegions {
    pub lifetime: Option<usize>,
    pub aggregate_lifetimes: Vec<usize>,
}

pub(crate) struct DataSchema<'a> {
    forest: &'a TypedTrees<'a>,
    regions: &'a RegionData<'a>,
    pub carrier: ValueType,
    pub declaration: &'a DeclarationId,
    pub site: Site,
    public: bool,
    properties: DataProperties,
    pub lifetime_count: usize,
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
    ) -> Result<(&'a ValueType, FieldRegions), LoweringError> {
        stored_field(self.declaration, self.lifetime_count, field, self.regions)
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
                    let (storage, FieldRegions { lifetime, aggregate_lifetimes }) = stored_field(self.declaration, self.lifetime_count, field, self.regions)?;
                    let carrier = types
                        .id_for(storage)
                        .map_err(|error| LoweringError::new(&field.site, error.to_string()))?;
                    Ok(LoweredField {
                        lifetime,
                        aggregate_lifetimes,
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
            write_only_cleanup: None,
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
    regions: &'a RegionData<'a>,
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
            NodeKind::Requires => {
                let applied;
                let owner = if lifetime_count != 0 {
                    let regions = (0..lifetime_count).map(|index| ValueType::Parameter { owner: Box::new(declaration.clone()), index }).collect();
                    applied = ValueType::Applied(Box::new(declaration.clone()), regions);
                    ClauseOwner::Subject(&applied)
                } else if value_indices.is_empty() {
                    ClauseOwner::Subject(&carrier)
                } else {
                    ClauseOwner::IndexedData(node, declaration, &value_indices)
                };
                requires.push(lower_clause(prepared, &crate::lowering::applied_tree::LoweringTree::plain(tree), owner, member)?);
            }
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
        regions,
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
    let typed = tree.forest();
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
    // Only an equality with the expansion's checked multiplicity is supported,
    // not a new ownership or carry policy on the alias.
    let asserted_multiplicity = match (others, typed.type_alias_properties.get(declaration)) {
        ([], None) => None,
        ([attr], Some(properties)) if file.nodes[*attr].kind == NodeKind::Attributes => {
            if properties.carry.is_some() {
                return Err(refuse(*attr, "transparent alias carry policy requires a checked relation to its expansion"));
            }
            match properties.multiplicity {
                omega_typed::Multiplicity::Copy | omega_typed::Multiplicity::Linear => Some(properties.multiplicity),
                _ => return Err(refuse(*attr, "data alias lacks a checked multiplicity assertion")),
            }
        }
        _ => return Err(refuse(target, "data alias or unsupported member requires declaration lowering")),
    };
    let enclosing = typed
        .enclosing_data_parameters
        .get(declaration)
        .map_or(&[][..], Vec::as_slice);
    let kind = file.nodes[target].kind;
    // A closed runtime signature names its row in the callable contract table, so
    // every use of the alias and the alias itself denote one contract.
    let runtime_signature = kind == NodeKind::MachineType && matches!(carrier, ValueType::Callable(c) if super::callables::supported(c));
    if !generic && enclosing.is_empty() && (kind == NodeKind::ArrayType || runtime_signature) {
        return Ok(PendingAlias::Structural {
            declaration,
            carrier,
            asserted_multiplicity,
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
                asserted_multiplicity,
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
        asserted_multiplicity,
        target: AliasTarget::Expansion(AliasExpansion {
            enclosing_parameters: enclosing
                .iter()
                .map(|id| crate::identity::declaration(id, &site))
                .collect::<Result<_, _>>()?,
            parameters,
            head,
            expansion: alias_expansion(prepared, carrier, &site_at(tree, target))?,
        }),
        public,
        site,
    }))
}

/// A callable expansion keeps the complete interface of the signature that
/// declares its contract, including its predicates and crash routes.
fn alias_expansion(
    prepared: &PreparedTrees<'_>,
    carrier: &ValueType,
    site: &Site,
) -> Result<crate::DeclarationTerm, LoweringError> {
    let ValueType::Callable(callable) = carrier else {
        return super::type_expression::declaration_term(carrier, site);
    };
    let (origin, node) = prepared
        .typed()
        .callable_origin(&callable.contract)
        .ok_or_else(|| LoweringError::new(site, "callable alias lacks its checked structural signature"))?;
    let interface = super::type_expression::callable_interface(prepared, &origin, node, callable)?;
    Ok(crate::DeclarationTerm::Type(crate::TypeExpression(vec![crate::TypeExpressionStep::Callable(Box::new(interface))])))
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

/// The type a field's row stores and the lifetime binders of `declaration`
/// it keeps. The row erases a lifetime argument, so a field that names a
/// binder anywhere the schema cannot record it has no concrete row.
pub(super) fn stored_field<'a>(
    declaration: &DeclarationId,
    lifetime_count: usize,
    field: &'a TypedField,
    regions: &RegionData<'_>,
) -> Result<(&'a ValueType, FieldRegions), LoweringError> {
    match super::regions::stored(&field.ty, declaration, lifetime_count, regions) {
        Some(Stored::Plain) => Ok((&field.ty, FieldRegions::default())),
        Some(Stored::Reference { storage, binder, referent }) => Ok((storage, FieldRegions { lifetime: Some(binder), aggregate_lifetimes: referent })),
        Some(Stored::Record(aggregate_lifetimes)) => Ok((&field.ty, FieldRegions { lifetime: None, aggregate_lifetimes })),
        None if matches!(field.ty, ValueType::AtLifetime(..)) => {
            Err(LoweringError::new(&field.site, "stored reference lifetime requires its owning region binder"))
        }
        None => Err(LoweringError::new(&field.site, "stored lifetime requires a reference or a record this package lowers at its binders")),
    }
}
