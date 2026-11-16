//! Nominal scalar membership is part of a value's declared type. Source lowering,
//! the sole lowered checker and artifact ingress share this judgment; neither
//! an ordinary result annotation nor an unbound coercion can mint membership.

use semantic_vocabulary::{
    EdgeId, Proposition, PropositionContext, ScalarDomainId, ScalarQualificationSetId, ValueId,
};
use terminal_psi::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarReturnOrigin, ScalarReturnSite,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};

#[derive(Debug)]
pub enum ScalarQualificationError {
    MalformedStructuralFoundation(&'static str),
    MalformedProposition(semantic_vocabulary::PropositionError),
    MalformedScalarReturnFoundation {
        machine: String,
        domain: String,
        edge: EdgeId,
        source: ValueId,
        result: ValueId,
        reason: String,
    },
}

fn malformed<T>(detail: &'static str) -> Result<T, ScalarQualificationError> {
    Err(ScalarQualificationError::MalformedStructuralFoundation(
        detail,
    ))
}

/// A compiler-owned policy's identity is independent of its carrier. The
/// domain declaration retains that carrier separately, including width binders
/// that later close without changing which arithmetic policy was selected.
pub fn arithmetic_policy_identity(
    policy: numerics::arithmetic::ArithmeticDomain,
) -> Option<&'static str> {
    match policy {
        numerics::arithmetic::ArithmeticDomain::Wrapping => Some("builtin:policy:Wrapping"),
        numerics::arithmetic::ArithmeticDomain::Saturating => Some("builtin:policy:Saturating"),
        numerics::arithmetic::ArithmeticDomain::Trapping => Some("builtin:policy:Trapping"),
        numerics::arithmetic::ArithmeticDomain::Exact => None,
    }
}

/// An issuer restriction's route, whichever domain family declares it. A
/// boundary requirement issues only at its boundary completion, never inside
/// a guest machine, so it has no form here.
pub(crate) enum IssuerRoute<'a> {
    ExactMachine {
        issuer: &'a terminal_psi::NominalDeclarationIdentity,
        machine_identity: terminal_psi::TerminalIdentityIndex,
    },
    CheckedRequirement {
        issuer: &'a terminal_psi::NominalDeclarationIdentity,
        requirement_identity: terminal_psi::TerminalIdentityIndex,
    },
}

/// Whether `machine`, by its checked declaration, is the callable a route
/// names: the exact machine (or the template a specialization instantiates),
/// or a closed conformance realizing the exact requirement with it.
pub(crate) fn issuer_route_authorizes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    declaration: &terminal_psi::CheckedMachineDeclaration,
    route: IssuerRoute<'_>,
) -> bool {
    match route {
        IssuerRoute::ExactMachine {
            issuer,
            machine_identity,
        } => {
            (module
                .identities
                .text(machine_identity)
                .zip(module.identities.text(declaration.callable))
                .is_some_and(|(issued, callable)| {
                    issued
                        == terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                            callable,
                            declaration
                                .specialization
                                .as_ref()
                                .map(|application| &application.commitment),
                        )
                })
                && issuer.identity() == declaration.semantic_declaration)
                || declaration
                    .specialization
                    .as_ref()
                    .is_some_and(|specialization| {
                        machine_identity == specialization.template_callable
                            && issuer.identity() == specialization.template_semantic_declaration
                            && module
                                .identities
                                .text(specialization.template_callable)
                                .is_some_and(|identity| !identity.is_empty())
                    })
        }
        IssuerRoute::CheckedRequirement {
            issuer,
            requirement_identity,
        } => module
            .closed_conformance_applications
            .iter()
            .any(|application| {
                application.rows.iter().any(|row| {
                    row.public_requirement_identity == requirement_identity
                        && row.requirement_declaration == *issuer
                        && row.realization_callable_identity == Some(declaration.callable)
                        && application.realization_callables.iter().any(|callable| {
                            callable.source_callable_identity == declaration.callable
                                && callable.callable
                                    == terminal_psi::TargetMachineCallable::Machine(machine.id)
                        })
                })
            }),
    }
}

fn same_type(left: &ValueDeclaration, right: &ValueDeclaration) -> bool {
    left.scalar_type == right.scalar_type && left.qualifications == right.qualifications
}

struct ScalarValues<'a>(Vec<&'a ValueDeclaration>);

impl<'a> ScalarValues<'a> {
    fn new(machine: &'a TerminalMachine) -> Result<Self, ScalarQualificationError> {
        let mut rows: Vec<_> = values(machine).collect();
        rows.sort_unstable_by_key(|value| value.id);
        if rows.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return malformed("scalar transfer has duplicate value declarations");
        }
        Ok(Self(rows))
    }

    fn get(&self, id: ValueId) -> Result<&'a ValueDeclaration, ScalarQualificationError> {
        self.0
            .binary_search_by_key(&id, |value| value.id)
            .ok()
            .map(|index| self.0[index])
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar transfer names an absent value",
            ))
    }
}

