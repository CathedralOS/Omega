//! A Build choice names the storage of an opaque value. Check each authored
//! choice, then retain only the ones the authoritative Build actually executed.

use super::authored_requests::{AuthoredRequests, has_operation, refusal};
use crate::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest, ProductOperandRole,
};
use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::opaque_representations::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
};
use crate::source_state::product_operand::ProductDeclarationLookup;
use crate::source_state::type_identity::ExactOwnerTypeIdentityRequest;
use crate::source_state::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use application_identity::conformance_identity::{
    ConformanceIdentityDescription, source_identity_commitments,
};
use diagnostics::Diagnostic;
use language_core::core_vocabulary::CoreVocabulary;
use language_core::{DataSupplyMode, Multiplicity};
use std::sync::Arc;
use symbols::SymbolHandle;

struct CarrierChoice {
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    carrier_type: TypeReferenceHandle,
    application: ClosedConformanceApplication,
    copy: OpaqueRepresentationCopyDisposition,
}

/// Every runtime field and every case can own storage, even if a particular
/// handler never inspects it. Independently owned cleanup cannot migrate
/// from the carrier to the opaque value.
fn inert_carrier(
    typed: &TypedTrees,
    root: TypeReferenceHandle,
    copy: OpaqueRepresentationCopyDisposition,
) -> Result<(), &'static str> {
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut checked = Vec::new();
    while let Some((reference, leaving)) = pending.pop() {
        if leaving {
            active.pop();
            checked.push(reference);
            continue;
        }
        if checked.contains(&reference) {
            continue;
        }
        if !typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("opaque carrier has an unresolved field type");
        }
        if active.contains(&reference) {
            return Err("opaque carrier contains recursive by-value storage");
        }
        active.push(reference);
        pending.push((reference, true));
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => {}
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in typed.type_reference_table.constraints(*constraints) {
                    use crate::source_state::types::TypeConstraintNode;
                    match constraint {
                        TypeConstraintNode::ArithmeticDomain(_) => {}
                        TypeConstraintNode::Domain(domain)
                            if typed.domain_definition(domain.symbol).is_some_and(
                                |definition| {
                                    definition.cleanup_selection.is_none()
                                        && definition.alias.is_none()
                                },
                            ) => {}
                        _ => {
                            return Err(
                                "opaque carrier field needs a domain with known inert lifecycle",
                            );
                        }
                    }
                }
                pending.push((*base_type, false));
            }
            TypeReferenceNode::FixedArray { element_type, .. }
                if typed.closed_fixed_array_length(reference).is_some() =>
            {
                pending.push((*element_type, false))
            }
            TypeReferenceNode::Named { .. }
                if typed.primitive_type_reference(reference).is_some() => {}
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let data = typed
                    .closed_data_definition(reference)
                    .ok_or("opaque carrier needs a closed concrete field type")?;
                if data.supply_mode != DataSupplyMode::CheckedShape
                    || data.cleanup_selection.is_some()
                    || data.properties.multiplicity == Multiplicity::Linear
                {
                    return Err("opaque carrier contains independent ownership or cleanup");
                }
                if copy == OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
                    && data.properties.multiplicity != Multiplicity::Unrestricted
                {
                    return Err("a copyable opaque value needs a structurally copyable carrier");
                }
                for member in typed.data_members(data) {
                    match member {
                        DataMember::Field(field) if !field.relevance.is_erased() => {
                            pending.push((field.type_reference, false))
                        }
                        DataMember::Variant(case) => pending.extend(
                            typed
                                .data_payload_fields(case)
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .map(|field| (field.type_reference, false)),
                        ),
                        DataMember::Field(_) => {}
                    }
                }
            }
            _ => {
                return Err(
                    "opaque carrier needs fixed, inert storage in every runtime field and case",
                );
            }
        }
    }
    Ok(())
}

