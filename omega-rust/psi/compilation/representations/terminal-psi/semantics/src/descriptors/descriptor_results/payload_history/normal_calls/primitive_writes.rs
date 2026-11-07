//! A helper may change a number while keeping the caller's rule true.
//! Prove the complete rule from the actual written value at an authenticated
//! storage path. Remembering the old duty alone does not prove its fulfillment.

use super::*;
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

#[derive(Clone)]
pub(super) struct PerformedPrimitiveWrite {
    pub(super) machine: MachineId,
    pub(super) position: DescriptorPosition,
    pub(super) destination: DescriptorLocation,
    pub(super) value: ScalarTerm,
}

impl PerformedPrimitiveWrite {
    pub(super) fn bound(
        mut self,
        bindings: &impl crate::substitution::ScalarValueSubstitutions,
    ) -> Result<Self, &'static str> {
        let mut complete = true;
        let closed = Proposition::Equal(self.value.clone(), self.value.clone())
            .visit_value_ids(|value| complete &= bindings.replacement(value).is_some());
        if !closed || !complete {
            return Err("the primitive writer has an unbound scalar value or storage observation");
        }
        self.value = crate::substitution::substitute_scalar_term_bindings(&self.value, bindings);
        Ok(self)
    }
}

impl NormalPayloadContext<'_> {
    pub(super) fn performed_primitive_write(
        &self,
        analysis: &mut DescriptorAnalysis<'_>,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Result<PerformedPrimitiveWrite, &'static str> {
        let body = self.executable_body(identity)?;
        let operation = body
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .ok_or("the primitive writer has no exact operation")?;
        if operation.result != terminal_psi::OperationResult::Unit {
            return Err("the primitive writer has an unexpected result value");
        }
        let (root, path, field, value) = match &operation.kind {
            OperationKind::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                ..
            } => (*destination, path.as_slice(), None, *value),
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } => (*destination, path.as_slice(), Some(*field), *value),
            _ => return Err("the performed operation is not a typed primitive store"),
        };
        let carrier = body
            .structural_place_type(root)
            .ok_or("the primitive writer has no unique destination carrier")?;
        let destination = crate::descriptors::descriptor_lifetimes::descriptor_location(
            &self.module.structural_types,
            root,
            carrier,
            path,
            field,
        )
        .ok_or("the primitive writer has no exact static destination")?;
        if destination
            .path
            .iter()
            .any(|step| matches!(step, DescriptorPathSegment::Case(_)))
        {
            return Err("the primitive writer requires unavailable case-selection evidence");
        }
        let query = DescriptorContentsQuery::from_location(&destination);
        analysis.retained_primitive_payload_write(identity, position, &query)?;
        let value = super::super::super::payload_predicates::TerminalScalarExpressions::new(body)
            .expression(value)
            .ok_or("the primitive writer has no completed scalar expression")?;
        if super::super::super::payload_predicates::observation_type(
            &self.module.structural_types,
            carrier,
            &destination.path,
        ) != Some(value.scalar_type())
        {
            return Err("the primitive writer's actual RHS has a different scalar type");
        }
        let graph = descriptor_machine(self.graph, identity)
            .ok_or("the primitive writer has no descriptor graph")?;
        let Some([(recorded_destination, recorded_value)]) = graph
            .scalar_payload_writes
            .get(&position)
            .map(Vec::as_slice)
        else {
            return Err("the primitive writer has no single retained scalar payload");
        };
        if recorded_destination != &destination || recorded_value != &value {
            return Err("the primitive writer's scalar payload belongs to another operation");
        }
        Ok(PerformedPrimitiveWrite {
            machine: identity,
            position,
            destination,
            value,
        })
    }
}