fn validate_transfers(module: &TerminalModule) -> Result<(), ScalarQualificationError> {
    use terminal_psi::{OperationKind, TargetMachineCallable};
    let catalog = &module.scalar_qualifications;
    validate_field_types(module)?;
    for contract in module
        .declaration_contracts
        .iter()
        .chain(module.machines.iter().map(|machine| &machine.contract))
    {
        for formal in contract
            .parameters
            .iter()
            .chain(&contract.erased_scalar_formals)
        {
            validate_predicate_binding(module, formal, contract.requires.iter())?;
        }
        if let Some(result) = contract.result.scalar_ref() {
            validate_predicate_binding(
                module,
                result,
                contract.ensures.iter().map(|clause| &clause.proposition),
            )?;
        }
    }
    for primitive in &catalog.primitives {
        if module
            .structural_types
            .iter()
            .find(|ty| ty.id == primitive.structural_type)
            .is_none_or(|ty| {
                ty.shape
                    != terminal_psi::StructuralTypeShape::PrimitiveScalar(
                        primitive.value_type.scalar_type,
                    )
            })
            || primitive.value_type.qualifications.is_empty()
            || qualification_domains(module, primitive.value_type.qualifications).is_none_or(
                |domains| {
                    domains.iter().any(|id| {
                        scalar_domain(module, *id)
                            .is_none_or(|domain| domain.carrier != primitive.value_type.scalar_type)
                    })
                },
            )
        {
            return malformed("primitive scalar storage lost its complete declared type");
        }
    }
    for machine in &module.machines {
        let declared = ScalarValues::new(machine)?;
        for declaration in values(machine)
            .chain(machine.result.scalar_ref())
            .chain(&machine.erased_scalar_formals)
        {
            for id in qualification_domains(module, declaration.qualifications).ok_or(
                ScalarQualificationError::MalformedStructuralFoundation(
                    "scalar value names an absent qualification set",
                ),
            )? {
                if scalar_domain(module, *id)
                    .is_none_or(|domain| domain.carrier != declaration.scalar_type)
                {
                    return malformed("scalar value's domain has a different carrier");
                }
            }
        }
        for block in &machine.blocks {
            if let Terminator::StructuralCase { cases, .. } = &block.terminator {
                for case in cases {
                    let target = machine
                        .blocks
                        .iter()
                        .find(|block| block.id == case.target)
                        .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                            "scalar payload binding lost its destination",
                        ))?;
                    if target.parameters.len() != case.payload_fields.len() {
                        return malformed(
                            "scalar payload binding has a different declared field roster",
                        );
                    }
                    for (field, parameter) in case.payload_fields.iter().zip(&target.parameters) {
                        if parameter.qualifications != field_qualifications(module, *field) {
                            return malformed(
                                "scalar payload binding does not preserve its complete declared type",
                            );
                        }
                    }
                }
            }
            if let Terminator::Return {
                edge,
                value: source,
                ..
            } = &block.terminator
            {
                let source = declared.get(*source)?;
                let Some(result) = machine.result.scalar_ref() else {
                    return malformed("scalar return has no declared scalar result");
                };
                if !same_type(source, result)
                    && !checked_introduction(
                        module,
                        machine.id,
                        *edge,
                        source,
                        result,
                        ScalarReturnSite::NormalExit,
                    )?
                {
                    return malformed(
                        "normal return does not preserve its complete declared scalar type",
                    );
                }
            }
            if let Some((_, inputs)) = block.terminator.declared_invocation() {
                validate_arguments(module, &declared, inputs)?;
            }
            for operation in &block.operations {
                if let Some(inputs) = operation.declared_invocation() {
                    validate_arguments(module, &declared, inputs)?;
                    if let TargetMachineCallable::Machine(callee) = inputs.callee {
                        let contract = module
                            .invocation_machine_contract(callee, inputs.required_contract)
                            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar call lost its exact declared contract",
                            ))?;
                        match (operation.result.scalar_ref(), contract.result.scalar_ref()) {
                            (Some(actual), Some(formal)) if same_type(actual, formal) => {}
                            (None, None) => {}
                            _ => {
                                return malformed(
                                    "scalar call result differs from its declared type",
                                );
                            }
                        }
                    } else if let TargetMachineCallable::Boundary(boundary) = inputs.callee {
                        let boundary = module.boundary_machine(boundary).ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar call lost its boundary declaration",
                            ),
                        )?;
                        match (operation.result.scalar_ref(), &boundary.result) {
                            (Some(actual), terminal_psi::BoundaryMachineResult::Scalar(formal))
                                if actual.scalar_type == formal.scalar_type
                                    && actual.qualifications == formal.qualifications => {}
                            (
                                None,
                                terminal_psi::BoundaryMachineResult::Unit
                                | terminal_psi::BoundaryMachineResult::Structural(_),
                            ) => {}
                            _ => {
                                return malformed(
                                    "boundary scalar result differs from its declared type",
                                );
                            }
                        }
                    }
                    continue;
                }
                let check_write = |field, value| {
                    if declared.get(value)?.qualifications != field_qualifications(module, field) {
                        return malformed(
                            "scalar field write does not preserve its complete declared type",
                        );
                    }
                    Ok(())
                };
                match &operation.kind {
                    OperationKind::EstablishZeroedStructural {} => {
                        let result = operation.result.structural().ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "zero provisioning lost its structural result",
                            ),
                        )?;
                        if !crate::zeroed_structural_types_and_fields_satisfy(
                            module.structural_types.iter(),
                            result.structural_type,
                            |ty| {
                                qualification_domains(
                                    module,
                                    primitive_qualifications(module, ty.id),
                                )
                                .is_some_and(|domains| {
                                    domains.iter().all(|id| {
                                        scalar_domain(module, *id).is_some_and(|domain| {
                                            domain.predicate.is_none()
                                                && domain.establishment_routes.is_none()
                                                && domain.has_no_required_memberships()
                                        })
                                    })
                                })
                            },
                            |field| {
                                qualification_domains(
                                    module,
                                    field_qualifications(module, field.id),
                                )
                                .is_some_and(|domains| {
                                    domains.iter().all(|id| {
                                        scalar_domain(module, *id).is_some_and(|domain| {
                                            domain.establishment_routes.is_none()
                                                && domain.has_no_required_memberships()
                                        })
                                    })
                                })
                            },
                        ) {
                            return malformed(
                                "zero provisioning cannot establish routed scalar field membership",
                            );
                        }
                    }
                    OperationKind::EstablishPrimitiveLocal { value, .. } => {
                        let result = operation.result.structural().ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "primitive initialization lost its storage declaration",
                            ),
                        )?;
                        let expected = primitive_qualifications(module, result.structural_type);
                        let source = declared.get(*value)?;
                        if source.qualifications != expected {
                            let zero = machine.operations().any(|definition| {
                                definition
                                    .result
                                    .scalar_ref()
                                    .is_some_and(|result| result.id == source.id)
                                    && matches!(
                                        definition.kind,
                                        OperationKind::IntegerConstant {
                                            value: semantic_vocabulary::IntegerValue::Signed(0)
                                                | semantic_vocabulary::IntegerValue::Unsigned(0)
                                        } | OperationKind::BooleanConstant { value: false }
                                    )
                            });
                            let free =
                                qualification_domains(module, expected).is_some_and(|domains| {
                                    domains.iter().all(|id| {
                                        scalar_domain(module, *id).is_some_and(|domain| {
                                            domain.predicate.is_none()
                                                && domain.establishment_routes.is_none()
                                                && domain.has_no_required_memberships()
                                        })
                                    })
                                });
                            if !source.qualifications.is_empty() || !zero || !free {
                                return malformed(
                                    "primitive initialization cannot mint its declared scalar membership",
                                );
                            }
                        }
                    }
                    OperationKind::EstablishScalarArray { elements } => {
                        let result = operation.result.structural().ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar array initialization lost its storage declaration",
                            ),
                        )?;
                        let (leaf, count) = crate::scalar_array_leaf_declaration(
                            module.structural_types.iter(),
                            result.structural_type,
                        )
                        .ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar array has no exact primitive leaf declaration",
                            ),
                        )?;
                        let expected = primitive_qualifications(module, leaf.id);
                        if count != elements.len() as u64 {
                            return malformed("scalar array has a different declared leaf count");
                        }
                        for element in elements {
                            if declared.get(*element)?.qualifications != expected {
                                return malformed(
                                    "scalar array element does not preserve its declared membership",
                                );
                            }
                        }
                    }
                    OperationKind::StructuralScalarFieldStore { field, value, .. } => {
                        check_write(*field, *value)?
                    }
                    OperationKind::EstablishRecord { fields }
                    | OperationKind::EstablishStructuralCase { fields, .. } => {
                        for field in fields {
                            if let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                                field.value
                            {
                                check_write(field.field, value)?;
                            }
                        }
                    }
                    OperationKind::EstablishScalarCase { fields, .. } => {
                        for field in fields {
                            check_write(field.field, field.value)?;
                        }
                    }
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        value,
                        ..
                    } if declared.get(*value)?.qualifications
                        != storage_qualifications(module, machine, *destination, path)? =>
                    {
                        return malformed(
                            "scalar storage write does not preserve its complete declared type",
                        );
                    }
                    _ => {}
                }
                let Some(result) = operation.result.scalar_ref() else {
                    continue;
                };
                let qualified = !result.qualifications.is_empty();
                let stored = match &operation.kind {
                    OperationKind::ScalarStructuralField { field, .. }
                    | OperationKind::BooleanStructuralField { field, .. } => {
                        Some(field_qualifications(module, *field))
                    }
                    OperationKind::PrimitiveScalarRead { source, path }
                    | OperationKind::StructuralLeafCopy { source, path } => {
                        Some(storage_qualifications(module, machine, *source, path)?)
                    }
                    OperationKind::ElementViewRead { source, path, .. } => {
                        let root = machine.structural_place_type(source.place).ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar element view lost its declared root",
                            ),
                        )?;
                        let view = crate::runtime_structural_path_tip(
                            module.structural_types.iter(),
                            root,
                            &source.path,
                        )
                        .ok_or(
                            ScalarQualificationError::MalformedStructuralFoundation(
                                "scalar element view lost its declared carrier",
                            ),
                        )?;
                        let element = match view.shape {
                            terminal_psi::StructuralTypeShape::ElementView { element, .. }
                            | terminal_psi::StructuralTypeShape::FixedArray { element, .. } => {
                                element
                            }
                            _ => {
                                return malformed(
                                    "scalar element view has no declared element type",
                                );
                            }
                        };
                        Some(storage_type_qualifications(module, element, path)?)
                    }
                    _ => None,
                };
                if let Some(expected) = stored {
                    if result.qualifications != expected {
                        return malformed(
                            "scalar field read differs from its declared qualification",
                        );
                    }
                } else if qualified {
                    return malformed(
                        "ordinary scalar operation cannot establish a declared domain",
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_field_types(module: &TerminalModule) -> Result<(), ScalarQualificationError> {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, ContentPlaceVersion, PlaceId, ScalarTerm, ScalarType,
    };
    use terminal_psi::StructuralTypeShape;
    for row in &module.scalar_qualifications.fields {
        let mut found = false;
        for owner in &module.structural_types {
            let (fields, cases) = match &owner.shape {
                StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), &[][..]),
                StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
                StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases.as_slice())
                }
                _ => continue,
            };
            for field in fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter(|field| field.id == row.field)
            {
                found = true;
                if field.relevance.is_erased()
                    || field.field_type.scalar_type() != Some(row.value_type.scalar_type)
                    || row.value_type.qualifications.is_empty()
                {
                    return malformed("scalar field qualification changes its declared carrier");
                }
                for id in qualification_domains(module, row.value_type.qualifications).ok_or(
                    ScalarQualificationError::MalformedStructuralFoundation(
                        "scalar field names an absent qualification set",
                    ),
                )? {
                    let domain = scalar_domain(module, *id)
                        .filter(|domain| domain.carrier == row.value_type.scalar_type)
                        .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                            "scalar field lost its same-carrier domain",
                        ))?;
                    let Some(predicate) = &domain.predicate else {
                        continue;
                    };
                    let root =
                        PlaceId::new(1).expect("the default domain has one symbolic self place");
                    let path = vec![CanonicalStructuralPathSegment::Field(field.id)];
                    let term = match row.value_type.scalar_type {
                        ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                            version: ContentPlaceVersion::Current,
                            root,
                            path,
                            scalar_type,
                        },
                        ScalarType::Boolean => ScalarTerm::BooleanField {
                            version: ContentPlaceVersion::Current,
                            root,
                            path,
                        },
                        _ if predicate.proposition == Proposition::Truth => continue,
                        _ => {
                            return malformed(
                                "scalar field predicate has no declared storage observation",
                            );
                        }
                    };
                    let expected = crate::substitution::clauses::substitute_clause(
                        &predicate.proposition,
                        &mut FieldSubject {
                            subject: predicate.subject,
                            term,
                        },
                    )
                    .map_err(ScalarQualificationError::MalformedStructuralFoundation)?;
                    if !retains_predicate(&expected, owner.requires.iter()) {
                        return malformed(
                            "scalar field's default domain lost its declared predicate",
                        );
                    }
                }
            }
        }
        if !found {
            return malformed("scalar field qualification has no declared field");
        }
    }
    Ok(())
}

