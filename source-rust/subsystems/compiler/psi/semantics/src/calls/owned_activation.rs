//! A saved program must not turn its own bodyless declaration into a park
//! operation, or use one publication to request a different reply.
mod demand;
mod preparation;
mod resume;
mod step;
use application_identity::{
    conformance_identity::MachineApplicationCommitment,
    type_identity::{
        encoding::{atom, byte_atom, compound},
        schema_region,
        vocabulary::{IndexForm, Label, NameForm, OwnerForm, TypeForm},
    },
};
pub use demand::caller_demand;
use language_core::sealed_intrinsics::ActivationOperation;
pub use preparation::PreparationShape;
pub use resume::{ResumeShape, resume_shape};
pub use step::{StepShape, step_shape};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryParameterKind, StructuralAccess, StructuralMultiplicity,
    StructuralTypeShape, TerminalModule,
};

fn core_owner(revision: &[u8; 32], path: &str) -> String {
    compound(
        NameForm::Nominal,
        [
            byte_atom(OwnerForm::ToolchainSource, revision),
            atom(Label::Path, path),
        ],
    )
}

fn source_owner(path: &str) -> String {
    core_owner(
        &language_core::bundled_files::core_revision::ACTIVATION,
        path,
    )
}

/// The issued request carries a reference, not an encoded identifier.
/// Both engines follow this field to the exact lending place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParkShape {
    pub anchor: semantic_vocabulary::StructuralFieldId,
    pub anchor_identity: terminal_psi::TerminalIdentityIndex,
    pub reply: terminal_psi::TerminalIdentityIndex,
}

fn toolchain_name(name: &str) -> String {
    use application_identity::type_identity::encoding::bare;
    compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(Label::Path, name)],
    )
}

/// These results transfer the original backing through distinct cases.
pub const fn transfers_owner(execution: terminal_psi::CheckedBoundaryExecution) -> bool {
    use ActivationOperation::{Prepare, Resume, StepFixed};
    matches!(
        execution,
        terminal_psi::CheckedBoundaryExecution::OwnedActivation(Prepare | StepFixed | Resume)
    )
}

/// A field returned to the caller preserves only the facts already available
/// on that exact input and projection. The sealed operation mints no domain.
fn forwarded_qualifications(
    output: &[terminal_psi::StructuralPathQualification],
    selected: semantic_vocabulary::StructuralCaseId,
    fields: &[(
        terminal_psi::TerminalIdentityIndex,
        Option<&terminal_psi::StructuralParameterDeclaration>,
    )],
) -> bool {
    use terminal_psi::StructuralPathSegment::{Case, Field};
    output.iter().all(|proof| {
        let [Case(case), Field(field), tail @ ..] = proof.path.as_slice() else {
            return false;
        };
        *case == selected
            && fields.iter().any(|(id, source)| {
                *id == *field
                    && source.is_some_and(|source| {
                        (tail.is_empty() && source.qualifications.contains(&proof.domain))
                            || source
                                .projected_qualifications
                                .iter()
                                .any(|q| q.path == tail && q.domain == proof.domain)
                    })
            })
    })
}

/// A forwarded field may store a structural reference to the declaration or
/// name its physical carrier explicitly on the inline field.
pub fn field_matches_carrier(
    module: &TerminalModule,
    field: &terminal_psi::StructuralFieldDeclaration,
    expected: semantic_vocabulary::StructuralTypeId,
) -> bool {
    if field.relevance.is_erased() {
        return false;
    }
    if field.inline_carrier.is_some() {
        return crate::inline_carrier_declaration(module.types.structural_types.iter(), field)
            .is_some_and(|carrier| carrier.id == expected);
    }
    if let terminal_psi::StructuralFieldType::Structural(actual) = field.field_type {
        return actual == expected;
    }
    module.structural_type(expected).is_some_and(|carrier| {
        field.field_type.canonical_leaf_shape().as_ref() == Some(&carrier.shape)
    })
}

