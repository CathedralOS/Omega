use omega_checker::{check_record_constructor, check_selected_policy_call};
use omega_lowering::{DeclarationId, LoweredPackage};
use omega_resolver::{ParsedSource, StageSource};

const OPEN: &str = include_str!("../../layers/04_typed-trees-to-prepared-trees/tests/fixtures/access_policy/selected.omg");

fn with_prepared<T>(
    text: &str,
    check: impl FnOnce(&mut omega_preparation::PreparedTrees<'_>) -> T,
) -> T {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "policy.omg",
        text,
        tokens: &tokens,
        tree: &tree,
    };
    let mut package = omega_toolchain::resolve_source_package(&[StageSource { package: "app", source }]).unwrap();
    package.bind_package_key("app", [0x42; 32]).unwrap();
    check(&mut omega_preparation::prepare_source_package(&package).unwrap())
}

fn toolchain() -> (LoweredPackage, DeclarationId) {
    let source = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&source).unwrap();
    let unit = omega_lowering::lower_package(&prepared).unwrap();
    let id = unit
        .machine_templates
        .iter()
        .find(|machine| machine.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && machine.declaration.path == r"language\core\access\new_plan")
        .unwrap()
        .declaration
        .clone();
    (unit, id)
}

#[test]
fn resume_checks_the_receivers_frozen_request_even_if_the_foreign_projection_matches_its_proof() {
    use super::resume_policy_construction;
    use omega_checker::check_policy_construction;
    let (unit, id) = toolchain();
    let constructor = check_record_constructor(&unit, &id).unwrap();
    // The fixture package key, domain, header position, schema image and
    // constructor are identical. Only the body supplying issuance differs.
    let twin = OPEN.replace("Unrestricted{}", "Only{..}");
    let (foreign_def, foreign_inputs) = with_prepared(&twin, |prepared| {
        let selection = &prepared.typed().access_policies[0];
        (
            omega_lowering::lower_selected_policy_definition(prepared, selection).unwrap(),
            omega_lowering::lower_policy_construction_inputs(prepared, selection).unwrap(),
        )
    });
    let foreign_call = check_selected_policy_call(&foreign_def, &constructor).unwrap();
    let foreign_proof = check_policy_construction(&foreign_call, &foreign_inputs).unwrap();

    with_prepared(OPEN, |prepared| {
        let selection = &prepared.typed().access_policies[0];
        let domain = selection.domain.clone();
        let own_def = omega_lowering::lower_selected_policy_definition(prepared, selection).unwrap();
        let own_inputs = omega_lowering::lower_policy_construction_inputs(prepared, selection).unwrap();
        let other_occurrence = with_prepared(OPEN, |other| other.policy_construction_occurrence(&domain).unwrap());
        let own_call = check_selected_policy_call(&own_def, &constructor).unwrap();
        let own_proof = check_policy_construction(&own_call, &own_inputs).unwrap();
        assert!(resume_policy_construction(prepared, other_occurrence, &own_proof).is_err());
        assert_eq!(foreign_inputs.package_key, own_inputs.package_key);
        assert_eq!(foreign_inputs.schema, own_inputs.schema);
        assert_eq!(foreign_inputs.subject_revision, own_inputs.subject_revision);
        assert_eq!(foreign_inputs.selection_site, own_inputs.selection_site);
        assert_eq!(foreign_inputs.construction_site, own_inputs.construction_site);
        assert_ne!(foreign_inputs.fields, own_inputs.fields);

        let occurrence = prepared.policy_construction_occurrence(&domain).unwrap();
        assert!(resume_policy_construction(prepared, occurrence, &foreign_proof).is_err());
        // Sharing the exact schema allocation still cannot make different
        // actual issuance inputs belong to the receiving construction.
        let mut mixed = foreign_inputs.clone();
        mixed.schema.storage = own_inputs.schema.storage.clone();
        let omega_lowering::PolicyConstructionField::Schema(copy) = &mut mixed.fields[0].1 else {
            unreachable!()
        };
        copy.storage = mixed.schema.storage.clone();
        let mixed_proof = check_policy_construction(&foreign_call, &mixed).unwrap();
        let occurrence = prepared.policy_construction_occurrence(&domain).unwrap();
        assert!(resume_policy_construction(prepared, occurrence, &mixed_proof).is_err());

        assert!(
            !prepared
                .pending_access_plan(&domain)
                .unwrap()
                .construction_checked()
        );
        let occurrence = prepared.policy_construction_occurrence(&domain).unwrap();
        resume_policy_construction(prepared, occurrence, &own_proof).unwrap();
        assert!(
            prepared
                .pending_access_plan(&domain)
                .unwrap()
                .construction_checked()
        );
    });
}
