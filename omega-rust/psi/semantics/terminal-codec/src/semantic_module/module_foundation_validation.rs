//! Closed representation-foundation validation run before encoding.
//!
//! Checks identities, exact carrier relationships, signatures, provider
//! attachments, and operation/result shape so the independent semantic
//! verifier can rely on a well-formed module. It does not prove
//! qualifications, reach closure, or claim dataflow.

use crate::codec_error::{CodecError, malformed};
use crate::semantic_module::structural_result_wire;

use semantic_vocabulary::{
    ClaimId, IntegerSign, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, Operation, OperationKind,
    OperationResult, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    is_bounded_structural_scalar_store_path,
};

/// Validate the closed representation foundation needed before the independent
/// semantic verifier learns these operations. This checks identities, exact
/// carrier relationships, signatures, and operation/result shape; it does not
/// prove qualifications, reach closure, or claim dataflow.
pub(crate) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), CodecError> {
    require_unique_nonempty_identities(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural type identity",
    )?;
    require_unique_nonempty_identities(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural domain identity",
    )?;
    require_unique_nonempty_identities(
        module
            .services
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "service identity",
    )?;
    require_unique_nonempty_identities(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "boundary machine identity",
    )?;

    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, access } => {
                if !has_structural_type(module, *referent)
                    || *access == terminal_psi::StructuralAccess::Owned
                {
                    return malformed(
                        "reference type requires a known referent and borrowed access",
                    );
                }
            }
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView) => {}
            StructuralTypeShape::ByteSequence(_) => {
                return malformed("first-class byte-sequence type must be a borrowed view");
            }
            StructuralTypeShape::Record { fields } => {
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity.as_str()),
                    "structural field identity",
                )?;
                for field in fields {
                    match &field.field_type {
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque structural field type must have a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Erased { .. }
                            if !field.relevance.is_erased()
                                && !module
                                    .machines
                                    .iter()
                                    .any(|machine| machine.attachment == Some(declaration.id)) =>
                        {
                            return malformed(
                                "provider-backed attachment specialization is incomplete",
                            );
                        }
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return malformed(
                                "erased structural field must use its opaque semantic type identity",
                            );
                        }
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                if !has_structural_type(module, *element) {
                    return malformed("fixed array references an unknown structural element type");
                }
            }
            StructuralTypeShape::Sum { cases } => {
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity.as_str()),
                    "structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("structural sum must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity.as_str()),
                        "structural case payload field identity",
                    )?;
                    for field in &case.fields {
                        match &field.field_type {
                            StructuralFieldType::Structural(field_type)
                                if !has_structural_type(module, *field_type) =>
                            {
                                return malformed(
                                    "structural case payload references an unknown structural type",
                                );
                            }
                            StructuralFieldType::Erased { type_identity }
                                if !field.relevance.is_erased() || type_identity.is_empty() =>
                            {
                                return malformed(
                                    "opaque structural case payload must have erased relevance and a nonempty type identity",
                                );
                            }
                            StructuralFieldType::Scalar(_)
                            | StructuralFieldType::BoundedInteger(_)
                            | StructuralFieldType::IeeeFloat(_)
                            | StructuralFieldType::Structural(_)
                                if field.relevance.is_erased() =>
                            {
                                return malformed(
                                    "erased structural case payload must use its opaque semantic type identity",
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                require_unique_nonempty_identities(
                    fields.iter().map(|field| field.identity.as_str()),
                    "mixed structural field identity",
                )?;
                require_unique_nonempty_identities(
                    cases.iter().map(|case| case.identity.as_str()),
                    "mixed structural case identity",
                )?;
                if cases.is_empty() {
                    return malformed("mixed structural type must declare at least one case");
                }
                for case in cases {
                    require_unique_nonempty_identities(
                        case.fields.iter().map(|field| field.identity.as_str()),
                        "mixed structural case payload field identity",
                    )?;
                }
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    match &field.field_type {
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "mixed structural field references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque mixed structural field must have erased relevance and a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return malformed(
                                "erased mixed structural field must use its opaque semantic type identity",
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    validate_structural_type_graph(module)?;
    for domain in &module.structural_domains {
        if !has_structural_type(module, domain.carrier) {
            return malformed("structural domain references an unknown carrier type");
        }
    }
    for service in &module.services {
        if service
            .parents
            .iter()
            .any(|parent| *parent == service.id || !has_service(module, *parent))
        {
            return malformed("service references itself or an unknown parent");
        }
    }
    validate_service_parent_graph(module)?;
    require_known_services(module, &module.root_service_reach.concrete)?;
    for dependency in &module.root_service_reach.installation_dependencies {
        if dependency.requirement_identity.is_empty() {
            return malformed("installation reach requirement identity must be nonempty");
        }
        require_known_services(module, &dependency.upper_bound)?;
    }
    for boundary in &module.boundary_machines {
        if boundary
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("boundary machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &boundary.structural_parameters)?;
        for requirement in &boundary.requires {
            let Some(parameter) = boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
            else {
                return malformed("boundary requirement has an unknown argument index");
            };
            let Some(domain) = module
                .structural_domains
                .iter()
                .find(|domain| domain.id == requirement.domain)
            else {
                return malformed("boundary requirement references an unknown domain");
            };
            if domain.carrier != parameter.structural_type {
                return malformed("boundary requirement domain has the wrong carrier type");
            }
        }
        require_known_services(module, &boundary.published_service_ceiling)?;
        require_known_services(module, &boundary.fixed_service_reach)?;
    }
    for candidate in &module.provider_candidates {
        if candidate.requirement_identity.is_empty()
            || candidate.provider_identity.is_empty()
            || candidate.candidate_identity.is_empty()
        {
            return malformed("provider candidate identities must be nonempty");
        }
        if !module
            .boundary_machines
            .iter()
            .any(|boundary| boundary.id == candidate.boundary)
            || !module
                .machines
                .iter()
                .any(|machine| machine.id == candidate.candidate)
        {
            return malformed("provider candidate references an unknown terminal ID");
        }
        for parameter in &candidate.signature.parameters {
            if !has_structural_type(module, parameter.structural_type) {
                return malformed("provider signature references an unknown structural type");
            }
            if parameter.qualifications.iter().any(|domain| {
                !module
                    .structural_domains
                    .iter()
                    .any(|row| row.id == *domain)
            }) {
                return malformed("provider signature references an unknown structural domain");
            }
        }
        require_known_services(module, &candidate.refinement.realized_service_ceiling)?;
    }

    for machine in &module.machines {
        if machine
            .attachment
            .is_some_and(|attachment| !has_structural_type(module, attachment))
        {
            return malformed("machine has an unknown attachment type");
        }
        validate_structural_parameters(module, &machine.structural_parameters)?;
        structural_result_wire::validate_reference_sources(module, machine)?;
        for block in &machine.blocks {
            validate_structural_parameters(module, &block.structural_parameters)?;
            if block.id == machine.entry && !block.structural_parameters.is_empty() {
                return malformed("entry block cannot declare structural parameters");
            }
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                if parameter.access == terminal_psi::StructuralAccess::Owned
                    && terminal_semantics::scalar_array_leaf_shape(
                        module.structural_types.iter(),
                        parameter.structural_type,
                    )
                    .is_some()
                {
                    return malformed(
                        "owned primitive-array payloads have no block-parameter transport",
                    );
                }
                if parameter.position as usize != position || parameter.is_self {
                    return malformed(
                        "structural block parameters require dense positions and no self",
                    );
                }
                if !machine.structural_places.iter().any(|place| {
                    place.id == parameter.place
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            }
                }) {
                    return malformed(
                        "structural block parameter place disagrees with its declaration",
                    );
                }
            }
        }
        for place in &machine.structural_places {
            if let StructuralPlaceKind::BlockParameter { block, position } = place.kind {
                let parameter = machine
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == block)
                    .and_then(|candidate| candidate.structural_parameters.get(position as usize));
                if parameter.is_none_or(|parameter| parameter.place != place.id) {
                    return malformed("structural block place has no matching parameter");
                }
            }
        }
        validate_provider_attachment_foundation(module, machine)?;
        require_known_services(module, &machine.published_service_ceiling)?;
        for parameter in &machine.structural_parameters {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == parameter.place)
            else {
                return malformed("structural parameter has no declared structural place");
            };
            if place.kind
                != (StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                })
            {
                return malformed("structural parameter place kind disagrees with its signature");
            }
        }
        for claim in &machine.entry_claims {
            let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == claim.input)
            else {
                return malformed("entry claim is not bound to a structural parameter");
            };
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                return malformed("entry claim cannot bind an unrestricted parameter");
            }
            validate_structural_path(module, parameter.structural_type, &claim.path)?;
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            validate_operation_foundation(module, machine, operation)?;
        }
        for place in &machine.structural_places {
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = place.kind
            else {
                continue;
            };
            let mut producers = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == producer);
            let Some(operation) = producers.next() else {
                return malformed("structural operation-result place has no producer");
            };
            if producers.next().is_some() {
                return malformed("structural operation-result place has duplicate producers");
            }
            let Some(result) = operation.result.structural() else {
                return malformed(
                    "structural operation-result place producer has no structural result",
                );
            };
            if result.place != place.id || result.structural_type != structural_type {
                return malformed("structural operation-result place disagrees with its producer");
            }
        }
        for block in &machine.blocks {
            if let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator {
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return malformed("nominal affine cleanup requires a Unit result");
                }
                if cleanups.is_empty() {
                    return malformed("nominal affine cleanup list is empty");
                }
                if cleanups.len() != machine.structural_parameters.len() {
                    return malformed(
                        "nominal affine cleanup list does not cover every structural parameter",
                    );
                }
                if cleanups.iter().any(|cleanup| {
                    !machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == cleanup.place)
                }) {
                    return malformed("nominal affine cleanup root is not a structural parameter");
                }
                let mut places = BTreeSet::new();
                for (cleanup, parameter) in cleanups
                    .iter()
                    .zip(machine.structural_parameters.iter().rev())
                {
                    if cleanup.place != parameter.place {
                        return malformed(
                            "nominal affine cleanup list is not in reverse parameter order",
                        );
                    }
                    if !places.insert(cleanup.place)
                        || parameter.multiplicity != StructuralMultiplicity::Affine
                        || !parameter.qualifications.is_empty()
                        || machine
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == cleanup.place)
                    {
                        return malformed(
                            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root",
                        );
                    }
                    if parameter.structural_type != cleanup.structural_type {
                        return malformed(
                            "nominal affine cleanup type does not match its structural parameter",
                        );
                    }
                }
            }
            let (trivial_affine_discards, residual_affine_discards) = match &block.terminator {
                Terminator::ReturnUnitPartialAffine {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => {
                    if !matches!(machine.result, TerminalMachineResult::Unit)
                        || residual_affine_discards.is_empty()
                    {
                        return malformed(
                            "partial affine cleanup requires a Unit result and a residual action",
                        );
                    }
                    (trivial_affine_discards, residual_affine_discards)
                }
                Terminator::Jump {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => (trivial_affine_discards, residual_affine_discards),
                _ => continue,
            };
            for discard in residual_affine_discards {
                let parameter = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == discard.place);
                let supported_root = parameter.map_or_else(
                    || is_plain_affine_call_result(machine, discard.place),
                    |parameter| parameter.multiplicity == StructuralMultiplicity::Affine,
                );
                if !supported_root
                    || discard.path.is_empty()
                    || trivial_affine_discards.contains(&discard.place)
                    || machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == discard.place)
                {
                    return malformed(
                        "partial affine cleanup is not a distinct claim-free affine path",
                    );
                }
                let Some(root_type) = structural_place_type(machine, discard.place) else {
                    return malformed("partial affine cleanup has no exact structural root type");
                };
                if validate_structural_path(module, root_type, &discard.path)?
                    != discard.structural_type
                {
                    return malformed("partial affine cleanup leaf type does not match its path");
                }
            }
        }
    }
    Ok(())
}