fn declared_operands<'a>(
    module: &'a TerminalModule,
    ty: terminal_psi::TerminalIdentityIndex,
    family: &str,
) -> Result<Vec<&'a str>, &'static str> {
    let identity = module
        .identities
        .text(ty)
        .ok_or("activation declaration lost a type")?;
    application_identity::type_identity::declared_application_operands(
        identity,
        &toolchain_name(family),
    )
    .map_err(|_| "activation declaration has malformed type operands")?
    .ok_or("activation declaration names a different type family")
}

/// A source declaration can spell the plan as a variant or as an evaluated
/// constant. Match each type's operands against the closed worker selection.
fn declares_worker(
    module: &TerminalModule,
    selected: terminal_psi::TerminalPreparedActivationEntry,
    ty: terminal_psi::TerminalIdentityIndex,
    family: &str,
    first_argument_region: u32,
    reply: Option<&str>,
) -> Result<bool, &'static str> {
    let operands = declared_operands(module, ty, family)?;
    let (arguments, result, entry, storage) = match (operands.as_slice(), reply) {
        ([backing, arguments, out, entry, plan], None) if *backing == schema_region(0) => {
            (*arguments, *out, *entry, *plan)
        }
        ([backing, permit, arguments, out, value, entry, plan], Some(reply))
            if *backing == schema_region(0) && *permit == schema_region(1) && *value == reply =>
        {
            (*arguments, *out, *entry, *plan)
        }
        _ => return Ok(false),
    };
    let worker = module
        .machine(selected.entry_machine)
        .and_then(|body| body.declaration.as_ref())
        .ok_or("activation lost its selected worker declaration")?;
    let method = module
        .boundary_machine(selected.preparation)
        .and_then(|boundary| boundary.declaration.as_ref())
        .ok_or("activation lost its checked preparation")?;
    let [
        terminal_psi::ClosedReachArgument::Type(a),
        terminal_psi::ClosedReachArgument::Type(r),
        ..,
    ] = method.method_arguments.as_slice()
    else {
        return Ok(false);
    };
    let checked = |actual, id, first| {
        application_identity::type_identity::schema_carrier_region_count(
            actual,
            module.identities.text(id)?,
            first,
        )
    };
    let Some(count) = checked(arguments, *a, Some(first_argument_region)) else {
        return Ok(false);
    };
    let first_result_region = first_argument_region
        .checked_add(count)
        .ok_or("activation has too many declared regions")?;
    Ok(checked(result, *r, Some(first_result_region)).is_some()
        && declared_worker_operand(module, worker).as_deref() == Some(entry)
        && selected.storage.matches_type_operand(storage))
}

fn declared_worker_operand(
    module: &TerminalModule,
    worker: &terminal_psi::CheckedMachineDeclaration,
) -> Option<String> {
    let text = |id| module.identities.text(id).filter(|s| !s.is_empty());
    Some(compound(
        IndexForm::StaticApplication,
        [
            atom(IndexForm::ConstName, text(worker.machine.identity())?),
            atom(IndexForm::ConstName, text(worker.state.identity())?),
            atom(IndexForm::ConstName, text(worker.callable)?),
        ],
    ))
}

