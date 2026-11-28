//! A saved program must not turn its own bodyless declaration into a park
//! operation, or use one publication to request a different reply.
mod preparation;
pub use preparation::PreparationShape;
use application_identity::{
    conformance_identity::MachineApplicationCommitment,
    type_identity::{
        encoding::{atom, byte_atom, compound},
        vocabulary::{Label, NameForm, OwnerForm, TypeForm},
    },
};
use language_core::sealed_intrinsics::ActivationOperation;
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

/// A preparation borrows storage; it does not call the worker. Verify the
/// worker selection here, not by searching for a matching machine at runtime.
/// The prepare signature and its custody rules are separate obligations.
pub fn prepare_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<PreparationShape, &'static str> {
    use application_identity::conformance_identity::{MachineOperandSelectionIdentity as Operand, requirement_application_commitment};
    use terminal_psi::{ClosedReachArgument as Arg, TerminalActivationOrigin};
    let checked = boundary.declaration.as_ref().ok_or("prepare has no checked declaration")?;
    let source = &checked.source;
    let closed = source.specialization.as_ref().ok_or("prepare is not specialized")?;
    let text = |id| module.identities.text(id).filter(|s| !s.is_empty()).ok_or("prepare has an unresolved identity");
    if checked.execution != terminal_psi::CheckedBoundaryExecution::OwnedActivation(ActivationOperation::Prepare)
        || boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
        || boundary.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || !boundary.crash_routes.is_empty() || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty() || !boundary.published_synchronous_entries.is_empty()
        || boundary.owning_service.is_some() || boundary.service_reach_is_installation_bound
        || boundary.attachment.is_some() || boundary.numeric_intrinsic.is_some()
        || text(closed.template.identity())? != source_owner("prepare")
        || text(closed.template_entry.identity())? != source_owner("prepare::entry")
        || closed.capture.is_some() || source.machine != boundary.owner
        || source.callable != boundary.identity || checked.service_declaration != source.contract
        || source.published_may_suspend || source.published_may_block {
        return Err("prepare differs from its sealed core declaration");
    }
    let [Arg::Type(args), Arg::Type(out), Arg::Machine { identity, .. }, Arg::Const(plan)] =
        checked.method_arguments.as_slice() else { return Err("prepare needs its exact argument, result, worker and storage binders"); };
    let operands = [
        Operand::Type(text(*args)?.into()), Operand::Type(text(*out)?.into()),
        Operand::Machine(text(*identity)?.into()), Operand::Const(text(*plan)?.into()),
    ];
    if requirement_application_commitment(text(closed.template_callable)?, &operands)? != closed.commitment {
        return Err("prepare selection differs from its closed application");
    }
    let mut plans = module.execution.task_activation_demands.iter().filter_map(|d| {
        if let TerminalActivationOrigin::Caller(p) = &d.origin
            && p.preparation == boundary.id { Some(p) } else { None }
    });
    let selected = plans.next().ok_or("prepare has no matching activation plan")?;
    if plans.next().is_some() || selected.selected_body(module).is_none() {
        return Err("prepare must select one checked worker in this program");
    }
    preparation::result_shape(module, boundary, *selected)
}

fn named(name: &str) -> String {
    compound(TypeForm::Named, [atom(Label::Name, name)])
}

fn published_request_types(reply: &str) -> (String, String, String) {
    use application_identity::type_identity::vocabulary::ConstraintForm;
    use language_core::bundled_files::core_revision;
    let carrier = named(&toolchain_name("Publication"));
    let family = "Publication::Issued";
    let source_domain = compound(
        ConstraintForm::DeclaredDomain,
        [
            atom(Label::Name, &toolchain_name(family)),
            named(&toolchain_name("RequestRole")),
            reply.to_owned(),
        ],
    );
    let declared = compound(
        TypeForm::Constrained,
        [
            carrier.clone(),
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
            named(&core_owner(&core_revision::REPLY, "RequestRole")),
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
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, ScalarQualificationSetId, ScalarType};
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
            let atom = match result.scalar_type {
                ScalarType::Boolean => "bool".to_string(),
                ScalarType::Integer(i) if i.is_address() => "address".to_string(),
                ScalarType::Integer(i) => {
                    let Some(bits) = i.bits() else { return false };
                    format!(
                        "{}{bits}",
                        if i.sign() == IntegerSign::Signed {
                            'i'
                        } else {
                            'u'
                        }
                    )
                }
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".into(),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".into(),
                ScalarType::Callable(_) => return false,
            };
            application_identity::type_identity::compiler_type_identity(&atom) == reply
        }
    }
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
        || !boundary.requires.iter().cloned().eq(
            terminal_psi::StructuralDomainRequirement::declared_roots(
                &boundary.structural_parameters,
            ),
        )
        || !boundary.normal_qualifications.is_empty()
        || !boundary.scalar_domain_requirements.is_empty()
        || !boundary.membership_ensures.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.reference_outputs.is_empty()
        || !boundary.result_reference_regions.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
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
    let carrier = module
        .structural_type(request.structural_type)
        .ok_or("park request type is absent")?;
    if !boundary.scalar_parameters.is_empty()
        || boundary.parameter_order != [BoundaryParameterKind::Structural]
        || request.access != StructuralAccess::Owned
        || request.multiplicity != StructuralMultiplicity::Linear
        || carrier.multiplicity != StructuralMultiplicity::Linear
        || text(carrier.declaration_identity) != Some("toolchain::Publication")
        || !carrier.owns_linear_claim
        || !request.projected_qualifications.is_empty()
        || !request.additional_input_qualifications.is_empty()
    {
        return Err("park must consume exactly one owned linear publication");
    }
    let [qualification] = request.qualifications.as_slice() else {
        return Err("park requires one protected request role");
    };
    let qualified = module
        .structural_domain(*qualification)
        .ok_or("park request domain is absent")?;
    let plan = qualified
        .access_policy
        .as_ref()
        .ok_or("park request lost its issuance policy")?;
    let Some([(_, terminal_psi::StructuralEstablishmentRoute::ExactMachine { declaration, .. })]) =
        plan.issuance.as_deref()
    else {
        return Err("only the checked reply issuer may mint a request");
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
        return Err("issued request must contain exactly its publication anchor");
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
        return Err("issued request must store an ordinary shared reference");
    };
    if text(field.declaration_identity) != Some("toolchain::Publication::anchor")
        || field.relevance.is_erased() || field.storage.is_some()
        || !module.structural_type(referent).is_some_and(|t| matches!(t.shape,
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(i))
                if i.sign() == semantic_vocabulary::IntegerSign::Unsigned && i.bits() == Some(8) && !i.is_address()))
    {
        return Err("park requires the exact field borrowing the slot's ordinary byte");
    }
    let (base, declared, issued) = published_request_types(reply);
    if text(*request_declared) != Some(declared.as_str())
        || text(qualified.identity) != Some(issued.as_str())
        || text(qualified.subject_identity) != Some(base.as_str())
        || text(carrier.identity) != Some(base.as_str())
    {
        return Err("the declared request does not carry the closed reply and request role");
    }
    Ok(ParkShape {
        anchor: field.id,
        anchor_identity: field.identity,
        reply: source.result_type,
    })
}
