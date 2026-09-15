//! Claim outcomes, content projections, and their checked ownership relationships.

#[cfg(test)]
mod tests;

use crate::encoding::manifest_coordinates::{
    callable_overload_identity, machine_overload_identity, qualification_symbol_label,
    state_label_from_symbol, symbol_label,
};
use crate::encoding::manifest_values::{
    push_claim_identity_json, push_json_string, push_permission_event_source_json,
};
use checked_trees::CheckedTrees;

/// Normalized per-state claim outcome maps and content projections retained by
/// the checked ownership and qualification passes. This proof/debug artifact
/// exposes exact output paths, input-or-established sources, and the closed
/// symbolic content expression without making presentation spelling part of
/// public contract identity. Exact identity-preserving content rewrites are
/// retained beside the outcome rows that justify them; admitted backing and
/// complete frontier witnesses join this same artifact when their source
/// surfaces land.
pub fn claim_outcome_manifest_json(program: &CheckedTrees) -> String {
    let ownership = &program.facts.flow.ownership;
    let mut json = String::from("{\n  \"claim_outcome_maps\": [");
    let mut claim_outcome_coordinates = Vec::new();
    for (map_index, (_, map)) in ownership.claim_outcome_maps.iter().enumerate() {
        if map_index > 0 {
            json.push(',');
        }
        let coordinate = (map.machine_symbol, map.state_symbol);
        assert!(
            !claim_outcome_coordinates.contains(&coordinate),
            "claim outcome maps must retain one row per exact machine and state",
        );
        claim_outcome_coordinates.push(coordinate);
        let entries = validated_claim_outcome_entries(program, map);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, map.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, map.machine_symbol)
                .expect("claim outcome map must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, map.state_symbol),
        );
        json.push_str(",\n      \"entries\": [");
        for (entry_index, entry) in entries.iter().enumerate() {
            if entry_index > 0 {
                json.push(',');
            }
            json.push_str("\n        {\n          \"output_path\": ");
            push_claim_path_json(
                &mut json,
                program,
                ownership.segments.span_or_empty(entry.output_segments),
            );
            json.push_str(",\n          \"source\": ");
            push_claim_outcome_source_json(&mut json, program, entry.source);
            json.push_str("\n        }");
        }
        json.push_str("\n      ]\n    }");
    }
    let mut content_projections = validated_content_projection_plans(program);
    content_projections.sort_by_key(|plan| {
        (
            qualification_symbol_label(program, plan.domain),
            plan.report_fingerprint,
        )
    });
    json.push_str("\n  ],\n  \"content_projections\": [");
    for (index, plan) in content_projections.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"domain\": ");
        push_json_string(&mut json, &qualification_symbol_label(program, plan.domain));
        json.push_str(",\n      \"semantic_domain_id\": ");
        json.push_str(&plan.semantic_domain.0.to_string());
        json.push_str(",\n      \"carrier\": ");
        push_json_string(&mut json, &plan.carrier_identity);
        json.push_str(",\n      \"projection_machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, plan.machine),
        );
        json.push_str(",\n      \"projection_machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, plan.machine)
                .expect("content projection plan must name an exact projection machine"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &plan.algebra);
        json.push_str(",\n      \"normalized_projection\": ");
        push_content_projection_json(&mut json, &plan.expression);
        json.push_str(",\n      \"report_fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint));
        json.push_str("\n    }");
    }
    let mut identity_reshuffles = program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .collect::<Vec<_>>();
    identity_reshuffles.sort_by_key(|row| {
        (
            state_label_from_symbol(program, row.state_symbol),
            language_semantics::content::content_conservation_plan_bytes(&row.plan),
        )
    });
    json.push_str("\n  ],\n  \"content_identity_reshuffles\": [");
    let mut identity_reshuffle_keys = Vec::new();
    for (index, row) in identity_reshuffles.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, &row.plan);
        assert!(
            row.plan.owner_kind
                == language_semantics::content::ContentConservationOwnerKind::Machine
                && row.machine_symbol == row.plan.owner
                && row.state_symbol == row.plan.callable,
            "content identity reshuffle must retain its exact plan owner and callable",
        );
        validate_content_identity_reshuffle(program, row);
        let key = (
            row.machine_symbol,
            row.state_symbol,
            row.claim_identity,
            row.input_parameter_symbol,
            row.input_segments,
            row.output_segments,
            row.plan.report_fingerprint,
        );
        assert!(
            !identity_reshuffle_keys.contains(&key),
            "content identity reshuffles must retain one exact witness row per plan",
        );
        identity_reshuffle_keys.push(key);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, row.machine_symbol)
                .expect("content identity reshuffle must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, row.state_symbol),
        );
        json.push_str(",\n      \"claim_identity\": ");
        push_claim_identity_json(&mut json, program, row.claim_identity);
        json.push_str(",\n      \"input\": {\"parameter\": ");
        push_json_string(
            &mut json,
            &symbol_label(program, row.input_parameter_symbol),
        );
        json.push_str(", \"path\": ");
        push_claim_path_json(
            &mut json,
            program,
            ownership
                .segments
                .span(row.input_segments)
                .expect("validated content identity reshuffle input path"),
        );
        json.push_str("},\n      \"output_path\": ");
        push_claim_path_json(
            &mut json,
            program,
            ownership
                .segments
                .span(row.output_segments)
                .expect("validated content identity reshuffle output path"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &row.plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.plan.report_fingerprint),
        );
        json.push_str("\n    }");
    }
    let mut partition_compositions = program
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .collect::<Vec<_>>();
    validate_content_partition_lineage(program);
    partition_compositions.sort_by_key(|row| {
        (
            state_label_from_symbol(program, row.state_symbol),
            language_semantics::content::content_conservation_plan_bytes(&row.plan),
        )
    });
    json.push_str("\n  ],\n  \"content_partition_compositions\": [");
    let mut partition_composition_keys = Vec::new();
    for (index, row) in partition_compositions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, &row.source_plan);
        validate_content_conservation_plan(program, &content_projections, &row.plan);
        assert!(
            row.plan.owner_kind
                == language_semantics::content::ContentConservationOwnerKind::Machine
                && row.machine_symbol == row.plan.owner
                && row.state_symbol == row.plan.callable,
            "content partition composition must retain its exact derived-plan owner and callable",
        );
        assert!(
            row.source_callable == row.source_plan.callable
                && row.source_report_fingerprint == row.source_plan.report_fingerprint,
            "content partition composition must retain its exact source-plan coordinates",
        );
        validate_content_partition_input_custody(program, row);
        validate_content_partition_substitution_replay(row);
        validate_content_partition_result_rewrites(program, row);
        let key = (
            row.machine_symbol,
            row.state_symbol,
            row.statement_index,
            row.call_ordinal,
            row.source_report_fingerprint,
            row.plan.report_fingerprint,
        );
        assert!(
            !partition_composition_keys.contains(&key),
            "content partition compositions must retain one exact row per call and plan",
        );
        partition_composition_keys.push(key);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, row.machine_symbol));
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, row.machine_symbol)
                .expect("content partition composition must name an exact owning machine"),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, row.state_symbol),
        );
        json.push_str(",\n      \"source_callable\": ");
        push_json_string(&mut json, &symbol_label(program, row.source_callable));
        json.push_str(",\n      \"source_callable_overload_identity\": ");
        push_json_string(
            &mut json,
            &callable_overload_identity(program, row.source_plan.owner, row.source_callable)
                .expect("content partition composition must name an exact source callable"),
        );
        json.push_str(",\n      \"source_report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.source_report_fingerprint),
        );
        json.push_str(",\n      \"source_derivation_depth\": ");
        json.push_str(&row.source_derivation_depth.to_string());
        json.push_str(",\n      \"source_equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.source_plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.source_plan.equation.right());
        json.push_str("},\n      \"substitutions\": [");
        for (substitution_index, substitution) in row.substitutions.iter().enumerate() {
            if substitution_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"source\": ");
            push_content_structural_place_json(&mut json, &substitution.source);
            json.push_str(", \"target\": ");
            push_content_structural_place_json(&mut json, &substitution.target);
            json.push('}');
        }
        json.push(']');
        json.push_str(",\n      \"call\": {\"statement_index\": ");
        json.push_str(&row.statement_index.to_string());
        json.push_str(", \"call_ordinal\": ");
        json.push_str(&row.call_ordinal.to_string());
        json.push_str("},\n      \"input_claim_identities\": [");
        for (claim_index, identity) in row.input_claim_identities.iter().enumerate() {
            if claim_index > 0 {
                json.push_str(", ");
            }
            push_claim_identity_json(&mut json, program, *identity);
        }
        json.push_str("],\n      \"input_claim_bindings\": [");
        for (binding_index, binding) in row.input_claim_bindings.iter().enumerate() {
            if binding_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"claim_identity\": ");
            push_claim_identity_json(&mut json, program, binding.claim_identity);
            json.push_str(", \"entry_place\": ");
            push_content_structural_place_json(&mut json, &binding.entry_place);
            json.push('}');
        }
        json.push_str("],\n      \"result_rewrites\": [");
        for (rewrite_index, rewrite) in row.result_rewrites.iter().enumerate() {
            if rewrite_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"claim_identity\": ");
            push_claim_identity_json(&mut json, program, rewrite.claim_identity);
            json.push_str(", \"source\": ");
            push_content_structural_place_json(&mut json, &rewrite.source);
            json.push_str(", \"target\": ");
            push_content_structural_place_json(&mut json, &rewrite.target);
            json.push('}');
        }
        json.push_str("],\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &row.plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, row.plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(
            &mut json,
            &format!("0x{:016x}", row.plan.report_fingerprint),
        );
        json.push_str("\n    }");
    }
    let mut conservation = program
        .facts
        .qualifications
        .content
        .conservation_plans
        .iter()
        .collect::<Vec<_>>();
    conservation.sort_by_key(|plan| {
        (
            symbol_label(program, plan.callable),
            plan.report_fingerprint,
        )
    });
    json.push_str("\n  ],\n  \"content_conservation\": [");
    let mut authored_keys = Vec::new();
    for (index, plan) in conservation.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        validate_content_conservation_plan(program, &content_projections, plan);
        let key = (
            plan.owner_kind,
            plan.owner,
            plan.callable,
            plan.algebra.clone(),
        );
        assert!(
            !authored_keys.contains(&key),
            "content conservation plans must retain one authored row per exact owner, callable, and algebra",
        );
        authored_keys.push(key);
        json.push_str("\n    {\n      \"owner_kind\": ");
        push_json_string(
            &mut json,
            match plan.owner_kind {
                language_semantics::content::ContentConservationOwnerKind::Machine => "machine",
                language_semantics::content::ContentConservationOwnerKind::TraitRequirement => {
                    "trait_requirement"
                }
            },
        );
        json.push_str(",\n      \"owner\": ");
        push_json_string(&mut json, &symbol_label(program, plan.owner));
        json.push_str(",\n      \"callable\": ");
        push_json_string(&mut json, &symbol_label(program, plan.callable));
        json.push_str(",\n      \"callable_overload_identity\": ");
        push_json_string(
            &mut json,
            &callable_overload_identity(program, plan.owner, plan.callable)
                .expect("content conservation plan must name an exact callable"),
        );
        json.push_str(",\n      \"algebra\": ");
        push_content_algebra_json(&mut json, &plan.algebra);
        json.push_str(",\n      \"equation\": {\"left\": ");
        push_content_conservation_term_json(&mut json, program, plan.equation.left());
        json.push_str(", \"right\": ");
        push_content_conservation_term_json(&mut json, program, plan.equation.right());
        json.push_str("},\n      \"report_fingerprint\": ");
        push_json_string(&mut json, &format!("0x{:016x}", plan.report_fingerprint));
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn validated_claim_outcome_entries<'program>(
    program: &'program CheckedTrees,
    map: &checked_trees::FlowClaimOutcomeMapFact,
) -> &'program [checked_trees::FlowClaimOutcomeEntryFact] {
    use checked_trees::FlowClaimOutcomeSource;
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        PermissionProvenance,
    };

    let ownership = &program.facts.flow.ownership;
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == map.machine_symbol);
    let machine = machines
        .next()
        .expect("claim outcome map must name an exact typed machine");
    assert!(
        machines.next().is_none(),
        "claim outcome map machine must resolve to exactly one typed machine",
    );
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == map.state_symbol);
    let state = states
        .next()
        .expect("claim outcome map state must belong to its exact typed machine");
    assert!(
        states.next().is_none(),
        "claim outcome map state must resolve to exactly one state owned by its machine",
    );
    let entries = ownership
        .claim_outcome_entries
        .span(map.entries)
        .expect("claim outcome map must retain an exact valid entry span");
    let mut output_paths = Vec::new();
    for entry in entries {
        let output_path = ownership
            .segments
            .span(entry.output_segments)
            .expect("claim outcome entry must retain an exact valid output path span");
        assert!(
            !output_paths.contains(&output_path),
            "claim outcome map must retain one entry per exact output path",
        );
        output_paths.push(output_path);
        match entry.source {
            FlowClaimOutcomeSource::Unknown => {
                panic!("claim outcome entry must retain an exact known source")
            }
            FlowClaimOutcomeSource::Input {
                parameter_symbol,
                segments,
            } => {
                assert!(
                    program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == parameter_symbol),
                    "claim outcome input source must name an exact parameter owned by its state",
                );
                let source_path = ownership
                    .segments
                    .span(segments)
                    .expect("claim outcome input source must retain an exact valid path span");
                let mut origins = Vec::new();
                for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
                    event.machine_symbol == map.machine_symbol
                        && event.state_symbol == map.state_symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.root == facts::PlaceRoot::Symbol(parameter_symbol)
                        && ownership.segments.span(event.segments) == Some(source_path)
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.provenance != PermissionProvenance::Unknown
                }) {
                    let origin = (event.claim_identity, event.provenance);
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                }
                assert_eq!(
                    origins.len(),
                    1,
                    "claim outcome input source must resolve to one distinct live retained permission origin",
                );
            }
            FlowClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } => {
                assert!(
                    claim_identity != PermissionClaimIdentity::Unknown,
                    "claim outcome established source must retain a non-unknown claim identity",
                );
                assert!(
                    provenance != PermissionProvenance::Unknown,
                    "claim outcome established source must retain non-unknown provenance",
                );
                assert!(
                    ownership.permissions.iter().any(|(_, event)| {
                        event.claim_identity == claim_identity && event.provenance == provenance
                    }),
                    "claim outcome established source must match one retained permission event",
                );
            }
        }
    }
    entries
}