struct FieldSubject {
    subject: ValueId,
    term: semantic_vocabulary::ScalarTerm,
}

impl crate::substitution::clauses::ClauseBindings for FieldSubject {
    type Error = &'static str;
    fn value(
        &mut self,
        id: ValueId,
        scalar_type: semantic_vocabulary::ScalarType,
    ) -> Result<semantic_vocabulary::ScalarTerm, Self::Error> {
        if id != self.subject || scalar_type != self.term.scalar_type() {
            return Err("scalar field predicate has a foreign subject");
        }
        Ok(self.term.clone())
    }
    fn field(
        &mut self,
        _: semantic_vocabulary::ContentPlaceVersion,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::ScalarType,
    ) -> Result<semantic_vocabulary::ScalarTerm, Self::Error> {
        Err("scalar domain predicate observes storage outside its subject")
    }
    fn case(
        &mut self,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("scalar domain predicate tests a structural case")
    }
}

fn validate_predicate_binding<'a>(
    module: &TerminalModule,
    value: &ValueDeclaration,
    clauses: impl Iterator<Item = &'a Proposition> + Clone,
) -> Result<(), ScalarQualificationError> {
    for id in qualification_domains(module, value.qualifications).ok_or(
        ScalarQualificationError::MalformedStructuralFoundation(
            "scalar signature names an absent qualification set",
        ),
    )? {
        let domain = scalar_domain(module, *id)
            .filter(|domain| domain.carrier == value.scalar_type)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar signature lost its same-carrier domain",
            ))?;
        if let Some(predicate) = &domain.predicate {
            let mut expected = predicate.proposition.clone();
            expected.remap_value_ids(|_| value.id);
            if !retains_predicate(&expected, clauses.clone()) {
                return malformed("scalar signature lost its declared domain predicate");
            }
        }
    }
    Ok(())
}

