//! What an accepted compilation publishes from its mathematical signature.
//!
//! `Peano` in the recovered `ring_law_conformance` program has no storage, so
//! the root module has no structural type for it. Its mathematical carrier
//! row is how the published program names the kernel type its law is about.
//! A program with no such data publishes exactly the module it had before.

use super::*;
use proof_admission::Term;

const CONFORMING: &str =
    include_str!("../../tests/fixtures/ring_law/pass/ring_law_conformance/main.omg");

/// The fixture's proofs, without the entry that needs the standard library.
fn proofs() -> String {
    let end = CONFORMING
        .find("data Main")
        .expect("the fixture's entry state");
    CONFORMING[..end].replace("use omega\\language\\core\\ProcessExit;\n", "")
}

fn typed() -> crate::lowering::PreparedTypedTrees {
    crate::prepare_fixture(super::build_purpose::frontier_with_core(
        &proofs(),
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    ))
}

fn compilation() -> crate::lowering::Compilation<'static> {
    static SUBJECTS: std::sync::OnceLock<[Option<semantic_vocabulary::PackageKeyIdentity>; 1]> =
        std::sync::OnceLock::new();
    let subjects = SUBJECTS.get_or_init(|| [Some(super::build_purpose::package())]);
    super::build_purpose::roster(&[], subjects, source_provenance::DependencyScope::Product)
}

/// The root module stage 06 accepted, before and after publication.
fn modules(
    prepared: crate::lowering::PreparedTypedTrees,
    compilation: crate::lowering::Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> (
    Vec<RouteVerdict>,
    terminal_psi::TerminalModule,
    terminal_psi::TerminalModule,
) {
    let judged: Vec<_> = crate::judge_compilation(
        crate::lowering::prepare_compilation(&prepared, compilation, publication)
            .unwrap()
            .program,
    )
    .into_iter()
    .map(|verdict| verdict.expect("a judged part"))
    .collect();
    let before = judged
        .iter()
        .find_map(|verdict| match verdict {
            RouteVerdict::Accepted(closure) => Some(closure.elaboration().module.clone()),
            _ => None,
        })
        .expect("an accepted root");
    let input = crate::lowering::lower_for_publication(prepared, compilation, publication).unwrap();
    let (published, _) = crate::judge_publication(input, Default::default()).unwrap();
    let (_, program, _) = published.into_source_and_program();
    (judged, before, program.elaboration().module.clone())
}

fn law_certificate(verdicts: &[RouteVerdict]) -> terminal_codec::DecodedMathematicalCertificate {
    let Some(RouteVerdict::Laws(judgment)) = verdicts
        .iter()
        .find(|verdict| matches!(verdict, RouteVerdict::Laws(_)))
    else {
        panic!("the signature's judgment: {verdicts:?}")
    };
    let [law] = judgment.laws.as_slice() else {
        panic!("one law: {judgment:?}")
    };
    terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap()
}

#[test]
fn the_root_module_publishes_the_carrier_of_the_data_its_law_is_about() {
    let (verdicts, before, module) = modules(
        typed(),
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    );
    assert!(before.types.mathematical_carriers.is_empty());
    let [carrier] = module.types.mathematical_carriers.as_slice() else {
        panic!("one carrier: {:?}", module.types.mathematical_carriers)
    };
    let identity = module
        .identities
        .text(carrier.type_identity)
        .expect("the carrier's identity is in the module's table");
    assert!(identity.contains("Peano"), "{identity}");
    // The rest of the module is what the checker accepted.
    let mut unpublished = module.clone();
    unpublished.types.mathematical_carriers.clear();
    assert_eq!(unpublished.machines, before.machines);
    assert_eq!(unpublished.types, before.types);

    // The key is the encoding's carrier form: a closed certificate whose term
    // and expected type are the same, here Peano's canonical trees.
    let key = terminal_codec::decode_mathematical_certificate(&carrier.carrier_key).unwrap();
    let (term, expected) = (key.certificate.term, key.certificate.expected);
    assert!(key.arena.structurally_equal(term, expected));
    assert!(key.certificate.context.is_empty() && key.certificate.level_arity == 0);
    let Term::Constant { declaration, .. } = key.arena.get(expected) else {
        panic!("the carrier is a declared constant")
    };
    assert_eq!(declaration as usize + 1, key.certificate.signature.len());
    let Some(Term::Sigma { domain: trees, .. }) = key
        .certificate
        .signature
        .get(declaration as usize)
        .and_then(|declaration| declaration.body)
        .map(|body| key.arena.get(body))
    else {
        panic!("the data pairs a raw tree with the evidence that it is canonical")
    };
    let Term::Constant {
        declaration: trees, ..
    } = key.arena.get(trees)
    else {
        panic!("the raw trees are a declared constant")
    };
    assert!(matches!(
        key.certificate.signature[trees as usize]
            .body
            .map(|body| key.arena.get(body)),
        Some(Term::W { .. })
    ));
    let mut checked = key.arena.clone();
    proof_admission::check_signature(
        &mut checked,
        &key.certificate.signature,
        &mut Default::default(),
    )
    .expect("the key's signature is well formed");

    // It names the type the law quantifies over, trimmed to the
    // declarations that type depends on.
    let law = law_certificate(&verdicts);
    let Term::Pi { domain, .. } = law.arena.get(law.certificate.expected) else {
        panic!("the law is stated over its formals")
    };
    let formal = terminal_codec::theorem_implication_key(
        &law.arena,
        &proof_admission::MathematicalCertificate {
            term: domain,
            expected: domain,
            ..law.certificate.clone()
        },
    )
    .unwrap();
    assert_eq!(formal, carrier.carrier_key);

    // The module survives the wire format and its validators.
    let bytes = terminal_codec::encode_module(&module).unwrap();
    assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), module);
}

#[test]
fn a_carrier_key_that_claims_another_type_does_not_decode() {
    let (_, _, module) = modules(
        typed(),
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    );
    let carrier = &module.types.mathematical_carriers[0];
    let mut key = terminal_codec::decode_mathematical_certificate(&carrier.carrier_key).unwrap();
    // The same signature and term, with the data's universe as the claimed
    // type: a typing judgment, not a carrier.
    key.certificate.expected = key.arena.insert(Term::Sort(proof_admission::Sort::Type(
        proof_admission::Level::Constant(0),
    )));
    let mut forged = module.clone();
    forged.types.mathematical_carriers[0].carrier_key =
        terminal_codec::encode_mathematical_certificate(&key.arena, &key.certificate).unwrap();
    let refused = terminal_codec::encode_module(&forged)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
        .expect_err("a carrier key must claim the type it names");
    assert!(
        format!("{refused:?}").contains("invalid mathematical carrier key"),
        "{refused:?}"
    );
}

#[test]
fn a_program_without_mathematical_data_publishes_its_module_unchanged() {
    let source = "machine entry() -> bool {
        let known: bool = false;
        !known
    }";
    let typed = super::boundary_calls::typed(source);
    let entry = typed.machines().at(0).symbol;
    let entries = [entry];
    let compilation = crate::lowering::Compilation {
        declaration_metadata: &[],
        entries: &entries,
        subjects: &[],
        purpose: source_provenance::DependencyScope::Product,
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    };
    let (verdicts, before, after) = modules(
        crate::prepare_fixture(typed),
        compilation,
        terminal_codec::TerminalPublication::Portable,
    );
    assert!(
        !verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::Laws(_)))
    );
    assert!(after.types.mathematical_carriers.is_empty());
    assert_eq!(
        terminal_codec::encode_module(&after).unwrap(),
        terminal_codec::encode_module(&before).unwrap()
    );
}