fn validated_content_projection_plans(
    program: &CheckedTrees,
) -> Vec<&language_semantics::content::ContentProjectionPlan> {
    use language_semantics::content::projection_report_fingerprint;

    let mut seen_domains = Vec::new();
    let mut seen_semantic_domains = Vec::new();
    program
        .facts
        .qualifications
        .content
        .plans
        .iter()
        .inspect(|plan| {
            assert!(
                plan.domain.is_valid(),
                "content projection plan must name a nonempty exact declared domain",
            );
            let mut domains = program
                .domain_definitions()
                .iter()
                .filter(|domain| domain.symbol == plan.domain);
            let domain = domains
                .next()
                .expect("content projection plan must name a nonempty exact declared domain");
            assert!(
                domains.next().is_none(),
                "content projection plan domain must resolve to exactly one declaration",
            );
            assert!(
                plan.semantic_domain.is_valid()
                    && domain.semantic_id == plan.semantic_domain
                    && program
                        .semantic_domains
                        .name(plan.semantic_domain)
                        .is_some(),
                "content projection plan must retain its exact registered semantic domain",
            );
            assert!(
                !plan.carrier_identity.is_empty()
                    && domain.target_type.is_valid()
                    && plan.carrier_identity
                        == program
                            .normalized_type_identity(domain.target_type)
                            .into_string(),
                "content projection plan must retain its exact normalized carrier identity",
            );
            let mut machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == plan.machine);
            machines
                .next()
                .expect("content projection plan must name an exact typed projection machine");
            assert!(
                machines.next().is_none(),
                "content projection plan machine must resolve to exactly one typed machine",
            );
            assert_eq!(
                plan.report_fingerprint,
                projection_report_fingerprint(&plan.algebra, &plan.expression),
                "content projection plan must retain its exact normalized report_fingerprint",
            );
            assert!(
                !seen_domains.contains(&plan.domain),
                "content projection plans must retain one row per exact domain",
            );
            seen_domains.push(plan.domain);
            assert!(
                !seen_semantic_domains.contains(&plan.semantic_domain),
                "content projection plans must retain one row per exact semantic domain",
            );
            seen_semantic_domains.push(plan.semantic_domain);
        })
        .collect()
}

