//! A saved program must not turn its own bodyless declaration into a park
//! operation, or use one publication to request a different reply.
use application_identity::{
    conformance_identity::MachineApplicationCommitment,
    type_identity::{
        encoding::{atom, byte_atom, compound},
        vocabulary::{Label, NameForm, OwnerForm, TypeForm},
    },
};
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryParameterKind, StructuralAccess, StructuralMultiplicity,
    StructuralTypeShape, TerminalModule,
};

fn source_owner(path: &str) -> String {
    use language_core::bundled_files::core_revision;
    compound(
        NameForm::Nominal,
        [
            byte_atom(OwnerForm::ToolchainSource, &core_revision::ACTIVATION),
            atom(Label::Path, path),
        ],
    )
}

fn applied(name: &str, reply: &str) -> String {
    use application_identity::type_identity::encoding::bare;
    let name = compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(Label::Path, name)],
    );
    compound(
        TypeForm::Generic,
        [atom(Label::Name, &name), reply.to_owned()],
    )
}

/// A preparation borrows storage; it does not call the worker. Verify the
/// worker selection here, not by searching for a matching machine at runtime.
/// The prepare signature and its custody rules are separate obligations.
pub(super) fn validate_prepared_entry(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    use application_identity::conformance_identity::{MachineOperandSelectionIdentity as Operand, requirement_application_commitment};
    use terminal_psi::{ClosedReachArgument as Arg, TerminalActivationOrigin};
    let checked = boundary.declaration.as_ref().ok_or("prepare has no checked declaration")?;
    let source = &checked.source;
    let closed = source.specialization.as_ref().ok_or("prepare is not specialized")?;
    let text = |id| module.identities.text(id).filter(|s| !s.is_empty()).ok_or("prepare has an unresolved identity");
    if text(closed.template.identity())? != source_owner("prepare")
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
    Ok(())
}

fn stored_reply(reply: &str) -> std::borrow::Cow<'_, str> {
    application_identity::type_identity::compiler_type_atom(reply).map_or_else(
        || reply.into(),
        |name| compound(TypeForm::Named, [atom(Label::Name, &name)]).into(),
    )
}

