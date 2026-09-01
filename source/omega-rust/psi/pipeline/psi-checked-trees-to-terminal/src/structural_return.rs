//! General structural-result transfer lowering.

use super::*;

pub(super) fn lower_structural_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_structural_returns;
    let Some(returned_plan) = plan.structural_parameters.first() else {
        return unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup",
        );
    };
    let discarded_plans = &plan.structural_parameters[1..];
    let expected_discards = (1..plan.structural_parameters.len())
        .rev()
        .map(|position| u32::try_from(position).ok())
        .collect::<Option<Vec<_>>>()
        .ok_or(LoweringError::Unsupported(
            "structural result cleanup position is not representable",
        ))?;
    let expected_local_discards = plan
        .trivial_affine_locals
        .iter()
        .rev()
        .map(|local| local.declaration_ordinal)
        .collect::<Vec<_>>();
    if plan.returned_parameter_index != 0
        || plan.trivial_affine_discards != expected_discards
        || plan.trivial_affine_local_discard_ordinals != expected_local_discards
        || plan
            .trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, local)| {
                u32::try_from(index).ok() != Some(local.declaration_ordinal)
                    || local.type_identity.is_empty()
            })
        || returned_plan.multiplicity != Multiplicity::Linear
        || returned_plan.is_self
        || discarded_plans
            .iter()
            .any(|discarded| discarded.multiplicity != Multiplicity::Affine || discarded.is_self)
        || plan.result.multiplicity != Multiplicity::Linear
        || plan.entry_claim.parameter_index != 0
        || !plan.entry_claim.path.is_empty()
        || plan.entry_claim.carry != CarryPolicy::STRICT
        || plan.entry_claim.claim_identity != plan.transferred_claim
        || returned_plan.type_identity != plan.result.type_identity
        || returned_plan.qualifications != plan.result.qualifications
    {
        return unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup",
        );
    }
    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ..
    } = plan.transferred_claim
    else {
        return unsupported("structural result claim is not an exact checked state-entry claim");
    };
    if machine_symbol != plan.machine || state_symbol != plan.state {
        return unsupported("structural result claim belongs to another checked state");
    }

    let (structural_types, type_ids) = lower_structural_type_plans(&plans.structural_types)?;
    let (structural_domains, domain_ids) =
        lower_structural_domain_plans(checked, &plans.structural_domains, &type_ids)?;
    let mut next_place = 1_u64;
    let parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let input = parameters.first().ok_or(LoweringError::Unsupported(
        "structural result plan has no input",
    ))?;
    let discarded = &parameters[1..];
    let result_place = place_id(RESULT_STRUCTURAL_PLACE_ID);
    if input.place == result_place {
        return unsupported("structural result place collides with its input namespace");
    }
    let mut result_qualifications = plan
        .result
        .qualifications
        .iter()
        .map(|domain| lookup_domain_id(&domain_ids, *domain))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    result_qualifications.sort();
    result_qualifications.dedup();
    if result_qualifications.len() != plan.result.qualifications.len() {
        return unsupported("structural result repeats a qualification");
    }
    let local_places = plan
        .trivial_affine_locals
        .iter()
        .map(|local| {
            let place = place_id(allocate_dense(&mut next_place)?);
            let structural_type = lookup_type_id(&type_ids, &local.type_identity)?;
            let Some(declaration) = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return unsupported("trivial affine local has no structural type declaration");
            };
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("trivial affine local is not a record");
            };
            if !fields.is_empty() {
                return unsupported("trivial affine local is not an empty record");
            }
            Ok((local.declaration_ordinal, structural_type, place))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == plan.machine && fact.state_symbol == plan.state)
        .cloned()
        .collect::<Vec<_>>();
    let identity = lower_content_identity_reshuffles(&identity_facts)?;
    let [(source_claim, content_claim)] = identity.source_claims.as_slice() else {
        return unsupported("structural result requires one exact identity reshuffle claim");
    };
    let [content_entry] = identity.entry_claims.as_slice() else {
        return unsupported("structural result requires one content entry binding");
    };
    let [reshuffle] = identity.reshuffles.as_slice() else {
        return unsupported("structural result requires one content identity reshuffle");
    };
    let claim = claim_id(1);
    if *source_claim != plan.transferred_claim
        || *content_claim != claim
        || content_entry.claim != claim
        || reshuffle.claim != claim
        || content_entry.input.root != input.place
        || reshuffle.input.root != input.place
        || reshuffle.output.root != result_place
        || !content_entry.input.segments.is_empty()
        || !reshuffle.input.segments.is_empty()
        || !reshuffle.output.segments.is_empty()
    {
        return unsupported("structural result claim/content identities do not unify exactly");
    }
    let content_places = BTreeMap::from([
        (
            input.place,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: input.is_self,
            },
        ),
        (result_place, StructuralPlaceKind::Result),
    ]);
    let actual_places = identity
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    if actual_places != content_places {
        return unsupported("structural result content roots do not match the checked signature");
    }
    let mut expected_places = content_places;
    for discarded in discarded {
        expected_places.insert(
            discarded.place,
            StructuralPlaceKind::Parameter {
                position: discarded.position,
                is_self: discarded.is_self,
            },
        );
    }
    for (declaration_ordinal, structural_type, place) in &local_places {
        expected_places.insert(
            *place,
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: *declaration_ordinal,
                structural_type: *structural_type,
                construction: None,
            },
        );
    }
    let input_place = input.place;
    let terminal_discards = local_places
        .iter()
        .rev()
        .map(|(_, _, place)| *place)
        .chain(discarded.iter().rev().map(|value| value.place))
        .collect();

    let terminal_machine = machine_id(1);
    let machine = TerminalMachine {
        id: terminal_machine,
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: Vec::new(),
        structural_parameters: parameters,
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: result_place,
            structural_type: lookup_type_id(&type_ids, &plan.result.type_identity)?,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: result_qualifications,
        }),
        structural_places: expected_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        entry_claims: vec![EntryClaim {
            claim,
            input: input_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: identity.entry_claims,
        content_identity_reshuffles: identity.reshuffles,
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: local_places
                .iter()
                .enumerate()
                .map(|(index, (_, _, destination))| {
                    Ok(Operation {
                        id: operation_id(dense_identity(index)?),
                        result: psi_terminal::OperationResult::Unit,
                        kind: OperationKind::EstablishTrivialAffineLocal {
                            destination: *destination,
                        },
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: input_place,
                returned_claims: vec![claim],
                trivial_affine_discards: terminal_discards,
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
            structural_types,
            structural_domains,
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn lower_structural_domain_plans(
    checked: &CheckedTrees,
    plans: &[psi_checked_trees::CheckedUnitStructuralDomainPlan],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let mut ordered = plans.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        (&left.identity, left.domain.0).cmp(&(&right.identity, right.domain.0))
    });
    if ordered.iter().any(|plan| {
        !plan.domain.is_valid() || plan.identity.is_empty() || plan.carrier_type_identity.is_empty()
    }) || ordered
        .windows(2)
        .any(|pair| pair[0].domain == pair[1].domain || pair[0].identity == pair[1].identity)
    {
        return unsupported("structural result domains are invalid or noncanonical");
    }
    let domain_ids = ordered
        .iter()
        .enumerate()
        .map(|(index, plan)| Ok((plan.domain, structural_domain_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = ordered
        .into_iter()
        .map(|plan| {
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                content_projection: content_conservation::lower_structural_content_projection(
                    checked,
                    plan.domain,
                    &plan.carrier_type_identity,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}
