//! Source-independent retention of checked foreign shared-borrow custody.

use super::*;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn unconstrained_type(checked: &CheckedTrees, mut ty: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(ty)
    {
        ty = *base_type;
    }
    ty
}

fn lower_projection(
    checked: &CheckedTrees,
    projection: &language_semantics::content::ContentProjectionPlan,
) -> Result<terminal_psi::RetainedBorrowContentProjection, LoweringError> {
    let exact = checked
        .facts
        .qualifications
        .content
        .for_semantic_domain(projection.semantic_domain)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow projection is absent from checked content facts",
        ))?;
    if exact != projection {
        return unsupported("retained-borrow projection drifted from checked content facts");
    }
    let projection = content_conservation::lower_structural_content_projection(
        checked,
        projection.semantic_domain,
        &projection.carrier_identity,
    )?
    .ok_or(LoweringError::Unsupported(
        "retained-borrow projection has no complete Terminal definition",
    ))?;
    Ok(terminal_psi::RetainedBorrowContentProjection {
        semantic_domain: DomainSemanticId::new(projection.identity.domain.get())
            .ok_or(LoweringError::InvalidContentDomainIdentity)?,
        carrier_identity: exact.carrier_identity.clone(),
        projection,
    })
}

fn lower_custody(
    checked: &CheckedTrees,
    fact: &checked_trees::RetainedBorrowCustodyFact,
) -> Result<terminal_psi::RetainedBorrowCustody, LoweringError> {
    let requirements = checked
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .filter(|signature| signature.symbol == fact.callable)
        .collect::<Vec<_>>();
    let [signature] = requirements.as_slice() else {
        return unsupported("retained-borrow callable is not one exact boundary requirement");
    };
    let callable_identity = checked_unit_boundary_identity(checked, fact.callable)?;
    let callable_lifetime_parameter_count = u32::try_from(signature.lifetime_parameters.len())
        .map_err(|_| {
            LoweringError::Unsupported("retained-borrow callable lifetime count exceeds u32")
        })?;
    if usize::try_from(fact.callable_lifetime_parameter_ordinal)
        .ok()
        .and_then(|ordinal| signature.lifetime_parameters.get(ordinal))
        != Some(&fact.lifetime)
    {
        return unsupported("retained-borrow callable lifetime ordinal does not replay");
    }

    let language_semantics::content::ContentPlaceRoot::Parameter {
        position,
        symbol,
        name,
        is_self,
    } = &fact.source.root
    else {
        return unsupported("retained-borrow source is not a direct parameter");
    };
    let parameter = checked
        .state_signature_parameters(signature)
        .get(*position as usize)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow source parameter is out of range",
        ))?;
    if parameter.symbol != *symbol
        || parameter.name.as_str() != name
        || parameter.is_self != *is_self
        || *is_self
        || fact.source.version != language_semantics::content::ContentPlaceVersion::Entry
        || !fact.source.segments.is_empty()
        || fact.access != language_semantics::ReferenceAccess::Shared
    {
        return unsupported("retained-borrow source place or access does not replay");
    }
    let source_ty = unconstrained_type(checked, parameter.type_reference);
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        lifetime: Some(source_lifetime),
    } = checked.type_reference_table.type_reference(source_ty)
    else {
        return unsupported("retained-borrow source is not one explicit direct shared reference");
    };
    if source_lifetime != &fact.lifetime
        || checked
            .normalized_type_identity(unconstrained_type(checked, *referee))
            .into_string()
            != fact.source_projection.carrier_identity
    {
        return unsupported("retained-borrow source lifetime or nominal carrier drifted");
    }

    if fact.result.version != language_semantics::content::ContentPlaceVersion::Current
        || !matches!(
            fact.result.root,
            language_semantics::content::ContentPlaceRoot::Result
        )
        || !fact.result.segments.is_empty()
    {
        return unsupported("retained-borrow result place does not replay");
    }
    let result_ty = unconstrained_type(checked, signature.return_type);
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = checked.type_reference_table.type_reference(result_ty)
    else {
        return unsupported("retained-borrow result is not the bounded nominal lifetime carrier");
    };
    if *base_symbol != fact.result_data {
        return unsupported("retained-borrow result nominal owner does not replay");
    }
    if lifetime_arguments.as_slice() != [fact.lifetime.clone()]
        || fact.result_lifetime_argument_ordinal != 0
    {
        return unsupported("retained-borrow result lifetime slot does not replay");
    }
    if !checked
        .type_reference_table
        .type_reference_handles(*arguments)
        .is_empty()
    {
        return unsupported("retained-borrow result has runtime generic arguments");
    }
    if checked.type_multiplicity(signature.return_type) != Multiplicity::Linear {
        return unsupported("retained-borrow result is not linear");
    }
    if fact.retained_semantic_domain != fact.result_projection.semantic_domain
        || fact.source_projection.algebra != fact.result_projection.algebra
    {
        return unsupported("retained-borrow result projection does not replay");
    }

    let source_identity = parameter.name.as_str().to_owned();
    let source_projection = lower_projection(checked, &fact.source_projection)?;
    let result_projection = lower_projection(checked, &fact.result_projection)?;
    Ok(terminal_psi::RetainedBorrowCustody {
        callable_identity,
        source: terminal_psi::RetainedBorrowPlace {
            version: ContentPlaceVersion::Entry,
            root: terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position: *position,
                identity: source_identity,
                is_self: false,
            },
            segments: Vec::new(),
        },
        result: terminal_psi::RetainedBorrowPlace {
            version: ContentPlaceVersion::Current,
            root: terminal_psi::RetainedBorrowPlaceRoot::Result,
            segments: Vec::new(),
        },
        access: StructuralAccess::SharedBorrow,
        callable_lifetime_parameter_count,
        callable_lifetime_parameter_ordinal: fact.callable_lifetime_parameter_ordinal,
        result_nominal_identity: fact.result_projection.carrier_identity.clone(),
        result_multiplicity: StructuralMultiplicity::Linear,
        result_lifetime_argument_count: 1,
        result_lifetime_argument_ordinal: fact.result_lifetime_argument_ordinal,
        result_lifetime_slot_is_erased: true,
        retained_semantic_domain: result_projection.semantic_domain,
        source_projection,
        result_projection,
    })
}