fn validate_provider_attachment_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    let provider_roots = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = machine
        .attachment
        .and_then(|attachment| {
            module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
        })
        .and_then(|attachment| match &attachment.shape {
            StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. })
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_roots.is_empty() && provider_fields.is_empty() {
        return Ok(());
    }
    let [provider_field] = provider_fields.as_slice() else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let Some(attachment) = machine.attachment else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let self_parameters = machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let invalid_self = match self_parameters.as_slice() {
        [] => false,
        [parameter] => parameter.position != 0 || parameter.structural_type != attachment,
        _ => true,
    };
    let mut boundaries = BTreeSet::new();
    if invalid_self
        || provider_roots
            .iter()
            .any(|(root_attachment, field, boundary)| {
                *root_attachment != attachment
                    || *field != provider_field.id
                    || !boundaries.insert(*boundary)
                    || module
                        .boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .is_none_or(|declaration| declaration.attachment.is_some())
            })
    {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    let called = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::BoundaryCall { boundary, .. } => Some(boundary),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if called != boundaries {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    Ok(())
}

pub(crate) fn validate_structural_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    for segment in path {
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
        else {
            return malformed("structural path has an unknown structural type");
        };
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (StructuralPathSegment::Referent, _) | (_, StructuralTypeShape::Reference { .. }) => {
                return malformed("referent projection requires a reference carrier");
            }
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                if identity.is_empty() {
                    return malformed("structural path field identity cannot be empty");
                }
                let Some(field) = fields.iter().find(|field| field.identity == *identity) else {
                    return malformed("structural path has an unknown structural field");
                };
                if field.relevance.is_erased() {
                    return malformed("structural path cannot select an erased structural field");
                }
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return malformed("structural path must retain structural custody");
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) => {
                if index >= length {
                    return malformed("structural path fixed index is out of bounds");
                }
                *element
            }
            (StructuralPathSegment::Field(_), StructuralTypeShape::FixedArray { .. }) => {
                return malformed("structural path field requires a record type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Record { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (_, StructuralTypeShape::Sum { .. }) => {
                return malformed("structural path cannot traverse a payload-less sum");
            }
            (_, StructuralTypeShape::PrimitiveScalar(_)) => {
                return malformed("primitive-scalar structural type has no projected children");
            }
            (_, StructuralTypeShape::ByteSequence(_)) => {
                return malformed("byte-sequence structural type has no projected children");
            }
        };
    }
    Ok(structural_type)
}

