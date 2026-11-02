//! Compare declared Const tuples with retained and decoded provider obligations.
//! The fixture's ordinary source creates the checked applications; no synthetic
//! compiler custody or source-text assertion supplies the expected instances.

use std::path::Path;

pub(super) struct ExpectedInstance {
    pub value: i128,
    reached: bool,
    requirement: String,
    candidate: String,
    template: String,
    owner: String,
    receiving: String,
    regions: Vec<String>,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
}

pub(super) fn prepare(
    checked: &compiler::CheckedCompilation,
    root: &Path,
) -> Result<Vec<ExpectedInstance>, String> {
    let oracle = root
        .parent()
        .ok_or("provider instance fixture has no parent")?
        .join("expected_provider_instance_arguments.txt");
    if !oracle.is_file() {
        return Ok(Vec::new());
    }
    let values = std::fs::read_to_string(&oracle).map_err(|error| error.to_string())?;
    let program = checked.execution().provider_source.program();
    let typed = &program.typed;
    let mut expected = Vec::new();
    for line in values.lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let [value, reached] = fields.as_slice() else {
            return Err(
                "provider instance oracle needs one Const and called/uncalled disposition".into(),
            );
        };
        let reached = match *reached {
            "called" => true,
            "uncalled" => false,
            _ => return Err("provider instance oracle has unknown reach disposition".into()),
        };
        let value: i128 = value
            .parse()
            .map_err(|error| format!("invalid provider instance Const oracle: {error}"))?;
        let identity = checked_trees::typed_trees::finite_family::integer_const_identity(value);
        let mut matches = Vec::new();
        for application in typed.closed_requirement_applications() {
            let [argument] = application.local_arguments else {
                continue;
            };
            if typed
                .static_const_type_reference_identity(*argument)
                .as_deref()
                != Some(&identity)
            {
                continue;
            }
            for fact in program
                .facts
                .provider_closed_requirements
                .iter()
                .filter(|fact| {
                    fact.template == application.template
                        && fact.closed_requirement == application.signature.symbol
                        && fact.receiving_application == application.trait_application_identity
                        && checked
                            .execution()
                            .selected_provider_provenance
                            .iter()
                            .any(|selected| {
                                selected.provider.provider_type == Some(fact.selecting_declaration)
                            })
                })
            {
                let (template_owner, template) = typed
                    .trait_machine_signature(fact.template)
                    .ok_or("provider instance oracle lost its template declaration")?;
                let declaring = typed
                    .trait_definition_by_symbol(application.declaring_trait)
                    .ok_or("provider instance oracle lost its declaring trait")?;
                let candidate = typed
                    .machine(fact.provider)
                    .ok_or("provider instance oracle lost its checked body")?;
                let owner = typed
                    .exact_owner_nominal_identity(
                        fact.selecting_declaration,
                        checked.sources().exact_toolchain_sources(),
                    )
                    .and_then(|identity| typed.type_identity_text(identity))
                    .ok_or("provider instance oracle lost its choosing owner")?;
                let receiving = typed
                    .type_identity_text(application.portable_trait_application_identity)
                    .ok_or("provider instance oracle lost its portable receiver")?;
                let regions = program.facts.provider_closed_requirements.receiving_regions(fact)
                    .ok_or("provider instance oracle lost its region tuple")?
                    .iter().map(|region| checked_trees::typed_trees::type_identity::portable_lifetime_parameter_identity(typed, *region)
                        .ok_or_else(|| "provider instance oracle has a nonportable region".to_owned()))
                    .collect::<Result<Vec<_>, String>>()?;
                matches.push(ExpectedInstance {
                    value,
                    reached,
                    requirement: typed
                        .trait_requirement_callable_identity_text(declaring, application.signature),
                    candidate: typed
                        .machine_callable_identity_text(candidate)
                        .ok_or("provider instance oracle has no body identity")?,
                    template: typed
                        .trait_requirement_callable_identity_text(template_owner, template),
                    owner: owner.to_owned(),
                    receiving: receiving.to_owned(),
                    regions,
                    package: typed.symbols.symbol_package_identity(fact.template),
                });
            }
        }
        if matches.len() != 1 {
            return Err(format!(
                "Const={value} needs exactly one original checked provider obligation, got {}",
                matches.len()
            ));
        }
        expected.extend(matches);
    }
    Ok(expected)
}