fn validate_content_conservation_plan(
    program: &CheckedTrees,
    projection_plans: &[&language_semantics::content::ContentProjectionPlan],
    plan: &language_semantics::content::ContentConservationPlan,
) {
    use language_semantics::content::{
        ContentConservationOwnerKind, conservation_report_fingerprint,
    };

    match plan.owner_kind {
        ContentConservationOwnerKind::Machine => {
            let mut owners = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == plan.owner);
            let owner = owners
                .next()
                .expect("content conservation machine owner must name an exact typed machine");
            assert!(
                owners.next().is_none(),
                "content conservation machine owner must resolve to exactly one typed machine",
            );
            let mut callables = program
                .machine_states(owner)
                .iter()
                .filter(|state| state.symbol == plan.callable);
            callables.next().expect(
                "content conservation machine callable must be a state owned by its exact machine",
            );
            assert!(
                callables.next().is_none(),
                "content conservation machine callable must resolve to exactly one owned state",
            );
        }
        ContentConservationOwnerKind::TraitRequirement => {
            let mut owners = program
                .traits()
                .iter()
                .filter(|definition| definition.symbol == plan.owner);
            let owner = owners.next().expect(
                "content conservation trait owner must name an exact typed trait definition",
            );
            assert!(
                owners.next().is_none(),
                "content conservation trait owner must resolve to exactly one trait definition",
            );
            let mut callables = program
                .trait_machine_signatures(owner)
                .iter()
                .filter(|signature| signature.symbol == plan.callable);
            callables.next().expect(
                "content conservation trait callable must be a requirement owned by its exact trait",
            );
            assert!(
                callables.next().is_none(),
                "content conservation trait callable must resolve to exactly one owned requirement",
            );
        }
    }
    assert_eq!(
        plan.report_fingerprint,
        conservation_report_fingerprint(&plan.algebra, &plan.equation),
        "content conservation plan must retain its exact normalized report_fingerprint",
    );
    validate_content_conservation_term(projection_plans, &plan.algebra, plan.equation.left());
    validate_content_conservation_term(projection_plans, &plan.algebra, plan.equation.right());
}