fn reply_result_matches(module: &TerminalModule, result: &terminal_psi::BoundaryMachineResult, reply: &str) -> bool {
    use semantic_vocabulary::{ScalarQualificationSetId, ScalarType, IntegerSign, IeeeFloatFormat};
    use terminal_psi::BoundaryMachineResult;
    match result {
        BoundaryMachineResult::Unit => reply == application_identity::type_identity::encoding::bare(TypeForm::Unit),
        BoundaryMachineResult::Structural(result) => module.structural_type(result.structural_type).is_some_and(|ty|
            result.qualifications.is_empty() && result.projected_qualifications.is_empty()
            && ty.multiplicity == result.multiplicity
            && module.identities.text(ty.identity) == Some(stored_reply(reply).as_ref())),
        BoundaryMachineResult::Scalar(result) => {
            if result.qualifications != ScalarQualificationSetId::ZERO { return false; }
            let atom = match result.scalar_type {
                ScalarType::Boolean => "bool".to_string(),
                ScalarType::Integer(i) if i.is_address() => "address".to_string(),
                ScalarType::Integer(i) => {
                    let Some(bits) = i.bits() else { return false };
                    format!("{}{bits}", if i.sign() == IntegerSign::Signed { 'i' } else { 'u' })
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
    operation: ActivationOperation,
) -> Result<&'m str, &'static str> {
    use terminal_psi::{
        BoundaryCompletion, BoundaryRequirementKind, CheckedBoundaryExecution, ClosedReachArgument,
    };
    let (name, suspends) = match operation {
        ActivationOperation::Park => ("park", true),
        ActivationOperation::NewReplyPair => ("new_reply_pair", false),
        ActivationOperation::Prepare => return Err("prepare does not take a reply"),
    };
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
        || source.published_may_suspend != suspends
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
        || !boundary.requires.is_empty()
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
    let reply = checked_reply(module, boundary, ActivationOperation::Park)?;
    let source = &boundary
        .declaration
        .as_ref()
        .ok_or("park lacks its declaration")?
        .source;
    let text = |id| module.identities.text(id);
    if text(source.result_type) != Some(reply) || !reply_result_matches(module, &boundary.result, reply) {
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
    let expected = applied("ParkRequest", reply);
    if text(*request_declared) != Some(expected.as_str())
        || !boundary.scalar_parameters.is_empty()
        || boundary.parameter_order != [BoundaryParameterKind::Structural]
        || request.access != StructuralAccess::Owned
        || request.multiplicity != StructuralMultiplicity::Linear
        || carrier.multiplicity != StructuralMultiplicity::Linear
        || carrier.shape != StructuralTypeShape::Opaque
        || text(carrier.declaration_identity) != Some("toolchain::ParkRequest")
        || text(carrier.identity) != Some(applied("ParkRequest", &stored_reply(reply)).as_str())
        || !carrier.owns_linear_claim
        || !request.qualifications.is_empty()
        || !request.projected_qualifications.is_empty()
        || !request.additional_input_qualifications.is_empty()
    {
        return Err("park must consume an opaque linear request for its exact reply type");
    }
    Ok(())
}

pub struct ReplyPairShape {
    pub result: StructuralTypeId,
    pub request: StructuralFieldId,
    pub request_type: StructuralTypeId,
    pub permit: StructuralFieldId,
    pub permit_type: StructuralTypeId,
}

/// Use the declared fields, not their order in an artifact's table. A second
/// opaque handle with the same storage shape is not an answer permit.
pub fn reply_pair_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<ReplyPairShape, &'static str> {
    use terminal_psi::{BoundaryMachineResult, StructuralFieldType, StructuralRecordStorage};
    let reply = checked_reply(module, boundary, ActivationOperation::NewReplyPair)?;
    let source = &boundary
        .declaration
        .as_ref()
        .ok_or("reply pair lacks its declaration")?
        .source;
    let text = |id| module.identities.text(id);
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return Err("reply pair requires a structural result");
    };
    let record = module
        .structural_type(result.structural_type)
        .ok_or("reply pair has no result type")?;
    let StructuralTypeShape::Record {
        fields,
        storage: StructuralRecordStorage::Packed,
    } = &record.shape
    else {
        return Err("reply pair result must be an ordinary record");
    };
    if !source.parameter_types.is_empty()
        || !boundary.parameter_order.is_empty()
        || !boundary.scalar_parameters.is_empty()
        || !boundary.structural_parameters.is_empty()
        || text(source.result_type) != Some(applied("ReplyPair", reply).as_str())
        || text(record.declaration_identity) != Some("toolchain::ReplyPair")
        || text(record.identity) != Some(applied("ReplyPair", &stored_reply(reply)).as_str())
        || fields.len() != 2
        || record.multiplicity != StructuralMultiplicity::Affine || record.owns_linear_claim
        || result.multiplicity != record.multiplicity
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err("reply pair must denote the sealed pair at the selected reply type");
    }
    let field = |member: &str, carrier: &str, name: &str| {
        let field = fields
            .iter()
            .find(|field| text(field.declaration_identity) == Some(member))
            .ok_or("reply pair field has another declaration")?;
        let StructuralFieldType::Structural(id) = field.field_type else {
            return Err("reply pair field is not an owned handle");
        };
        let handle = module
            .structural_type(id)
            .ok_or("reply pair field has no handle declaration")?;
        if field.relevance != language_core::BindingRelevance::Relevant
            || field.storage.is_some()
            || text(handle.declaration_identity) != Some(carrier)
            || text(handle.identity) != Some(applied(name, &stored_reply(reply)).as_str())
            || handle.shape != StructuralTypeShape::Opaque
            || handle.multiplicity != StructuralMultiplicity::Linear
            || !handle.owns_linear_claim
        {
            return Err("reply pair requires distinct linear opaque handles");
        }
        Ok((field.id, id))
    };
    let (request, request_type) = field("toolchain::ReplyPair::request", "toolchain::ParkRequest", "ParkRequest")?;
    let (permit, permit_type) = field("toolchain::ReplyPair::permit", "toolchain::AnswerPermit", "AnswerPermit")?;
    if request == permit || request_type == permit_type {
        return Err("reply pair aliases its two handles");
    }
    Ok(ReplyPairShape {
        result: record.id,
        request,
        request_type,
        permit,
        permit_type,
    })
}
