//! One immutable scalar qualification namespace for an explicitly selected call closure.
//!
//! Source checking and catalog production use exact declaration applications.
//! Catalog membership names a theory; it never substitutes for a cast's checked
//! introduction evidence. Preparation freezes identities before any machine emits.

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::types::{
    DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use checked_trees::{CheckedScalarComputationKind, CheckedTrees};
use language_semantics::{DomainEstablishmentRoute, SemanticDomainId};
use semantic_vocabulary::{
    DomainSemanticId, QualifiedScalarType, ScalarDomainId, ScalarQualificationSetId,
};
use symbols::SymbolHandle;
use terminal_psi::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarQualificationCatalog,
    ScalarQualificationSet,
};

mod fields;
mod parameters;
pub(crate) use fields::FieldType;
mod returns;
pub(crate) use returns::SourceScalarReturn;
pub(crate) use returns::retain_returns;

#[derive(Clone)]
pub(crate) struct PreparedScalarQualifications {
    catalog: ScalarQualificationCatalog,
    parameters: Vec<parameters::ParameterQualification>,
}

impl PreparedScalarQualifications {
    /// `signatures` are the machines whose scalar parameters and results
    /// cross a call through this namespace (the scalar callees); `bodies`
    /// are every machine whose retained computations resolve qualified
    /// values against it (the scalar callees and the closure's Unit bodies).
    /// A Unit body's scalar signature joins this namespace for vacuous tags
    /// and routed theories with a complete predicate projection. Route-free
    /// predicates keep their existing numeric-contract representation.
    /// Boundary requirements join this same namespace before declarations emit;
    /// a bare formal with explicit membership names the same theory as a
    /// qualified formal, without manufacturing a constrained source type.
    pub(crate) fn prepare(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        signatures: &[SymbolHandle],
        bodies: &[SymbolHandle],
        boundaries: &[&checked_trees::CheckedBoundaryMachinePlan],
        fields: &[FieldType],
    ) -> Result<Self, LoweringError> {
        let mut references = fields
            .iter()
            .map(|(_, reference)| *reference)
            .collect::<Vec<_>>();
        for symbol in signatures {
            let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
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
        for symbol in bodies.iter().filter(|symbol| !signatures.contains(symbol)) {
            let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
                "scalar qualification closure lost a machine",
            ))?;
            for state in checked.machine_states(machine) {
                references.extend(
                    checked
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.type_reference)
                        .chain([state.return_type])
                        .filter(|reference| carries_scalar_qualifications(checked, *reference)),
                );
            }
        }
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans
            .roots
            .iter()
            .filter_map(|(_, root)| bodies.contains(&root.machine).then_some(root.root))
            .collect::<Vec<_>>();
        // Structural operands can themselves contain qualified computations.
        // Consume the same retained dependency graph as operand preparation.
        for handle in super::computation_graph::reachable_nodes(checked, &roots)? {
            match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::Qualification { result_type, .. }
                | CheckedScalarComputationKind::StructuralField { result_type, .. }
                | CheckedScalarComputationKind::AuthoredCast { result_type, .. } => {
                    references.push(*result_type)
                }
                _ => {}
            }
        }
        for boundary in boundaries {
            let signature =
                crate::declarations::target_signature(checked, boundary.machine, boundary.machine)?;
            if carries_scalar_qualifications(checked, signature.return_type) {
                references.push(signature.return_type);
            }
        }
        let mut catalog = ScalarQualificationCatalog::default();
        let mut atom_sets = Vec::new();
        for reference in references {
            let (primitive, atoms) = type_atoms(checked, reference)?;
            let scalar_type = terminal_scalar_type(primitive)?;
            let mut atom_identities = Vec::new();
            for (symbol, semantic_id) in atoms {
                let predicate =
                    domain_predicate(identities, checked, reference, symbol, semantic_id)?;
                atom_identities.push(retain_domain(
                    &mut catalog,
                    identities,
                    checked,
                    symbol,
                    semantic_id,
                    scalar_type,
                    predicate,
                )?);
            }
            atom_identities.sort();
            atom_identities.dedup();
            if !atom_identities.is_empty() {
                atom_sets.push(atom_identities);
            }
        }
        for boundary in boundaries {
            let mut parameter_sets = vec![Vec::new(); boundary.scalar_parameters.len()];
            for requirement in &boundary.scalar_domain_requirements {
                let scalar_type = terminal_scalar_type(requirement.primitive_type)?;
                let routed = validation::field_domains::domain_requires_provenance(
                    &checked.typed,
                    requirement.domain_symbol,
                );
                let predicate = if routed {
                    Some(retained_domain_predicate(
                        identities,
                        &requirement.predicate,
                        scalar_type,
                    )?)
                } else {
                    None
                };
                let identity = retain_domain(
                    &mut catalog,
                    identities,
                    checked,
                    requirement.domain_symbol,
                    requirement.domain,
                    scalar_type,
                    predicate,
                )?;
                parameter_sets
                    .get_mut(requirement.argument_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "boundary membership has no scalar formal",
                    ))?
                    .push(identity);
            }
            for mut atoms in parameter_sets {
                atoms.sort();
                atoms.dedup();
                if !atoms.is_empty() {
                    atom_sets.push(atoms);
                }
            }
        }
        let parameter_atoms =
            parameters::prepare(identities, checked, bodies, &mut catalog, &mut atom_sets)?;
        catalog.domains.sort_by(|left, right| {
            identities
                .text(left.identity)
                .unwrap_or_default()
                .cmp(&identities.text(right.identity).unwrap_or_default())
        });
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
        for (ordinal, atom_identities) in atom_sets.into_iter().enumerate() {
            let domains = atom_identities
                .iter()
                .map(|identity| {
                    catalog
                        .domains
                        .iter()
                        .find(|domain| domain.identity == identities.intern(identity))
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
        let parameters = parameters::resolve(&catalog, parameter_atoms)?;
        let mut prepared = Self {
            catalog,
            parameters,
        };
        prepared.retain_fields(checked, fields)?;
        Ok(prepared)
    }

    /// An empty namespace for emission contexts that own no prepared scalar
    /// callees — boundary returns and dynamic leaves. Qualified lookups
    /// decline against it exactly as they do against a prepared catalog
    /// missing the domain.
    pub(crate) fn empty() -> Self {
        Self {
            catalog: ScalarQualificationCatalog::default(),
            parameters: Vec::new(),
        }
    }

    pub(crate) fn catalog(&self) -> &ScalarQualificationCatalog {
        &self.catalog
    }

    /// The declared type of a Unit body's scalar parameter or result. Unit
    /// planning retains vacuous tags and routed atoms with complete predicate
    /// components. Qualification keeps identity while the numeric contract
    /// independently carries predicates; bare carriers add no nominal evidence.
    pub(crate) fn signature_value_type(
        &self,
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<QualifiedScalarType, LoweringError> {
        signature_value_type(checked, &self.catalog, reference)
    }

    pub(crate) fn value_type(
        &self,
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<QualifiedScalarType, LoweringError> {
        scalar_value_type(checked, &self.catalog, reference)
    }

    pub(crate) fn boundary_domain_id(
        &self,
        requirement: &checked_trees::CheckedBoundaryScalarDomainRequirementPlan,
    ) -> Result<ScalarDomainId, LoweringError> {
        let carrier = terminal_scalar_type(requirement.primitive_type)?;
        self.catalog
            .domains
            .iter()
            .find(|domain| {
                domain.semantic_domain.get() == u64::from(requirement.domain.0)
                    && domain.carrier == carrier
            })
            .map(|domain| domain.id)
            .ok_or(LoweringError::Unsupported(
                "boundary scalar membership lost its prepared declaration",
            ))
    }

    pub(crate) fn boundary_signature_types(
        &self,
        boundary: &checked_trees::CheckedBoundaryMachinePlan,
    ) -> Result<Vec<QualifiedScalarType>, LoweringError> {
        boundary
            .scalar_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let mut domains = boundary
                    .scalar_domain_requirements
                    .iter()
                    .filter(|row| row.argument_index as usize == position)
                    .map(|row| self.boundary_domain_id(row))
                    .collect::<Result<Vec<_>, _>>()?;
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
                            "boundary scalar signature lost its prepared qualification set",
                        ))?
                };
                Ok(QualifiedScalarType {
                    scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                    qualifications,
                })
            })
            .collect()
    }

    pub(crate) fn scalar_state_types(
        &self,
        checked: &CheckedTrees,
        symbol: SymbolHandle,
    ) -> Result<(Vec<QualifiedScalarType>, QualifiedScalarType), LoweringError> {
        let mut states = checked.machine_states_named(symbol).map(|(_, state)| state);
        let state = states.next().ok_or(LoweringError::Unsupported(
            "scalar signature lost its state",
        ))?;
        if states.next().is_some() {
            return unsupported("scalar signature has ambiguous state identity");
        }
        let parameters = checked
            .state_parameters(state)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                // An `[erased]` binding owns no scalar entry; the typed
                // relevance decides that here, independently of the plan.
                !parameter.relevance.is_erased()
                    && checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
            })
            .map(|(position, _)| {
                self.parameter_value_type(
                    state.symbol,
                    u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("scalar formal position exceeds u32")
                    })?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((parameters, self.value_type(checked, state.return_type)?))
    }
}

