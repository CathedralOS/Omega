//! One immutable scalar qualification namespace for an explicitly selected call closure.
//!
//! Source membership replay and catalog production share declaration validation.
//! Catalog membership names a theory; it never substitutes for a cast's checked
//! introduction evidence. Preparation freezes identities before any machine emits.

use crate::scalar_graph_lowering::terminal_scalar_type;
use crate::{LoweringError, unsupported};
use checked_trees::types::{
    DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use checked_trees::{CheckedScalarComputationKind, CheckedScalarDispatchPattern, CheckedTrees};
use language_semantics::SemanticDomainId;
use semantic_vocabulary::{
    DomainSemanticId, QualifiedScalarType, ScalarDomainId, ScalarQualificationSetId,
};
use symbols::SymbolHandle;
use terminal_psi::{ScalarDomainDeclaration, ScalarQualificationCatalog, ScalarQualificationSet};

pub(crate) struct PreparedScalarQualifications {
    catalog: ScalarQualificationCatalog,
}

impl PreparedScalarQualifications {
    pub(crate) fn prepare(
        checked: &CheckedTrees,
        machines: &[SymbolHandle],
    ) -> Result<Self, LoweringError> {
        let mut references = Vec::new();
        for symbol in machines {
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == *symbol)
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification closure lost a machine",
                ))?;
            for state in checked.machine_states(machine) {
                for parameter in checked.state_parameters(state) {
                    if checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                    {
                        references.push(parameter.type_reference);
                    }
                }
                if checked
                    .primitive_type_reference(state.return_type)
                    .is_some()
                {
                    references.push(state.return_type);
                }
            }
        }
        let plans = &checked.facts.values.scalar_computations;
        let mut pending = plans
            .roots
            .iter()
            .filter_map(|(_, root)| machines.contains(&root.machine).then_some(root.root))
            .collect::<Vec<_>>();
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if visited.contains(&handle) {
                continue;
            }
            if !plans.nodes.is_valid(handle) {
                return unsupported("scalar qualification closure has a stale computation");
            }
            visited.push(handle);
            match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([*left, *right])
                }
                CheckedScalarComputationKind::Qualification {
                    operand,
                    result_type,
                    ..
                } => {
                    references.push(*result_type);
                    pending.push(*operand);
                }
                CheckedScalarComputationKind::Value(_) => {}
                CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                    pending.push(*subject);
                    for arm in plans
                        .dispatch_arms
                        .span(*arms)
                        .ok_or(LoweringError::Unsupported(
                            "scalar qualification closure has stale dispatch arms",
                        ))?
                    {
                        pending.push(arm.value);
                        if let CheckedScalarDispatchPattern::Value(value) = arm.pattern {
                            pending.push(value);
                        }
                    }
                }
                CheckedScalarComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    pending.extend([*condition, *when_true, *when_false]);
                }
                CheckedScalarComputationKind::Call { arguments, .. }
                | CheckedScalarComputationKind::Apply {
                    operands: arguments,
                    ..
                } => {
                    pending.extend(
                        plans
                            .operands
                            .span(*arguments)
                            .ok_or(LoweringError::Unsupported(
                                "scalar qualification closure has stale operands",
                            ))?
                            .iter()
                            .copied(),
                    );
                }
            }
        }
        let mut catalog = ScalarQualificationCatalog::default();
        let mut atom_sets = Vec::new();
        for reference in references {
            let (primitive, atoms) = type_atoms(checked, reference)?;
            let scalar_type = terminal_scalar_type(primitive)?;
            let mut identities = Vec::new();
            for (_, semantic_id) in atoms {
                let identity = checked
                    .semantic_domains
                    .name(semantic_id)
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification lost its canonical identity",
                    ))?
                    .to_owned();
                let semantic_domain = DomainSemanticId::new(u64::from(semantic_id.0)).ok_or(
                    LoweringError::Unsupported(
                        "scalar qualification has an invalid semantic identity",
                    ),
                )?;
                if let Some(existing) = catalog
                    .domains
                    .iter()
                    .find(|domain| domain.identity == identity)
                {
                    if existing.carrier != scalar_type
                        || existing.semantic_domain != semantic_domain
                    {
                        return unsupported(
                            "scalar qualification identity has conflicting declarations",
                        );
                    }
                } else {
                    catalog.domains.push(ScalarDomainDeclaration {
                        id: ScalarDomainId::new(1).ok_or(LoweringError::Unsupported(
                            "invalid initial scalar domain identity",
                        ))?,
                        semantic_domain,
                        identity: identity.clone(),
                        carrier: scalar_type,
                    });
                }
                identities.push(identity);
            }
            identities.sort();
            identities.dedup();
            if !identities.is_empty() {
                atom_sets.push(identities);
            }
        }
        catalog
            .domains
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        for (ordinal, domain) in catalog.domains.iter_mut().enumerate() {
            domain.id = ScalarDomainId::new(
                u64::try_from(ordinal)
                    .ok()
                    .and_then(|value| value.checked_add(1))
                    .ok_or(LoweringError::Unsupported(
                        "scalar domain catalog exceeds identity capacity",
                    ))?,
            )
            .ok_or(LoweringError::Unsupported(
                "scalar domain catalog has an invalid identity",
            ))?;
        }
        atom_sets.sort();
        atom_sets.dedup();
        for (ordinal, identities) in atom_sets.into_iter().enumerate() {
            let domains = identities
                .iter()
                .map(|identity| {
                    catalog
                        .domains
                        .iter()
                        .find(|domain| &domain.identity == identity)
                        .map(|domain| domain.id)
                        .ok_or(LoweringError::Unsupported(
                            "scalar qualification set lost a domain",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let id = u64::try_from(ordinal)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification sets exceed identity capacity",
                ))?;
            catalog.sets.push(ScalarQualificationSet {
                id: ScalarQualificationSetId::new(id),
                domains,
            });
        }
        Ok(Self { catalog })
    }

    pub(crate) fn catalog(&self) -> &ScalarQualificationCatalog {
        &self.catalog
    }

    pub(crate) fn value_type(
        &self,
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<QualifiedScalarType, LoweringError> {
        let (primitive, atoms) = type_atoms(checked, reference)?;
        let scalar_type = terminal_scalar_type(primitive)?;
        let mut domains = Vec::new();
        for (_, identity) in atoms {
            let domain = self
                .catalog
                .domains
                .iter()
                .find(|domain| {
                    domain.semantic_domain.get() == u64::from(identity.0)
                        && domain.carrier == scalar_type
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification is absent from the prepared closure",
                ))?;
            domains.push(domain.id);
        }
        domains.sort();
        domains.dedup();
        let qualifications = if domains.is_empty() {
            ScalarQualificationSetId::ZERO
        } else {
            self.catalog
                .sets
                .iter()
                .find(|set| set.domains == domains)
                .map(|set| set.id)
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification set is absent from the prepared closure",
                ))?
        };
        Ok(QualifiedScalarType {
            scalar_type,
            qualifications,
        })
    }

    pub(crate) fn scalar_state_types(
        &self,
        checked: &CheckedTrees,
        symbol: SymbolHandle,
    ) -> Result<(Vec<QualifiedScalarType>, QualifiedScalarType), LoweringError> {
        let mut states = checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .filter(|state| state.symbol == symbol);
        let state = states.next().ok_or(LoweringError::Unsupported(
            "scalar signature lost its state",
        ))?;
        if states.next().is_some() {
            return unsupported("scalar signature has ambiguous state identity");
        }
        let parameters = checked
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .map(|parameter| self.value_type(checked, parameter.type_reference))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((parameters, self.value_type(checked, state.return_type)?))
    }
}

