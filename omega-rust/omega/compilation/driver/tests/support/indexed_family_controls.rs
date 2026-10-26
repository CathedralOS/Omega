//! Source-produced indexed evidence, transported independently at artifact ingress.
//! The source fixture remains the oracle; this module never fabricates its proof.

use std::collections::BTreeSet;
use std::time::Instant;

use proof_admission::{
    Budget, DEFAULT_CONVERSION_STEPS, MathematicalCertificate, Term, TermArena, TermHandle,
};
use psi_compiler::{TerminalMachineSelection, TerminalProductionRequest, UnmeasuredProduction};
use terminal_codec::{
    CanonicalTerminalArtifact, MathematicalProofActualError, decode_mathematical_certificate,
    encode_mathematical_certificate, mathematical_carrier_key, verify_indexed_theorem_roots,
    verify_module_mathematical_actuals,
};
use terminal_psi::TerminalModule;

struct SourceTheorem {
    machine: symbols::SymbolHandle,
    machine_identity: String,
    contract: [u8; 32],
    ensure_ordinal: u32,
    evidence_source_position: u32,
    carrier: String,
    goal_key: Vec<u8>,
    closure: Vec<u32>,
}

pub(super) fn produce(
    request: compiler::CheckedCompileRequest,
    machine: &str,
) -> Result<CanonicalTerminalArtifact, String> {
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .map_err(|errors| format!("indexed inspection source checking: {errors:?}"))?;
    let program = &checked.execution().program;
    let facts = &program.facts.proof;
    // This policy is captured from the checked source before any artifact
    // exists. Neither an actual certificate nor its claimed closure grants it.
    let grants = facts
        .mathematical_safety_assumptions
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let signature = facts
        .mathematical_signature
        .as_ref()
        .ok_or("indexed inspection has no checked mathematical signature")?;
    let mut source_carriers = Vec::<(String, Vec<u8>)>::new();
    for carrier in facts
        .source_evidence_carriers
        .iter()
        .filter(|carrier| carrier.domains.is_empty())
    {
        let identity = program
            .type_identity_text(carrier.type_identity)
            .ok_or("checked indexed carrier has no source identity")?;
        let judgment = MathematicalCertificate {
            signature: signature.declarations.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: carrier.ty,
            expected: carrier.ty,
        };
        let key = mathematical_carrier_key(&signature.arena, &judgment)
            .map_err(|error| format!("checked indexed carrier key: {error:?}"))?;
        if let Some((_, previous)) = source_carriers
            .iter()
            .find(|(source, _)| *source == identity)
        {
            if *previous != key {
                return Err("checked source carrier identity is ambiguous".into());
            }
        } else {
            source_carriers.push((identity, key));
        }
    }
    let mut source_roots = Vec::new();
    for theorem in &facts.indexed_theorems {
        let identity = program
            .exact_owner_nominal_identity(
                theorem.machine,
                checked.sources().exact_toolchain_sources(),
            )
            .and_then(|identity| program.type_identity_text(identity))
            .ok_or("indexed source theorem has no exact owner")?;
        let contract = program
            .facts
            .contract_plans
            .for_machine(theorem.machine)
            .ok_or("indexed source theorem has no source contract")?
            .commitment
            .as_bytes();
        let source_judgment = MathematicalCertificate {
            signature: signature.declarations.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: theorem.term,
            expected: theorem.ty,
        };
        source_roots.push(SourceTheorem {
            machine: theorem.machine,
            machine_identity: identity,
            contract,
            ensure_ordinal: theorem.ensure_ordinal,
            evidence_source_position: theorem.evidence_source_position,
            carrier: program
                .type_identity_text(theorem.type_identity)
                .ok_or("indexed source theorem has no carrier identity")?,
            goal_key: mathematical_carrier_key(&signature.arena, &source_judgment)
                .map_err(|error| format!("indexed source theorem goal: {error:?}"))?,
            closure: theorem.assumption_closure.clone(),
        });
    }
    let source_machines = program
        .machines()
        .iter()
        .filter_map(|source| {
            program
                .exact_owner_nominal_identity(
                    source.symbol,
                    checked.sources().exact_toolchain_sources(),
                )
                .and_then(|identity| program.type_identity_text(identity))
                .map(|identity| (source.symbol, identity))
        })
        .collect::<Vec<_>>();
    let citations = facts.indexed_citations.clone();
    let source_signature_terms = signature.arena.len();
    let source_signature_declarations = signature.declarations.len();
    // Identify the actual used fixed distributive law in the trusted source
    // signature, rather than assigning a guessed declaration position.
    let distributive_positions = facts
        .indexed_theorems
        .iter()
        .flat_map(|root| root.assumption_closure.iter().copied())
        .filter(|position| {
            signature
                .declarations
                .get(*position as usize)
                .is_some_and(|declaration| {
                    declaration.body.is_none() && distribute_add(&signature.arena, declaration.ty)
                })
        })
        .collect::<BTreeSet<_>>();
    // Reuse the actual source Nil child, retained beneath the source Cons
    // constructor's child function. It is checked against the independently
    // retained closed carrier; no kernel witness is invented for this control.
    let nil_carrier = facts
        .source_evidence_carriers
        .iter()
        .find(|carrier| {
            carrier.domains.is_empty()
                && program
                    .type_identity_text(carrier.type_identity)
                    .is_some_and(|text| text.matches("integer-const(0)").count() == 2)
        })
        .ok_or("source Nil has no independently checked closed carrier")?;
    let nil_identity = program
        .type_identity_text(nil_carrier.type_identity)
        .ok_or("source Nil has no identity")?;
    // pipeline05 scalar_lowering/branch_states/call_lowering reads this
    // exact located ledger for the source bounded consumer's scalar call.
    let nil_term = program
        .facts
        .values
        .proof_terms
        .terms
        .iter()
        .map(|row| &row.term)
        .find_map(|term| {
            let checked_trees::CheckedProofTerm::Mathematical {
                term,
                type_identity,
                ..
            } = term
            else {
                return None;
            };
            if *type_identity == nil_carrier.type_identity {
                return None;
            }
            // The authored retain<1,7>(Cons...) wraps the constructor in a
            // transparent mathematical application. Follow its last value
            // argument; never normalize or synthesize a replacement witness.
            let mut term = *term;
            for _ in 0..128 {
                let mut head = term;
                let mut arguments = Vec::new();
                while let Term::Apply { function, argument } = signature.arena.get(head) {
                    arguments.push(argument);
                    head = function;
                }
                let Term::Constant { declaration, .. } = signature.arena.get(head) else {
                    return None;
                };
                if declaration == proof_admission::INDEXED_SUP {
                    if arguments.len() != 7 {
                        return None;
                    }
                    return match signature.arena.get(arguments[0]) {
                        Term::Lambda { body, .. } => Some(body),
                        _ => None,
                    };
                }
                if signature
                    .declarations
                    .get(declaration as usize)?
                    .body
                    .is_none()
                {
                    return None;
                }
                term = *arguments.first()?;
            }
            None
        })
        .ok_or("source Cons did not retain its source Nil child")?;
    let nil_judgment = MathematicalCertificate {
        signature: signature.declarations.clone(),
        level_arity: 0,
        context: Vec::new(),
        term: nil_term,
        expected: nil_carrier.ty,
    };
    let mut nil_arena = signature.arena.clone();
    proof_admission::verify_mathematical_certificate(
        &mut nil_arena,
        &nil_judgment,
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    )
    .map_err(|error| format!("retained source Nil child: {error:?}"))?;
    let nil_certificate = encode_mathematical_certificate(&signature.arena, &nil_judgment)
        .map_err(|error| format!("source Nil certificate: {error:?}"))?;
    let artifact = TerminalProductionRequest::new(
        program,
        TerminalMachineSelection::Name(machine),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut UnmeasuredProduction)
    .map_err(|error| format!("indexed inspection Terminal production: {error:?}"))?
    .into_artifact()
    .map_err(|error| error.to_string())?;
    let expected = artifact.semantic_module();
    for carrier in &expected.mathematical_carriers {
        if !source_carriers.iter().any(|(identity, key)| {
            *key == carrier.carrier_key
                && expected.identities.text(carrier.type_identity) == Some(identity.as_str())
        }) {
            return Err(
                "portable indexed carrier changed its independently checked source type".into(),
            );
        }
    }
    if expected.indexed_theorems.is_empty() {
        return Err("source indexed witness lost every cited induction root".into());
    }
    let mut reachable = source_machines
        .iter()
        .filter_map(|(symbol, identity)| {
            expected
                .machines
                .iter()
                .any(|output| {
                    output.declaration.as_ref().is_some_and(|declaration| {
                        expected.identities.text(declaration.machine.identity())
                            == Some(identity.as_str())
                    })
                })
                .then_some(*symbol)
        })
        .collect::<Vec<_>>();
    loop {
        let before = reachable.len();
        for edge in &citations {
            if reachable.contains(&edge.caller) && !reachable.contains(&edge.theorem) {
                reachable.push(edge.theorem);
            }
        }
        if before == reachable.len() {
            break;
        }
    }
    let source_roots = source_roots
        .iter()
        .filter(|root| reachable.contains(&root.machine))
        .collect::<Vec<_>>();
    if expected.indexed_theorems.len() != source_roots.len() {
        return Err("source indexed theorem roots changed during citation retirement".into());
    }
    for root in &expected.indexed_theorems {
        let source = source_roots
            .iter()
            .find(|source| {
                expected
                    .identities
                    .text(root.source_contract.machine.identity())
                    == Some(source.machine_identity.as_str())
                    && source.ensure_ordinal == root.ensure_ordinal
            })
            .ok_or("portable induction root has no checked source clause")?;
        let decoded = decode_mathematical_certificate(&root.certificate)
            .map_err(|error| format!("indexed theorem certificate: {error:?}"))?;
        if root.source_contract.contract != source.contract
            || root.evidence_source_position != source.evidence_source_position
            || expected.identities.text(root.type_identity) != Some(source.carrier.as_str())
            || root.assumption_closure != source.closure
            || mathematical_carrier_key(&decoded.arena, &decoded.certificate)
                .map_err(|error| format!("indexed theorem key: {error:?}"))?
                != source.goal_key
        {
            return Err("portable induction root changed its checked source association or dependency closure".into());
        }
    }
    let bytes = artifact.to_bytes();
    if let Some(directory) = std::env::var_os("OMEGA_CORPUS_ARTIFACT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("indexed artifact directory: {error}"))?;
        let fingerprint = artifact
            .manifest()
            .semantic()
            .program_fingerprint
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        std::fs::write(directory.join(format!("{fingerprint}.psiart")), &bytes)
            .map_err(|error| format!("indexed artifact preservation: {error}"))?;
    }
    let decode_started = Instant::now();
    let decoded = CanonicalTerminalArtifact::from_bytes(&bytes)
        .map_err(|error| format!("source indexed artifact decode: {error:?}"))?;
    let decode_ms = decode_started.elapsed().as_secs_f64() * 1000.0;
    let received = decoded.semantic_module();
    if artifact.semantic_bytes() != decoded.semantic_bytes()
        || artifact.manifest() != decoded.manifest()
    {
        return Err("source indexed artifact changed canonical identity across transport".into());
    }
    for (ordinal, root) in received.indexed_theorems.iter().enumerate() {
        let decoded = decode_mathematical_certificate(&root.certificate)
            .map_err(|error| format!("root metrics: {error:?}"))?;
        eprintln!(
            "corpus_runner: indexed_family_root ordinal={ordinal} certificate_bytes={} arena_terms={} signature_declarations={} judgment_context_bindings={} assumption_count={}",
            root.certificate.len(),
            decoded.arena.len(),
            decoded.certificate.signature.len(),
            decoded.certificate.context.len(),
            root.assumption_closure.len()
        );
    }
    let mut root_budget = Budget::new(DEFAULT_CONVERSION_STEPS);
    let root_started = Instant::now();
    verify_indexed_theorem_roots(received, expected, |_| Ok(grants.clone()), &mut root_budget)
        .map_err(|error| format!("source indexed theorem receiver: {error:?}; elapsed_ms={:.3}; used_kernel_steps={}",
            root_started.elapsed().as_secs_f64()*1000.0, DEFAULT_CONVERSION_STEPS-root_budget.remaining()))?;
    let root_ms = root_started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "corpus_runner: indexed_family_root_receiver roots={} check_ms={root_ms:.3} kernel_steps={}",
        received.indexed_theorems.len(),
        DEFAULT_CONVERSION_STEPS - root_budget.remaining()
    );
    let actual_count = mathematical_actuals(received).len();
    if actual_count == 0 {
        return Err("source indexed artifact has no mathematical actual slots; receiver actual validation would be vacuous".into());
    }
    let mut actual_budget = Budget::new(DEFAULT_CONVERSION_STEPS);
    let actual_started = Instant::now();
    verify_module_mathematical_actuals(
        received,
        expected,
        |carrier| {
            source_carriers
                .iter()
                .any(|(identity, key)| {
                    *key == carrier.carrier_key
                        && expected.identities.text(carrier.type_identity)
                            == Some(identity.as_str())
                })
                .then(|| grants.clone())
                .ok_or(MathematicalProofActualError::KernelCarrierMismatch)
        },
        &mut actual_budget,
    )
    .map_err(|error| {
        format!(
            "source indexed actual receiver: {error:?}; elapsed_ms={:.3}; used_kernel_steps={}",
            actual_started.elapsed().as_secs_f64() * 1000.0,
            DEFAULT_CONVERSION_STEPS - actual_budget.remaining()
        )
    })?;
    let actual_ms = actual_started.elapsed().as_secs_f64() * 1000.0;
    let mut swapped = received.clone();
    let (callee, slot) = first_mathematical_actual_mut(&mut swapped)?;
    *slot = semantic_vocabulary::ProofTerm::Mathematical {
        type_identity: nil_identity.clone(),
        certificate: nil_certificate.clone(),
    };
    must_refuse_actual(
        &swapped,
        expected,
        &source_carriers,
        &grants,
        "valid source Nil actual swap",
        |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
    )?;
    let mut coordinated = swapped.clone();
    let nil_index = coordinated
        .identities
        .index_of(&nil_identity)
        .ok_or("source Nil carrier absent from artifact identities")?;
    coordinated
        .machines
        .iter_mut()
        .find(|machine| machine.id == callee)
        .ok_or("source consumer absent")?
        .contract
        .erased_proof_formals[0]
        .type_identity = nil_index;
    must_refuse_actual(
        &coordinated,
        expected,
        &source_carriers,
        &grants,
        "coordinated source Nil actual and callee formal swap",
        |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
    )?;
    let mut wrong_body = received.clone();
    let (_, term) = first_mathematical_actual_mut(&mut wrong_body)?;
    let semantic_vocabulary::ProofTerm::Mathematical { certificate, .. } = term else {
        unreachable!()
    };
    let mut decoded_body = decode_mathematical_certificate(certificate)
        .map_err(|error| format!("wrong body decode: {error:?}"))?;
    decoded_body.certificate.term = decoded_body.arena.insert(Term::TwoZero);
    *certificate = encode_mathematical_certificate(&decoded_body.arena, &decoded_body.certificate)
        .map_err(|error| format!("wrong body encode: {error:?}"))?;
    must_refuse_actual(
        &wrong_body,
        expected,
        &source_carriers,
        &grants,
        "wrong proof body under unchanged source carrier",
        |error| matches!(error, MathematicalProofActualError::KernelBatch(failure) if matches!(&failure.error, proof_admission::CoreError::TypeMismatch { .. })),
    )?;
    let mut foreign = received.clone();
    let (_, term) = first_mathematical_actual_mut(&mut foreign)?;
    let semantic_vocabulary::ProofTerm::Mathematical { type_identity, .. } = term else {
        unreachable!()
    };
    type_identity.push_str("::foreign");
    must_refuse_actual(
        &foreign,
        expected,
        &source_carriers,
        &grants,
        "foreign nominal identity with unchanged certificate",
        |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
    )?;
    let used_distribution = received
        .indexed_theorems
        .iter()
        .flat_map(|root| root.assumption_closure.iter().copied())
        .find(|position| distributive_positions.contains(position))
        .ok_or("source vector induction did not retain its used distributive law")?;
    let mut missing_grant = grants.clone();
    if !missing_grant.remove(&used_distribution) {
        return Err("used law was not independently granted".into());
    }
    if !matches!(
        verify_indexed_theorem_roots(
            received,
            expected,
            |_| Ok(missing_grant.clone()),
            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
        ),
        Err(MathematicalProofActualError::UngrantedAssumptions(_))
    ) {
        return Err(
            "source indexed theorem accepted its missing used distributive-law grant".into(),
        );
    }
    let mut changed_contract = received.clone();
    changed_contract.indexed_theorems[0]
        .source_contract
        .contract[0] ^= 1;
    must_refuse(
        &changed_contract,
        expected,
        &grants,
        "changed source contract",
        |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
    )?;
    let mut changed_identity = received.clone();
    changed_identity.indexed_theorems[0].ensure_ordinal ^= 1;
    must_refuse(
        &changed_identity,
        expected,
        &grants,
        "changed theorem identity",
        |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
    )?;
    let mut changed_closure = received.clone();
    let root = changed_closure
        .indexed_theorems
        .iter_mut()
        .find(|root| root.assumption_closure.contains(&used_distribution))
        .unwrap();
    root.assumption_closure
        .retain(|position| *position != used_distribution);
    must_refuse(
        &changed_closure,
        expected,
        &grants,
        "changed raw assumption closure",
        |error| matches!(error, MathematicalProofActualError::KernelCarrierMismatch),
    )?;
    let certificate_bytes = received
        .indexed_theorems
        .iter()
        .map(|root| root.certificate.len())
        .sum::<usize>();
    eprintln!(
        "corpus_runner: indexed_family_transport artifact_bytes={} semantic_bytes={} source_arena_terms={} source_signature_declarations={} theorem_roots={} theorem_certificate_bytes={} carriers={} mathematical_actuals={} used_distribute_add={} decode_ms={decode_ms:.3} root_check_ms={root_ms:.3} root_check_steps={} actual_check_ms={actual_ms:.3} actual_check_steps={}",
        bytes.len(),
        artifact.semantic_bytes().len(),
        source_signature_terms,
        source_signature_declarations,
        received.indexed_theorems.len(),
        certificate_bytes,
        received.mathematical_carriers.len(),
        actual_count,
        used_distribution,
        DEFAULT_CONVERSION_STEPS - root_budget.remaining(),
        DEFAULT_CONVERSION_STEPS - actual_budget.remaining()
    );
    Ok(artifact)
}