pub(super) fn preserves_domain(
    module: &TerminalModule,
    prior: &PayloadMembershipContract,
    domain: semantic_vocabulary::StructuralDomainId,
    written: &[DescriptorPathSegment],
    value: &ScalarTerm,
) -> Result<(), &'static str> {
    let declaration = module
        .structural_domains
        .iter()
        .find(|row| row.id == domain)
        .ok_or("the primitive preservation duty has no domain declaration")?;
    if declaration.carrier != prior.carrier {
        return Err("the primitive preservation duty has a different carrier");
    }
    if !prior.qualifications.contains(&domain) {
        return Err("the primitive preservation rule cannot establish a new membership");
    }
    let path = descriptor_path(&module.structural_types, prior.carrier, written)
        .ok_or("the primitive preservation write has no typed relative path")?;
    if path.iter().any(|step| {
        !matches!(
            step,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return Err("the primitive preservation write has unknown or separately backed geometry");
    }
    require_ordinary_storage_path(&module.structural_types, prior.carrier, &path)?;
    if super::super::super::payload_predicates::observation_type(
        &module.structural_types,
        prior.carrier,
        written,
    ) != Some(value.scalar_type())
    {
        return Err("the primitive preservation value changes its receiving scalar type");
    }
    let closure = declaration
        .required_domain_closure(&module.structural_domains)
        .ok_or("the primitive preservation duty has an incomplete prerequisite theory")?;
    // Each writer must preserve the rule for every state satisfying it. A
    // later writer cannot reuse the first writer's pre-call field values.
    let current = PayloadMembershipContract {
        carrier: prior.carrier,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
    };
    let immutable =
        super::super::super::scalar_contracts::scalar_conjuncts(&prior.scalar_requirements);
    for duty in closure {
        if !duty.complete_predicate_theory || duty.byte_predicate.is_some() {
            return Err("the primitive preservation duty has unrepresented value requirements");
        }
        if !crate::store_preserves_domain_content(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            &module.identities,
            prior.qualifications.as_slice(),
            prior.projected_qualifications.as_slice(),
            &path,
        ) {
            return Err("the primitive preservation write may change conserved content");
        }
        let predicate = duty
            .predicate
            .as_ref()
            .ok_or("the primitive preservation duty has an opaque predicate")?;
        if !crate::domain_predicate_is_closed(
            module.structural_types.iter(),
            duty.carrier,
            predicate,
        ) {
            return Err("the primitive preservation duty has an open predicate");
        }
        if written.is_empty() && !duty.establishment_routes.is_empty() {
            // Replacing the whole qualified scalar, including a scalar field,
            // needs the new SSA operand's issuer evidence. Predicate truth
            // alone cannot provide that separate receiving judgment.
            return Err(
                "the whole routed scalar replacement needs exact RHS membership provenance",
            );
        }
        let goal = after_write(
            &module.structural_types,
            duty.carrier,
            predicate,
            written,
            value,
        )?;
        let mut requirements = immutable.clone();
        requirements.push(predicate.proposition.clone());
        if !super::super::super::payload_predicates::predicate_is_proved(
            &module.structural_types,
            &current,
            &goal,
            &requirements,
        ) {
            return Err("the actual scalar write does not prove the complete original predicate");
        }
    }
    Ok(())
}

pub(super) fn preserves_roster(
    module: &TerminalModule,
    prior: &PayloadMembershipContract,
    written: &[DescriptorPathSegment],
    value: &ScalarTerm,
) -> Result<(), &'static str> {
    let path = descriptor_path(&module.structural_types, prior.carrier, written)
        .ok_or("the primitive roster write has no exact typed path")?;
    for domain in &prior.qualifications {
        preserves_domain(module, prior, *domain, written, value)?;
    }
    if prior.projected_qualifications.is_empty() {
        return Ok(());
    }
    let written_geometry = OrdinaryStoragePath::new(&module.structural_types, prior.carrier, path);
    for row in &prior.projected_qualifications {
        let subject_geometry =
            OrdinaryStoragePath::new(&module.structural_types, prior.carrier, row.path.clone());
        if !written_geometry.may_overlap(&subject_geometry)? {
            require_inline_projected_dependencies(module, prior.carrier, row)?;
            continue;
        }
        if row.path.iter().any(|step| {
            !matches!(
                step,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Referent
            )
        }) {
            return Err(
                "the primitive roster duty needs conditional or represented subject evidence",
            );
        }
        let subject = crate::descriptors::descriptor_lifetimes::descriptor_location(
            &module.structural_types,
            PlaceId::new(1).unwrap(),
            prior.carrier,
            &row.path,
            None,
        )
        .ok_or("the primitive roster duty has no exact subject path")?;
        let relative = written
            .strip_prefix(subject.path.as_slice())
            .ok_or("the primitive roster write replaces an ancestor of its qualified subject")?;
        let selected = select_payload_memberships_with_kind(
            &module.structural_types,
            &module.structural_domains,
            prior,
            &subject.path,
            PayloadMembershipKind::Preserved,
        )
        .ok_or("the primitive roster duty has no complete selected contract")?;
        preserves_domain(module, &selected, row.domain, relative, value)?;
    }
    Ok(())
}