pub(crate) fn type_atoms(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<(PrimitiveType, Vec<(SymbolHandle, SemanticDomainId)>), LoweringError> {
    let primitive =
        checked
            .primitive_type_reference(reference)
            .ok_or(LoweringError::Unsupported(
                "scalar qualification has no primitive carrier",
            ))?;
    let mut current = reference;
    let mut active = Vec::new();
    let mut atoms = Vec::new();
    let mut projects_reference = false;
    loop {
        if !checked
            .type_reference_table
            .contains_type_reference(current)
            || active.contains(&current)
        {
            return unsupported("scalar qualification has a stale or cyclic type");
        }
        active.push(current);
        match checked.type_reference_table.type_reference(current) {
            // Existing scalar channels project the payload of a bare borrow;
            // loan/access custody remains in its dedicated call owner. This
            // does not admit transporting semantic membership on a reference.
            TypeReferenceNode::Reference { referee, .. } => {
                projects_reference = true;
                current = *referee;
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if checked.symbols.builtin_type_atom(*symbol).is_none() {
                    return unsupported(
                        "scalar qualification carrier is not a builtin declaration",
                    );
                }
                break;
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in checked
                    .type_reference_table
                    .constraint_span(*constraints)
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification has stale constraints",
                    ))?
                {
                    match constraint {
                        // These retain their existing arithmetic/range proof
                        // owners. This catalog transports declared semantic
                        // atoms, not a replacement store or overflow judgment.
                        TypeConstraintNode::ArithmeticDomain(_)
                        | TypeConstraintNode::Range { .. } => {}
                        TypeConstraintNode::Domain(domain)
                            if domain.subject == DomainConstraintSubject::Declared
                                && !domain.predicate_body.is_present()
                                && domain.establishment_routes.is_empty() =>
                        {
                            declared_atoms(
                                checked,
                                domain.symbol,
                                &domain.arguments,
                                domain.semantic_id,
                                primitive,
                                &mut Vec::new(),
                                &mut atoms,
                            )?;
                            let declaration = checked
                                .domain_definitions()
                                .iter()
                                .find(|declaration| declaration.symbol == domain.symbol)
                                .ok_or(LoweringError::Unsupported(
                                    "scalar qualification lost its declaration",
                                ))?;
                            let roles = language_semantics::DomainSemanticRoles {
                                denotation_dimension: declaration
                                    .semantic_roles
                                    .denotation_dimension
                                    .map(|_| domain.semantic_id),
                                arithmetic_policy: declaration
                                    .semantic_roles
                                    .arithmetic_policy
                                    .map(|_| domain.semantic_id),
                            };
                            if domain.classification != declaration.classification
                                || domain.semantic_roles != roles
                            {
                                return unsupported(
                                    "scalar qualification changed its declared semantic roles",
                                );
                            }
                        }
                        _ => {
                            return unsupported(
                                "scalar qualification requires non-vacuous or unsupported evidence",
                            );
                        }
                    }
                }
                current = *base_type;
            }
            _ => {
                return unsupported(
                    "scalar qualification cannot erase a reference or structural shell",
                );
            }
        }
    }
    if projects_reference && !atoms.is_empty() {
        return unsupported("qualified scalar references require their own transport judgment");
    }
    atoms.sort_by_key(|(_, identity)| identity.0);
    atoms.dedup();
    Ok((primitive, atoms))
}