fn validate_content_conservation_term(
    projection_plans: &[&language_semantics::content::ContentProjectionPlan],
    algebra: &language_semantics::content::ContentAlgebraIdentity,
    term: &language_semantics::content::ContentConservationTerm,
) {
    use language_semantics::content::ContentConservationTerm;

    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            ..
        } => {
            let mut matches = projection_plans.iter().filter(|plan| {
                plan.domain == *domain
                    && plan.semantic_domain == *semantic_domain
                    && plan.machine == *projection_machine
                    && plan.report_fingerprint == *projection_report_fingerprint
            });
            let projection = matches.next().expect(
                "content conservation projection term must join one exact retained projection plan",
            );
            assert!(
                matches.next().is_none(),
                "content conservation projection term must join exactly one retained projection plan",
            );
            assert_eq!(
                &projection.algebra, algebra,
                "content conservation projection term must retain the plan's exact algebra",
            );
        }
        ContentConservationTerm::Separate(terms) => {
            for term in terms {
                validate_content_conservation_term(projection_plans, algebra, term);
            }
        }
    }
}

fn validate_content_identity_reshuffle(
    program: &CheckedTrees,
    row: &checked_trees::ContentIdentityReshuffleFact,
) {
    use checked_trees::FlowClaimOutcomeSource;
    use language_semantics::content::{
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace,
    };
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let ownership = &program.facts.flow.ownership;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content identity reshuffle must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content identity reshuffle state must belong to its exact typed machine");
    let mut parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.symbol == row.input_parameter_symbol);
    let (parameter_position, parameter) = parameters
        .next()
        .expect("content identity reshuffle input must name an exact parameter owned by its state");
    assert!(
        parameters.next().is_none(),
        "content identity reshuffle input must resolve to exactly one parameter owned by its state",
    );
    let input_path = ownership
        .segments
        .span(row.input_segments)
        .expect("content identity reshuffle input must retain an exact valid path span");
    let output_path = ownership
        .segments
        .span(row.output_segments)
        .expect("content identity reshuffle output must retain an exact valid path span");
    assert!(
        row.claim_identity != PermissionClaimIdentity::Unknown,
        "content identity reshuffle must retain a non-unknown claim identity",
    );
    let mut entry_identities = Vec::new();
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == row.machine_symbol
            && event.state_symbol == row.state_symbol
            && event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned
            && event.obligation_live
            && event.root == facts::PlaceRoot::Symbol(row.input_parameter_symbol)
            && ownership.segments.span(event.segments) == Some(input_path)
            && event.claim_identity != PermissionClaimIdentity::Unknown
    }) {
        if !entry_identities.contains(&event.claim_identity) {
            entry_identities.push(event.claim_identity);
        }
    }
    let [entry_identity] = entry_identities.as_slice() else {
        panic!(
            "content identity reshuffle input must resolve to one distinct live retained permission identity"
        )
    };
    assert_eq!(
        row.claim_identity, *entry_identity,
        "content identity reshuffle must retain its exact input permission identity",
    );

    let mut maps = ownership.claim_outcome_maps.iter().filter(|(_, map)| {
        map.machine_symbol == row.machine_symbol && map.state_symbol == row.state_symbol
    });
    let map = maps
        .next()
        .expect("content identity reshuffle must name one exact retained claim outcome map")
        .1;
    assert!(
        maps.next().is_none(),
        "content identity reshuffle must name exactly one retained claim outcome map",
    );
    let matching_outcomes = validated_claim_outcome_entries(program, map)
        .iter()
        .filter(|entry| {
            ownership.segments.span(entry.output_segments) == Some(output_path)
                && matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Input {
                        parameter_symbol,
                        segments,
                    } if parameter_symbol == row.input_parameter_symbol
                        && ownership.segments.span(segments) == Some(input_path)
                )
        })
        .count();
    assert_eq!(
        matching_outcomes, 1,
        "content identity reshuffle must retain one exact input-relative claim outcome",
    );

    let input_subject = ContentStructuralPlace {
        version: ContentPlaceVersion::Entry,
        root: ContentPlaceRoot::Parameter {
            position: u32::try_from(parameter_position)
                .expect("content identity reshuffle parameter position must fit u32"),
            symbol: parameter.symbol,
            name: parameter.name.as_str().to_owned(),
            is_self: parameter.is_self,
        },
        segments: exact_content_path(program, input_path),
    };
    let output_subject = ContentStructuralPlace {
        version: ContentPlaceVersion::Current,
        root: ContentPlaceRoot::Result,
        segments: exact_content_path(program, output_path),
    };
    fn projection_subject(term: &ContentConservationTerm) -> Option<&ContentStructuralPlace> {
        match term {
            ContentConservationTerm::Projection { subject, .. } => Some(subject),
            ContentConservationTerm::Separate(_) => None,
        }
    }
    let left = projection_subject(row.plan.equation.left());
    let right = projection_subject(row.plan.equation.right());
    assert!(
        matches!(
            (left, right),
            (Some(left), Some(right))
                if (left == &input_subject && right == &output_subject)
                    || (left == &output_subject && right == &input_subject)
        ),
        "content identity reshuffle equation must retain its exact input and output projection subjects",
    );
}