/// A preparation borrows storage; it does not call the worker. Verify the
/// worker selection here, not by searching for a matching machine at runtime.
/// The prepare signature and its custody rules are separate obligations.
pub fn prepare_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<PreparationShape, &'static str> {
    use terminal_psi::{
        ClosedReachArgument as Arg, TerminalActivationStorage, TerminalPreparedActivationEntry,
    };
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("prepare has no checked declaration")?;
    let source = &checked.source;
    let closed = source
        .specialization
        .as_ref()
        .ok_or("prepare is not specialized")?;
    let text = |id| {
        module
            .identities
            .text(id)
            .filter(|s| !s.is_empty())
            .ok_or("prepare has an unresolved identity")
    };
    if checked.execution
        != terminal_psi::CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Prepare)
        || boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
        || boundary.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || !boundary.published_synchronous_entries.is_empty()
        || boundary.owning_service.is_some()
        || boundary.service_reach_is_installation_bound
        || boundary.attachment.is_some()
        || boundary.numeric_intrinsic.is_some()
        || !only_declared_inputs(boundary)
        || !boundary.unconditional_termination
        || text(closed.template.identity())? != source_owner("prepare")
        || text(closed.template_entry.identity())? != source_owner("prepare::entry")
        || closed.capture.is_some()
        || source.machine != boundary.owner
        || source.callable != boundary.identity
        || checked.service_declaration != source.contract
        || source.published_may_suspend
        || source.published_may_block
    {
        return Err("prepare differs from its sealed core declaration");
    }
    let [
        Arg::Type(args),
        Arg::Type(out),
        Arg::Machine {
            identity,
            contract_commitment,
        },
        Arg::Const(plan),
    ] = checked.method_arguments.as_slice()
    else {
        return Err("prepare needs its exact argument, result, worker and storage binders");
    };
    if worker_application_commitment(
        text(closed.template_callable)?,
        &[text(*args)?, text(*out)?],
        text(*identity)?,
        text(*plan)?,
    )? != closed.commitment
    {
        return Err("prepare selection differs from its closed application");
    }
    // Loan replay precedes the suspension-frontier report. The declaration
    // and contract in this already-closed operand identify the exact code
    // independently of the later frame and storage measurements.
    let mut workers = module.machines.iter().filter_map(|machine| {
        let worker = machine.declaration.as_ref()?;
        worker
            .matches_selection(*identity, contract_commitment)
            .then_some((machine.id, worker))
    });
    let (entry_machine, worker) = workers.next().ok_or("prepare lost its selected worker")?;
    if workers.next().is_some() {
        return Err("prepare selects more than one body for the same checked declaration");
    }
    let selected = TerminalPreparedActivationEntry {
        preparation: boundary.id,
        entry_machine,
        storage: TerminalActivationStorage::from_core_argument(text(*plan)?)
            .ok_or("prepare did not select its declared storage plan")?,
    };
    let matches = |id, argument, first| {
        module
            .identities
            .text(id)
            .zip(module.identities.text(argument))
            .is_some_and(|(declared, carrier)| {
                application_identity::type_identity::schema_carrier_region_count(
                    declared, carrier, first,
                )
                .is_some()
            })
    };
    if !matches!(worker.parameter_types.as_slice(), [input] if matches(*input, *args, None))
        || !matches(worker.result_type, *out, None)
        || source.parameter_types.len() != 2
        || !matches(source.parameter_types[0], *args, Some(1))
    {
        return Err("prepare's argument and result binders differ from the worker's signature");
    }
    let backing = compound(
        TypeForm::MutableReference,
        [
            compound(
                TypeForm::Slice,
                [application_identity::type_identity::compiler_type_identity(
                    "u8",
                )],
            ),
            schema_region(0),
        ],
    );
    if text(source.parameter_types[1])? != backing
        || !declares_worker(
            module,
            selected,
            source.result_type,
            "ActivationPreparation",
            1,
            None,
        )?
    {
        return Err("prepare lost its declared backing or worker types");
    }
    preparation::result_shape(module, boundary, selected)
}

/// Recover the single published preparation for an operation received without a
/// live owner. Runtime calls additionally check the preparation held by that owner.
pub fn preparation_for(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<semantic_vocabulary::BoundaryMachineId, &'static str> {
    let mut candidates = module
        .interfaces
        .boundary_machines
        .iter()
        .filter_map(|preparation| {
            same_preparation(module, boundary, preparation).then_some(preparation.id)
        });
    let first = candidates
        .next()
        .ok_or("activation has no matching checked preparation")?;
    if candidates.next().is_some() {
        return Err("activation has more than one matching preparation");
    }
    Ok(first)
}

