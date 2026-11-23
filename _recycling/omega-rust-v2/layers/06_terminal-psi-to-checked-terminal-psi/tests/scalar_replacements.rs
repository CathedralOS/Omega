use omega_checker::check_scalar_replacements;
use omega_psi::{BlockTerminator, CompletionOrigin, DomainIssuers, LoweredPackage, QualificationDirection, SuccessorEdge};

const DOMAINS: &str = "domain i32::Km; domain i32::Degrees; domain i32::Positive requires self > 0;";

fn lower(
    definitions: &str,
    program: &str,
) -> LoweredPackage {
    let text = format!("{definitions} {program}");
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = omega_resolver::ParsedSource {
        logical_path: "units.omg",
        text: &text,
        tokens: &tokens,
        tree: &tree,
    };
    let package = omega_toolchain::resolve_source_package(&[omega_resolver::StageSource { package: "app", source }]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

fn literal(
    value: i32,
    dest: &str,
) -> LoweredPackage {
    lower(
        DOMAINS,
        &format!("machine change()->i32 in {dest} {{ let distance = ({value}i32) as +Km; distance as -Km +{dest} }}"),
    )
}

fn replacement(unit: &mut LoweredPackage) -> &mut SuccessorEdge {
    unit.machines
        .iter_mut()
        .flat_map(|machine| &mut machine.body.blocks)
        .find_map(|block| {
            let BlockTerminator::Jump(edge) = &mut block.terminator else { return None };
            edge.scalar_qualification_changes
                .iter()
                .any(|change| change.direction == QualificationDirection::Replace)
                .then_some(edge)
        })
        .expect("the replacement's jump")
}

#[test]
fn an_unchanged_negative_payload_can_gain_a_tag_but_cannot_establish_positivity() {
    check_scalar_replacements(&literal(-7, "Degrees")).unwrap();
    check_scalar_replacements(&literal(7, "Positive")).unwrap();
    for n in [-7, 0] {
        let error = check_scalar_replacements(&literal(n, "Positive")).unwrap_err();
        assert!(error.message.contains("predicate is not established"), "{error:?}");
    }
}

#[test]
fn only_facts_about_the_actual_argument_establish_the_new_predicate() {
    let defs = "domain i32::Hot requires self > 0; domain i32::Positive requires self > 0; domain i32::Km;";
    let mut established = lower(defs, "machine change(x:i32 in Hot)->i32 in Positive { x as -Hot +Positive }");
    check_scalar_replacements(&established).unwrap();
    established.machines[0].contract.requires.clear();
    assert!(check_scalar_replacements(&established).is_err());

    let sibling = lower(defs, "machine change(x:i32 in Km,y:i32 in Hot)->i32 in Positive { x as -Km +Positive }");
    assert!(check_scalar_replacements(&sibling).is_err());
}

#[test]
fn a_marker_a_proof_row_and_issuance_each_owe_their_own_evidence() {
    let mut unit = literal(7, "Positive");
    replacement(&mut unit).scalar_qualification_changes.clear();
    assert!(
        check_scalar_replacements(&unit)
            .unwrap_err()
            .message
            .contains("marker")
    );

    let mut unit = literal(7, "Positive");
    replacement(&mut unit).domain_completions[0].origin = CompletionOrigin::Vacuous;
    assert!(
        check_scalar_replacements(&unit)
            .unwrap_err()
            .message
            .contains("origin")
    );

    let mut unit = literal(7, "Positive");
    let dest = replacement(&mut unit).target;
    unit.machines[0].body.blocks[dest.get() - 1]
        .scalar_domain_invariants
        .clear();
    assert!(
        check_scalar_replacements(&unit)
            .unwrap_err()
            .message
            .contains("invariants")
    );

    let mut unit = literal(7, "Positive");
    unit.domains
        .iter_mut()
        .find(|d| d.declaration.path.ends_with("::Positive"))
        .unwrap()
        .issuers = DomainIssuers::Nobody;
    assert!(
        check_scalar_replacements(&unit)
            .unwrap_err()
            .message
            .contains("issuer")
    );
}

#[test]
fn a_destination_cannot_supply_its_own_source_or_its_own_proof() {
    let mut unit = literal(7, "Positive");
    let dest = replacement(&mut unit).target;
    let future = unit.machines[0].body.blocks[dest.get() - 1].scalar_parameters[0];
    replacement(&mut unit).arguments[0] = future;
    assert!(check_scalar_replacements(&unit).is_err());
}

#[test]
fn at_a_join_every_arriving_value_must_satisfy_the_new_predicate() {
    let program = |second: &str| {
        format!(
            "machine change(flag:bool, x:i32 in Km, y:i32 in Km)->i32 in Positive
        requires (x as -Km) > 0 {second}
        {{ let chosen = match flag {{true -> x, false -> y,}}; chosen as -Km +Positive }}"
        )
    };
    check_scalar_replacements(&lower(DOMAINS, &program("requires (y as -Km) > 0"))).unwrap();
    let incomplete = lower(DOMAINS, &program(""));
    assert!(
        check_scalar_replacements(&incomplete)
            .unwrap_err()
            .message
            .contains("Positive")
    );
}

#[test]
fn a_checked_comparison_is_evidence_only_on_the_matching_edge() {
    let source = |yes: &str, no: &str| {
        format!(
            "machine change(distance:i32 in Km)->i32 in Positive {{
            transition (distance as -Km) > 0 {{true -> {yes}, false -> {no},}} }}"
        )
    };
    let mint = "distance as -Km +Positive";
    let fallback = "1i32 as +Positive";
    check_scalar_replacements(&lower(DOMAINS, &source(mint, fallback))).unwrap();
    let wrong_arm = lower(DOMAINS, &source(fallback, mint));
    assert!(
        check_scalar_replacements(&wrong_arm)
            .unwrap_err()
            .message
            .contains("Positive")
    );
}