fn must_refuse(
    actual: &TerminalModule,
    expected: &TerminalModule,
    grants: &BTreeSet<u32>,
    name: &str,
    expected_error: impl FnOnce(&MathematicalProofActualError) -> bool,
) -> Result<(), String> {
    // Change the source-produced semantic module, re-encode it canonically,
    // and cross ingress again. A stale envelope hash is not the reject oracle.
    let decoded = terminal_codec::encode_module(actual)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
        .map_err(|error| format!("{name} control refused structurally before the intended receiver obligation: {error:?}"))?;
    match verify_indexed_theorem_roots(
        &decoded,
        expected,
        |_| Ok(grants.clone()),
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    ) {
        Err(error) if expected_error(&error) => Ok(()),
        other => Err(format!(
            "source indexed artifact {name} had the wrong receiver outcome: {other:?}"
        )),
    }
}

fn mathematical_actuals(module: &TerminalModule) -> Vec<&semantic_vocabulary::ProofTerm> {
    use terminal_psi::{OperationKind, Terminator};
    let mut found = Vec::new();
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                let actuals = match &operation.kind {
                    OperationKind::Call {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallUnit {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallDynamic {
                        erased_proof_arguments,
                        ..
                    } => erased_proof_arguments.as_slice(),
                    _ => &[],
                };
                found.extend(actuals.iter().filter(|term| {
                    matches!(term, semantic_vocabulary::ProofTerm::Mathematical { .. })
                }));
            }
            match &block.terminator {
                Terminator::Jump {
                    erased_proof_arguments,
                    ..
                } => found.extend(erased_proof_arguments.iter().filter(|term| {
                    matches!(term, semantic_vocabulary::ProofTerm::Mathematical { .. })
                })),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => found.extend(
                    when_true
                        .erased_proof_arguments
                        .iter()
                        .chain(&when_false.erased_proof_arguments)
                        .filter(|term| {
                            matches!(term, semantic_vocabulary::ProofTerm::Mathematical { .. })
                        }),
                ),
                _ => {}
            }
        }
    }
    found
}