/// The separation proof concerns the subject's inline storage. Frame every
/// prerequisite's predicate and conserved Content before using it to keep a
/// projected duty; an aggregate carrier alone does not rule out borrowed data.
fn require_inline_projected_dependencies(
    module: &TerminalModule,
    root: StructuralTypeId,
    row: &terminal_psi::StructuralPathQualification,
) -> Result<(), &'static str> {
    let carrier =
        crate::qualification_path_carrier(module.structural_types.iter(), root, &row.path)
            .ok_or("the projected preservation duty has no exact carrier")?;
    let declaration = crate::exactly_one::exactly_one(module.structural_domains.iter(), |domain| {
        domain.id == row.domain && domain.carrier == carrier
    })
    .ok_or("the projected preservation duty has no unique matching declaration")?;
    // An independently unchanged, wholly inline subject keeps its exact held
    // duty even when that rule's definition is opaque. This preserves the same
    // value; it does not interpret missing prerequisite metadata as no duties.
    if plain_owned_shape(module.structural_types.iter(), carrier) {
        return Ok(());
    }
    let closure = declaration
        .required_domain_closure(&module.structural_domains)
        .ok_or("the projected preservation duty has an incomplete prerequisite theory")?;
    if !closure.into_iter().all(|domain| {
        crate::domain_preservation::domain_has_inline_storage_dependencies(
            module.structural_types.iter(),
            &module.identities,
            domain,
        )
    }) {
        return Err("scalar preservation needs captured domain dependency evidence");
    }
    Ok(())
}

