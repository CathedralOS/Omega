//! Checked operand capture through owned movement or unrestricted copying.
//! Psi owns logical establishment; this boundary checks shape, live source
//! contracts and the prohibition on manufacturing routed authority.

use super::*;

/// The source checker owns predicate establishment; decoding owns the typed
/// transfer shape and cannot reinterpret a receiving roster as routed issuance.
/// Exact live claim transfer is checked by ordered ownership, as for aggregate
/// construction. This operation never changes the selected carrier.
pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_move_structural_value(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::MoveStructuralValue { source } = &operation.kind else {
        return malformed("owned value transfer dispatch differs");
    };
    let Some(result) = operation.result.structural() else {
        return malformed("owned value transfer has no structural result");
    };
    let whole_multiplicity = machine
        .structural_parameter(source.place)
        .map(|parameter| parameter.multiplicity)
        .or_else(|| {
            machine
                .producing_operation(source.place)
                .and_then(|(producer, _)| producer.result.structural())
                .map(|result| result.multiplicity)
        });
    if source.access != terminal_psi::StructuralAccess::Owned
        || result.multiplicity == StructuralMultiplicity::Unrestricted
        || whole_multiplicity == Some(StructuralMultiplicity::Unrestricted)
        || (!source.path.is_empty()
            && result.multiplicity != StructuralMultiplicity::Linear
            && result.claims.iter().any(|claim| claim.path.is_empty()))
        || !result.qualification_establishments.is_empty()
        || source.place == result.place
        || (source.path.is_empty() && whole_multiplicity != Some(result.multiplicity))
        || machine
            .structural_parameter(source.place)
            .is_some_and(|parameter| parameter.access != terminal_psi::StructuralAccess::Owned)
    {
        return malformed("owned value transfer requires owned custody and cannot issue authority");
    }
    let root = structural_place_type(machine, source.place).ok_or(
        CodecError::MalformedStructuralFoundation("owned transfer source is absent"),
    )?;
    let selected = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        root,
        &source.path,
    )
    .map(|(_, selected)| selected)
    .ok_or(CodecError::MalformedStructuralFoundation(
        "owned transfer requires a finite owned path",
    ))?;
    if selected.id != result.structural_type {
        return malformed("owned value transfer changes its carrier");
    }
    if !matches!(machine.structural_place(result.place).map(|place| place.kind),
        Some(StructuralPlaceKind::OperationResult { producer, structural_type })
            if producer == operation.id && structural_type == result.structural_type)
    {
        return malformed("owned value transfer result is not a fresh occurrence");
    }
    let position = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation.id)
                .and_then(|ordinal| {
                    u32::try_from(ordinal)
                        .ok()
                        .map(|ordinal| (block.id, ordinal))
                })
        })
        .ok_or(CodecError::MalformedStructuralFoundation(
            "owned transfer position is absent",
        ))?;
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let contract = terminal_semantics::descriptor_results::current_selected_payload_contract(
        &module.structural_types,
        &module.structural_domains,
        &graph,
        machine.id,
        position,
        source.place,
        &source.path,
        &|place| terminal_semantics::descriptor_results::terminal_payload_contract(machine, place),
    )
    .map_err(CodecError::MalformedStructuralFoundation)?;
    if contract.carrier != result.structural_type
        || contract
            .qualifications
            .iter()
            .any(|domain| !result.qualifications.contains(domain))
        || contract
            .projected_qualifications
            .iter()
            .any(|row| !result.projected_qualifications.contains(row))
    {
        return malformed("owned value transfer loses source memberships");
    }
    let added = result
        .qualifications
        .iter()
        .filter(|domain| !contract.qualifications.contains(domain))
        .chain(
            result
                .projected_qualifications
                .iter()
                .filter(|row| !contract.projected_qualifications.contains(row))
                .map(|row| &row.domain),
        );
    validate_transfer_domain_additions(module, added)
}