fn retains_predicate<'a>(
    expected: &Proposition,
    clauses: impl Iterator<Item = &'a Proposition> + Clone,
) -> bool {
    match expected {
        Proposition::Truth => true,
        Proposition::Conjunction(children) => children
            .iter()
            .all(|child| retains_predicate(child, clauses.clone())),
        _ => clauses
            .into_iter()
            .any(|clause| retains_conjunct(clause, expected)),
    }
}

fn validate_arguments(
    module: &TerminalModule,
    values: &ScalarValues<'_>,
    inputs: terminal_psi::ModuleInvocationInputs<'_, terminal_psi::TargetMachineCallable>,
) -> Result<(), ScalarQualificationError> {
    match inputs.callee {
        terminal_psi::TargetMachineCallable::Machine(callee) => {
            let contract = module
                .invocation_machine_contract(callee, inputs.required_contract)
                .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                    "scalar call lost its exact declared contract",
                ))?;
            if inputs.arguments.len() != contract.parameters.len() {
                return malformed("scalar call has a different declared argument count");
            }
            for (actual, formal) in inputs.arguments.iter().zip(&contract.parameters) {
                if !same_type(values.get(*actual)?, formal) {
                    return malformed(
                        "scalar call argument does not preserve its complete declared type",
                    );
                }
            }
        }
        terminal_psi::TargetMachineCallable::Boundary(boundary) => {
            let boundary = module.boundary_machine(boundary).ok_or(
                ScalarQualificationError::MalformedStructuralFoundation(
                    "scalar call lost its boundary declaration",
                ),
            )?;
            if inputs.required_contract.is_some()
                || inputs.arguments.len() != boundary.scalar_parameters.len()
            {
                return malformed("boundary scalar call has another declared argument roster");
            }
            for (index, (actual, carrier)) in inputs
                .arguments
                .iter()
                .zip(&boundary.scalar_parameters)
                .enumerate()
            {
                let actual = values.get(*actual)?;
                let mut required: Vec<_> = boundary
                    .scalar_domain_requirements
                    .iter()
                    .filter(|row| row.argument_index as usize == index)
                    .map(|row| row.domain)
                    .collect();
                required.sort_unstable();
                if actual.scalar_type != *carrier
                    || qualification_domains(module, actual.qualifications)
                        != Some(required.as_slice())
                {
                    return malformed(
                        "boundary scalar argument does not preserve its complete declared type",
                    );
                }
            }
        }
    }
    Ok(())
}