// Recognize the exact fixed law in the trusted checked signature solely to
// choose the dependency removed by the negative control. This grants nothing.
fn distribute_add(arena: &TermArena, mut ty: TermHandle) -> bool {
    let mut carrier = None;
    for _ in 0..3 {
        let Term::Pi { domain, codomain } = arena.get(ty) else {
            return false;
        };
        let Some(head) = constant(arena, domain) else {
            return false;
        };
        if carrier.is_some_and(|prior| prior != head) {
            return false;
        }
        carrier = Some(head);
        ty = codomain;
    }
    let Term::Id {
        ty: domain,
        left: lhs,
        right: rhs,
    } = arena.get(ty)
    else {
        return false;
    };
    if constant(arena, domain) != carrier {
        return false;
    }
    let Some((mul, scale, sum)) = binary(arena, lhs) else {
        return false;
    };
    let Some((add, left, right)) = binary(arena, sum) else {
        return false;
    };
    if mul == add
        || arena.get(scale) != Term::Variable(2)
        || arena.get(left) != Term::Variable(1)
        || arena.get(right) != Term::Variable(0)
    {
        return false;
    }
    let Some((rhs_add, left_product, right_product)) = binary(arena, rhs) else {
        return false;
    };
    let Some((left_mul, left_scale, left_value)) = binary(arena, left_product) else {
        return false;
    };
    let Some((right_mul, right_scale, right_value)) = binary(arena, right_product) else {
        return false;
    };
    rhs_add == add
        && left_mul == mul
        && right_mul == mul
        && arena.get(left_scale) == Term::Variable(2)
        && arena.get(left_value) == Term::Variable(1)
        && arena.get(right_scale) == Term::Variable(2)
        && arena.get(right_value) == Term::Variable(0)
}