pub(super) fn inspect(
    expected: &[ExpectedInstance],
    module: &terminal_psi::TerminalModule,
    selection: &backend_plan::TerminalTargetProviderSelection,
    target: &str,
    roots: Option<&[terminal_psi::ProofDeclarationId]>,
) -> Result<Option<terminal_psi::ProviderCandidateConformance>, String> {
    if expected.is_empty() {
        return Ok(None);
    }
    let roots = roots.ok_or("provider instance oracle requires actual deployment roots")?;
    let scope = &module.scalar_applications.proof_scope;
    let declaration = |callable| {
        scope
            .callable_declaration(callable)
            .ok_or_else(|| "provider instance oracle lost a canonical proof declaration".to_owned())
    };
    let edges = selection
        .checked_candidates
        .iter()
        .map(|selected| {
            Ok((
                declaration(terminal_psi::TargetMachineCallable::Boundary(
                    selected.boundary,
                ))?,
                declaration(terminal_psi::TargetMachineCallable::Machine(
                    selected.candidate,
                ))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let reached = scope
        .applications()
        .invocation_reachability(target, &edges, roots);
    let mut uncalled = None;
    for original in expected {
        let text = |identity| module.identities.text(identity).unwrap_or_default();
        let mut debt = module.provider_instance_requirements.iter().filter(|row| {
            text(row.template) == original.template
                && row.package == original.package
                && row
                    .selecting_declaration
                    .as_ref()
                    .is_some_and(|owner| text(owner.identity()) == original.owner)
        });
        let row = debt.next().ok_or_else(|| {
            format!(
                "Const={} lost its independent obligation roster",
                original.value
            )
        })?;
        if debt.next().is_some() || row.required_instances.len() != expected.len() {
            return Err(
                "provider obligation roster changed its complete original tuple count".into(),
            );
        }
        let matches = row
            .required_instances
            .iter()
            .filter(|required| {
                text(required.requirement_identity) == original.requirement
                    && text(required.candidate_identity) == original.candidate
                    && text(required.receiving_application) == original.receiving
                    && required
                        .receiving_region_arguments
                        .iter()
                        .map(|region| text(*region))
                        .eq(original.regions.iter().map(String::as_str))
            })
            .collect::<Vec<_>>();
        let [required] = matches.as_slice() else {
            return Err(format!(
                "Const={} changed its exact checked requirement/body/receiver obligation",
                original.value
            ));
        };
        let mut candidates = module.provider_candidates.iter().filter(|candidate| {
            candidate.requirement_identity == required.requirement_identity
                && candidate.candidate_identity == required.candidate_identity
                && Some(&candidate.provider_declaration) == row.selecting_declaration.as_ref()
                && candidate.source_requirement_identity == row.template
                && candidate.source_candidate_identity == row.source_candidate_identity
                && candidate.receiving_application == required.receiving_application
                && candidate.receiving_region_arguments == required.receiving_region_arguments
                && selection.checked_candidates.iter().any(|selected| {
                    selected.boundary == candidate.boundary
                        && selected.candidate == candidate.candidate
                        && selected.provider_declaration == original.owner
                })
        });
        let candidate = candidates
            .next()
            .ok_or("checked obligation has no exact canonical selected realization")?;
        if candidates.next().is_some() {
            return Err("checked obligation has competing canonical realizations".into());
        }
        let declarations = selection
            .plans
            .iter()
            .filter(|plan| plan.provider_declaration.as_deref() == Some(&original.owner));
        let mut covered = false;
        for declaration in declarations {
            for projected in backend_plan::selected_provider_candidate_rows(
                &declaration.plan,
                &original.owner,
                module,
            )? {
                covered |= projected
                    .candidate_positions
                    .iter()
                    .any(|position| &module.provider_candidates[*position] == candidate);
            }
        }
        if !covered {
            return Err(
                "provider obligation escaped the complete canonical candidate projection".into(),
            );
        }
        let actual_reach = reached.calls.contains(&declaration(
            terminal_psi::TargetMachineCallable::Boundary(candidate.boundary),
        )?);
        if actual_reach != original.reached {
            return Err(format!(
                "Const={} has a different runtime invocation scope",
                original.value
            ));
        }
        if !original.reached {
            uncalled = Some(candidate.clone());
        }
    }
    Ok(uncalled)
}