fn exact_content_path(
    program: &CheckedTrees,
    path: &[facts::PlaceSegment],
) -> Vec<language_semantics::content::ContentPlaceSegment> {
    use language_semantics::content::{
        ContentCaseSegment, ContentFieldSegment, ContentPlaceSegment,
    };

    path.iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Case { variant } => {
                let mut variants = program.data_definitions().iter().flat_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Variant(candidate)
                                if candidate.symbol == *variant =>
                            {
                                Some(candidate)
                            }
                            typed_trees::data::DataMember::Field(_)
                            | typed_trees::data::DataMember::Variant(_) => None,
                        })
                });
                let candidate = variants.next().expect(
                    "content identity reshuffle case path must name an exact typed variant",
                );
                assert!(
                    variants.next().is_none(),
                    "content identity reshuffle case path must resolve to exactly one typed variant",
                );
                ContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: candidate.symbol,
                    name: candidate.name.as_str().to_owned(),
                })
            }
            facts::PlaceSegment::Field { symbol } => {
                let mut fields = program.data_definitions().iter().flat_map(|definition| {
                    program
                        .data_members(definition)
                        .iter()
                        .flat_map(|member| match member {
                            typed_trees::data::DataMember::Field(field) => {
                                std::slice::from_ref(field)
                            }
                            typed_trees::data::DataMember::Variant(variant) => {
                                program.data_payload_fields(variant)
                            }
                        })
                        .filter(|field| field.symbol == *symbol)
                });
                let field = fields.next().expect(
                    "content identity reshuffle field path must name an exact typed field",
                );
                assert!(
                    fields.next().is_none(),
                    "content identity reshuffle field path must resolve to exactly one typed field",
                );
                ContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: field.symbol,
                    name: field.name.as_str().to_owned(),
                })
            }
            facts::PlaceSegment::FixedIndex { index } => {
                ContentPlaceSegment::FixedIndex(
                    u64::try_from(*index)
                        .expect("content identity reshuffle fixed index must fit u64"),
                )
            }
            facts::PlaceSegment::FixedRange { .. } => {
                panic!("content identity reshuffle paths must not retain a range")
            }
            facts::PlaceSegment::Index { .. } => {
                panic!("content identity reshuffle paths must not retain a runtime index")
            }
        })
        .collect()
}

fn validate_content_partition_input_custody(
    program: &CheckedTrees,
    row: &checked_trees::ContentPartitionCompositionFact,
) {
    use language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion};
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content partition composition must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content partition composition state must belong to its exact typed machine");
    program
        .statement_table
        .statements(state.statement_nodes)
        .get(row.statement_index)
        .expect("content partition composition statement index must be within its exact state");
    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, fact)| {
            fact.machine_symbol == row.machine_symbol && fact.state_symbol == row.state_symbol
        });
    let flow_state = flow_states
        .next()
        .expect("content partition composition must name one exact checked flow state")
        .1;
    assert!(
        flow_states.next().is_none(),
        "content partition composition must name exactly one checked flow state",
    );
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .expect("content partition composition flow state must retain an exact valid call span");
    let mut coordinate_calls = calls.iter().filter(|call| {
        call.statement_index == row.statement_index && call.call_ordinal == row.call_ordinal
    });
    let call = coordinate_calls
        .next()
        .expect("content partition composition must retain one exact checked call coordinate");
    assert!(
        coordinate_calls.next().is_none(),
        "content partition composition must retain exactly one checked call coordinate",
    );
    assert_eq!(
        call.target_symbol, row.source_callable,
        "content partition composition must retain its exact checked source target",
    );

    assert!(
        !row.input_claim_identities.is_empty(),
        "content partition composition must retain at least one input claim identity",
    );
    let mut identities = Vec::new();
    for identity in &row.input_claim_identities {
        assert!(
            *identity != PermissionClaimIdentity::Unknown,
            "content partition composition input claim identities must be non-unknown",
        );
        assert!(
            !identities.contains(identity),
            "content partition composition input claim identities must be unique",
        );
        identities.push(*identity);
    }
    assert_eq!(
        row.input_claim_identities,
        row.input_claim_bindings
            .iter()
            .map(|binding| binding.claim_identity)
            .collect::<Vec<_>>(),
        "content partition composition input identities must exactly match ordered bindings",
    );
    for binding in &row.input_claim_bindings {
        let (position, symbol, name, is_self) = match &binding.entry_place.root {
            ContentPlaceRoot::Parameter {
                position,
                symbol,
                name,
                is_self,
            } if binding.entry_place.version == ContentPlaceVersion::Entry => {
                (*position, *symbol, name, *is_self)
            }
            _ => panic!(
                "content partition composition input binding must retain an entry parameter place"
            ),
        };
        let parameter = program
            .state_parameters(state)
            .get(usize::try_from(position).expect("partition input position must fit usize"))
            .filter(|parameter| {
                parameter.symbol == symbol
                    && parameter.name.as_str() == name
                    && parameter.is_self == is_self
            })
            .expect(
                "content partition composition input binding must name its exact caller parameter",
            );
        let matching_events = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == row.machine_symbol
                    && event.state_symbol == row.state_symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == binding.claim_identity
                    && event.root == facts::PlaceRoot::Symbol(parameter.symbol)
            })
            .filter(|(_, event)| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .expect("partition input permission event must retain an exact valid path");
                exact_content_path(program, path) == binding.entry_place.segments
            })
            .count();
        assert_eq!(
            matching_events, 1,
            "content partition composition input binding must match one live retained permission event",
        );
    }
}