fn constant(arena: &TermArena, term: TermHandle) -> Option<u32> {
    match arena.get(term) {
        Term::Constant {
            declaration,
            levels,
        } if levels.is_empty() => Some(declaration),
        _ => None,
    }
}
fn binary(arena: &TermArena, term: TermHandle) -> Option<(u32, TermHandle, TermHandle)> {
    let Term::Apply {
        function,
        argument: right,
    } = arena.get(term)
    else {
        return None;
    };
    let Term::Apply {
        function,
        argument: left,
    } = arena.get(function)
    else {
        return None;
    };
    Some((constant(arena, function)?, left, right))
}

fn first_mathematical_actual_mut(
    module: &mut TerminalModule,
) -> Result<
    (
        semantic_vocabulary::MachineId,
        &mut semantic_vocabulary::ProofTerm,
    ),
    String,
> {
    use terminal_psi::OperationKind;
    for machine in &mut module.machines {
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                let (callee, arguments) = match &mut operation.kind {
                    OperationKind::Call {
                        callee,
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallUnit {
                        callee,
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralScalar {
                        callee,
                        erased_proof_arguments,
                        ..
                    }
                    | OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        erased_proof_arguments,
                        ..
                    } => (*callee, erased_proof_arguments),
                    _ => continue,
                };
                if let Some(term) = arguments.iter_mut().find(|term| {
                    matches!(term, semantic_vocabulary::ProofTerm::Mathematical { .. })
                }) {
                    return Ok((callee, term));
                }
            }
        }
    }
    Err("source artifact has no direct mathematical actual for transport controls".into())
}
pub(super) fn must_refuse_actual(
    actual: &TerminalModule,
    expected: &TerminalModule,
    carriers: &[(String, Vec<u8>)],
    grants: &BTreeSet<u32>,
    name: &str,
    predicate: impl FnOnce(&MathematicalProofActualError) -> bool,
) -> Result<(), String> {
    let decoded = terminal_codec::encode_module(actual)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
        .map_err(|error| {
            format!("{name} refused structurally before its transport obligation: {error:?}")
        })?;
    match verify_module_mathematical_actuals(
        &decoded,
        expected,
        |carrier| {
            carriers
                .iter()
                .any(|(identity, key)| {
                    *key == carrier.carrier_key
                        && expected.identities.text(carrier.type_identity)
                            == Some(identity.as_str())
                })
                .then(|| grants.clone())
                .ok_or(MathematicalProofActualError::KernelCarrierMismatch)
        },
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    ) {
        Err(error) if predicate(&error) => Ok(()),
        other => Err(format!("{name} had wrong receiving outcome: {other:?}")),
    }
}
