//! Physical signature of the pure observation of an issued layout target.
//! The field identities come from the admitted artifact and remain local to it.
use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryRequirementKind, CheckedBoundaryExecution, StructuralAccess, StructuralFieldType,
    StructuralTypeShape, TerminalModule,
};

pub struct LayoutTargetSnapshotFields {
    pub result: StructuralTypeId,
    pub profile: StructuralFieldId,
    pub pointer_size: StructuralFieldId,
    pub pointer_alignment: StructuralFieldId,
    pub little_endian: StructuralFieldId,
}

pub fn snapshot_fields(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<LayoutTargetSnapshotFields, &'static str> {
    use application_identity::type_identity::encoding::{atom, byte_atom, compound};
    use application_identity::type_identity::vocabulary::{Label, NameForm, OwnerForm};
    use language_core::bundled_files::core_revision;

    let owner = |path| {
        compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, &core_revision::LAYOUT),
                atom(Label::Path, path),
            ],
        )
    };
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("target snapshot has no checked declaration")?;
    let text = |id| module.identities.text(id);
    if checked.execution != CheckedBoundaryExecution::LayoutTargetFacts
        || text(boundary.owner.identity()) != Some(owner("layout_target_facts").as_str())
        || checked.source.machine != boundary.owner
        || text(checked.source.state.identity())
            != Some(owner("layout_target_facts::entry").as_str())
        || checked.source.contract == [0; 32]
        || checked.service_declaration != checked.source.contract
        || boundary.identity != checked.source.callable
        || checked.source.specialization.is_some()
        || !checked.method_arguments.is_empty()
        || checked.source.parameter_types.len() != 1
        || boundary.attachment.is_some()
        || boundary.numeric_intrinsic.is_some()
        || boundary.kind != BoundaryRequirementKind::Machine
        || boundary.completion != BoundaryCompletion::NormalReturn
    {
        return Err("layout target snapshot differs from its sealed core declaration");
    }
    if checked.source.published_may_suspend
        || checked.source.published_may_block
        || !boundary.unconditional_termination
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || !boundary.published_synchronous_entries.is_empty()
        || boundary.owning_service.is_some()
        || boundary.service_reach_is_installation_bound
        || !boundary.requires.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.scalar_domain_requirements.is_empty()
        || !boundary.normal_qualifications.is_empty()
        || !boundary.membership_requires.is_empty()
        || !boundary.membership_ensures.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
    {
        return Err("layout target snapshot changed its pure callable contract");
    }
    if !boundary.scalar_parameters.is_empty()
        || boundary.parameter_order != [BoundaryParameterKind::Structural]
    {
        return Err("layout target snapshot changed its parameter list");
    }
    let [input] = boundary.structural_parameters.as_slice() else {
        return Err("layout target snapshot needs one shared target");
    };
    let carrier = module
        .structural_type(input.structural_type)
        .ok_or("layout target snapshot lost its input")?;
    if input.access != StructuralAccess::SharedBorrow
        || carrier.shape != StructuralTypeShape::Opaque
        || text(carrier.declaration_identity) != Some("toolchain::LayoutTarget")
        || !input.qualifications.is_empty()
        || !input.projected_qualifications.is_empty()
        || !input.additional_input_qualifications.is_empty()
    {
        return Err("layout target snapshot requires the opaque core LayoutTarget");
    }
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return Err("layout target snapshot needs an owned record result");
    };
    let record = module
        .structural_type(result.structural_type)
        .ok_or("layout target snapshot lost its result")?;
    let StructuralTypeShape::Record {
        fields,
        storage: terminal_psi::StructuralRecordStorage::Packed,
    } = &record.shape
    else {
        return Err("layout target snapshot result has no fields");
    };
    if text(record.declaration_identity) != Some("toolchain::LayoutTargetFacts")
        || fields.len() != 4
        || fields.iter().any(|field| {
            field.relevance != language_core::BindingRelevance::Relevant || field.storage.is_some()
        })
        || !record.is_copyable()
        || !result.qualifications.is_empty()
    {
        return Err("layout target snapshot requires the core LayoutTargetFacts result");
    }
    let member = |name| {
        fields
            .iter()
            .find(|field| text(field.declaration_identity) == Some(name))
            .ok_or("layout target snapshot has an unknown or missing field")
    };
    let profile = member("toolchain::LayoutTargetFacts::profile")?;
    let size = member("toolchain::LayoutTargetFacts::pointer_size")?;
    let alignment = member("toolchain::LayoutTargetFacts::pointer_alignment")?;
    let endian = member("toolchain::LayoutTargetFacts::little_endian")?;
    let word = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    if !profile
        .field_type
        .byte_sequence_carrier()
        .is_some_and(|bytes| bytes.is_shared_readable_view())
        || size.field_type != StructuralFieldType::Scalar(word)
        || alignment.field_type != StructuralFieldType::Scalar(word)
        || endian.field_type != StructuralFieldType::Scalar(ScalarType::Boolean)
    {
        return Err("layout target snapshot changed its field carriers");
    }
    Ok(LayoutTargetSnapshotFields {
        result: result.structural_type,
        profile: profile.id,
        pointer_size: size.id,
        pointer_alignment: alignment.id,
        little_endian: endian.id,
    })
}