fn validate_structural_parameters(
    module: &TerminalModule,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    let mut places = BTreeSet::new();
    let mut self_count = 0_u32;
    for parameter in parameters {
        if !places.insert(parameter.place) {
            return malformed("structural parameters reuse a place identity");
        }
        self_count += u32::from(parameter.is_self);
        if self_count > 1 {
            return malformed("structural signature declares more than one self parameter");
        }
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("structural parameter references an unknown type");
        }
        for qualification in &parameter.qualifications {
            let Some(domain) = module
                .structural_domains
                .iter()
                .find(|domain| domain.id == *qualification)
            else {
                return malformed("structural parameter references an unknown qualification");
            };
            if domain.carrier != parameter.structural_type {
                return malformed("structural parameter qualification has the wrong carrier");
            }
        }
    }
    Ok(())
}

fn validate_operation_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    match &operation.kind {
        OperationKind::EstablishReference { source } => {
            let Some(result) = operation.result.structural() else {
                return malformed("reference establishment requires a structural result");
            };
            let Some(StructuralTypeShape::Reference { referent, access }) = module
                .structural_types
                .iter()
                .find(|row| row.id == result.structural_type)
                .map(|row| &row.shape)
            else {
                return malformed("reference establishment requires a reference carrier type");
            };
            let Some(source_type) = structural_place_type(machine, source.place) else {
                return malformed("reference establishment source is unknown");
            };
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || *access != source.access
                || source.access == terminal_psi::StructuralAccess::Owned
                || validate_structural_path(module, source_type, &source.path)? != *referent
            {
                return malformed("reference establishment has inconsistent custody");
            }
        }
        OperationKind::ReleaseReference { source } => {
            if operation.result != OperationResult::Unit
                || !structural_place_type(machine, *source).is_some_and(|source_type| {
                    module.structural_types.iter().any(|row| {
                        row.id == source_type
                            && matches!(row.shape, StructuralTypeShape::Reference { .. })
                    })
                })
            {
                return malformed("reference release requires a reference carrier and Unit result");
            }
        }
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            ..
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte field store requires Unit");
            }
            let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
            else {
                return malformed("byte field store destination is not a parameter");
            };
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !is_bounded_structural_scalar_store_path(path)
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination || claim.input == *source)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination || claim.input.root == *source)
            {
                return malformed("byte field store has invalid destination custody");
            }
            let parent_type = validate_structural_path(module, parameter.structural_type, path)?;
            let bounded_field = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parent_type)
                .is_some_and(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => fields.iter().any(|candidate| {
                        candidate.id == *field
                            && !candidate.relevance.is_erased()
                            && matches!(
                                candidate.field_type,
                                StructuralFieldType::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                    }),
                    _ => false,
                });
            if !bounded_field {
                return malformed("byte field store does not select a bounded byte field");
            }
            let Some(source_place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *source)
            else {
                return malformed("byte field store source is unknown");
            };
            let source_type = match source_place.kind {
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } => structural_type,
                StructuralPlaceKind::Parameter { position, is_self } => {
                    let Some(parameter) = machine.structural_parameters.iter().find(|parameter| {
                        parameter.place == *source
                            && parameter.position == position
                            && parameter.is_self == is_self
                            && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && parameter.qualifications.is_empty()
                            && parameter.projected_qualifications.is_empty()
                    }) else {
                        return malformed("byte field store source is not an immutable whole view");
                    };
                    parameter.structural_type
                }
                StructuralPlaceKind::BlockParameter { block, position } => {
                    let Some(parameter) = machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)
                        .and_then(|block| block.structural_parameters.get(position as usize))
                        .filter(|parameter| {
                            parameter.place == *source
                                && parameter.position == position
                                && !parameter.is_self
                                && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                && parameter.qualifications.is_empty()
                                && parameter.projected_qualifications.is_empty()
                        })
                    else {
                        return malformed(
                            "byte field store block source is not an immutable whole view",
                        );
                    };
                    parameter.structural_type
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    let mut producers = machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter(|operation| operation.id == producer);
                    let Some(producer) = producers.next() else {
                        return malformed("byte field store subslice source has no producer");
                    };
                    if producers.next().is_some()
                        || !matches!(producer.kind, OperationKind::ByteSequenceSubslice { .. })
                        || !producer.result.structural().is_some_and(|result| {
                            result.place == *source
                                && result.structural_type == structural_type
                                && result.multiplicity == StructuralMultiplicity::Unrestricted
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.is_empty()
                                && result.claims.is_empty()
                        })
                    {
                        return malformed(
                            "byte field store subslice source has inexact producer custody",
                        );
                    }
                    structural_type
                }
                _ => return malformed("byte field store source is not an immutable whole view"),
            };
            if !module.structural_types.iter().any(|declaration| declaration.id == source_type
                && matches!(declaration.shape, StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)))
                || !machine.blocks.iter().flat_map(|block| &block.operations).any(|candidate| {
                    matches!(candidate.kind, OperationKind::ByteSequenceLength { source: measured } if measured == *source)
                        && candidate.result.scalar_ref().is_some_and(|result| result.id == *length
                            && matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64))
                })
            {
                return malformed("byte field store source or exact length is invalid");
            }
        }
        OperationKind::StructuralByteSequenceFieldLength { .. } => {
            if operation.result.scalar().is_none_or(|result| {
                !matches!(result.scalar_type, ScalarType::Integer(integer)
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
            }) {
                return malformed("byte field length requires an unsigned 64-bit scalar result");
            }
            // Independent module validation checks the exact bounded field,
            // borrowed parameter custody, and absence of live claims.
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte field byte store requires Unit");
            }
            // Independent module validation reconstructs exact current length
            // provenance, scalar operand types, custody, and index bounds.
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            let Some(result) = operation.result.structural() else {
                return malformed("byte-sequence subslice requires a structural result");
            };
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !module.structural_types.iter().any(|row| row.id == result.structural_type
                    && matches!(row.shape, StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)))
                || !machine.structural_places.iter().any(|row| row.id == result.place
                    && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type))
            {
                return malformed("byte-sequence subslice requires its exact immutable borrowed result place");
            }
        }
        OperationKind::ByteSequenceWrite { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence write requires a Unit result");
            }
        }
        OperationKind::StructuralCaseMembership { .. } => {
            if operation.result.scalar().is_none_or(|result| {
                result.scalar_type != ScalarType::Boolean || !result.qualifications.is_empty()
            }) {
                return malformed("case membership requires an unqualified Boolean result");
            }
            // Full module validation independently checks the source's nominal
            // owner, readable access, establishment order and live custody.
        }
        OperationKind::ByteSequenceRead { .. } => {
            let expected = ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8)
                    .expect("u8 is valid"),
            );
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != expected)
            {
                return malformed("byte-sequence read requires an unsigned 8-bit scalar result");
            }
            // Full module validation checks direct length provenance, custody,
            // operand types and dominance before encoding or after decoding.
        }
        OperationKind::ByteSequenceLength { .. } => {
            let expected = ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                    .expect("u64 is valid"),
            );
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != expected)
            {
                return malformed("byte-sequence length requires an unsigned 64-bit scalar result");
            }
            // The independent module verifier checks exact source custody and
            // literal establishment before encode or after decode.
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            path,
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("write-only primitive store declares a non-Unit result");
            }
            let destination_type = if !path.is_empty() {
                if let Some(parameter) = machine
                    .structural_parameters
                    .iter()
                    .chain(
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .find(|parameter| parameter.place == *destination)
                {
                    if !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    ) || !matches!(
                        parameter.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    ) || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                        || machine
                            .entry_claims
                            .iter()
                            .any(|claim| claim.input == *destination)
                        || machine
                            .content_entry_claims
                            .iter()
                            .any(|claim| claim.input.root == *destination)
                    {
                        return malformed(
                            "projected primitive store has invalid destination custody",
                        );
                    }
                    parameter.structural_type
                } else {
                    let Some(result) = machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|producer| producer.result.structural())
                        .find(|result| {
                            result.place == *destination
                                && result.claims.is_empty()
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.is_empty()
                                && matches!(
                                    result.multiplicity,
                                    StructuralMultiplicity::Unrestricted
                                        | StructuralMultiplicity::Affine
                                )
                        })
                    else {
                        return malformed("projected primitive store has no owned root");
                    };
                    // Exact producer, liveness and reference custody belong to independent verification.
                    result.structural_type
                }
            } else if let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
            {
                if !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || machine
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == *destination)
                    || machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == *destination)
                    || !matches!(
                        machine.structural_places.iter().find(|place| place.id == *destination),
                        Some(StructuralPlaceDeclaration {
                            kind: StructuralPlaceKind::Parameter { position, is_self },
                            ..
                        }) if *position == parameter.position && *is_self == parameter.is_self
                    )
                {
                    return malformed("write-only primitive store has invalid destination custody");
                }
                parameter.structural_type
            } else {
                let Some(result) = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|producer| {
                        matches!(producer.kind, OperationKind::EstablishPrimitiveLocal { .. })
                    })
                    .filter_map(|producer| producer.result.structural())
                    .find(|result| result.place == *destination)
                else {
                    return malformed(
                        "primitive store destination is neither a parameter nor an initialized local",
                    );
                };
                // Each establishment is independently checked against its exact
                // operation-result declaration below; the verifier checks dominance.
                if machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination)
                    || machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == *destination)
                {
                    return malformed("primitive local store cannot carry entry claims");
                }
                result.structural_type
            };
            let Some(expected) = terminal_semantics::primitive_place_type(
                module.structural_types.iter(),
                destination_type,
                path,
            ) else {
                return malformed("write-only primitive store requires a primitive-scalar root");
            };
            let actual = machine
                .parameters
                .iter()
                .chain(machine.result.scalar_ref())
                .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                .chain(machine.blocks.iter().flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .filter_map(|candidate| candidate.result.scalar_ref())
                }))
                .find(|declaration| declaration.id == *value)
                .map(|declaration| declaration.scalar_type);
            if actual != Some(expected) {
                return malformed("write-only primitive store value type does not match referent");
            }
        }
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation,
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("structural scalar field store declares a non-Unit result");
            }
            let destination_type = if let Some(parameter) = machine
                .structural_parameters
                .iter()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .find(|parameter| parameter.place == *destination)
            {
                if !matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                ) || !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return malformed(
                        "structural scalar field store has invalid destination custody",
                    );
                }
                parameter.structural_type
            } else {
                let Some(result) = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|producer| {
                        matches!(
                            producer.kind,
                            OperationKind::EstablishRecord { .. }
                                | OperationKind::CallStructural { .. }
                                | OperationKind::CallStructuralWithScalarArguments { .. }
                        )
                    })
                    .filter_map(|producer| producer.result.structural())
                    .find(|result| result.place == *destination)
                else {
                    return malformed(
                        "structural scalar field store destination has no record home",
                    );
                };
                if !matches!(
                    result.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                ) || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return malformed("structural scalar field store has invalid local custody");
                }
                // Foundation validation also checks the exact producer/place binding;
                // ordered availability and whole affine liveness belong to verification.
                result.structural_type
            };
            if !is_bounded_structural_scalar_store_path(path)
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *destination)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *destination)
            {
                return malformed("structural scalar field store has invalid destination custody");
            }
            let parent_type = validate_structural_path(module, destination_type, path)?;
            let Some(expected) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parent_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => fields.iter().find_map(|candidate| {
                        (candidate.id == *field && !candidate.relevance.is_erased())
                            .then_some(&candidate.field_type)
                            .and_then(|field_type| match field_type {
                                StructuralFieldType::Scalar(scalar_type)
                                    if range_obligation.is_none() =>
                                {
                                    Some(*scalar_type)
                                }
                                StructuralFieldType::BoundedInteger(bounds)
                                    if range_obligation.is_some() =>
                                {
                                    Some(semantic_vocabulary::ScalarType::Integer(
                                        bounds.integer_type(),
                                    ))
                                }
                                StructuralFieldType::IeeeFloat(format)
                                    if range_obligation.is_none() =>
                                {
                                    Some(semantic_vocabulary::ScalarType::IeeeFloat(*format))
                                }
                                _ => None,
                            })
                    }),
                    _ => None,
                })
            else {
                return malformed(
                    "structural scalar field store does not select a relevant scalar field",
                );
            };
            let actual = machine
                .parameters
                .iter()
                .chain(machine.result.scalar_ref())
                .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                .chain(machine.blocks.iter().flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .filter_map(|candidate| candidate.result.scalar_ref())
                }))
                .find(|declaration| declaration.id == *value)
                .map(|declaration| declaration.scalar_type);
            if actual != Some(expected) {
                return malformed("structural scalar field store value type does not match field");
            }
        }
        OperationKind::IntegerStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        } => {
            let Some(result) = operation.result.scalar_ref() else {
                return malformed("scalar structural field has no scalar result");
            };
            if !matches!(
                (&operation.kind, result.scalar_type),
                (
                    OperationKind::IntegerStructuralField { .. },
                    ScalarType::Integer(_)
                ) | (
                    OperationKind::BooleanStructuralField { .. },
                    ScalarType::Boolean
                )
            ) {
                return malformed("scalar structural field has an invalid result type");
            }
            // A constructed/call-result record remains a local result, not a
            // synthetic parameter. The verifier separately checks its producer,
            // dominance, live ownership and loans at this observation.
            if let Some(local) = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|producer| {
                    matches!(
                        producer.kind,
                        OperationKind::EstablishRecord { .. }
                            | OperationKind::CallStructural { .. }
                            | OperationKind::CallStructuralWithScalarArguments { .. }
                    )
                })
                .filter_map(|producer| producer.result.structural())
                .find(|local| local.place == *source)
            {
                let carrier = terminal_semantics::record_field_carrier(
                    module.structural_types.iter(),
                    local.structural_type,
                    path,
                );
                let matching = carrier.and_then(|carrier| module.structural_types.iter().find(|declaration|
                    declaration.id == carrier.structural_type)).is_some_and(|declaration|
                        matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                            if fields.iter().any(|candidate| candidate.id == *field && !candidate.relevance.is_erased()
                                    && candidate.field_type.scalar_type() == Some(result.scalar_type))));
                if !matching
                    || local.multiplicity == StructuralMultiplicity::Linear
                    || !local.qualifications.is_empty()
                    || !local.projected_qualifications.is_empty()
                    || !local.claims.is_empty()
                {
                    return malformed("scalar record field has invalid local result custody");
                }
                return Ok(());
            }
            let Some(parameter) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *source)
                .or_else(|| {
                    let declaration = machine
                        .structural_places
                        .iter()
                        .find(|place| place.id == *source)?;
                    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind
                    else {
                        return None;
                    };
                    machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)?
                        .structural_parameters
                        .get(position as usize)
                        .filter(|parameter| {
                            parameter.place == *source
                                && parameter.position == position
                                && !parameter.is_self
                        })
                })
            else {
                return malformed("scalar structural field source is not a parameter");
            };
            let carrier = terminal_semantics::record_field_carrier(
                module.structural_types.iter(),
                parameter.structural_type,
                path,
            );
            let matching = carrier
                .and_then(|carrier| {
                    module
                        .structural_types
                        .iter()
                        .find(|declaration| declaration.id == carrier.structural_type)
                })
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => fields.iter().find(|candidate| {
                        candidate.id == *field
                            && !candidate.relevance.is_erased()
                            && match candidate.field_type {
                                StructuralFieldType::Scalar(scalar_type) => {
                                    scalar_type == result.scalar_type
                                }
                                StructuralFieldType::BoundedInteger(bounded) => {
                                    ScalarType::Integer(bounded.integer_type())
                                        == result.scalar_type
                                }
                                _ => false,
                            }
                    }),
                    _ => None,
                });
            if !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == *source)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *source)
                || matching.is_none()
            {
                return malformed("scalar structural field has invalid source custody");
            }
        }
        OperationKind::EstablishScalarArray { elements } => {
            let Some(result) = operation.result.structural() else {
                return malformed("scalar array establishment has no structural result");
            };
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !matches!(
                    machine.structural_places.iter().find(|place| place.id == result.place),
                    Some(StructuralPlaceDeclaration {
                        kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
                    }) if *producer == operation.id && *structural_type == result.structural_type
                )
                || terminal_semantics::scalar_array_leaf_shape(
                    module.structural_types.iter(),
                    result.structural_type,
                )
                .is_none_or(|(_, count)| u64::try_from(elements.len()).ok() != Some(count))
            {
                return malformed("scalar array establishment has an invalid result shape");
            }
        }
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => {
            let Some(result) = operation.result.structural() else {
                return malformed("scalar case establishment has no structural result");
            };
            if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return malformed("scalar case establishment has an invalid result surface");
            }
            if !matches!(
                machine.structural_places.iter().find(|place| place.id == result.place),
                Some(StructuralPlaceDeclaration {
                    kind: StructuralPlaceKind::OperationResult { producer, structural_type },
                    ..
                }) if *producer == operation.id && *structural_type == result.structural_type
            ) {
                return malformed("scalar case establishment has no matching result place");
            }
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
            else {
                return malformed("scalar case establishment has an unknown structural type");
            };
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return malformed("scalar case establishment requires a sum type");
            };
            let Some(selected) = cases.iter().find(|case| case.id == *result_case) else {
                return malformed("scalar case establishment requires an exact member");
            };
            if selected.fields.len() != fields.len() {
                return malformed("scalar case establishment requires every selected field");
            }
            for (declaration, field) in selected.fields.iter().zip(fields) {
                if declaration.id != field.field
                    || declaration.relevance.is_erased()
                    || declaration.field_type.scalar_type().is_none()
                    || matches!(
                        declaration.field_type,
                        StructuralFieldType::BoundedInteger(_)
                    ) != field.range_obligation.is_some()
                {
                    return malformed("scalar case establishment has an invalid field binding");
                }
            }
            // Full module validation independently checks exact operand types,
            // dominance and declaration-derived obligations before acceptance.
        }
        OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence literal establishment declares a scalar result");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return malformed("byte-sequence literal establishment has no literal declaration");
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == *structural_type)
            else {
                return malformed("byte-sequence literal has an unknown structural type");
            };
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
            ) {
                return malformed("byte-sequence literal must use a borrowed byte-sequence type");
            }
        }
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("unit call declares a scalar result");
            }
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("unit call references an unknown callee");
            };
            if callee.result != TerminalMachineResult::Unit
                || structural_arguments.len() != callee.structural_parameters.len()
            {
                return malformed("unit call has the wrong callee result or structural arity");
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                claim_transfers
                    .iter()
                    .map(|transfer| (transfer.claim, transfer.argument_index)),
            )?;
        }
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("structural scalar call references an unknown callee");
            };
            if arguments.len() != callee.parameters.len()
                || operation.result.scalar().map(|result| result.scalar_type)
                    != callee.result.scalar().map(|result| result.scalar_type)
                || operation.result == OperationResult::Unit
                || structural_arguments.len() != callee.structural_parameters.len()
            {
                return malformed(
                    "structural scalar call has the wrong callee signature or structural arity",
                );
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                claim_transfers
                    .iter()
                    .map(|transfer| (transfer.claim, transfer.argument_index)),
            )?;
        }
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } if operation
            .result
            .structural()
            .is_none_or(|result| result.multiplicity != StructuralMultiplicity::Linear) =>
        {
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("mixed structural-result call references an unknown callee");
            };
            let Some(expected_result) = callee.result.structural() else {
                return malformed(
                    "mixed structural-result call references a non-structural-result callee",
                );
            };
            let Some(actual_result) = operation.result.structural() else {
                return malformed("mixed structural-result call has no structural result");
            };
            if arguments.len() != callee.parameters.len()
                || structural_arguments.len() != callee.structural_parameters.len()
                || actual_result.structural_type != expected_result.structural_type
                || actual_result.multiplicity != expected_result.multiplicity
                || actual_result.qualifications != expected_result.qualifications
                || !actual_result.projected_qualifications.is_empty()
                || !actual_result.claims.is_empty()
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || if is_claim_free_structural_call(actual_result, callee) {
                    requirement_obligations.len() != callee.contract.requires.len()
                } else {
                    !requirement_obligations.is_empty() || !crash_continuations.is_empty()
                }
            {
                return malformed("mixed structural-result call exceeds its bounded signature");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == actual_result.place)
            else {
                return malformed(
                    "mixed structural-result call result has no operation-result declaration",
                );
            };
            if *producer != operation.id || *structural_type != actual_result.structural_type {
                return malformed(
                    "mixed structural-result call result disagrees with its producer",
                );
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            let arguments = match &operation.kind {
                OperationKind::CallStructuralWithScalarArguments { arguments, .. } => {
                    arguments.as_slice()
                }
                _ => &[],
            };
            let selected_evidence = match &operation.kind {
                OperationKind::CallStructural {
                    selected_evidence, ..
                } => selected_evidence.as_slice(),
                _ => &[],
            };
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("structural call references an unknown callee");
            };
            let Some(expected_result) = callee.result.structural() else {
                return malformed("structural call references a non-structural-result callee");
            };
            let Some(actual_result) = operation.result.structural() else {
                return malformed("structural call has no structural operation result");
            };
            let exact_payloadless = callee.parameters.is_empty()
                && callee.structural_parameters.is_empty()
                && callee.entry_claims.is_empty()
                && callee.content_entry_claims.is_empty()
                && callee.contract.requires.is_empty()
                && callee.contract.ensures.is_empty()
                && callee.contract.crash_routes.is_empty()
                && module
                    .evidence_contract_lanes
                    .iter()
                    .all(|lane| lane.machine != callee.id)
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
                && returned_claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
                && actual_result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && expected_result.multiplicity
                    == terminal_psi::StructuralMultiplicity::Unrestricted
                && actual_result.qualifications.is_empty()
                && expected_result.qualifications.is_empty()
                && actual_result.claims.is_empty()
                && callee_exact_payloadless_return(callee);
            let claim_free_result = is_claim_free_structural_call(actual_result, callee)
                && selected_evidence.is_empty()
                && claim_transfers.is_empty()
                && returned_claim_transfers.is_empty()
                && requirement_obligations.len() == callee.contract.requires.len();
            if arguments.len() != callee.parameters.len()
                || structural_arguments.len() != callee.structural_parameters.len()
                || (!selected_evidence.is_empty() && !exact_payloadless)
                || (!exact_payloadless
                    && !claim_free_result
                    && (structural_arguments.len() != 1 || callee.structural_parameters.len() != 1))
                || actual_result.structural_type != expected_result.structural_type
                || actual_result.multiplicity != expected_result.multiplicity
                || actual_result.qualifications != expected_result.qualifications
            {
                return malformed(
                    "structural call has the wrong callee signature or structural arity",
                );
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == actual_result.place)
            else {
                return malformed(
                    "structural call result has no operation-result place declaration",
                );
            };
            if *producer != operation.id || *structural_type != actual_result.structural_type {
                return malformed("structural call result place disagrees with its producer");
            }
            if !has_structural_type(module, actual_result.structural_type) {
                return malformed("structural call result has an unknown structural type");
            }
            for qualification in &actual_result.qualifications {
                if !module
                    .structural_domains
                    .iter()
                    .any(|domain| domain.id == *qualification)
                {
                    return malformed("structural call result has an unknown structural domain");
                }
            }
            if exact_payloadless {
                return Ok(());
            }
            if claim_free_result {
                return validate_structural_arguments(
                    module,
                    machine,
                    structural_arguments,
                    &callee.structural_parameters,
                    StructuralArgumentPresentation::Ordinary,
                );
            }
            if actual_result.claims.is_empty()
                || claim_transfers.is_empty()
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index != 0)
                || returned_claim_transfers.is_empty()
            {
                return malformed("structural call requires a nonempty whole-root claim map");
            }
            let mut result_paths = Vec::with_capacity(actual_result.claims.len());
            for binding in &actual_result.claims {
                validate_structural_path(module, actual_result.structural_type, &binding.path)?;
                if result_paths
                    .iter()
                    .any(|previous: &Vec<StructuralPathSegment>| {
                        previous.starts_with(&binding.path) || binding.path.starts_with(previous)
                    })
                {
                    return malformed("structural call result has overlapping claim paths");
                }
                result_paths.push(binding.path.clone());
            }
            let caller_result_claims = actual_result
                .claims
                .iter()
                .map(|binding| (binding.claim, binding.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let callee_claims = callee
                .entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.path.as_slice()))
                .collect::<BTreeMap<_, _>>();
            let transferred_caller_claims = claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<BTreeSet<_>>();
            let returned_callee_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.callee_claim)
                .collect::<BTreeSet<_>>();
            let returned_caller_claims = returned_claim_transfers
                .iter()
                .map(|transfer| transfer.caller_claim)
                .collect::<BTreeSet<_>>();
            if callee_claims.is_empty()
                || callee_claims.len() != callee.entry_claims.len()
                || caller_result_claims.len() != actual_result.claims.len()
                || transferred_caller_claims.len() != claim_transfers.len()
                || returned_callee_claims.len() != returned_claim_transfers.len()
                || returned_caller_claims.len() != returned_claim_transfers.len()
                || returned_callee_claims != callee_claims.keys().copied().collect()
                || returned_caller_claims != caller_result_claims.keys().copied().collect()
                || transferred_caller_claims != caller_result_claims.keys().copied().collect()
                || returned_claim_transfers.iter().any(|transfer| {
                    callee_claims.get(&transfer.callee_claim)
                        != caller_result_claims.get(&transfer.caller_claim)
                })
            {
                return malformed(
                    "structural call returned claims disagree with its result bindings",
                );
            }
            let expected_callee_returns = callee
                .entry_claims
                .iter()
                .map(|claim| claim.claim)
                .collect::<Vec<_>>();
            if callee.blocks.iter().any(|block| {
                matches!(
                    &block.terminator,
                    Terminator::ReturnStructural {
                        returned_claims,
                        ..
                    } if returned_claims != &expected_callee_returns
                )
            }) {
                return malformed(
                    "structural callee return does not preserve its exact entry claim map",
                );
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                claim_transfers
                    .iter()
                    .map(|transfer| (transfer.claim, transfer.argument_index)),
            )?;
        }
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let Some(boundary) = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
            else {
                return malformed("boundary call references an unknown boundary");
            };
            let actual_result = match &operation.result {
                OperationResult::Unit => Some(BoundaryMachineResult::Unit),
                OperationResult::Scalar(result) => {
                    Some(BoundaryMachineResult::Scalar(result.scalar_type))
                }
                OperationResult::Structural(result)
                    if result.projected_qualifications.is_empty() && result.claims.is_empty() =>
                {
                    Some(BoundaryMachineResult::Structural(
                        BoundaryStructuralResultDeclaration {
                            structural_type: result.structural_type,
                            multiplicity: result.multiplicity,
                            qualifications: result.qualifications.clone(),
                        },
                    ))
                }
                OperationResult::Structural(_) => None,
            };
            if actual_result.as_ref() != Some(&boundary.result) {
                return malformed("boundary call result disagrees with its declaration");
            }
            if arguments.len() != boundary.scalar_parameters.len() {
                return malformed("boundary call has the wrong scalar arity");
            }
            if structural_arguments.len() != boundary.structural_parameters.len() {
                return malformed("boundary call has the wrong structural arity");
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                StructuralArgumentPresentation::Boundary,
            )?;
            validate_claim_indices(
                machine,
                structural_arguments,
                completion_receipts
                    .iter()
                    .map(|settlement| (settlement.claim, settlement.argument_index)),
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("port write declares a scalar result");
            }
            if !has_service(module, *service) {
                return malformed("port write references an unknown service");
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            if operation.result != OperationResult::Unit {
                return malformed("trivial affine local establishment declares a scalar result");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return malformed("trivial affine local establishment has no local declaration");
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == *structural_type)
            else {
                return malformed("trivial affine local has an unknown structural type");
            };
            if !matches!(
                &declaration.shape,
                StructuralTypeShape::Record { fields } if fields.is_empty()
            ) {
                return malformed("trivial affine local must have an empty record type");
            }
        }
        OperationKind::EstablishRecord { fields } => {
            let Some(result) = operation.result.structural() else {
                return malformed("record has no structural result");
            };
            if !matches!(result.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted)
                || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() || !result.claims.is_empty()
                || !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
                return malformed("record result custody is noncanonical");
            }
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
            else {
                return malformed("record type is absent");
            };
            let StructuralTypeShape::Record {
                fields: declarations,
            } = &declaration.shape
            else {
                return malformed("record result is not a record");
            };
            if fields.len() != declarations.len() {
                return malformed("record field roster differs");
            }
            for (field, declaration) in fields.iter().zip(declarations) {
                if field.field != declaration.id
                    || declaration.relevance != terminal_psi::BindingRelevance::Relevant
                {
                    return malformed("record field identity differs");
                }
                match (&field.value, &declaration.field_type) {
                    (
                        terminal_psi::RecordFieldValue::Scalar {
                            range_obligation, ..
                        },
                        field_type,
                    ) if field_type.scalar_type().is_some() => {
                        if range_obligation.is_some()
                            != matches!(field_type, StructuralFieldType::BoundedInteger(_))
                        {
                            return malformed("record range obligation differs");
                        }
                    }
                    (
                        terminal_psi::RecordFieldValue::Structural(argument),
                        StructuralFieldType::Structural(_),
                    ) if argument.access == terminal_psi::StructuralAccess::Owned
                        && argument.path.is_empty() => {}
                    _ => return malformed("record field operand differs"),
                }
            }
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            let Some(result) = operation.result.structural() else {
                return malformed("primitive local has no structural result");
            };
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !machine.structural_places.iter().any(|place| {
                    place.id == result.place && matches!(place.kind,
                        StructuralPlaceKind::OperationResult { producer, structural_type }
                            if producer == operation.id && structural_type == result.structural_type)
                })
                || !module.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                })
            {
                return malformed("primitive local result custody is noncanonical");
            }
        }
        OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
            if operation.result != OperationResult::Unit
                || !module
                    .dynamic_dispatch
                    .stored_descriptors
                    .iter()
                    .any(|descriptor| {
                        descriptor.owner == machine.id
                            && descriptor.ordinal == *descriptor_ordinal
                            && descriptor.establishment_operation == operation.id
                    })
            {
                return malformed("dynamic descriptor storage has no exact catalog row");
            }
        }
        OperationKind::Call { .. } => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar call declares a Unit result");
            }
        }
        OperationKind::CallDynamicUnit { .. } | OperationKind::CallDynamicParameterUnit { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("dynamic Unit call declares a result value");
            }
        }
        _ => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar operation declares a Unit result");
            }
        }
    }
    Ok(())
}