/// Catalog registration is shared by executable signatures and boundary
/// admission rows. They name one exact theory, never equivalent predicates.
fn retain_domain(
    catalog: &mut ScalarQualificationCatalog,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    symbol: SymbolHandle,
    semantic_id: SemanticDomainId,
    scalar_type: semantic_vocabulary::ScalarType,
    predicate: Option<terminal_psi::ScalarDomainPredicate>,
) -> Result<String, LoweringError> {
    let identity = checked
        .semantic_domains
        .name(semantic_id)
        .ok_or(LoweringError::Unsupported(
            "scalar qualification lost its canonical identity",
        ))?
        .to_owned();
    let semantic_domain = DomainSemanticId::new(u64::from(semantic_id.0)).ok_or(
        LoweringError::Unsupported("scalar qualification has an invalid semantic identity"),
    )?;
    let establishment_routes = scalar_domain_establishment_routes(identities, checked, symbol)?;
    let retained_identity = identities.intern(&identity);
    if let Some(existing) = catalog
        .domains
        .iter()
        .find(|domain| domain.identity == retained_identity)
    {
        if existing.carrier != scalar_type
            || existing.semantic_domain != semantic_domain
            || existing.establishment_routes != establishment_routes
            || existing.predicate != predicate
        {
            return unsupported("scalar qualification identity has conflicting declarations");
        }
    } else {
        catalog.domains.push(ScalarDomainDeclaration {
            predicate,
            id: ScalarDomainId::new(1).ok_or(LoweringError::Unsupported(
                "invalid initial scalar domain identity",
            ))?,
            semantic_domain,
            identity: retained_identity,
            carrier: scalar_type,
            establishment_routes,
        });
    }
    Ok(identity)
}