fn validate_content_partition_substitution_replay(
    row: &checked_trees::ContentPartitionCompositionFact,
) {
    use language_semantics::content::{ContentConservationEquation, ContentConservationTerm};

    fn contains_separate(term: &ContentConservationTerm) -> bool {
        match term {
            ContentConservationTerm::Projection { .. } => false,
            ContentConservationTerm::Separate(_) => true,
        }
    }

    fn collect_subjects<'term>(
        term: &'term ContentConservationTerm,
        subjects: &mut Vec<&'term language_semantics::content::ContentStructuralPlace>,
    ) {
        match term {
            ContentConservationTerm::Projection { subject, .. } => subjects.push(subject),
            ContentConservationTerm::Separate(terms) => {
                for term in terms {
                    collect_subjects(term, subjects);
                }
            }
        }
    }

    fn replay(
        term: &ContentConservationTerm,
        substitutions: &[checked_trees::ContentPartitionPlaceSubstitution],
    ) -> ContentConservationTerm {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => {
                let target = substitutions
                    .iter()
                    .find(|substitution| substitution.source == *subject)
                    .expect(
                        "content partition substitution replay must cover every source subject",
                    );
                ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_report_fingerprint: *projection_report_fingerprint,
                    subject: target.target.clone(),
                }
            }
            ContentConservationTerm::Separate(terms) => ContentConservationTerm::Separate(
                terms
                    .iter()
                    .map(|term| replay(term, substitutions))
                    .collect(),
            ),
        }
    }

    assert!(
        contains_separate(row.source_plan.equation.left())
            || contains_separate(row.source_plan.equation.right()),
        "content partition composition source equation must retain an authored partition",
    );
    assert!(
        !row.substitutions.is_empty(),
        "content partition composition must retain a nonempty exact substitution map",
    );
    for (index, substitution) in row.substitutions.iter().enumerate() {
        assert!(
            row.substitutions[..index]
                .iter()
                .all(|previous| previous.source != substitution.source),
            "content partition composition substitution sources must be unique",
        );
        assert!(
            row.substitutions[..index]
                .iter()
                .all(|previous| previous.target != substitution.target),
            "content partition composition substitution targets must be unique",
        );
    }
    let mut subjects = Vec::new();
    collect_subjects(row.source_plan.equation.left(), &mut subjects);
    collect_subjects(row.source_plan.equation.right(), &mut subjects);
    for substitution in &row.substitutions {
        assert!(
            subjects.contains(&&substitution.source),
            "content partition composition substitution source must occur in the source equation",
        );
    }
    for subject in subjects {
        assert_eq!(
            row.substitutions
                .iter()
                .filter(|substitution| substitution.source == *subject)
                .count(),
            1,
            "content partition composition must cover every source subject exactly once",
        );
    }
    let replayed = ContentConservationEquation::new(
        replay(row.source_plan.equation.left(), &row.substitutions),
        replay(row.source_plan.equation.right(), &row.substitutions),
    );
    assert_eq!(
        row.source_plan.algebra, row.plan.algebra,
        "content partition composition replay must preserve the exact source algebra",
    );
    assert_eq!(
        replayed, row.plan.equation,
        "content partition composition derived equation must equal exact substitution replay",
    );
}

