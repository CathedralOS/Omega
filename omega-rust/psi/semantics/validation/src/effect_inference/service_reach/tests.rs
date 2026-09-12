use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

fn dependency<'a>(
    program: &TypedTrees,
    plan: &'a ServiceReachInferencePlan,
    name: &str,
) -> (Vec<String>, &'a [SymbolHandle]) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("machine");
    let dependency = plan
        .for_machine(machine.symbol)
        .expect("summary")
        .dependency;
    let services = plan
        .rows
        .services(dependency.concrete)
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("service")
                .name
                .clone()
        })
        .collect();
    (
        services,
        plan.dependency_parameters
            .span_or_empty(dependency.parameters),
    )
}

const CONTRACT: &str = "boundary trait Console {}\ntrait Task { machine run() reaches Console; }\n";

#[test]
fn fixed_boundary_basis_uses_exact_requirement_and_keeps_overlapping_services() {
    let program = typed(
        r#"
        boundary trait Audit {}
        boundary trait Console {}
        boundary trait Installer: Audit {
            machine step() invokes Console; reaches <= Installer + Console;
        }
        trait Ordinary { machine step() reaches Console; }
        boundary trait Fixed { machine step() reaches Console; }
        machine ordinary() reaches Console {}
    "#,
    );
    let requirement = |name| {
        let owner = program
            .traits()
            .iter()
            .find(|owner| owner.name.as_str() == name)
            .expect("trait");
        program.trait_machine_signatures(owner)[0].symbol
    };
    let fixed = fixed_installation_boundary_service_reach(&program, requirement("Installer"))
        .expect("exact boundary requirement");
    let names = fixed
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("service")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["Audit", "Console", "Installer"]);
    assert_eq!(
        fixed_installation_boundary_service_reach(&program, requirement("Fixed")),
        Some(Vec::new())
    );
    assert!(fixed_installation_boundary_service_reach(&program, requirement("Ordinary")).is_none());
    let machine = &program.machines()[0];
    assert!(
        fixed_installation_boundary_service_reach(
            &program,
            program.machine_states(machine)[0].symbol
        )
        .is_none()
    );
    assert!(fixed_installation_boundary_service_reach(&program, SymbolHandle::default()).is_none());
}

#[test]
fn nominal_dependencies_substitute_per_call_and_preserve_structural_rows() {
    let program = typed(&format!("{CONTRACT}
        machine relay<Element, machine Forward>() where machine Forward satisfies Task::run; {{ Forward(); }}
        machine both<machine First, machine Second>()
        where machine First satisfies Task::run;
        where machine Second satisfies Task::run;
        {{ relay<u64, Second>(); relay<u64, First>(); relay<u64, Second>(); }}
        machine fixed<machine Step>() where machine Step() reaches Console; {{ Step(); }}"));
    let plan = infer_service_reaches(&program, &crate::infer_operational_may(&program));
    let (services, parameters) = dependency(&program, &plan, "both");
    assert!(services.is_empty());
    let owner = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "both")
        .unwrap();
    let expected = program
        .machine_type_parameters(owner)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    assert_eq!(parameters, expected);
    let (services, parameters) = dependency(&program, &plan, "fixed");
    assert_eq!(services, ["Console"]);
    assert!(parameters.is_empty());
}

#[test]
fn dependency_fixed_point_composes_recursive_generic_components() {
    let program = typed(&format!(
        "{CONTRACT}
        machine note() reaches Console {{}}
        machine first<machine Step>() where machine Step satisfies Task::run;
        {{ Step(); second<Step>(); }}
        machine second<machine Next>() where machine Next satisfies Task::run;
        {{ first<Next>(); note(); }}"
    ));
    let plan = infer_service_reaches(&program, &crate::infer_operational_may(&program));
    for name in ["first", "second"] {
        let (services, parameters) = dependency(&program, &plan, name);
        let owner = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert_eq!(services, ["Console"]);
        assert_eq!(
            parameters,
            &[program.machine_type_parameters(owner)[0].symbol]
        );
    }
}

#[test]
fn closed_dependency_uses_the_selected_contract_not_its_empty_body() {
    let program = typed(&format!(
        "{CONTRACT}
        machine quiet() satisfies Task::run {{}}
        machine conservative() satisfies Task::run reaches Console {{}}
        machine traverse<machine Step>() where machine Step satisfies Task::run; {{ Step(); }}
        machine empty() {{ traverse<quiet>(); }}
        machine published() {{ traverse<conservative>(); }}"
    ));
    let plan = infer_service_reaches(&program, &crate::infer_operational_may(&program));
    let (services, parameters) = dependency(&program, &plan, "empty");
    assert!(services.is_empty());
    assert!(parameters.is_empty());
    let (services, parameters) = dependency(&program, &plan, "published");
    assert_eq!(services, ["Console"]);
    assert!(parameters.is_empty());
}

#[test]
fn nested_static_applications_substitute_their_own_nominal_arguments() {
    let program = typed(&format!(
        "{CONTRACT}
        machine relay<machine Forward>() where machine Forward satisfies Task::run; {{ Forward(); }}
        machine traverse<machine Work>() where machine Work satisfies Task::run; {{ Work(); }}
        machine outer<machine Step>() where machine Step satisfies Task::run;
        {{ traverse<relay<Step>>(); }}"
    ));
    let plan = infer_service_reaches(&program, &crate::infer_operational_may(&program));
    let (services, parameters) = dependency(&program, &plan, "outer");
    let owner = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .unwrap();
    assert!(services.is_empty());
    assert_eq!(
        parameters,
        &[program.machine_type_parameters(owner)[0].symbol]
    );
}