fn after_write(
    types: &[StructuralTypeDeclaration],
    carrier: StructuralTypeId,
    predicate: &terminal_psi::StructuralDomainPredicate,
    written: &[DescriptorPathSegment],
    value: &ScalarTerm,
) -> Result<terminal_psi::StructuralDomainPredicate, &'static str> {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerMathTerm, ScalarReadProjection, ScalarType,
    };
    enum Pending<'a> {
        Proposition(&'a mut Proposition),
        Scalar(&'a mut ScalarTerm),
        Math(&'a mut IntegerMathTerm),
    }
    let written_canonical = super::super::super::payload_predicates::canonical_path(written)
        .ok_or("the scalar preservation path has no canonical spelling")?;
    let written_storage = descriptor_path(types, carrier, written)
        .ok_or("the scalar preservation path has no storage spelling")?;
    let mut goal = predicate.clone();
    let mut pending = vec![Pending::Proposition(&mut goal.proposition)];
    let mut remaining = 4096usize;
    while let Some(node) = pending.pop() {
        remaining = remaining
            .checked_sub(1)
            .ok_or("the scalar preservation predicate exceeds its traversal budget")?;
        match node {
            Pending::Proposition(proposition) => match proposition {
                Proposition::Truth | Proposition::Falsehood => {}
                semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                    pending.push(Pending::Scalar(right));
                    pending.push(Pending::Scalar(left));
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    pending.push(Pending::Math(right));
                    pending.push(Pending::Math(left));
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    pending.extend(children.iter_mut().map(Pending::Proposition))
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(Pending::Proposition(conclusion));
                    pending.push(Pending::Proposition(premise));
                }
                // Guarded case observations and legacy floating-field forms
                // need their own partial-observation preservation judgment.
                _ => {
                    return Err(
                        "the scalar preservation predicate contains an unsupported observation",
                    );
                }
            },
            Pending::Math(term) => match term {
                IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                IntegerMathTerm::MathValue { .. } => {
                    return Err(
                        "the scalar preservation predicate has an ambient mathematical value",
                    );
                }
                term => pending.extend(term.operands_mut().into_iter().map(Pending::Math)),
            },
            Pending::Scalar(term) => {
                let observed = match term {
                    ScalarTerm::BooleanField { root, path } => {
                        Some((*root, path.clone(), ScalarType::Boolean))
                    }
                    ScalarTerm::IntegerField {
                        root,
                        path,
                        scalar_type,
                    } => Some((*root, path.clone(), ScalarType::Integer(*scalar_type))),
                    ScalarTerm::IeeeFloatField { root, path, format } => {
                        Some((*root, path.clone(), ScalarType::IeeeFloat(*format)))
                    }
                    ScalarTerm::StructuralRead {
                        root,
                        path,
                        scalar_type,
                    } => Some((
                        *root,
                        path.iter()
                            .map(|step| match step {
                                ScalarReadProjection::Static(step) => Ok(*step),
                                _ => Err(
                                    "the scalar preservation predicate has a dynamic observation",
                                ),
                            })
                            .collect::<Result<Vec<CanonicalStructuralPathSegment>, _>>()?,
                        *scalar_type,
                    )),
                    ScalarTerm::ByteSequencePredicate { .. } => {
                        return Err(
                            "the scalar preservation predicate needs a byte-content preservation judgment",
                        );
                    }
                    _ => None,
                };
                if let Some((root, path, scalar_type)) = observed {
                    if root != predicate.subject
                        || path.iter().any(|step| {
                            matches!(
                                step,
                                CanonicalStructuralPathSegment::Case(_)
                                    | CanonicalStructuralPathSegment::Referent
                            )
                        })
                    {
                        return Err(
                            "the scalar preservation predicate observes an unbound or separately backed subject",
                        );
                    }
                    if path == written_canonical {
                        if scalar_type != value.scalar_type()
                            || matches!(scalar_type, ScalarType::IeeeFloat(_))
                        {
                            return Err(
                                "the scalar preservation replacement has an unsupported observation type",
                            );
                        }
                        *term = value.clone();
                        continue;
                    }
                    let storage =
                        crate::canonical_structural_storage_path(types.iter(), carrier, &path)
                            .ok_or(
                                "the scalar preservation observation has no exact storage path",
                            )?;
                    require_ordinary_storage_path(types, carrier, &storage)?;
                    if crate::structural_paths_may_overlap(&storage, &written_storage) {
                        return Err(
                            "the scalar preservation observation has uncertain write overlap",
                        );
                    }
                }
                pending.extend(term.operands_mut().into_iter().map(Pending::Scalar));
            }
        }
    }
    Ok(goal)
}