fn retained_domain_predicate(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    predicate: &checked_trees::CheckedBooleanExpression,
    carrier: semantic_vocabulary::ScalarType,
) -> Result<terminal_psi::ScalarDomainPredicate, LoweringError> {
    let subject = crate::terminal_identities::value_id(1);
    let proposition = crate::scalar_lowering::contracts::proposition(
        identities,
        predicate,
        &[terminal_psi::ValueDeclaration {
            id: subject,
            scalar_type: carrier,
            qualifications: ScalarQualificationSetId::ZERO,
        }],
        &[],
        &crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY,
    )?;
    Ok(terminal_psi::ScalarDomainPredicate {
        subject,
        proposition,
    })
}

/// The canonical issuer routes one declared domain carries into the
/// artifact, resolved to the requirement and machine identities the
/// checked-side evidence vocabulary produces and the module's own rows name.
fn scalar_domain_establishment_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    symbol: SymbolHandle,
) -> Result<Vec<ScalarDomainEstablishmentRoute>, LoweringError> {
    let domain = checked
        .domain_definition(symbol)
        .ok_or(LoweringError::Unsupported(
            "scalar qualification lost its domain declaration",
        ))?;
    let mut routes = Vec::new();
    for route in &domain.establishment_routes {
        routes.push(establishment_route(identities, checked, route)?);
    }
    routes.sort();
    routes.dedup();
    Ok(routes)
}

