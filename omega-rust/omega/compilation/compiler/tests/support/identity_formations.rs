//! The checked source supplies Id values and capture associations before
//! publication. Canonical receiving compares those independent inputs and
//! rejects changed carriers, subjects and well-typed replacement propositions.

use proof_admission::{Budget, MathematicalCertificate, Term};
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

    let position = module
        .identity_formations
        .rows
        .iter()
        .position(|row| {
            row.subject_origins.len() >= 2 && row.subject_origins[0] != row.subject_origins[1]
        })
        .ok_or("Id inspection has no distinct ordered source subjects")?;
    let mut wrong_subject = module.clone();
    wrong_subject.identity_formations.rows[position]
        .subject_origins
        .swap(0, 1);
    refuse_source(
        &wrong_subject,
        &expected,
        "source carrier or subject association",
    )?;

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