/// Keep the path and its ordinary-prefix boundary together. A metadata-only
/// suffix can remain untouched after an ordinary containing member separates
/// the write; it need not be a runtime address. A referent is a separate alias
/// space and cannot inherit that conclusion from inline sibling names.
pub(super) struct OrdinaryStoragePath {
    carrier: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    first_problem: Option<(usize, &'static str)>,
    separately_backed: bool,
}

impl OrdinaryStoragePath {
    pub(super) fn new(
        types: &[StructuralTypeDeclaration],
        carrier: StructuralTypeId,
        path: Vec<StructuralPathSegment>,
    ) -> Self {
        let first_problem = qualification_storage_problem(types, carrier, &path);
        let separately_backed = path.iter().enumerate().any(|(position, step)| {
            if matches!(step, StructuralPathSegment::Referent) {
                return true;
            }
            if !matches!(
                step,
                StructuralPathSegment::AllElements
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::FixedRange { .. }
                    | StructuralPathSegment::RuntimeIndex { .. }
            ) {
                return false;
            }
            let Some(container) =
                crate::qualification_path_carrier(types.iter(), carrier, &path[..position])
            else {
                return true;
            };
            let mut declarations = types.iter().filter(|row| row.id == container);
            let Some(declaration) = declarations.next() else {
                return true;
            };
            declarations.next().is_some()
                || !matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
        });
        let separately_backed = separately_backed
            || crate::qualification_path_carrier(types.iter(), carrier, &path)
                .and_then(|id| {
                    let mut declarations = types.iter().filter(|row| row.id == id);
                    let declaration = declarations.next()?;
                    declarations.next().is_none().then_some(declaration)
                })
                .is_some_and(|declaration| separately_backed_shape(&declaration.shape));
        Self {
            carrier,
            path,
            first_problem,
            separately_backed,
        }
    }

    pub(super) fn may_overlap(&self, other: &Self) -> Result<bool, &'static str> {
        if self.carrier != other.carrier {
            return Err("the provider write and captured subject have different carriers");
        }
        let separation = crate::structural::static_path::structural_path_separation_depth(
            &self.path,
            &other.path,
        );
        let relevant = separation.unwrap_or_else(|| self.path.len().min(other.path.len()));
        for path in [self, other] {
            if let Some((prefix, reason)) = path.first_problem
                && prefix <= relevant
            {
                return Err(reason);
            }
        }
        let overlaps = separation.is_none();
        if !overlaps && (self.separately_backed || other.separately_backed) {
            return Err("scalar preservation needs captured referent alias evidence");
        }
        Ok(overlaps)
    }
}

fn require_ordinary_storage_path(
    types: &[StructuralTypeDeclaration],
    carrier: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<(), &'static str> {
    ordinary_storage_problem(types, carrier, path).map_or(Ok(()), |(_, reason)| Err(reason))
}

fn ordinary_storage_problem(
    types: &[StructuralTypeDeclaration],
    carrier: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(usize, &'static str)> {
    storage_prefix_problem(path, |end| {
        crate::runtime_structural_path_tip(types.iter(), carrier, &path[..end])
    })
}

fn qualification_storage_problem(
    types: &[StructuralTypeDeclaration],
    carrier: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(usize, &'static str)> {
    storage_prefix_problem(path, |end| {
        let id = crate::qualification_path_carrier(types.iter(), carrier, &path[..end])?;
        let mut declarations = types.iter().filter(|row| row.id == id);
        let declaration = declarations.next()?;
        declarations.next().is_none().then_some(declaration)
    })
}

fn separately_backed_shape(shape: &StructuralTypeShape) -> bool {
    matches!(
        shape,
        StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
    )
}

fn storage_prefix_problem<'types>(
    path: &[StructuralPathSegment],
    mut declaration: impl FnMut(usize) -> Option<&'types StructuralTypeDeclaration>,
) -> Option<(usize, &'static str)> {
    for end in 0..=path.len() {
        let Some(tip) = declaration(end) else {
            if end == path.len() && end != 0 {
                // A final inline scalar can lack a standalone declaration.
                // Its exact scalar type is independently checked by the caller.
                continue;
            }
            return Some((
                end,
                "the scalar preservation path has an unknown enclosing carrier",
            ));
        };
        if matches!(
            tip.shape,
            StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_),
                ..
            }
        ) {
            return Some((
                end,
                "scalar preservation needs independently proved external record geometry",
            ));
        }
        if separately_backed_shape(&tip.shape) {
            return Some((
                end,
                "scalar preservation needs captured referent alias evidence",
            ));
        }
    }
    None
}