fn field_qualifications(
    module: &TerminalModule,
    field: semantic_vocabulary::StructuralFieldId,
) -> ScalarQualificationSetId {
    let fields = &module.scalar_qualifications.fields;
    fields
        .binary_search_by_key(&field, |row| row.field)
        .ok()
        .map(|index| &fields[index])
        .map_or(ScalarQualificationSetId::ZERO, |row| {
            row.value_type.qualifications
        })
}

fn primitive_qualifications(
    module: &TerminalModule,
    ty: semantic_vocabulary::StructuralTypeId,
) -> ScalarQualificationSetId {
    let primitives = &module.scalar_qualifications.primitives;
    primitives
        .binary_search_by_key(&ty, |row| row.structural_type)
        .ok()
        .map(|index| &primitives[index])
        .map_or(ScalarQualificationSetId::ZERO, |row| {
            row.value_type.qualifications
        })
}

fn checked_introduction(
    module: &TerminalModule,
    machine: semantic_vocabulary::MachineId,
    edge: EdgeId,
    source: &ValueDeclaration,
    destination: &ValueDeclaration,
    site: ScalarReturnSite,
) -> Result<bool, ScalarQualificationError> {
    let members = |set| {
        qualification_domains(module, set).ok_or(
            ScalarQualificationError::MalformedStructuralFoundation(
                "scalar introduction has an absent qualification set",
            ),
        )
    };
    let before = members(source.qualifications)?;
    let after = members(destination.qualifications)?;
    Ok(source.scalar_type == destination.scalar_type
        && before.len() < after.len()
        && before.iter().all(|domain| after.contains(domain))
        && after
            .iter()
            .filter(|domain| !before.contains(domain))
            .all(|domain| {
                module.scalar_qualifications.returns.iter().any(|row| {
                    row.machine == machine
                        && row.edge == edge
                        && row.source == source.id
                        && row.result == destination.id
                        && row.site == site
                        && row.domain == *domain
                        && !row.origins.is_empty()
                })
            }))
}

fn storage_qualifications(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<ScalarQualificationSetId, ScalarQualificationError> {
    let root = machine.structural_place_type(place).ok_or(
        ScalarQualificationError::MalformedStructuralFoundation(
            "scalar storage has no declared root",
        ),
    )?;
    storage_type_qualifications(module, root, path)
}

fn storage_type_qualifications(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<ScalarQualificationSetId, ScalarQualificationError> {
    if let Some(field) = crate::runtime_structural_field(module.structural_types.iter(), root, path)
    {
        return Ok(field_qualifications(module, field.id));
    }
    let tip = crate::runtime_structural_path_tip(module.structural_types.iter(), root, path)
        .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
            "scalar storage path has no declared carrier",
        ))?;
    if !matches!(
        tip.shape,
        terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
    ) {
        return malformed("scalar storage path does not select a primitive carrier");
    }
    Ok(module
        .scalar_qualifications
        .primitives
        .iter()
        .find(|row| row.structural_type == tip.id)
        .map_or(ScalarQualificationSetId::ZERO, |row| {
            row.value_type.qualifications
        }))
}

pub fn qualification_domains(
    module: &TerminalModule,
    set: ScalarQualificationSetId,
) -> Option<&[ScalarDomainId]> {
    if set.is_empty() {
        return Some(&[]);
    }
    let sets = &module.scalar_qualifications.sets;
    sets.binary_search_by_key(&set, |row| row.id)
        .ok()
        .map(|index| sets[index].domains.as_slice())
}

pub fn scalar_domain(
    module: &TerminalModule,
    id: ScalarDomainId,
) -> Option<&ScalarDomainDeclaration> {
    let domains = &module.scalar_qualifications.domains;
    domains
        .binary_search_by_key(&id, |domain| domain.id)
        .ok()
        .map(|index| &domains[index])
}

pub fn values(machine: &TerminalMachine) -> impl Iterator<Item = &ValueDeclaration> {
    machine
        .parameters
        .iter()
        .chain(machine.block_parameters())
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
}

