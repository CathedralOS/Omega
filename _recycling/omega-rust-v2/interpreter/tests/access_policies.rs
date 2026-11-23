use omega_interpreter::{Ceilings, Value, execute_policy};
use omega_resolver::{ParsedSource, StageSource};
use std::sync::Arc;

const OPEN: &str = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/unrestricted/access_plan.omg");
const EMPTY: &str = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/domain_access/empty-issuers/policy.omg");

#[test]
fn the_actual_return_and_measured_cost_belong_to_the_selected_policy() {
    let mut work = Vec::new();
    for (source, case_name) in [(OPEN, "Issuance::Unrestricted"), (EMPTY, "Issuance::Only")] {
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        let parsed = ParsedSource {
            logical_path: "policy.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        };
        let mut resolved = omega_toolchain::resolve_source_package(&[StageSource {
            package: "app",
            source: parsed,
        }])
        .unwrap();
        resolved.bind_package_key("app", [0x72; 32]).unwrap();
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let [selection] = prepared.typed().access_policies.as_slice() else {
            panic!("one selection")
        };
        let unit = omega_lowering::lower_selected_policy_unit(&prepared, selection)
            .unwrap()
            .unwrap();
        let inputs = omega_lowering::lower_policy_construction_inputs(&prepared, selection).unwrap();
        let definition = omega_lowering::lower_selected_policy_definition(&prepared, selection).unwrap();
        let constructor = omega_checker::check_record_constructor(&unit, &inputs.constructor).unwrap();
        let call = omega_checker::check_selected_policy_call(&definition, &constructor).unwrap();
        let checked = omega_checker::check_closed_policy_call(&call).unwrap();
        let receipt = execute_policy(&checked, &inputs.schema, Ceilings::default()).unwrap();
        assert!(std::ptr::eq(receipt.unit(), checked.unit()));
        assert_eq!(receipt.entry().declaration, inputs.plan);
        assert!(Arc::ptr_eq(receipt.schema(), &inputs.schema.storage));
        let used = receipt.usage().work_used;
        let peak = receipt.usage().peak_live_cells;
        assert_eq!((used, peak), if source == EMPTY { (111, 144) } else { (110, 142) });
        let counts = [checked.plan(), checked.constructor()].map(|body| body.body.operations.len() as u64 + 1);
        assert_eq!(used, counts.iter().sum::<u64>());
        let run = |work, cells| {
            execute_policy(
                &checked,
                &inputs.schema,
                Ceilings {
                    work: Some(work),
                    live_cells: Some(cells),
                },
            )
        };
        assert!(run(used - 1, peak).is_err());
        assert!(run(used, peak - 1).is_err());
        assert_eq!(run(used, peak).unwrap().usage(), receipt.usage());
        let Value::Record { data, fields } = receipt.into_result() else {
            panic!("expected returned plan")
        };
        let plan = &unit.data[data.get() - 1];
        let field = |name: &str| {
            fields
                .get(
                    plan.fields
                        .iter()
                        .position(|f| f.declaration.path.ends_with(name))
                        .unwrap(),
                )
                .unwrap()
        };
        let Value::Record { fields: schema, .. } = field("AccessPlan::schema") else {
            panic!("expected returned schema")
        };
        assert!(matches!(schema.as_slice(), [Value::Bytes(backing)] if Arc::ptr_eq(backing, &inputs.schema.storage)));
        let Value::Case { data, case, fields } = field("AccessPlan::issuance") else {
            panic!("expected returned issuance")
        };
        assert!(
            unit.data[data.get() - 1].cases[*case]
                .declaration
                .path
                .ends_with(case_name)
        );
        if source == EMPTY {
            assert!(matches!(fields.as_slice(), [Value::Array(routes)] if routes.is_empty()));
        } else {
            assert!(fields.is_empty());
        }
        work.push(used);
    }
    // The empty roster comes from one additional structural array operation.
    assert_eq!(work[1], work[0] + 1);
}