fn form(
    typed: &TypedTrees,
    declarations: &ProductDeclarationLookup<'_>,
    sources: &[(source_text::SourceId, [u8; 32])],
    request: &ProductDeclarationRequest,
) -> Result<CarrierChoice, String> {
    request.validate_shape()?;
    let [subject, implementation] = request.operands.as_ref() else {
        return Err("opaque representation needs a datum and a named implementation".into());
    };
    if subject.application.is_some() || implementation.application.is_some() {
        return Err("opaque representation operand needs an unapplied declaration".into());
    }
    let opaque = declarations
        .resolve(&subject.path, subject.source_span, ProductOperandRole::Data)
        .and_then(|symbol| typed.data_definition_by_symbol(symbol))
        .ok_or_else(|| {
            format!(
                "opaque representation subject `{}` has no visible data declaration",
                subject.path
            )
        })?;
    if opaque.supply_mode != DataSupplyMode::BoundaryOpaque {
        return Err(format!(
            "selected subject `{}` is not boundary-opaque data",
            subject.path
        ));
    }
    let conformance = declarations
        .resolve(
            &implementation.path,
            implementation.source_span,
            ProductOperandRole::Representation,
        )
        .and_then(|symbol| typed.conformance_by_symbol(symbol))
        .ok_or_else(|| {
            format!(
                "opaque representation `{}` has no visible named implementation",
                implementation.path
            )
        })?;
    if typed.core_vocabulary(conformance.trait_symbol) != Some(CoreVocabulary::OpaqueRepresentation)
    {
        return Err(format!(
            "implementation `{}` must satisfy the bundled OpaqueRepresentation trait",
            implementation.path
        ));
    }
    if !conformance.type_parameters.is_empty()
        || !conformance.lifetime_parameters.is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !conformance.rows.is_empty()
    {
        return Err("opaque representation needs a closed, memberless implementation".into());
    }
    let [opaque_type, carrier_type] = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        return Err("opaque representation needs exactly two trait arguments".into());
    };
    if !matches!(typed.type_reference_table.type_reference(*opaque_type), TypeReferenceNode::Named {symbol, ..} if *symbol == opaque.symbol)
        || !opaque.type_parameters.is_empty()
        || !opaque.lifetime_parameters.is_empty()
    {
        return Err(
            "opaque representation implementation represents a different or open datum".into(),
        );
    }
    let carrier = typed
        .closed_data_definition(*carrier_type)
        .ok_or("opaque representation lacks a concrete data carrier")?;
    let copy = if opaque.properties.multiplicity == Multiplicity::Unrestricted {
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    } else {
        OpaqueRepresentationCopyDisposition::PlacementOnly
    };
    inert_carrier(typed, *carrier_type, copy)?;
    let nominal = |symbol| {
        typed
            .exact_owner_nominal_identity(symbol, sources)
            .and_then(|id| typed.type_identity_text(id))
            .ok_or("opaque representation declaration has no exact source identity")
    };
    let trait_arguments = [*opaque_type, *carrier_type]
        .into_iter()
        .map(|type_reference| {
            typed
                .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                    type_reference,
                    lifetime_scope: None,
                    binders: &[],
                    substitutions: &[],
                    exact_toolchain_sources: sources,
                })
                .and_then(|id| typed.type_identity_text(id))
                .ok_or("opaque representation has an unidentified trait argument")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = ConformanceIdentityDescription {
        declaration: &nominal(conformance.symbol)?,
        trait_name: &nominal(conformance.trait_symbol)?,
        trait_arguments: &trait_arguments,
        ..Default::default()
    }
    .encoded_bytes();
    let (report_fingerprint, digest) = source_identity_commitments(&bytes);
    Ok(CarrierChoice {
        opaque: opaque.symbol,
        carrier: carrier.symbol,
        carrier_type: *carrier_type,
        copy,
        application: ClosedConformanceApplication {
            declaration: conformance.symbol,
            trait_definition: conformance.trait_symbol,
            trait_arguments,
            report_fingerprint,
            commitment: ClosedConformanceApplicationCommitment::from_digest(digest),
            identity_encoding: Arc::from(bytes),
            ..Default::default()
        },
    })
}