// Claim-free results share the ordinary exact call signature. A reference
// carrier still owes loan custody: the independent verifier reconstructs its
// source mapping and lifetime instead of treating an empty claim roster as
// permission to discard or duplicate the referent.
// Both codec entrances require that validation before accepting a module. The
// wire format has no separate payload-shape distinction for claim-free calls;
// a second scalar/flat-record whitelist here would reject verified nested and
// reference-bearing records without adding an encoding or custody check.
fn is_claim_free_structural_call(
    result: &terminal_psi::StructuralOperationResult,
    callee: &TerminalMachine,
) -> bool {
    matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.outcome_specific_ensures.is_empty()
}

fn callee_exact_payloadless_return(callee: &TerminalMachine) -> bool {
    let Some(result) = callee.result.structural() else {
        return false;
    };
    let mut has_return = false;
    for block in &callee.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        has_return = true;
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(operation) = callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
            return false;
        };
        if !matches!(&operation.kind, OperationKind::EstablishScalarCase { fields, .. } if fields.is_empty())
            || !operation
                .result
                .structural()
                .is_some_and(|operation_result| {
                    operation_result.place == *source
                        && operation_result.structural_type == result.structural_type
                        && operation_result.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        && operation_result.qualifications.is_empty()
                        && operation_result.claims.is_empty()
                })
        {
            return false;
        }
    }
    has_return
        && callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            })
}

