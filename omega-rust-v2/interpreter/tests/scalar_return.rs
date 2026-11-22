use omega_checker::check_scalar_return;
use omega_interpreter::{Argument, Ceilings, Stop, Value, admit_scalar_return};
use omega_resolver::{ParsedSource, StageSource};

fn lower(source: &str) -> omega_psi::LoweredPackage {
    // These are the dependency's exported machines, resolved in its own package.
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let resolved = omega_toolchain::resolve_source_package(&[StageSource {
        package: "kit",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

#[test]
fn checked_dependency_exports_return_their_actual_values_under_the_ordinary_meter() {
    for (source, expected) in [
        ("public machine v()->i32 {7}", Value::Integer(7.into())),
        ("public machine v()->i32 {8}", Value::Integer(8.into())),
        ("public machine v()->bool {true}", Value::Boolean(true)),
    ] {
        let unit = lower(source);
        let entry = &unit
            .machines
            .iter()
            .find(|m| m.declaration.path == "v")
            .unwrap()
            .declaration;
        let proof = check_scalar_return(&unit, entry).unwrap();
        let admitted = admit_scalar_return(&proof).unwrap();
        let completed = admitted.execute(vec![], Ceilings::default()).unwrap();
        assert_eq!(completed.result, Some(expected));
        assert_eq!(completed.usage.work_used, 2);
        assert_eq!(completed.usage.work_used, admitted.certified_work());
        assert!(matches!(
            admitted.execute(
                vec![],
                Ceilings {
                    work: Some(1),
                    ..Default::default()
                }
            ),
            Err(Stop::Resource { .. })
        ));
    }
}

#[test]
fn scalar_formals_and_their_return_are_checked_in_the_same_unit() {
    let mut unit = lower("public machine echo(x:i32)->i32 {x}");
    let entry = unit.machines[0].declaration.clone();
    let proof = check_scalar_return(&unit, &entry).unwrap();
    let executable = admit_scalar_return(&proof).unwrap();
    let result = executable
        .execute(vec![Argument::Owned(Value::Integer((-17).into()))], Ceilings::default())
        .unwrap();
    assert_eq!(result.result, Some(Value::Integer((-17).into())));
    assert!(matches!(
        executable.execute(vec![Argument::Owned(Value::Boolean(false))], Ceilings::default()),
        Err(Stop::Argument { .. })
    ));
    unit.machines[0].body.parameters.clear();
    assert!(check_scalar_return(&unit, &entry).is_err());
}

#[test]
fn changing_a_literal_outside_its_result_carrier_invalidates_the_proof() {
    let mut unit = lower("public machine v()->u8 {255u8}");
    let entry = unit.machines[0].declaration.clone();
    assert!(check_scalar_return(&unit, &entry).is_ok());
    unit.machines[0].body.operations[0].kind = omega_psi::OperationKind::Constant(omega_psi::ScalarLiteral::Integer(256.into()));
    assert!(check_scalar_return(&unit, &entry).is_err());
}

#[test]
fn a_checked_caller_runs_the_imported_machine_and_compares_its_result() {
    let caller = r"use kit; machine observed()->bool { let n:i32 = kit\v(); n == 7 }";
    for (source, expected) in [
        ("public machine v()->i32 {7}", true),
        ("public machine v()->i32 {8}", false),
    ] {
        let files = [("root", caller), ("library", source)];
        let parsed: Vec<_> = files
            .iter()
            .map(|(_, text)| {
                let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
                let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
                (tokens, tree)
            })
            .collect();
        let inputs: Vec<_> = files
            .iter()
            .zip(&parsed)
            .map(|((package, text), (tokens, tree))| StageSource {
                package,
                source: ParsedSource {
                    logical_path: "main.omg",
                    text,
                    tokens,
                    tree,
                },
            })
            .collect();
        let edge = omega_resolver::Dependency {
            requester: "root",
            package: "library",
            alias: "kit",
        };
        let resolved = omega_toolchain::resolve_admitted_packages(&inputs, &[edge]).unwrap();
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let mut unit = omega_lowering::lower_package(&prepared).unwrap();
        let i = unit
            .machines
            .iter()
            .position(|m| m.declaration.path == "observed")
            .unwrap();
        let entry = unit.machines[i].declaration.clone();
        let proof = check_scalar_return(&unit, &entry).unwrap();
        assert_eq!(proof.bodies().count(), 2);
        let executable = admit_scalar_return(&proof).unwrap();
        let result = executable.execute(vec![], Ceilings::default()).unwrap();
        assert_eq!(result.result, Some(Value::Boolean(expected)));
        assert_eq!(result.usage.work_used, executable.certified_work());
        let comparison = unit.machines[i]
            .body
            .operations
            .iter_mut()
            .find_map(|op| match &mut op.kind {
                omega_psi::OperationKind::Integer { operation, .. } => Some(operation),
                _ => None,
            })
            .unwrap();
        comparison.primitive = omega_psi::IntegerPrimitive::Divide;
        assert!(check_scalar_return(&unit, &entry).is_err());
    }
}