pub fn collect_opaque_representations(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    let operation = BuildOperation::RepresentationSelection;
    if !has_operation(declared, executed, operation) {
        return Ok(Vec::new());
    }
    let authored = AuthoredRequests::new(typed, declared, operation)?;
    let declarations = ProductDeclarationLookup::new(
        typed,
        &[ProductOperandRole::Data, ProductOperandRole::Representation],
    );
    let sources = source_admission::source_identity::toolchain_source_identities(
        typed.symbols.source_files(),
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let formations = authored
        .rows
        .iter()
        .map(|row| {
            form(typed, &declarations, &sources, row.request)
                .map_err(|reason| refusal(row.request, reason))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut selected = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == operation)
    {
        let (index, _, activation) = authored.execution(activation, event)?;
        if event.target.is_some() || event.composition.is_some() {
            return Err(refusal(
                &event.request,
                "opaque representation selection has an unexpected target or composition argument",
            ));
        }
        let choice = &formations[index];
        selected.push(OpaqueRepresentationSelection::from_validated_application(
            choice.opaque,
            choice.carrier,
            choice.carrier_type,
            choice.application.clone(),
            OpaqueRepresentationLifecycleDisposition::Inert,
            choice.copy,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            activation,
            typed
                .symbols
                .rejoin_product_occurrence(event.request.source_span),
        ));
    }
    selected.sort_unstable_by_key(|row| (row.opaque().arena_index(), row.opaque().generation()));
    for pair in selected.windows(2) {
        if pair[0].opaque() == pair[1].opaque() {
            return Err(vec![
                Diagnostic::error(format!(
                    "Build selected more than one representation for `{}`",
                    typed.symbols.name(pair[1].opaque())
                ))
                .with_source_span(pair[1].source_span()),
            ]);
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::collect_opaque_representations;
    use crate::declaration_selection::{
        BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
    };
    use crate::source_state::TypedTrees;
    use std::sync::Arc;

    fn request(operation: BuildOperation) -> Arc<ProductDeclarationRequest> {
        Arc::new(ProductDeclarationRequest {
            source_span: source_text::SourceSpan::default(),
            operation,
            operands: Box::new([]),
        })
    }

    fn executed(request: Arc<ProductDeclarationRequest>) -> ExecutedProductSelection {
        ExecutedProductSelection {
            test_group: None,
            static_environment: None,
            target: None,
            request,
            selecting_machine: None,
            composition: None,
        }
    }

    // Preparation installs these choices without walking the Build requests
    // again, so a request with no authored occurrence or no Build activation
    // must be refused here, before it can become a choice.
    #[test]
    fn representation_requests_without_their_original_authority_are_refused() {
        let selection = request(BuildOperation::RepresentationSelection);
        let declared = [Arc::clone(&selection)];
        let ran = [executed(selection)];
        for (declared, ran, expected) in [
            (
                &declared[..],
                &[][..],
                "Build request has no original authored occurrence",
            ),
            (
                &[][..],
                &ran[..],
                "executed Build request has no original Build activation",
            ),
        ] {
            let refusal =
                collect_opaque_representations(&TypedTrees::default(), None, declared, ran)
                    .expect_err(expected);
            assert_eq!(refusal[0].message, expected);
        }
    }

    #[test]
    fn a_build_that_selects_no_representation_owes_no_representation_choice() {
        let audit = request(BuildOperation::ServiceAudit);
        let selected = collect_opaque_representations(
            &TypedTrees::default(),
            Some(symbols::SymbolHandle::invalid()),
            &[Arc::clone(&audit)],
            &[executed(audit)],
        )
        .expect("a Build without a representation selection is admitted");
        assert!(selected.is_empty());
    }
}