pub(crate) fn declared_atoms(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
    arguments: &[TypeReferenceHandle],
    semantic_id: SemanticDomainId,
    primitive: PrimitiveType,
    active: &mut Vec<symbols::SymbolHandle>,
    atoms: &mut Vec<(symbols::SymbolHandle, SemanticDomainId)>,
) -> Result<(), LoweringError> {
    if !symbol.is_valid() || active.contains(&symbol) || !semantic_id.is_valid() {
        return unsupported("scalar qualification has a stale or cyclic domain definition");
    }
    let mut declarations = checked
        .domain_definitions()
        .iter()
        .filter(|domain| domain.symbol == symbol);
    let domain = declarations.next().ok_or(LoweringError::Unsupported(
        "scalar qualification has no domain definition",
    ))?;
    if declarations.next().is_some()
        || domain.predicate_body.is_present()
        || !domain.facts.is_empty()
        || !domain.establishment_routes.is_empty()
        || (!checked_trees::domain::has_generic_carrier(checked, domain)
            && checked.primitive_type_reference(domain.target_type) != Some(primitive))
    {
        return unsupported(
            "scalar qualification declaration needs non-vacuous evidence or another carrier",
        );
    }
    let parameters = checked_trees::domain::index_parameters(checked, domain);
    if parameters.len() != arguments.len()
        || arguments.iter().any(|argument| {
            !checked
                .type_reference_table
                .contains_type_reference(*argument)
        })
    {
        return unsupported("scalar qualification changed its domain index arity or identity");
    }
    let identity =
        checked_trees::domain::indexed_domain_instance_name(checked, domain, parameters, arguments)
            .map_err(|_| {
                LoweringError::Unsupported("scalar qualification has unresolved domain indices")
            })?;
    if checked.semantic_domains.name(semantic_id) != Some(identity.as_str())
        || (arguments.is_empty() && semantic_id != domain.semantic_id)
    {
        return unsupported("scalar qualification changed its canonical domain instance");
    }
    if let Some(alias) = &domain.alias {
        if alias.constituents.is_empty() || !arguments.is_empty() {
            return unsupported("scalar qualification has an empty or indexed alias expansion");
        }
        active.push(symbol);
        for constituent in &alias.constituents {
            let identity = checked
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == constituent.domain_symbol)
                .map(|domain| domain.semantic_id)
                .ok_or(LoweringError::Unsupported(
                    "scalar qualification has an unresolved alias atom",
                ))?;
            declared_atoms(
                checked,
                constituent.domain_symbol,
                &[],
                identity,
                primitive,
                active,
                atoms,
            )?;
        }
        active.pop();
    } else {
        atoms.push((symbol, semantic_id));
    }
    Ok(())
}