fn same_preparation(
    module: &TerminalModule,
    operation: &BoundaryMachineDeclaration,
    preparation: &BoundaryMachineDeclaration,
) -> bool {
    use terminal_psi::{
        CheckedBoundaryExecution::OwnedActivation, ClosedReachArgument, TerminalActivationStorage,
    };
    let (Some(operation), Some(preparation)) = (&operation.declaration, &preparation.declaration)
    else {
        return false;
    };
    let [args, result, worker, plan] = preparation.method_arguments.as_slice() else {
        return false;
    };
    preparation.execution == OwnedActivation(ActivationOperation::Prepare)
        && match (operation.execution, operation.method_arguments.as_slice()) {
            (OwnedActivation(ActivationOperation::StepFixed), [a, r, w]) => {
                (a, r, w) == (args, result, worker)
                    && matches!(plan, ClosedReachArgument::Const(plan) if
                        module.identities.text(*plan).and_then(TerminalActivationStorage::from_core_argument)
                            == Some(TerminalActivationStorage::Fixed))
            }
            (OwnedActivation(ActivationOperation::Resume), [a, r, _, w, p]) => {
                (a, r, w, p) == (args, result, worker, plan)
            }
            _ => false,
        }
}

fn worker_application_commitment(
    callable: &str,
    types: &[&str],
    worker: &str,
    plan: &str,
) -> Result<[u8; 32], &'static str> {
    let (_, value) = terminal_psi::TerminalActivationStorage::decode_core_argument(plan)
        .ok_or("prepare storage is not the exact core enum value")?;
    let mut commitment = MachineApplicationCommitment::new(callable)?;
    for ty in types {
        commitment.type_argument(ty)?;
    }
    commitment.machine_argument(worker)?;
    commitment.const_argument(&value)?;
    Ok(commitment.finish())
}

fn named(name: &str) -> String {
    compound(TypeForm::Named, [atom(Label::Name, name)])
}

// These operations transport their inputs. None of them is a provider for
// additional regions, writes through reference parameters, or arbitrary facts.
fn only_declared_inputs(boundary: &BoundaryMachineDeclaration) -> bool {
    boundary
        .requires
        .iter()
        .cloned()
        .eq(terminal_psi::StructuralDomainRequirement::declared_roots(
            &boundary.structural_parameters,
        ))
        && boundary.normal_qualifications.is_empty()
        && boundary.scalar_domain_requirements.is_empty()
        && boundary.membership_ensures.is_empty()
        && boundary.scalar_requires.is_empty()
        && boundary.scalar_ensures.is_empty()
        && boundary.reference_outputs.is_empty()
        && boundary.program_local_root_introductions.is_empty()
        && boundary.content_guarantees.is_empty()
}

fn publication_types(role: &str, reply: &str, region: usize) -> (String, String, String) {
    use application_identity::type_identity::vocabulary::ConstraintForm;
    use language_core::bundled_files::core_revision;
    let carrier = named(&toolchain_name("Publication"));
    let family = "Publication::Issued";
    let source_domain = compound(
        ConstraintForm::DeclaredDomain,
        [
            atom(Label::Name, &toolchain_name(family)),
            named(&toolchain_name(role)),
            reply.to_owned(),
        ],
    );
    let declared = compound(
        TypeForm::Constrained,
        [
            compound(
                TypeForm::Generic,
                [
                    atom(Label::Name, &toolchain_name("Publication")),
                    schema_region(region),
                ],
            ),
            compound(
                ConstraintForm::DeclaredDomain,
                [atom(Label::Name, &source_domain)],
            ),
        ],
    );
    let owner = compound(
        Label::Declaration,
        [
            atom(Label::Path, family),
            compound(Label::Owner, [carrier.clone()]),
        ],
    );
    let issued = compound(
        TypeForm::Generic,
        [
            atom(Label::Name, &core_owner(&core_revision::REPLY, &owner)),
            named(&core_owner(&core_revision::REPLY, role)),
            reply.to_owned(),
        ],
    );
    (carrier, declared, issued)
}

fn stored_reply(reply: &str) -> std::borrow::Cow<'_, str> {
    application_identity::type_identity::compiler_type_atom(reply).map_or_else(
        || reply.into(),
        |name| compound(TypeForm::Named, [atom(Label::Name, &name)]).into(),
    )
}