#[derive(Clone, Copy)]
enum StructuralArgumentPresentation {
    Ordinary,
    Boundary,
}

fn validate_structural_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    presentation: StructuralArgumentPresentation,
) -> Result<(), CodecError> {
    for (argument, expected) in arguments.iter().zip(expected) {
        let Some(actual_type) = structural_place_type(machine, argument.place) else {
            return malformed("structural argument references an unknown structural place");
        };
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place);
        let result = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place);
        let owned_array_payload = terminal_semantics::scalar_array_leaf_shape(
            module.structural_types.iter(),
            actual_type,
        )
        .is_some()
            && (parameter.is_some_and(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            }) || result.is_some());
        if owned_array_payload {
            let plain_source = parameter.is_some_and(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            }) || result.is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            });
            if matches!(presentation, StructuralArgumentPresentation::Boundary)
                || !plain_source
                || argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || expected.access != terminal_psi::StructuralAccess::Owned
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !expected.qualifications.is_empty()
                || !expected.projected_qualifications.is_empty()
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
            {
                return malformed(
                    "primitive-array argument requires whole plain owned internal-call custody",
                );
            }
        }
        // The canonical form retains the real array type at both ordinary and
        // boundary calls; the shared extent check recognizes the exact loan.
        if machine.structural_parameters.iter().any(|actual| {
            terminal_semantics::mutable_fixed_byte_array_extent(module, actual, argument, expected)
                .is_some()
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
        }) {
            continue;
        }
        let inline_byte_view = match presentation {
            StructuralArgumentPresentation::Boundary => true,
            StructuralArgumentPresentation::Ordinary => {
                expected.multiplicity == StructuralMultiplicity::Unrestricted
                    && !argument.path.is_empty()
                    && argument
                        .path
                        .iter()
                        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    && machine.structural_parameters.iter().any(|parameter| {
                        parameter.place == argument.place
                            && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    })
            }
        };
        if inline_byte_view
            && terminal_semantics::boundary_buffer_capacity(module, actual_type, argument, expected)
                .is_some()
        {
            continue;
        }
        // The shared counterpart: a borrowed view reads the field's live bytes.
        // Boundary actuals admit it directly; ordinary borrowed calls admit the
        // same unrestricted subloan the verifier's shared-field rule describes.
        let shared_byte_field_loan = terminal_semantics::shared_boundary_buffer_capacity(
            module,
            actual_type,
            argument,
            expected,
        )
        .is_some()
            && match presentation {
                StructuralArgumentPresentation::Boundary => true,
                StructuralArgumentPresentation::Ordinary => {
                    expected.multiplicity == StructuralMultiplicity::Unrestricted
                        && is_bounded_structural_scalar_store_path(&argument.path)
                        && machine.structural_parameters.iter().any(|parameter| {
                            parameter.place == argument.place
                                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        })
                }
            };
        if shared_byte_field_loan {
            continue;
        }
        let actual_type = validate_structural_path(module, actual_type, &argument.path)?;
        if actual_type != expected.structural_type {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
}

