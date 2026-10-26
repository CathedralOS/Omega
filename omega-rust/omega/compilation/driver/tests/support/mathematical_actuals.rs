//! The source fixture supplies every proof term. Capture the source's carrier
//! formation and fixed-law grants before production, then receive the canonical
//! artifact against those inputs. This oracle never manufactures a witness,
//! chooses grants from a certificate, or grants executable authority.

#[path = "mixed_application_controls.rs"]
mod mixed_application_controls;
use super::indexed_family_controls;

use proof_admission::{Budget, DEFAULT_CONVERSION_STEPS, MathematicalCertificate};
use std::collections::BTreeSet;
use std::time::Instant;
use terminal_codec::{
    CanonicalTerminalArtifact, MathematicalProofActualError, mathematical_carrier_key,
    verify_module_mathematical_actuals,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionRequest, produce_terminal_artifact,
};

pub(super) fn produce(
    request: compiler::CheckedCompileRequest,
    machine: &str,
) -> Result<CanonicalTerminalArtifact, String> {
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .map_err(|errors| format!("mathematical actual source checking: {errors:?}"))?;
    let program = &checked.execution().program;
    let facts = &program.facts.proof;
    let signature = facts
        .mathematical_signature
        .as_ref()
        .ok_or("mathematical actual inspection has no source signature")?;
    let grants = facts
        .mathematical_safety_assumptions
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut carriers = Vec::new();
    for carrier in facts
        .source_evidence_carriers
        .iter()
        .filter(|carrier| carrier.domains.is_empty())
    {
        let identity = program
            .type_identity_text(carrier.type_identity)
            .ok_or("source mathematical carrier has no identity")?;
        let judgment = MathematicalCertificate {
            signature: signature.declarations.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: carrier.ty,
            expected: carrier.ty,
        };
        let key = mathematical_carrier_key(&signature.arena, &judgment)
            .map_err(|error| format!("source carrier key: {error:?}"))?;
        if let Some((_, existing)) = carriers.iter().find(|(source, _)| *source == identity) {
            if *existing != key {
                return Err("checked source mathematical carrier identity is ambiguous".into());
            }
        } else {
            carriers.push((identity, key));
        }
    }
    let source_terms = signature.arena.len();
    let source_declarations = signature.declarations.len();
    let source_contracts = program
        .machines()
        .iter()
        .filter_map(|source| {
            let identity = program
                .exact_owner_nominal_identity(
                    source.symbol,
                    checked.sources().exact_toolchain_sources(),
                )
                .and_then(|identity| program.type_identity_text(identity))?;
            let contract = program
                .facts
                .contract_plans
                .for_machine(source.symbol)?
                .commitment
                .as_bytes();
            let formals = program
                .facts
                .flow
                .terminal_unit_effects
                .callable_entry_for_machine(source.symbol)
                .map(|entry| {
                    entry
                        .erased_proof_parameters
                        .iter()
                        .map(|parameter| {
                            program
                                .type_identity_text(parameter.type_identity)
                                .map(|identity| (parameter.source_position, identity))
                                .ok_or("checked source proof formal has no carrier identity")
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose();
            Some(formals.map(|formals| (identity, contract, formals)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut timings = Default::default();
    let artifact = produce_terminal_artifact(
        TerminalProductionRequest::new(
            program,
            TerminalMachineSelection::Name(machine),
            checked.sources().exact_toolchain_sources(),
        ),
        &[],
        &mut timings,
    )
    .map_err(|error| format!("mathematical actual Terminal production: {error:?}"))?
    .into_artifact()
    .map_err(|error| error.to_string())?;
    let expected = artifact.semantic_module();
    for carrier in &expected.mathematical_carriers {
        if !carriers.iter().any(|(identity, key)| {
            expected.identities.text(carrier.type_identity) == Some(identity.as_str())
                && *key == carrier.carrier_key
        }) {
            return Err(
                "portable mathematical carrier changed its independently formed source type".into(),
            );
        }
    }
    let mut source_bound_formals = 0;
    for body in &expected.machines {
        let source = body.declaration.as_ref().and_then(|declaration| {
            source_contracts.iter().find(|(identity, contract, _)| {
                expected.identities.text(declaration.machine.identity()) == Some(identity.as_str())
                    && declaration.contract == *contract
            })
        });
        let Some((_, _, source_formals)) = source else {
            if !body.contract.erased_proof_formals.is_empty() {
                return Err("proof actual machine has no exact checked source contract".into());
            }
            continue;
        };
        let Some(source_formals) = source_formals else {
            if !body.contract.erased_proof_formals.is_empty() {
                return Err("proof actual machine has no checked source invocation roster".into());
            }
            continue;
        };
        if source_formals.len() != body.contract.erased_proof_formals.len()
            || source_formals
                .iter()
                .zip(&body.contract.erased_proof_formals)
                .any(|((position, identity), formal)| {
                    *position != formal.source_position
                        || expected.identities.text(formal.type_identity) != Some(identity.as_str())
                })
        {
            return Err(
                "portable invocation proof roster changed its checked source inputs".into(),
            );
        }
        for (_, identity) in source_formals {
            if let Some((_, key)) = carriers.iter().find(|(source, _)| source == identity) {
                if !expected.mathematical_carriers.iter().any(|carrier| {
                    expected.identities.text(carrier.type_identity) == Some(identity.as_str())
                        && carrier.carrier_key == *key
                }) {
                    return Err(
                        "portable proof invocation omitted its independently formed source carrier"
                            .into(),
                    );
                }
                source_bound_formals += 1;
            }
        }
    }
    if source_bound_formals == 0 {
        return Err("mathematical actual inspection has no source-bound invocation formals".into());
    }
    let received = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .map_err(|error| format!("mathematical actual decoding: {error:?}"))?;
    if received.semantic_bytes() != artifact.semantic_bytes() {
        return Err("mathematical actual wire changed during canonical roundtrip".into());
    }
    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
    let start = Instant::now();
    verify_module_mathematical_actuals(
        received.view().module(),
        expected,
        |carrier| {
            carriers
                .iter()
                .any(|(identity, key)| {
                    expected.identities.text(carrier.type_identity) == Some(identity.as_str())
                        && *key == carrier.carrier_key
                })
                .then(|| grants.clone())
                .ok_or(MathematicalProofActualError::KernelCarrierMismatch)
        },
        &mut budget,
    )
    .map_err(|error| format!("mathematical actual receiving: {error:?}"))?;
    let mixed_controls =
        mixed_application_controls::check(received.view().module(), expected, &carriers, &grants)?;
    eprintln!("corpus_runner: mixed_mathematical_application_controls={mixed_controls}");
    eprintln!(
        "corpus_runner: mathematical_actual_receiving source_arena_terms={source_terms} source_declarations={source_declarations} artifact_bytes={} semantic_bytes={} carriers={} indexed_roots={} source_bound_formals={source_bound_formals} receiver_ms={:.3} receiver_steps={}",
        artifact.to_bytes().len(),
        artifact.semantic_bytes().len(),
        expected.mathematical_carriers.len(),
        expected.indexed_theorems.len(),
        start.elapsed().as_secs_f64() * 1000.0,
        DEFAULT_CONVERSION_STEPS - budget.remaining()
    );
    Ok(received)
}