fn reply_result_matches(
    module: &TerminalModule,
    result: &terminal_psi::BoundaryMachineResult,
    reply: &str,
) -> bool {
    use semantic_vocabulary::ScalarQualificationSetId;
    use terminal_psi::BoundaryMachineResult;
    match result {
        BoundaryMachineResult::Unit => {
            reply == application_identity::type_identity::encoding::bare(TypeForm::Unit)
        }
        BoundaryMachineResult::Structural(result) => module
            .structural_type(result.structural_type)
            .is_some_and(|ty| {
                result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && ty.multiplicity == result.multiplicity
                    && module.identities.text(ty.identity) == Some(stored_reply(reply).as_ref())
            }),
        BoundaryMachineResult::Scalar(result) => {
            if result.qualifications != ScalarQualificationSetId::ZERO {
                return false;
            }
            scalar_identity(result.scalar_type).as_deref() == Some(reply)
        }
    }
}

fn scalar_identity(scalar: semantic_vocabulary::ScalarType) -> Option<String> {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, ScalarType};
    let atom = match scalar {
        ScalarType::Boolean => "bool".into(),
        ScalarType::Integer(i) if i.is_address() => "address".into(),
        ScalarType::Integer(i) => format!(
            "{}{}",
            if i.sign() == IntegerSign::Signed {
                'i'
            } else {
                'u'
            },
            i.bits()?
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".into(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".into(),
        ScalarType::Callable(_) => return None,
    };
    Some(application_identity::type_identity::compiler_type_identity(
        &atom,
    ))
}

fn checked_reply<'m>(
    module: &'m TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<&'m str, &'static str> {
    use terminal_psi::{
        BoundaryCompletion, BoundaryRequirementKind, CheckedBoundaryExecution, ClosedReachArgument,
    };
    let operation = ActivationOperation::Park;
    let name = "park";
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("park has no checked declaration")?;
    let source = &checked.source;
    let text = |id| module.identities.text(id);
    let specialized = source
        .specialization
        .as_ref()
        .ok_or("park has no closed reply type")?;
    if checked.execution != CheckedBoundaryExecution::OwnedActivation(operation)
        || text(specialized.template.identity()) != Some(source_owner(name).as_str())
        || text(specialized.template_entry.identity())
            != Some(source_owner(&format!("{name}::entry")).as_str())
        || specialized.capture.is_some()
        || source.machine != boundary.owner
        || source.callable != boundary.identity
        || checked.service_declaration != source.contract
        || boundary.kind != BoundaryRequirementKind::Machine
        || boundary.completion != BoundaryCompletion::NormalReturn
        || !source.published_may_suspend
        || source.published_may_block
        || boundary.unconditional_termination
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || !boundary.published_synchronous_entries.is_empty()
        || boundary.owning_service.is_some()
        || boundary.service_reach_is_installation_bound
        || boundary.attachment.is_some()
        || boundary.numeric_intrinsic.is_some()
        || source.contract == [0; 32]
        || !only_declared_inputs(boundary)
        || !boundary.result_reference_regions.is_empty()
    {
        return Err("park differs from its sealed source or suspension contract");
    }
    let [ClosedReachArgument::Type(reply)] = checked.method_arguments.as_slice() else {
        return Err("owned activation requires one closed reply type");
    };
    let reply = text(*reply).ok_or("owned activation lacks its closed reply identity")?;
    let template = text(specialized.template_callable).ok_or("park lacks its template callable")?;
    let mut commitment = MachineApplicationCommitment::new(template)?;
    commitment.type_argument(reply)?;
    if specialized.commitment != commitment.finish() {
        return Err("park reply differs from its closed application");
    }
    Ok(reply)
}

pub fn validate_park(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    park_shape(module, boundary).map(drop)
}

pub fn park_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<ParkShape, &'static str> {
    let reply = checked_reply(module, boundary)?;
    let source = &boundary
        .declaration
        .as_ref()
        .ok_or("park lacks its declaration")?
        .source;
    let text = |id| module.identities.text(id);
    if text(source.result_type) != Some(reply)
        || !reply_result_matches(module, &boundary.result, reply)
    {
        return Err("park result differs from its reply");
    }
    let [request] = boundary.structural_parameters.as_slice() else {
        return Err("park needs one owned request");
    };
    let [request_declared] = source.parameter_types.as_slice() else {
        return Err("park lacks its sole declared request");
    };
    if !boundary.scalar_parameters.is_empty()
        || boundary.parameter_order != [BoundaryParameterKind::Structural]
    {
        return Err("park consumes the request alone");
    }
    publication_shape(
        module,
        request,
        *request_declared,
        source.result_type,
        "RequestRole",
        0,
    )
}

