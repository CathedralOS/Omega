//! The checked source supplies Id values and capture associations before
//! publication. Canonical receiving compares those independent inputs and
//! rejects changed carriers, subjects and well-typed replacement propositions.

use proof_admission::{Budget, MathematicalCertificate, Signature, Term, weak_head_normalize};
use psi_compiler::{TerminalMachineSelection, TerminalProductionRequest, UnmeasuredProduction};
use terminal_codec::{
    CanonicalTerminalArtifact, ExpectedIdentityFormation, ExpectedIdentityFormations,
};

pub(super) fn produce(
    checked: &compiler::CheckedCompilation,
    machine: &str,
) -> Result<CanonicalTerminalArtifact, String> {
    let program = checked.source_inspection().program();
    let facts = &program.facts.proof;
    let signature = facts
        .mathematical_signature
        .as_ref()
        .ok_or("Id inspection has no source signature")?;
    let mut expected = Vec::new();
    for formation in &facts.identity_formations {
        let source_owner = program.normalized_hermetic_symbol_identity(formation.owner)?;
        let source_carrier = program
            .type_identity_text(formation.type_identity)
            .ok_or("Id source carrier is absent")?;
        let subject_carriers = formation
            .captures
            .iter()
            .map(|(_, ty)| {
                program
                    .type_identity_text(*ty)
                    .ok_or("Id source subject carrier is absent")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let subject_origins = formation
            .captures
            .iter()
            .map(|(expression, _)| {
                let span = program.expression_table.source_span(*expression).span;
                Ok((
                    u32::try_from(span.start).map_err(|_| "Id source offset exceeds u32")?,
                    u32::try_from(span.end).map_err(|_| "Id source offset exceeds u32")?,
                ))
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let judgment = terminal_codec::encode_mathematical_certificate(
            &signature.arena,
            &MathematicalCertificate {
                signature: Vec::new(),
                level_arity: 0,
                context: formation.domains.clone(),
                term: formation.term,
                expected: formation.ty,
            },
        )
        .map_err(|error| format!("source Id formation key: {error:?}"))?;
        expected.push(ExpectedIdentityFormation {
            source_owner,
            source_carrier,
            subject_carriers,
            subject_origins,
            judgment,
        });
    }
    if expected.is_empty() {
        return Err("Id inspection has no checked source formations".into());
    }
    let mut arena = signature.arena.clone();
    let term = arena.insert(Term::TwoZero);
    let ty = arena.insert(Term::Two);
    let shared = terminal_codec::encode_mathematical_certificate(
        &arena,
        &MathematicalCertificate {
            signature: signature.declarations.clone(),
            level_arity: 0,
            context: Vec::new(),
            term,
            expected: ty,
        },
    )
    .map_err(|error| format!("source Id signature key: {error:?}"))?;
    let expected = ExpectedIdentityFormations {
        signature: shared,
        rows: expected,
    };
    let artifact = TerminalProductionRequest::new(
        program,
        TerminalMachineSelection::Name(machine),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut UnmeasuredProduction)
    .map_err(|error| format!("Id Terminal production: {error:?}"))?
    .into_artifact()
    .map_err(|error| error.to_string())?;
    let received = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .map_err(|error| format!("Id canonical receiving: {error:?}"))?;
    if received.semantic_bytes() != artifact.semantic_bytes() {
        return Err("Id wire changed its canonical meaning".into());
    }
    let module = received.view().module();
    terminal_codec::verify_identity_formations(module, &expected, &mut Budget::default())
        .map_err(|error| format!("source-bound Id receiving: {error:?}"))?;

    let mut wrong_carrier = module.clone();
    wrong_carrier.identity_formations.rows[0].source_carrier =
        wrong_carrier.identity_formations.rows[0].source_owner;
    refuse_source(&wrong_carrier, &expected, "source carrier")?;

    let mut wrong_subject = module.clone();
    let position = replace_endpoint(&mut wrong_subject)?;
    refuse_source(&wrong_subject, &expected, "Id value differs")?;

    let mut wrong_value = module.clone();
    replace_family(&mut wrong_value, position, true)?;
    refuse_source(&wrong_value, &expected, "Id value differs")?;

    let mut boolean_family = module.clone();
    replace_family(&mut boolean_family, position, false)?;
    let bytes = terminal_codec::encode_module(&boolean_family)
        .map_err(|error| format!("Id mutant encoding: {error:?}"))?;
    let error = terminal_codec::decode_module(&bytes)
        .err()
        .ok_or("Id ingress accepted a Boolean type family")?;
    if !format!("{error:?}").contains("kernel Id constructor") {
        return Err(format!(
            "Boolean Id replacement refused for another reason: {error:?}"
        ));
    }
    eprintln!(
        "corpus_runner: identity_formation_controls source_roots={} received_roots={} semantic_bytes={} controls=4",
        expected.rows.len(),
        module.identity_formations.rows.len(),
        received.semantic_bytes().len()
    );
    Ok(received)
}

fn refuse_source(
    module: &terminal_psi::TerminalModule,
    expected: &ExpectedIdentityFormations,
    reason: &str,
) -> Result<(), String> {
    let bytes = terminal_codec::encode_module(module)
        .map_err(|error| format!("Id source mutant encoding: {error:?}"))?;
    let decoded = terminal_codec::decode_module(&bytes)
        .map_err(|error| format!("Id source mutant formation: {error:?}"))?;
    let error =
        terminal_codec::verify_identity_formations(&decoded, expected, &mut Budget::default())
            .err()
            .ok_or("Id receiver accepted changed source custody")?;
    if !format!("{error:?}").contains(reason) {
        return Err(format!(
            "Id source mutant refused for another reason: {error:?}"
        ));
    }
    Ok(())
}

/// Change one actual Id endpoint while retaining its exact carrier, source
/// association and subject domains. Ordinary receiving must still establish
/// formation before the independent source authority rejects the changed Id.
fn replace_endpoint(module: &mut terminal_psi::TerminalModule) -> Result<usize, String> {
    let mut decoded =
        terminal_codec::decode_mathematical_certificate(&module.identity_formations.certificate)
            .map_err(|error| format!("Id subject mutant certificate: {error:?}"))?;
    let signature = Signature::from_declarations(decoded.certificate.signature.clone());
    for (position, row) in module.identity_formations.rows.iter().enumerate() {
        if row.subject_carriers.len() != 2 {
            continue;
        }
        let declaration = &decoded.certificate.signature[row.declaration as usize];
        let mut body = declaration
            .body
            .ok_or("Id subject mutant has no definition")?;
        let mut domains = Vec::new();
        for _ in &row.subject_carriers {
            let Term::Lambda { domain, body: next } = decoded.arena.get(body) else {
                return Err("Id subject mutant lost its telescope".into());
            };
            domains.push(domain);
            body = next;
        }
        let original = body;
        body = weak_head_normalize(&mut decoded.arena, &signature, body, &mut Budget::default())
            .map_err(|error| format!("Id subject mutant reduction: {error:?}"))?;
        let Term::Id { left, right, .. } = decoded.arena.get(body) else {
            continue;
        };
        if decoded.arena.structurally_equal(left, right) {
            continue;
        }
        // Keep the emitted sealed application itself. Unfolding it to raw
        // Id would change the exact source judgment even without a tamper.
        let Term::Apply { function, .. } = decoded.arena.get(original) else {
            return Err("Id subject mutant has no emitted endpoint application".into());
        };
        let Term::Apply {
            argument: original_left,
            ..
        } = decoded.arena.get(function)
        else {
            return Err("Id subject mutant lost its first endpoint".into());
        };
        body = decoded.arena.insert(Term::Apply {
            function,
            argument: original_left,
        });
        for domain in domains.into_iter().rev() {
            body = decoded.arena.insert(Term::Lambda { domain, body });
        }
        decoded.certificate.signature[row.declaration as usize].body = Some(body);
        module.identity_formations.certificate =
            terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
                .map_err(|error| format!("Id subject mutant encoding: {error:?}"))?;
        return Ok(position);
    }
    Err("Id inspection has no two-subject family with distinct kernel endpoints".into())
}

/// Both replacements are well-typed type-valued functions. One changes the
/// source carrier to Two but keeps Id; the other returns Two instead of Id.
fn replace_family(
    module: &mut terminal_psi::TerminalModule,
    position: usize,
    keep_id: bool,
) -> Result<(), String> {
    let row = &module.identity_formations.rows[position];
    let mut decoded =
        terminal_codec::decode_mathematical_certificate(&module.identity_formations.certificate)
            .map_err(|error| format!("Id mutant certificate: {error:?}"))?;
    let two = decoded.arena.insert(Term::Two);
    let mut body = if keep_id {
        let left = decoded.arena.insert(Term::Variable(1));
        let right = decoded.arena.insert(Term::Variable(0));
        decoded.arena.insert(Term::Id {
            ty: two,
            left,
            right,
        })
    } else {
        two
    };
    let mut ty = decoded.arena.insert(Term::Sort(proof_admission::Sort::Type(
        proof_admission::Level::Constant(0),
    )));
    if row.subject_carriers.len() != 2 {
        return Err("Id mutant requires the two-subject source family".into());
    }
    for _ in &row.subject_carriers {
        body = decoded.arena.insert(Term::Lambda { domain: two, body });
        ty = decoded.arena.insert(Term::Pi {
            domain: two,
            codomain: ty,
        });
    }
    let declaration = &mut decoded.certificate.signature[row.declaration as usize];
    declaration.body = Some(body);
    declaration.ty = ty;
    module.identity_formations.certificate =
        terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .map_err(|error| format!("Id mutant certificate encoding: {error:?}"))?;
    Ok(())
}