/// Both owned moves and unrestricted captures can introduce memberships that
/// Psi checked at this exact occurrence. Consumption validates the carrier and
/// custody, not the erased predicate proof. Forwarded routed memberships still
/// need the current source contract; copying cannot manufacture their origin.
/// Source checking owns aliases and inherited restrictions: an empty direct
/// route roster is not an independent proof of predicate-only establishment.
fn validate_transfer_domain_additions<'domain>(
    module: &TerminalModule,
    added: impl Iterator<Item = &'domain semantic_vocabulary::StructuralDomainId>,
) -> Result<(), CodecError> {
    for domain in added {
        let declaration = module
            .structural_domains
            .iter()
            .find(|row| row.id == *domain)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "value transfer domain is absent",
            ))?;
        if !declaration.establishment_routes.is_empty() {
            return malformed("value transfer cannot establish a routed domain");
        }
    }
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_leaf_copy(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (terminal_psi::structural_leaf_copy_pair_pattern!()) = &operation.kind else {
        unreachable!("dispatched validate_structural_leaf_copy")
    };
    if operation.result.structural().is_none_or(|result| {
        (result.multiplicity != StructuralMultiplicity::Unrestricted
            // Descriptor disposal is affine even though shared access is
            // copyable. The copy retains backing and parent obligations;
            // ordinary affine payloads still require an owned transfer.
            && !(result.multiplicity == StructuralMultiplicity::Affine
                && terminal_semantics::is_copyable_carrier(
                    module.structural_types.iter(), result.structural_type,
                )))
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
    }) {
        return malformed("leaf copy has invalid multiplicity or qualification custody");
    }
    let result = operation
        .result
        .structural()
        .expect("checked structural copy result");
    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() {
        let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
            return malformed("case leaf copy does not authenticate memberships");
        };
        let descriptors = terminal_semantics::descriptor_results::terminal_graph(module);
        let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
            &module.structural_types,
            &descriptors,
        );
        let mut history_failures = Vec::new();
        let mut query_phase = "root";
        let memberships = (|| {
            let root = terminal_semantics::descriptor_results::terminal_payload_contract(
                machine, *source,
            )?;
            query_phase = "query";
            let query = terminal_semantics::descriptor_results::descriptor_contents_query(
                &module.structural_types,
                *source,
                root.carrier,
                path,
                None,
            )?;
            query_phase = "position";
            let position = machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|candidate| candidate.id == operation.id)
                    .and_then(|ordinal| {
                        u32::try_from(ordinal)
                            .ok()
                            .map(|ordinal| (block.id, ordinal))
                    })
            })?;
            query_phase = "targets";
            let targets = analysis.payload_targets(machine.id, position, &query)?;
            query_phase = "memberships";
            terminal_semantics::descriptor_results::payload_memberships(
                &module.structural_types,
                &module.structural_domains,
                targets,
                |place| {
                    terminal_semantics::descriptor_results::current_payload_contract(
                        &module.structural_types,
                        &module.structural_domains,
                        &descriptors,
                        machine.id,
                        position,
                        place,
                        &|source| {
                            terminal_semantics::descriptor_results::terminal_payload_contract(
                                machine, source,
                            )
                        },
                    )
                    .map_err(|reason| history_failures.push((place, reason)))
                    .ok()
                },
            )
        })()
        .and_then(|contract| {
            query_phase = "presentation";
            contract.presented(
                &module.structural_types,
                &module.structural_domains,
                result.structural_type,
            )
        });
        let Some(contract) = memberships
            .as_ref()
            .filter(|contract| contract.carrier == result.structural_type)
        else {
            return Err(CodecError::MalformedStructuralOperation {
                machine: machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{:?}", machine.id)),
                operation: operation.id,
                reason: format!(
                    "leaf copy memberships do not follow from every current payload source: source={source:?}, path={path:?}, requested={:?}/{:?}/{:?}, current={memberships:?}, history={history_failures:?}, phase={query_phase}, machine_present={}",
                    result.structural_type,
                    result.qualifications,
                    result.projected_qualifications,
                    descriptors.iter().any(|row| row.id == machine.id),
                ),
            });
        };
        validate_transfer_domain_additions(
            module,
            result
                .qualifications
                .iter()
                .filter(|domain| !contract.qualifications.contains(domain))
                .chain(
                    result
                        .projected_qualifications
                        .iter()
                        .filter(|row| !contract.projected_qualifications.contains(row))
                        .map(|row| &row.domain),
                ),
        )?;
    }
    // Full module validation independently checks the source's readable
    // access, the resolved leaf type, custody and liveness.
    Ok(())
}