/// Check the caller's one-shot answer against the reply binder of the actual
/// resume declaration. A handle to the same slot with the request role is
/// not an answer permit. This check does not establish the parked owner.
pub fn resume_permit(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<ParkShape, &'static str> {
    use terminal_psi::{CheckedBoundaryExecution, ClosedReachArgument as Arg};
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("resume has no declaration")?;
    let source = &checked.source;
    let closed = source
        .specialization
        .as_ref()
        .ok_or("resume has no specialisation")?;
    let [
        Arg::Type(args),
        Arg::Type(out),
        Arg::Type(reply),
        Arg::Machine { identity, .. },
        Arg::Const(plan),
    ] = checked.method_arguments.as_slice()
    else {
        return Err("resume lost its closed reply type");
    };
    let [_, declared, value] = source.parameter_types.as_slice() else {
        return Err("resume needs owner, permit and value");
    };
    if checked.execution != CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Resume)
        || module.identities.text(closed.template.identity())
            != Some(source_owner("resume").as_str())
        || module.identities.text(closed.template_entry.identity())
            != Some(source_owner("resume::entry").as_str())
        || closed.capture.is_some()
        || source.machine != boundary.owner
        || source.callable != boundary.identity
        || checked.service_declaration != source.contract
        || source.published_may_suspend
        || source.published_may_block
        || boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
        || boundary.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || !boundary.published_synchronous_entries.is_empty()
        || boundary.owning_service.is_some()
        || boundary.service_reach_is_installation_bound
        || boundary.attachment.is_some()
        || boundary.numeric_intrinsic.is_some()
        || !only_declared_inputs(boundary)
        || !boundary.unconditional_termination
        || boundary.parameter_order.get(1) != Some(&BoundaryParameterKind::Structural)
        || module.identities.text(*value) != module.identities.text(*reply)
    {
        return Err("resume has a different operation or value type");
    }
    let text = |id| {
        module
            .identities
            .text(id)
            .filter(|s| !s.is_empty())
            .ok_or("resume has an unresolved binder")
    };
    if worker_application_commitment(
        text(closed.template_callable)?,
        &[text(*args)?, text(*out)?, text(*reply)?],
        text(*identity)?,
        text(*plan)?,
    )? != closed.commitment
    {
        return Err("resume differs from its checked reply, worker or storage application");
    }
    let permit = boundary
        .structural_parameters
        .get(1)
        .filter(|p| p.position == 1)
        .ok_or("resume needs its original answer permit")?;
    publication_shape(module, permit, *declared, *reply, "AnswerRole", 1)
}