fn structural_place_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            machine.structural_places.iter().find_map(|declaration| {
                if declaration.id != place {
                    return None;
                }
                match declaration.kind {
                    StructuralPlaceKind::BlockParameter { block, position } => machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)?
                        .structural_parameters
                        .get(position as usize)
                        .filter(|parameter| parameter.place == place)
                        .map(|parameter| parameter.structural_type),
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => Some(structural_type),
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::Result => None,
                }
            })
        })
}

/// Result cleanup may reuse an existing owned operation result place, but a
/// merely matching type declaration does not establish that custody.
fn is_plain_affine_call_result(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    let mut places = machine
        .structural_places
        .iter()
        .filter(|candidate| candidate.id == place);
    let Some(declaration) = places.next() else {
        return false;
    };
    if places.next().is_some() {
        return false;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return false;
    };
    let mut operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let Some(operation) = operations.next() else {
        return false;
    };
    // An atomic record establishment is the same claim-free whole owner: a
    // selected projected child leaves its sibling residual to die on the edge.
    if operations.next().is_some()
        || !matches!(
            operation.kind,
            OperationKind::CallStructuralWithScalarArguments { .. }
                | OperationKind::BoundaryCall { .. }
                | OperationKind::EstablishRecord { .. }
        )
    {
        return false;
    }
    matches!(&operation.result, OperationResult::Structural(result)
        if result.place == place
            && result.structural_type == structural_type
            && result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
}

fn validate_claim_indices(
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    claims: impl Iterator<Item = (ClaimId, u32)>,
) -> Result<(), CodecError> {
    for (claim, argument_index) in claims {
        let Some(argument) = arguments.get(argument_index as usize) else {
            return malformed("claim action has an unknown structural argument index");
        };
        let Some(entry_claim) = machine
            .entry_claims
            .iter()
            .find(|entry_claim| entry_claim.claim == claim)
        else {
            return malformed("claim action references an unknown entry claim");
        };
        // A returned claim keeps its identity but moves to the successful
        // operation's result place. The codec checks that exact occurrence;
        // the verifier, not this structural check, establishes its liveness.
        let path = if entry_claim.input == argument.place {
            Some(entry_claim.path.as_slice())
        } else {
            machine.structural_places.iter().find_map(|declaration| {
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return None;
                };
                if declaration.id != argument.place {
                    return None;
                }
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| {
                        if operation.id != producer
                            || !matches!(
                                operation.kind,
                                OperationKind::CallStructural { .. }
                                    | OperationKind::CallStructuralWithScalarArguments { .. }
                            )
                        {
                            return None;
                        }
                        let result = operation.result.structural()?;
                        if result.place != argument.place
                            || result.structural_type != structural_type
                            || result.multiplicity != StructuralMultiplicity::Linear
                        {
                            return None;
                        }
                        result
                            .claims
                            .iter()
                            .find(|binding| binding.claim == claim)
                            .map(|binding| binding.path.as_slice())
                    })
            })
        };
        if !path.is_some_and(|path| argument.path.is_empty() || path == argument.path) {
            return malformed("claim action does not match its structural argument path");
        }
    }
    Ok(())
}