fn validate_content_partition_result_rewrites(
    program: &CheckedTrees,
    row: &checked_trees::ContentPartitionCompositionFact,
) {
    use checked_trees::FlowClaimOutcomeSource;
    use language_semantics::content::{ContentPlaceRoot, ContentPlaceVersion};
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };

    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .expect("content partition result rewrite must name an exact typed machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state_symbol)
        .expect("content partition result rewrite state must belong to its exact typed machine");
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(row.statement_index)
        .expect("content partition result rewrite statement must be within its exact state");

    for (index, rewrite) in row.result_rewrites.iter().enumerate() {
        assert!(
            rewrite.claim_identity != PermissionClaimIdentity::Unknown,
            "content partition result rewrite must retain a non-unknown claim identity",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.claim_identity != rewrite.claim_identity),
            "content partition result rewrite claim identities must be unique",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.source != rewrite.source),
            "content partition result rewrite sources must be unique",
        );
        assert!(
            row.result_rewrites[..index]
                .iter()
                .all(|previous| previous.target != rewrite.target),
            "content partition result rewrite targets must be unique",
        );
        assert!(
            rewrite.source.version == ContentPlaceVersion::Current
                && rewrite.source.root == ContentPlaceRoot::Result,
            "content partition result rewrite source must be an exact current result place",
        );
        assert!(
            rewrite.target.version == ContentPlaceVersion::Current
                && rewrite.target.root == ContentPlaceRoot::Result,
            "content partition result rewrite target must be an exact current result place",
        );
        assert_eq!(
            row.substitutions
                .iter()
                .filter(|substitution| {
                    substitution.source == rewrite.source && substitution.target == rewrite.target
                })
                .count(),
            1,
            "content partition result rewrite must retain one exact substitution pair",
        );
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
            panic!("content partition result rewrite must belong to its exact staged local")
        };
        let matching_events = program
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                let source_matches = matches!(
                    event.source,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        target_symbol,
                        ..
                    } if statement_index == row.statement_index
                        && call_ordinal == row.call_ordinal
                        && target_symbol == row.source_callable
                ) || event.source
                    == PermissionEventSource::Statement {
                        statement_index: row.statement_index,
                    };
                event.machine_symbol == row.machine_symbol
                    && event.state_symbol == row.state_symbol
                    && source_matches
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == rewrite.claim_identity
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
            })
            .filter(|(_, event)| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .expect("content partition result event must retain an exact valid path");
                exact_content_path(program, path) == rewrite.source.segments
            })
            .count();
        assert_eq!(
            matching_events, 1,
            "content partition result rewrite must match one live staged-local permission event",
        );

        let mut outcome_maps = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| {
                map.machine_symbol == row.machine_symbol && map.state_symbol == row.state_symbol
            });
        let outcome_map = outcome_maps
            .next()
            .expect("content partition result rewrite must name one exact checked outcome map")
            .1;
        assert!(
            outcome_maps.next().is_none(),
            "content partition result rewrite must name exactly one checked outcome map",
        );
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span(outcome_map.entries)
            .expect("content partition result outcome map must retain an exact valid entry span");
        let matching_outcomes = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Established { claim_identity, .. }
                        if claim_identity == rewrite.claim_identity
                )
            })
            .filter(|entry| {
                let path = program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(entry.output_segments)
                    .expect("content partition result outcome must retain an exact valid path");
                exact_content_path(program, path) == rewrite.target.segments
            })
            .count();
        assert_eq!(
            matching_outcomes, 1,
            "content partition result rewrite must match one exact established outcome",
        );
    }
}

fn validate_content_partition_lineage(program: &CheckedTrees) {
    let authored = &program.facts.qualifications.content.conservation_plans;
    let compositions = &program.facts.qualifications.content.partition_compositions;

    for (index, row) in compositions.iter().enumerate() {
        if row.source_derivation_depth == 0 {
            assert_eq!(
                authored
                    .iter()
                    .filter(|plan| **plan == row.source_plan)
                    .count(),
                1,
                "content partition depth-zero source must match one exact authored plan",
            );
            continue;
        }

        let mut parents = compositions
            .iter()
            .enumerate()
            .filter(|(candidate_index, candidate)| {
                *candidate_index != index && candidate.plan == row.source_plan
            });
        let parent = parents
            .next()
            .expect("content partition derived source must match one distinct exact parent row")
            .1;
        assert!(
            parents.next().is_none(),
            "content partition derived source must match exactly one distinct parent row",
        );
        let expected_depth = parent
            .source_derivation_depth
            .checked_add(1)
            .expect("content partition source derivation depth must not overflow");
        assert_eq!(
            row.source_derivation_depth, expected_depth,
            "content partition source derivation depth must exactly increment its parent",
        );
    }
}

fn push_content_algebra_json(
    json: &mut String,
    algebra: &language_semantics::content::ContentAlgebraIdentity,
) {
    use language_semantics::content::ContentAlgebraIdentity;

    match algebra {
        ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
            json.push_str("{\"kind\": \"interval_set\", \"coordinate_space\": ");
            push_json_string(json, coordinate_space);
            json.push('}');
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            json.push_str("{\"kind\": \"counted_quantity\", \"unit\": ");
            push_json_string(json, unit);
            json.push('}');
        }
    }
}

fn push_content_projection_json(
    json: &mut String,
    projection: &language_semantics::content::ContentProjectionExpression,
) {
    use language_semantics::content::ContentProjectionExpression;

    match projection {
        ContentProjectionExpression::IntervalSet { members } => {
            json.push_str("{\"kind\": \"interval_set\", \"members\": [");
            for (index, member) in members.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                json.push_str("{\"start\": ");
                push_content_scalar_json(json, member.start());
                json.push_str(", \"end\": ");
                push_content_scalar_json(json, member.end());
                json.push('}');
            }
            json.push_str("]}");
        }
        ContentProjectionExpression::CountedQuantity { magnitude } => {
            json.push_str("{\"kind\": \"counted_quantity\", \"magnitude\": ");
            push_content_scalar_json(json, magnitude);
            json.push('}');
        }
    }
}

fn push_content_scalar_json(
    json: &mut String,
    scalar: &language_semantics::content::ContentScalarExpression,
) {
    use language_semantics::content::{ContentArithmeticOperator, ContentScalarExpression};

    match scalar {
        ContentScalarExpression::SubjectField(path) => {
            json.push_str("{\"kind\": \"subject_field\", \"path\": ");
            push_content_field_path_json(json, path);
            json.push('}');
        }
        ContentScalarExpression::RuntimeScalarEmbedding(path) => {
            json.push_str("{\"kind\": \"runtime_scalar_embedding\", \"path\": ");
            push_content_field_path_json(json, path);
            json.push('}');
        }
        ContentScalarExpression::Natural(value) => {
            json.push_str("{\"kind\": \"natural\", \"value\": ");
            push_json_string(json, value);
            json.push('}');
        }
        ContentScalarExpression::Successor(value) => {
            json.push_str("{\"kind\": \"successor\", \"value\": ");
            push_content_scalar_json(json, value);
            json.push('}');
        }
        ContentScalarExpression::Arithmetic {
            operator,
            left,
            right,
        } => {
            json.push_str("{\"kind\": \"arithmetic\", \"operator\": ");
            push_json_string(
                json,
                match operator {
                    ContentArithmeticOperator::Add => "add",
                    ContentArithmeticOperator::Subtract => "subtract",
                    ContentArithmeticOperator::Multiply => "multiply",
                },
            );
            json.push_str(", \"left\": ");
            push_content_scalar_json(json, left);
            json.push_str(", \"right\": ");
            push_content_scalar_json(json, right);
            json.push('}');
        }
    }
}

