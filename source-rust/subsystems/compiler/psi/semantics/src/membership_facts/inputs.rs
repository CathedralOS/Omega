//! An input membership travels with one argument. The declaration's formal
//! selects the argument, while the call site's path selects the original storage.

use super::*;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryParameterKind, MachineContract, StructuralArgument,
};

#[derive(Clone, Copy)]
pub(super) struct Arguments<'a> {
    pub machine: &'a TerminalMachine,
    pub values: &'a [ValueId],
    pub places: &'a [StructuralArgument],
}

pub(super) fn of_boundary(
    module: &TerminalModule,
    declaration: &BoundaryMachineDeclaration,
    arguments: Option<Arguments<'_>>,
) -> Judged<Vec<Fact>> {
    if declaration.membership_requires.is_empty() {
        return Ok(Vec::new());
    }
    let scalar = declaration.scalar_contract_parameters().ok_or_else(|| {
        unavailable(
            None,
            "boundary input membership has no complete scalar parameter list",
        )
    })?;
    bind(
        module,
        &scalar,
        &declaration.structural_parameters,
        &declaration.membership_requires,
        arguments,
    )
}

pub(super) fn of_contract(
    module: &TerminalModule,
    declaration: &MachineContract,
    arguments: Option<Arguments<'_>>,
) -> Judged<Vec<Fact>> {
    bind(
        module,
        &declaration.parameters,
        &declaration.structural_parameters,
        &declaration.membership_requires,
        arguments,
    )
}

fn bind(
    module: &TerminalModule,
    values: &[ValueDeclaration],
    places: &[StructuralParameterDeclaration],
    rows: &[RuntimeDomainMembership],
    arguments: Option<Arguments<'_>>,
) -> Judged<Vec<Fact>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let machine = arguments.map(|args| args.machine.id);
    validate_roster(machine, rows)?;
    let invalid = || {
        unavailable(
            machine,
            "input membership must name its exact supplied formal",
        )
    };
    if arguments
        .is_some_and(|args| args.values.len() != values.len() || args.places.len() != places.len())
    {
        return Err(invalid());
    }
    rows.iter()
        .map(|row| {
            let (_, _, binding) = formal_binding(module, values, places, row, arguments)?;
            binding.membership(module, machine, row)
        })
        .collect()
}

pub(super) fn formal_binding(
    module: &TerminalModule,
    values: &[ValueDeclaration],
    places: &[StructuralParameterDeclaration],
    row: &RuntimeDomainMembership,
    arguments: Option<Arguments<'_>>,
) -> Judged<(BoundaryParameterKind, usize, SubjectBinding)> {
    let machine = arguments.map(|args| args.machine.id);
    let invalid = || {
        unavailable(
            machine,
            "input membership must name its exact supplied formal",
        )
    };
    match row {
        RuntimeDomainMembership::Scalar {
            subject: ScalarTerm::Value { id, .. },
            ..
        } => {
            let (index, formal) = values
                .iter()
                .enumerate()
                .find(|(_, formal)| formal.id == *id)
                .ok_or_else(invalid)?;
            Ok((
                BoundaryParameterKind::Scalar,
                index,
                SubjectBinding::Scalar {
                    formal: *id,
                    carrier: formal.scalar_type,
                    actual: arguments.map_or(*id, |args| args.values[index]),
                },
            ))
        }
        RuntimeDomainMembership::Structural { root, .. }
        | RuntimeDomainMembership::Scalar {
            subject:
                ScalarTerm::IntegerField { root, .. }
                | ScalarTerm::BooleanField { root, .. }
                | ScalarTerm::IeeeFloatField { root, .. },
            ..
        } => {
            let (index, formal) = places
                .iter()
                .enumerate()
                .find(|(_, formal)| formal.place == *root)
                .ok_or_else(invalid)?;
            let location = match arguments {
                None => DescriptorLocation::whole(*root),
                Some(args) => {
                    let argument = &args.places[index];
                    let carrier = args
                        .machine
                        .structural_place_type(argument.place)
                        .ok_or_else(invalid)?;
                    descriptor_location(
                        &module.types.structural_types,
                        argument.place,
                        carrier,
                        &argument.path,
                        None,
                    )
                    .ok_or_else(invalid)?
                }
            };
            Ok((
                BoundaryParameterKind::Structural,
                index,
                SubjectBinding::Structural {
                    formal: *root,
                    carrier: formal.structural_type,
                    actual: location,
                },
            ))
        }
        _ => Err(invalid()),
    }
}