pub fn validate(module: &TerminalModule) -> Result<(), ScalarQualificationError> {
    let catalog = &module.scalar_qualifications;
    if catalog
        .domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
        || catalog.sets.windows(2).any(|pair| pair[0].id >= pair[1].id)
        || catalog
            .fields
            .windows(2)
            .any(|pair| pair[0].field >= pair[1].field)
        || catalog
            .primitives
            .windows(2)
            .any(|pair| pair[0].structural_type >= pair[1].structural_type)
        || catalog.coercions.windows(2).any(|pair| {
            (pair[0].machine, pair[0].edge, pair[0].argument_ordinal)
                >= (pair[1].machine, pair[1].edge, pair[1].argument_ordinal)
        })
        || catalog.returns.windows(2).any(|pair| {
            (pair[0].machine, pair[0].edge, pair[0].domain)
                >= (pair[1].machine, pair[1].edge, pair[1].domain)
        })
        || catalog.sets.iter().any(|set| {
            set.id.is_empty()
                || set.domains.is_empty()
                || set.domains.windows(2).any(|pair| pair[0] >= pair[1])
        })
        || catalog.returns.iter().any(|row| {
            row.origins.is_empty() || row.origins.windows(2).any(|pair| pair[0] >= pair[1])
        })
    {
        return malformed("scalar qualification catalog is not canonical");
    }
    let mut sets: Vec<_> = catalog
        .sets
        .iter()
        .map(|set| set.domains.as_slice())
        .collect();
    sets.sort_unstable();
    if sets.windows(2).any(|pair| pair[0] == pair[1]) {
        return malformed("scalar qualification catalog repeats an equivalent set");
    }
    validate_transfers(module)?;
    for domain in &catalog.domains {
        if !domain.required_memberships_are_well_formed(&catalog.domains) {
            return malformed("scalar domain has ambiguous, cyclic or wrong-carrier prerequisites");
        }
        if module
            .identities
            .text(domain.identity)
            .is_none_or(str::is_empty)
        {
            return malformed("scalar domain has no retained identity");
        }
        if domain
            .establishment_routes
            .as_deref()
            .is_some_and(|routes| !routes.is_empty())
            && domain.predicate.is_none()
        {
            return malformed("routed scalar domain lost its predicate component");
        }
        if let Some(predicate) = &domain.predicate {
            PropositionContext::from_value_types([(predicate.subject, domain.carrier)])
                .and_then(|context| context.validate(&predicate.proposition))
                .map_err(ScalarQualificationError::MalformedProposition)?;
            let mut exact = predicate.subject.get() == 1;
            exact &= predicate.proposition.visit_value_ids(|value| {
                exact &= value == predicate.subject;
            });
            if !exact {
                return malformed("scalar domain predicate escapes its formal subject");
            }
        }
        if domain
            .establishment_routes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|route| {
                module
                    .identities
                    .text(route.identity())
                    .is_none_or(str::is_empty)
                    || module
                        .identities
                        .text(route.declaration().identity())
                        .is_none_or(str::is_empty)
            })
        {
            return malformed("scalar domain lost an issuer identity");
        }
    }
    for set in &catalog.sets {
        if set
            .domains
            .iter()
            .any(|id| !catalog.domains.iter().any(|domain| domain.id == *id))
        {
            return malformed("scalar qualification set names an absent domain");
        }
    }
    for coercion in &catalog.coercions {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == coercion.machine)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar coercion lost its machine",
            ))?;
        let source = values(machine)
            .find(|value| value.id == coercion.source)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar coercion lost its source",
            ))?;
        let destination = values(machine)
            .find(|value| value.id == coercion.destination)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar coercion lost its destination",
            ))?;
        let source_domains = qualification_domains(module, source.qualifications).ok_or(
            ScalarQualificationError::MalformedStructuralFoundation(
                "scalar coercion lost its source set",
            ),
        )?;
        let destination_domains = qualification_domains(module, destination.qualifications).ok_or(
            ScalarQualificationError::MalformedStructuralFoundation(
                "scalar coercion lost its destination set",
            ),
        )?;
        let target = machine
            .blocks
            .iter()
            .find_map(|block| {
                let Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    ..
                } = &block.terminator
                else {
                    return None;
                };
                (*edge == coercion.edge
                    && arguments.get(coercion.argument_ordinal as usize) == Some(&source.id))
                .then_some(*target)
            })
            .and_then(|target| machine.blocks.iter().find(|block| block.id == target))
            .filter(|block| {
                block
                    .parameters
                    .get(coercion.argument_ordinal as usize)
                    .is_some_and(|parameter| parameter.id == destination.id)
            });
        let Some(target) = target else {
            return malformed("scalar coercion is not bound to its exact successor argument");
        };
        if source.scalar_type != destination.scalar_type
            || source.id == destination.id
            || source_domains == destination_domains
            || (!source_domains
                .iter()
                .all(|id| destination_domains.contains(id))
                && !destination_domains
                    .iter()
                    .all(|id| source_domains.contains(id)))
        {
            return malformed(
                "scalar coercion must strictly add or erase same-carrier memberships",
            );
        }
        for id in destination_domains {
            let domain = scalar_domain(module, *id).ok_or(
                ScalarQualificationError::MalformedStructuralFoundation(
                    "scalar coercion domain is absent",
                ),
            )?;
            if domain
                .missing_membership_requirements(&catalog.domains, source_domains)
                .is_none_or(|missing| {
                    missing
                        .iter()
                        .any(|node| !node.has_no_required_memberships())
                })
            {
                return malformed("scalar coercion has unresolved prerequisite memberships");
            }
            if source_domains.contains(id) {
                continue;
            }
            if let Some(predicate) = &domain.predicate {
                let mut expected = predicate.proposition.clone();
                expected.remap_value_ids(|_| destination.id);
                if !module.scalar_block_invariants.iter().any(|invariant| {
                    invariant.machine == machine.id
                        && invariant.header == target.id
                        && retains_conjunct(&invariant.predicate, &expected)
                }) {
                    return malformed(
                        "scalar coercion lost its declared predicate arrival obligation",
                    );
                }
            }
            if (domain.establishment_routes.is_some() || domain.predicate.is_some())
                && !catalog.returns.iter().any(|returned| {
                    returned.machine == machine.id
                        && returned.edge == coercion.edge
                        && returned.source == source.id
                        && returned.result == destination.id
                        && returned.domain == *id
                        && !returned.origins.is_empty()
                        && matches!(returned.site, ScalarReturnSite::Completion { argument_ordinal }
                            if argument_ordinal == coercion.argument_ordinal)
                })
            {
                return malformed(
                    "scalar coercion has no retained checked establishment completion",
                );
            }
        }
    }
    for returned in &catalog.returns {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == returned.machine)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar return lost its machine",
            ))?;
        let result = machine
            .result
            .scalar_ref()
            .into_iter()
            .chain(values(machine))
            .find(|result| result.id == returned.result)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar return lost its result",
            ))?;
        let domain = scalar_domain(module, returned.domain).ok_or(
            ScalarQualificationError::MalformedStructuralFoundation(
                "scalar return lost its domain",
            ),
        )?;
        let bound = match returned.site {
            ScalarReturnSite::NormalExit => machine.result.scalar_ref().is_some_and(|result| result.id == returned.result)
                && machine.blocks.iter().any(|block| matches!(block.terminator,
                    Terminator::Return { edge, value, .. } if edge == returned.edge && value == returned.source)),
            ScalarReturnSite::Completion { argument_ordinal } => machine.blocks.iter().any(|block| {
                let Terminator::Jump { edge, target, ref arguments, .. } = block.terminator else { return false; };
                edge == returned.edge && arguments.get(argument_ordinal as usize) == Some(&returned.source)
                    && machine.blocks.iter().find(|block| block.id == target)
                        .and_then(|block| block.parameters.get(argument_ordinal as usize))
                        .is_some_and(|destination| destination.id == returned.result && destination.id != returned.source)
            }),
        };
        if result.scalar_type != domain.carrier
            || !catalog
                .sets
                .iter()
                .any(|set| set.id == result.qualifications && set.domains.contains(&domain.id))
            || !bound
        {
            return malformed("scalar membership is not bound to its exact normal result");
        }
        if let Some(predicate) = &domain.predicate
            && returned.site == ScalarReturnSite::NormalExit
        {
            let mut expected = predicate.proposition.clone();
            expected.remap_value_ids(|_| result.id);
            let complete = match &expected {
                Proposition::Conjunction(predicates) => predicates.iter().all(|predicate| {
                    machine
                        .contract
                        .ensures
                        .iter()
                        .any(|clause| retains_conjunct(&clause.proposition, predicate))
                }),
                Proposition::Truth => true,
                _ => machine
                    .contract
                    .ensures
                    .iter()
                    .any(|clause| retains_conjunct(&clause.proposition, &expected)),
            };
            if !complete {
                return malformed("scalar result contract lost its domain predicate");
            }
        }
        if let Some(predicate) = &domain.predicate
            && matches!(returned.site, ScalarReturnSite::Completion { .. })
        {
            let mut expected = predicate.proposition.clone();
            expected.remap_value_ids(|_| result.id);
            if !module.scalar_block_invariants.iter().any(|invariant| {
                invariant.machine == machine.id
                    && machine.blocks.iter().any(|block| {
                        block.id == invariant.header
                            && block
                                .parameters
                                .iter()
                                .any(|parameter| parameter.id == result.id)
                    })
                    && retains_conjunct(&invariant.predicate, &expected)
            }) {
                return malformed(
                    "scalar establishment completion lost its declared predicate obligation",
                );
            }
        }
        let source = values(machine)
            .find(|source| source.id == returned.source && source.scalar_type == result.scalar_type)
            .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                "scalar return lost its same-carrier source",
            ))?;
        if matches!(returned.site, ScalarReturnSite::Completion { .. })
            && catalog
                .sets
                .iter()
                .any(|set| set.id == source.qualifications && set.domains.contains(&domain.id))
        {
            return malformed("scalar return completion does not add its named membership");
        }
        for origin in &returned.origins {
            let authorized = match origin {
                ScalarReturnOrigin::Vacuous => {
                    matches!(returned.site, ScalarReturnSite::Completion { .. })
                        && domain.predicate.is_none()
                        && domain.establishment_routes.is_none()
                        && domain.has_no_required_memberships()
                }
                ScalarReturnOrigin::Proved => {
                    matches!(returned.site, ScalarReturnSite::Completion { .. })
                        && domain.predicate.is_some()
                        && domain.establishment_routes.is_none()
                        && domain.has_no_required_memberships()
                }
                ScalarReturnOrigin::Forwarded => catalog.sets.iter().any(|set| {
                    set.id == source.qualifications
                        && domain
                            .missing_membership_requirements(&catalog.domains, &set.domains)
                            .is_some_and(|missing| missing.is_empty())
                }),
                ScalarReturnOrigin::Established { route } => {
                    let declaration = machine.declaration.as_ref().ok_or(
                        ScalarQualificationError::MalformedStructuralFoundation(
                            "scalar issuer lost its checked declaration",
                        ),
                    )?;
                    match domain
                        .establishment_routes
                        .as_deref()
                        .unwrap_or_default()
                        .get(*route as usize)
                    {
                        Some(ScalarDomainEstablishmentRoute::ExactMachine {
                            declaration: issuer,
                            machine_identity,
                        }) => issuer_route_authorizes(
                            module,
                            machine,
                            declaration,
                            IssuerRoute::ExactMachine {
                                issuer,
                                machine_identity: *machine_identity,
                            },
                        ),
                        Some(ScalarDomainEstablishmentRoute::CheckedRequirement {
                            declaration: issuer,
                            requirement_identity,
                        }) => issuer_route_authorizes(
                            module,
                            machine,
                            declaration,
                            IssuerRoute::CheckedRequirement {
                                issuer,
                                requirement_identity: *requirement_identity,
                            },
                        ),
                        _ => false,
                    }
                }
            };
            let prerequisites = if matches!(origin, ScalarReturnOrigin::Established { .. }) {
                let supplied = qualification_domains(module, source.qualifications).ok_or(
                    ScalarQualificationError::MalformedStructuralFoundation(
                        "scalar issuer lost its source qualification set",
                    ),
                )?;
                domain
                    .missing_membership_requirements(&catalog.domains, supplied)
                    .is_some_and(|missing| {
                        missing
                            .iter()
                            .all(|node| node.id == domain.id || node.establishment_routes.is_none())
                    })
            } else {
                true
            };
            if !authorized || !prerequisites {
                let machine_identity = machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("machine:{}", machine.id.get()));
                let source_memberships: Option<Vec<String>> =
                    qualification_domains(module, source.qualifications).map(|memberships| {
                        memberships
                            .iter()
                            .map(|membership| {
                                scalar_domain(module, *membership)
                                    .and_then(|domain| module.identities.text(domain.identity))
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| format!("domain:{}", membership.get()))
                            })
                            .collect()
                    });
                let issuer = match origin {
                    ScalarReturnOrigin::Established { route } => domain
                        .establishment_routes
                        .as_deref()
                        .unwrap_or_default()
                        .get(*route as usize)
                        .map(|route| {
                            let callable = match route {
                                ScalarDomainEstablishmentRoute::ExactMachine {
                                    machine_identity,
                                    ..
                                } => module.identities.text(*machine_identity),
                                _ => None,
                            };
                            (
                                callable,
                                module.identities.text(route.declaration().identity()),
                            )
                        }),
                    _ => None,
                };
                let returned_declaration = machine
                    .declaration
                    .as_ref()
                    .map(|declaration| module.identities.text(declaration.semantic_declaration));
                return Err(ScalarQualificationError::MalformedScalarReturnFoundation {
                    machine: machine_identity,
                    domain: module
                        .identities
                        .text(domain.identity)
                        .unwrap_or("<missing domain identity>")
                        .to_owned(),
                    edge: returned.edge,
                    source: returned.source,
                    result: returned.result,
                    reason: format!(
                        "scalar return origin does not match its source or authorized issuer: site={:?}, origin={origin:?}, source memberships={source_memberships:?}, origin authorized={authorized}, prerequisites satisfied={prerequisites}, issuer={issuer:?}, returned declaration={returned_declaration:?}",
                        returned.site,
                    ),
                });
            }
        }
    }
    for machine in &module.machines {
        for block in &machine.blocks {
            let successors = match &block.terminator {
                Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    ..
                } => vec![(*edge, *target, arguments.as_slice())],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (
                        when_true.edge,
                        when_true.target,
                        when_true.arguments.as_slice(),
                    ),
                    (
                        when_false.edge,
                        when_false.target,
                        when_false.arguments.as_slice(),
                    ),
                ],
                _ => Vec::new(),
            };
            for (edge, target, arguments) in successors {
                let target = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .ok_or(ScalarQualificationError::MalformedStructuralFoundation(
                        "scalar successor target is absent",
                    ))?;
                for (ordinal, (argument, destination)) in
                    arguments.iter().zip(&target.parameters).enumerate()
                {
                    let source = values(machine).find(|value| value.id == *argument).ok_or(
                        ScalarQualificationError::MalformedStructuralFoundation(
                            "scalar successor source is absent",
                        ),
                    )?;
                    if source.scalar_type != destination.scalar_type {
                        return malformed("scalar successor changes its payload carrier");
                    }
                    if source.qualifications != destination.qualifications
                        && !catalog.coercions.iter().any(|row| {
                            row.machine == machine.id
                                && row.edge == edge
                                && row.source == source.id
                                && row.destination == destination.id
                                && row.argument_ordinal as usize == ordinal
                        })
                        && !checked_introduction(
                            module,
                            machine.id,
                            edge,
                            source,
                            destination,
                            ScalarReturnSite::Completion {
                                argument_ordinal: ordinal as u32,
                            },
                        )?
                    {
                        return malformed(
                            "scalar successor changes membership without its explicit cast or checked completion",
                        );
                    }
                    for domain in &catalog.domains {
                        let contains = |qualification| {
                            catalog.sets.iter().any(|set| {
                                set.id == qualification && set.domains.contains(&domain.id)
                            })
                        };
                        if contains(destination.qualifications) && !contains(source.qualifications)
                            && !catalog.coercions.iter().any(|row| row.machine == machine.id && row.edge == edge
                                && row.source == source.id && row.destination == destination.id && row.argument_ordinal as usize == ordinal)
                            && !catalog.returns.iter().any(|row| row.machine == machine.id && row.edge == edge
                                && row.source == source.id && row.result == destination.id && row.domain == domain.id
                                && matches!(row.site, ScalarReturnSite::Completion { argument_ordinal } if argument_ordinal as usize == ordinal))
                        {
                            return malformed("scalar successor introduces membership without its checked return completion");
                        }
                    }
                }
            }
        }
        let Some(result) = machine.result.scalar() else {
            continue;
        };
        let Some(set) = catalog
            .sets
            .iter()
            .find(|set| set.id == result.qualifications)
        else {
            continue;
        };
        for domain in set.domains.iter().filter(|id| {
            catalog.domains.iter().any(|domain| {
                domain.id == **id
                    && domain
                        .required_domain_closure(&catalog.domains)
                        .is_none_or(|required| {
                            required
                                .iter()
                                .any(|node| node.establishment_routes.is_some())
                        })
            })
        }) {
            for block in &machine.blocks {
                if let Terminator::Return { edge, .. } = block.terminator
                    && !catalog.returns.iter().any(|row| {
                        row.machine == machine.id && row.edge == edge && row.domain == *domain
                    })
                {
                    return malformed(
                        "routed scalar return has no establishment or forwarding row",
                    );
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn retains_conjunct(actual: &Proposition, expected: &Proposition) -> bool {
    actual == expected
        || reordered_predicate(actual) == reordered_predicate(expected)
        || matches!(actual, Proposition::Conjunction(children)
        if children.iter().any(|child| retains_conjunct(child, expected)))
}

/// Substituting a field for a scalar subject changes the wire ordering of
/// otherwise identical clauses. Compare only the order-insensitive logical
/// structure; no arithmetic rewriting or new predicate premise is admitted.
fn reordered_predicate(proposition: &Proposition) -> Proposition {
    let mut proposition = proposition.clone();
    match &mut proposition {
        Proposition::Equal(left, right) => {
            if *left > *right {
                std::mem::swap(left, right);
            }
        }
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            for part in parts.iter_mut() {
                *part = reordered_predicate(part);
            }
            parts.sort_unstable();
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            **premise = reordered_predicate(premise);
            **conclusion = reordered_predicate(conclusion);
        }
        _ => {}
    }
    proposition
}