fn push_content_field_path_json(
    json: &mut String,
    path: &[language_semantics::content::ContentFieldSegment],
) {
    json.push('[');
    for (index, segment) in path.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, &segment.name);
    }
    json.push(']');
}

fn push_content_conservation_term_json(
    json: &mut String,
    program: &CheckedTrees,
    term: &language_semantics::content::ContentConservationTerm,
) {
    use language_semantics::content::ContentConservationTerm;

    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            subject,
        } => {
            json.push_str("{\"kind\": \"projection\", \"domain\": ");
            push_json_string(json, &qualification_symbol_label(program, *domain));
            json.push_str(", \"semantic_domain_id\": ");
            json.push_str(&semantic_domain.0.to_string());
            json.push_str(", \"projection_machine\": ");
            push_json_string(
                json,
                &qualification_symbol_label(program, *projection_machine),
            );
            json.push_str(", \"projection_report_fingerprint\": ");
            push_json_string(json, &format!("0x{projection_report_fingerprint:016x}"));
            json.push_str(", \"place\": ");
            push_content_structural_place_json(json, subject);
            json.push('}');
        }
        ContentConservationTerm::Separate(terms) => {
            json.push_str("{\"kind\": \"separate\", \"terms\": [");
            for (index, term) in terms.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                push_content_conservation_term_json(json, program, term);
            }
            json.push_str("]}");
        }
    }
}

fn push_content_structural_place_json(
    json: &mut String,
    subject: &language_semantics::content::ContentStructuralPlace,
) {
    use language_semantics::content::{ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion};

    json.push_str("{\"version\": ");
    push_json_string(
        json,
        match subject.version {
            ContentPlaceVersion::Entry => "entry",
            ContentPlaceVersion::Current => "current",
        },
    );
    json.push_str(", \"root\": ");
    match &subject.root {
        ContentPlaceRoot::Parameter {
            position,
            name,
            is_self,
            ..
        } => {
            json.push_str("{\"kind\": ");
            push_json_string(json, if *is_self { "self" } else { "parameter" });
            json.push_str(", \"position\": ");
            json.push_str(&position.to_string());
            json.push_str(", \"name\": ");
            push_json_string(json, name);
            json.push('}');
        }
        ContentPlaceRoot::Result => json.push_str("{\"kind\": \"result\"}"),
    }
    json.push_str(", \"path\": [");
    for (index, segment) in subject.segments.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        match segment {
            ContentPlaceSegment::Case(case) => {
                json.push_str("{\"kind\": \"case\", \"name\": ");
                push_json_string(json, &case.name);
                json.push('}');
            }
            ContentPlaceSegment::Field(field) => {
                json.push_str("{\"kind\": \"field\", \"name\": ");
                push_json_string(json, &field.name);
                json.push('}');
            }
            ContentPlaceSegment::FixedIndex(index) => {
                json.push_str("{\"kind\": \"fixed_index\", \"index\": ");
                json.push_str(&index.to_string());
                json.push('}');
            }
        }
    }
    json.push_str("]}");
}

fn push_claim_outcome_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: checked_trees::FlowClaimOutcomeSource,
) {
    match source {
        checked_trees::FlowClaimOutcomeSource::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        checked_trees::FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } => {
            json.push_str("{\"kind\": \"input\", \"parameter\": ");
            push_json_string(json, &symbol_label(program, parameter_symbol));
            json.push_str(", \"path\": ");
            push_claim_path_json(
                json,
                program,
                program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(segments),
            );
            json.push('}');
        }
        checked_trees::FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => {
            json.push_str("{\"kind\": \"established\", \"claim_identity\": ");
            push_claim_identity_json(json, program, claim_identity);
            json.push_str(", \"provenance\": ");
            push_claim_provenance_json(json, program, provenance);
            json.push('}');
        }
    }
}

fn push_claim_path_json(json: &mut String, program: &CheckedTrees, path: &[facts::PlaceSegment]) {
    json.push('[');
    for (index, segment) in path.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                json.push_str("{\"field\": ");
                push_json_string(json, &symbol_label(program, *symbol));
                json.push('}');
            }
            facts::PlaceSegment::Case { variant } => {
                json.push_str("{\"case\": ");
                push_json_string(json, &symbol_label(program, *variant));
                json.push('}');
            }
            facts::PlaceSegment::FixedIndex { index } => {
                json.push_str("{\"fixed_index\": ");
                json.push_str(&index.to_string());
                json.push('}');
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                json.push_str("{\"fixed_range\": {\"start\": ");
                json.push_str(&start.to_string());
                json.push_str(", \"end\": ");
                json.push_str(&end.to_string());
                json.push_str("}}");
            }
            facts::PlaceSegment::Index { expression } => {
                json.push_str("{\"index\": ");
                push_json_string(json, &program.expression_table.display_name(*expression));
                json.push('}');
            }
        }
    }
    json.push(']');
}

fn push_claim_provenance_json(
    json: &mut String,
    program: &CheckedTrees,
    provenance: language_semantics::PermissionProvenance,
) {
    match provenance {
        language_semantics::PermissionProvenance::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        language_semantics::PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push('}');
        }
    }
}