pub(super) fn validate_header(
    module: &TerminalModule,
    values: &[ValueDeclaration],
    places: &[StructuralParameterDeclaration],
    rows: &[RuntimeDomainMembership],
    requires: &[semantic_vocabulary::Proposition],
) -> Judged<()> {
    validate_roster(None, rows)?;
    for row in rows {
        let (_, _, binding) = formal_binding(module, values, places, row, None)?;
        let _ = binding.membership(module, None, row)?;
        validate_predicates(
            module,
            None,
            &binding,
            std::slice::from_ref(row),
            requires.iter(),
        )?;
    }
    Ok(())
}

pub(super) fn validate_contract(module: &TerminalModule, contract: &MachineContract) -> Judged<()> {
    validate_header(
        module,
        &contract.parameters,
        &contract.structural_parameters,
        &contract.membership_requires,
        &contract.requires,
    )
}

/// A caller that meets the published input contract also meets the selected
/// body's preconditions. Rebase each requested witness onto the public
/// signature's *same argument*, independently of machine-local IDs.
pub(crate) fn refinement(
    module: &TerminalModule,
    required: crate::calls::callable_refinement::CallableContractView<'_>,
    selected: crate::calls::callable_refinement::CallableContractView<'_>,
) -> Result<(), &'static str> {
    entails(
        module,
        Formals {
            values: required.scalar_parameters,
            places: required.structural_parameters,
            rows: &required.proof.membership_requires,
            scalar_domains: &[],
        },
        Formals {
            values: selected.scalar_parameters,
            places: selected.structural_parameters,
            rows: &selected.proof.membership_requires,
            scalar_domains: &[],
        },
    )
}

pub(super) struct Formals<'a> {
    pub values: &'a [ValueDeclaration],
    pub places: &'a [StructuralParameterDeclaration],
    pub rows: &'a [RuntimeDomainMembership],
    pub scalar_domains: &'a [terminal_psi::ScalarDomainRequirement],
}

impl<'a> From<&'a MachineContract> for Formals<'a> {
    fn from(contract: &'a MachineContract) -> Self {
        Self {
            values: &contract.parameters,
            places: &contract.structural_parameters,
            rows: &contract.membership_requires,
            scalar_domains: &[],
        }
    }
}

pub(super) fn entails(
    module: &TerminalModule,
    required: Formals<'_>,
    selected: Formals<'_>,
) -> Result<(), &'static str> {
    if selected.rows.is_empty() {
        return Ok(());
    }
    let mut provided = Facts::default();
    let exact = bind(
        module,
        required.values,
        required.places,
        required.rows,
        None,
    )
    .map_err(|e| e.detail)?;
    let scalar = scalar_declarations(module, required.values).map_err(|e| e.detail)?;
    for fact in exact.into_iter().chain(scalar) {
        provided.insert(fact, Some(BTreeSet::new()));
    }
    for row in required.scalar_domains {
        let value = required
            .values
            .get(row.argument_index as usize)
            .ok_or("declared domain lies outside the public scalar parameters")?;
        provided.insert(
            Fact {
                subject: Subject::Value(value.id),
                domain: Domain::Scalar(row.domain),
            },
            Some(BTreeSet::new()),
        );
    }
    for formal in required.places {
        let facts = structural_declaration(
            module,
            None,
            formal.place,
            formal.structural_type,
            &formal.qualifications,
            &formal.projected_qualifications,
        )
        .map_err(|e| e.detail)?;
        for fact in facts {
            provided.insert(fact, Some(BTreeSet::new()));
        }
    }
    for row in selected.rows {
        let (_, index, mut binding) =
            formal_binding(module, selected.values, selected.places, row, None)
                .map_err(|e| e.detail)?;
        match &mut binding {
            SubjectBinding::Scalar { actual, .. } => {
                *actual = required
                    .values
                    .get(index)
                    .ok_or("input refinement lost its scalar argument")?
                    .id;
            }
            SubjectBinding::Structural { actual, .. } => {
                *actual = DescriptorLocation::whole(
                    required
                        .places
                        .get(index)
                        .ok_or("input refinement lost its structural argument")?
                        .place,
                );
            }
            SubjectBinding::Unit => return Err("input refinement names the unit result"),
        }
        let query = binding
            .membership(module, None, row)
            .map_err(|e| e.detail)?;
        if let (Domain::Scalar(_), Subject::Storage { location, .. }) =
            (query.domain, &query.subject)
            && !query.has_case_projection()
            && let Some(promised) = required.places.get(index)
            && let Some(domains) =
                declared_scalar_domains(module, promised.structural_type, location)
        {
            for domain in domains {
                provided.insert(
                    Fact {
                        subject: query.subject.clone(),
                        domain: Domain::Scalar(*domain),
                    },
                    Some(BTreeSet::new()),
                );
            }
        }
        if !provided.proves(module, &query) {
            return Err(
                "selected callable requires a routed input membership absent from its public contract",
            );
        }
    }
    Ok(())
}