/// Ordinary and opaque call signatures share one scalar result namespace.
/// Numeric predicates remain independent contract propositions; nominal routed
/// memberships survive the call without changing its runtime representation.
pub(crate) fn signature_value_type(
    checked: &CheckedTrees,
    catalog: &ScalarQualificationCatalog,
    reference: TypeReferenceHandle,
) -> Result<QualifiedScalarType, LoweringError> {
    if carries_scalar_qualifications(checked, reference) {
        return scalar_value_type(checked, catalog, reference);
    }
    let primitive =
        checked
            .primitive_type_reference(reference)
            .ok_or(LoweringError::Unsupported(
                "scalar signature slot is not a primitive scalar",
            ))?;
    Ok(terminal_scalar_type(primitive)?.into())
}

fn scalar_value_type(
    checked: &CheckedTrees,
    catalog: &ScalarQualificationCatalog,
    reference: TypeReferenceHandle,
) -> Result<QualifiedScalarType, LoweringError> {
    let (primitive, atoms) = type_atoms(checked, reference)?;
    let scalar_type = terminal_scalar_type(primitive)?;
    let mut domains = Vec::new();
    for (_, identity) in atoms {
        let domain = catalog
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
        catalog
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

fn establishment_route(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    route: &DomainEstablishmentRoute,
) -> Result<ScalarDomainEstablishmentRoute, LoweringError> {
    Ok(match *route {
        DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => ScalarDomainEstablishmentRoute::CheckedRequirement {
            requirement_identity: identities.intern(
                &crate::proofs::evidence_lowering::checked_evidence_requirement_identity(
                    checked,
                    trait_definition,
                    requirement,
                )?,
            ),
        },
        DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => ScalarDomainEstablishmentRoute::BoundaryRequirement {
            requirement_identity: identities.intern(
                &crate::proofs::evidence_lowering::checked_evidence_requirement_identity(
                    checked,
                    boundary_trait,
                    requirement,
                )?,
            ),
        },
        DomainEstablishmentRoute::ExactMachine { machine } => {
            ScalarDomainEstablishmentRoute::ExactMachine {
                machine_identity: identities.intern(
                    &crate::proofs::evidence_lowering::checked_evidence_machine_identity(
                        checked, machine,
                    )?,
                ),
            }
        }
    })
}

/// Whether a primitive scalar slot carries a scalar tag its Terminal
/// signature must declare (`validation::scalar_type_tags`).
fn carries_scalar_qualifications(checked: &CheckedTrees, reference: TypeReferenceHandle) -> bool {
    checked.primitive_type_reference(reference).is_some()
        && (!validation::scalar_type_tags(&checked.typed, reference).is_empty()
            || !validation::scalar_type_routed_domains(&checked.typed, reference).is_empty())
}

fn domain_predicate(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
    symbol: SymbolHandle,
    semantic_id: SemanticDomainId,
) -> Result<Option<terminal_psi::ScalarDomainPredicate>, LoweringError> {
    let primitive_type =
        checked
            .primitive_type_reference(reference)
            .ok_or(LoweringError::Unsupported(
                "scalar domain predicate lost its carrier",
            ))?;
    let mut active = Vec::new();
    while !active.contains(&reference) {
        active.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = checked.type_reference_table.type_reference(reference)
        else {
            break;
        };
        for constraint in checked.type_reference_table.constraints(*constraints) {
            let TypeConstraintNode::Domain(domain) = constraint else {
                continue;
            };
            if domain.symbol != symbol
                || domain.semantic_id != semantic_id
                || domain.establishment_routes.is_empty()
            {
                continue;
            }
            let predicate = validation::scalar_domain_value_predicate(
                &checked.typed,
                domain,
                checked_trees::CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                },
            )
            .ok_or(LoweringError::Unsupported(
                "routed scalar domain has no complete predicate",
            ))?;
            let subject = crate::terminal_identities::value_id(1);
            let proposition = crate::scalar_lowering::contracts::proposition(
                identities,
                &predicate,
                &[terminal_psi::ValueDeclaration {
                    id: subject,
                    scalar_type: terminal_scalar_type(primitive_type)?,
                    qualifications: ScalarQualificationSetId::ZERO,
                }],
                &[],
                &crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY,
            )?;
            return Ok(Some(terminal_psi::ScalarDomainPredicate {
                subject,
                proposition,
            }));
        }
        reference = *base_type;
    }
    Ok(None)
}

pub(crate) fn type_atoms(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<(PrimitiveType, Vec<(SymbolHandle, SemanticDomainId)>), LoweringError> {
    let primitive = checked
        .primitive_type_reference(reference)
        .or_else(|| {
            // A retained scalar field read can name its borrowed source.
            // Classify exactly that observation's referee, while the full
            // traversal below still checks the original reference type and
            // refuses transporting semantic atoms through it. Permission to
            // observe remains owned by the checked storage-read operation.
            validation::reference_shell(&checked.typed, reference)
                .and_then(|shell| checked.primitive_type_reference(shell.referee))
        })
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
            // A selector is a bare scalar carrier; its trait application
            // names no semantic atom.
            TypeReferenceNode::DynamicTrait { .. } => break,
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
                            if !domain.establishment_routes.is_empty()
                                && checked
                                    .facts
                                    .values
                                    .scalar_type_refinement_predicates(current)
                                    .is_some() =>
                        {
                            // Keep the nominal atom AND its independent predicate.
                            // This is not a vacuous tag or a membership proof.
                            atoms.push((domain.symbol, domain.semantic_id));
                        }
                        // Value-domain obligations travel as scalar predicates
                        // at entry, return and arrival boundaries. They are not
                        // predicate-free semantic atoms in this catalog.
                        TypeConstraintNode::Domain(domain)
                            if matches!(domain.subject, DomainConstraintSubject::Value(value_domain)
                                if validation::scalar_value_domain_predicate(value_domain,
                                    checked_trees::CheckedScalarExpression::Parameter { position: 0, primitive_type: primitive }
                                ).is_some()) => {}
                        // A domain whose membership is exactly an interval is
                        // retained as the same closed entry range a bracketed
                        // range is, so the range owner carries it too.
                        TypeConstraintNode::Domain(domain)
                            if validation::exact_declared_domain_carrier_interval(
                                &checked.typed,
                                primitive,
                                domain,
                            )
                            .filter(|(minimum, maximum)| minimum <= maximum)
                            .is_some() => {}
                        // A complete declared predicate belongs to value
                        // contracts, just like an interval. It is never a
                        // predicate-free membership atom in this catalog.
                        TypeConstraintNode::Domain(domain)
                            if domain.predicate_body.is_present()
                                && checked
                                    .facts
                                    .values
                                    .scalar_type_refinement_predicates(current)
                                    .is_some() => {}
                        TypeConstraintNode::Domain(domain)
                            if domain.subject == DomainConstraintSubject::Declared
                                && !domain.predicate_body.is_present() =>
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
                            if !checked.domain_definition(domain.symbol).is_some() {
                                return unsupported("scalar qualification lost its declaration");
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
        || (!checked_trees::domain::has_generic_carrier(checked, domain)
            && checked.primitive_type_reference(domain.target_type) != Some(primitive))
    {
        return unsupported(
            "scalar qualification declaration needs non-vacuous evidence or another carrier",
        );
    }
    let parameters = checked_trees::domain::index_parameters(checked, domain);
    checked_trees::domain::indexed_domain_instance_name(checked, domain, parameters, arguments)
        .map_err(|_| {
            LoweringError::Unsupported("scalar qualification has unresolved domain indices")
        })?;
    if let Some(alias) = &domain.alias {
        if alias.constituents.is_empty() || !arguments.is_empty() {
            return unsupported("scalar qualification has an empty or indexed alias expansion");
        }
        active.push(symbol);
        for constituent in &alias.constituents {
            let identity = checked
                .domain_definition(constituent.domain_symbol)
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