fn has_structural_type(module: &TerminalModule, id: StructuralTypeId) -> bool {
    module
        .structural_types
        .iter()
        .any(|declaration| declaration.id == id)
}

fn validate_structural_type_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: StructuralTypeId,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("structural type graph contains a by-value cycle");
        }
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == id)
            .expect("structural field targets were validated before graph traversal");
        match &declaration.shape {
            // A reference does not inline its referent storage. The target's
            // existence was checked above; following it here invents a by-value cycle.
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(module, *element, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.structural_types {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_service_parent_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: ServiceId,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("service parent graph contains a cycle");
        }
        let declaration = module
            .services
            .iter()
            .find(|declaration| declaration.id == id)
            .expect("service parent targets were validated before graph traversal");
        for parent in &declaration.parents {
            visit(module, *parent, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.services {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    for declaration in &module.services {
        for parent in &declaration.parents {
            let parent = module
                .services
                .iter()
                .find(|candidate| candidate.id == *parent)
                .expect("service parent targets were validated before closure validation");
            if parent
                .parents
                .iter()
                .any(|ancestor| !declaration.parents.contains(ancestor))
            {
                return malformed("service parent closure is incomplete");
            }
        }
    }
    Ok(())
}

fn has_service(module: &TerminalModule, id: ServiceId) -> bool {
    module
        .services
        .iter()
        .any(|declaration| declaration.id == id)
}

fn require_known_services(
    module: &TerminalModule,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    if services
        .iter()
        .any(|service| !has_service(module, *service))
    {
        return malformed("published service ceiling references an unknown service");
    }
    Ok(())
}

fn require_unique_nonempty_identities<'a>(
    identities: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Result<(), CodecError> {
    let mut seen = BTreeSet::new();
    for identity in identities {
        if identity.is_empty() || !seen.insert(identity) {
            return Err(CodecError::MalformedStructuralFoundation(label));
        }
    }
    Ok(())
}