fn publication_shape(
    module: &TerminalModule,
    request: &terminal_psi::StructuralParameterDeclaration,
    declared_type: terminal_psi::TerminalIdentityIndex,
    reply_type: terminal_psi::TerminalIdentityIndex,
    role: &str,
    region: usize,
) -> Result<ParkShape, &'static str> {
    let text = |id| module.identities.text(id);
    let reply = text(reply_type).ok_or("publication has no closed reply type")?;
    let carrier = module
        .structural_type(request.structural_type)
        .ok_or("publication type is absent")?;
    if request.access != StructuralAccess::Owned
        || request.multiplicity != StructuralMultiplicity::Linear
        || carrier.multiplicity != StructuralMultiplicity::Linear
        || text(carrier.declaration_identity) != Some("toolchain::Publication")
        || !carrier.owns_linear_claim
        || !request.projected_qualifications.is_empty()
        || !request.additional_input_qualifications.is_empty()
    {
        return Err("the operation must consume an owned linear publication");
    }
    let [qualification] = request.qualifications.as_slice() else {
        return Err("the publication needs one protected role");
    };
    let qualified = module
        .structural_domain(*qualification)
        .ok_or("publication domain is absent")?;
    let plan = qualified
        .access_policy
        .as_ref()
        .ok_or("publication lost its issuance policy")?;
    let Some([(_, terminal_psi::StructuralEstablishmentRoute::ExactMachine { declaration, .. })]) =
        plan.issuance.as_deref()
    else {
        return Err("only the checked reply issuer may mint a publication");
    };
    let expected_issuer = core_owner(
        &language_core::bundled_files::core_revision::REPLY,
        "new_reply_pair",
    );
    if text(declaration.identity()) != Some(expected_issuer.as_str())
        || qualified.carrier != carrier.id
        || !matches!(plan.writers.as_slice(), [((root, Some(path)), writers)]
            if *root == plan.schema.root() && path.is_empty() && writers.is_empty())
    {
        return Err("park request must retain its exact issuer and deny re-targeting the slot");
    }
    let StructuralTypeShape::Record {
        fields,
        storage: terminal_psi::StructuralRecordStorage::Packed,
    } = &carrier.shape
    else {
        return Err("issued request must be a record");
    };
    let [field] = fields.as_slice() else {
        return Err("an issued publication must contain exactly its slot anchor");
    };
    let terminal_psi::StructuralFieldType::Structural(reference) = field.field_type else {
        return Err("the publication anchor is not a reference field");
    };
    let reference = module
        .structural_type(reference)
        .ok_or("anchor reference type is absent")?;
    let StructuralTypeShape::Reference {
        referent,
        access: StructuralAccess::SharedBorrow,
    } = reference.shape
    else {
        return Err("a publication must store an ordinary shared reference");
    };
    if text(field.declaration_identity) != Some("toolchain::Publication::anchor")
        || field.relevance.is_erased() || field.storage.is_some()
        || !module.structural_type(referent).is_some_and(|t| matches!(t.shape,
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(i))
                if i.sign() == semantic_vocabulary::IntegerSign::Unsigned && i.bits() == Some(8) && !i.is_address()))
    {
        return Err("the publication must borrow the slot's ordinary byte");
    }
    let (base, declared, issued) = publication_types(role, reply, region);
    if text(declared_type) != Some(declared.as_str())
        || text(qualified.identity) != Some(issued.as_str())
        || text(qualified.subject_identity) != Some(base.as_str())
        || text(carrier.identity) != Some(base.as_str())
    {
        return Err("the publication has a different role or closed reply type");
    }
    Ok(ParkShape {
        anchor: field.id,
        anchor_identity: field.identity,
        reply: reply_type,
    })
}

#[cfg(test)]
mod preparation_commitment_tests {
    use super::*;
    use terminal_psi::TerminalActivationStorage::{Fixed, Split};

    #[test]
    fn the_result_plan_may_name_the_case_or_its_evaluated_constant() {
        use application_identity::type_identity::vocabulary::IndexForm;
        let direct = atom(
            IndexForm::ConstName,
            &toolchain_name("ActivationStorage::Fixed"),
        );
        let evaluated = Fixed.canonical_value().unwrap().index_identity().unwrap();
        for operand in [direct, evaluated] {
            assert!(Fixed.matches_type_operand(&operand));
            assert!(!Split.matches_type_operand(&operand));
        }
    }

    #[test]
    fn reopens_the_source_workers_typed_storage_constant() {
        let fixed = Fixed.canonical_value().unwrap();
        let mut source = MachineApplicationCommitment::new("prepare").unwrap();
        source.type_argument("args").unwrap();
        source.type_argument("out").unwrap();
        source.machine_argument("worker").unwrap();
        source.const_argument(&fixed).unwrap();
        let published = source.finish();
        let reopen =
            |key: &str| worker_application_commitment("prepare", &["args", "out"], "worker", key);
        assert_eq!(
            reopen(&fixed.application_identity().unwrap()),
            Ok(published)
        );
        let split = Split.canonical_value().unwrap();
        assert_ne!(
            reopen(&split.application_identity().unwrap()).unwrap(),
            published
        );
        let mut unrelated = fixed;
        unrelated.type_name = "OtherStorage".into();
        assert!(reopen(&unrelated.application_identity().unwrap()).is_err());
        assert!(reopen("Fixed").is_err());
    }
}