pub(super) fn retain_foreign_borrow_custodies(
    checked: &CheckedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let mut rows = checked
        .facts
        .qualifications
        .content
        .retained_borrow_custodies
        .iter()
        .map(|fact| lower_custody(checked, fact))
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by(|left, right| left.callable_identity.cmp(&right.callable_identity));
    if rows
        .windows(2)
        .any(|pair| pair[0].callable_identity == pair[1].callable_identity)
    {
        return unsupported("retained-borrow callable has multiple custody rows");
    }
    let mut next_boundary = module
        .boundary_machines
        .iter()
        .map(|boundary| boundary.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "retained-borrow boundary identity space is exhausted",
        ))?;
    for row in rows {
        if module
            .boundary_machines
            .iter()
            .any(|boundary| boundary.identity == row.callable_identity)
        {
            return unsupported("retained-borrow callable collides with an executable boundary");
        }
        let id = BoundaryMachineId::new(next_boundary).ok_or(LoweringError::Unsupported(
            "retained-borrow boundary identity is invalid",
        ))?;
        next_boundary = next_boundary
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "retained-borrow boundary identity space is exhausted",
            ))?;
        module.boundary_machines.push(BoundaryMachineDeclaration {
            id,
            identity: row.callable_identity.clone(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: vec![BoundaryContentGuarantee::RetainedBorrow(row)],
            published_service_ceiling: Vec::new(),
        });
    }
    Ok(())
}
