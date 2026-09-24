use super::{checked_tree_pruning_project, diagnostic_messages, fixture_package_identity, project};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct, compile_to_checked,
};
use optimization_core::{Optimization, OptimizationReportRequest};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};

#[test]
fn terminal_product_eliminates_unused_block_parameters_and_edge_arguments() {
    let root = project(
        "terminal-dead-block-parameters",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal_dead_block_parameters");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::DeadPureScalarElimination);
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (1)\n\
                 _ -> (2)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    )
    .expect("write dead block parameter source");
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::DeadPureScalarElimination]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    selected.artifact().unwrap().validate().unwrap();
    let identity_module =
        terminal_codec::decode_module(identity.artifact().unwrap().semantic_bytes())
            .expect("identity artifact decodes");
    let selected_module =
        terminal_codec::decode_module(selected.artifact().unwrap().semantic_bytes())
            .expect("selected artifact decodes");
    let compute_id = |module: &terminal_psi::TerminalModule| {
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. } => Some(*callee),
                _ => None,
            })
            .expect("main calls compute")
    };
    fn machine(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::MachineId,
    ) -> &terminal_psi::TerminalMachine {
        module
            .machines
            .iter()
            .find(|machine| machine.id == id)
            .expect("machine exists")
    }
    let old_compute = machine(&identity_module, compute_id(&identity_module));
    let new_compute = machine(&selected_module, compute_id(&selected_module));
    for operation in new_compute
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
    {
        assert!(
            !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerLessThan { .. }
            ),
            "the dead comparison is eliminated before publication"
        );
        assert!(
            !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Signed(7)
                }
            ),
            "the dead constant is eliminated before publication"
        );
    }
    let scalar_wiring = |machine: &terminal_psi::TerminalMachine| -> (usize, usize) {
        let parameters = machine
            .blocks
            .iter()
            .map(|block| block.parameters.len())
            .sum();
        let arguments = machine
            .blocks
            .iter()
            .map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump { arguments, .. } => arguments.len(),
                terminal_psi::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true.arguments.len() + when_false.arguments.len(),
                _ => 0,
            })
            .sum();
        (parameters, arguments)
    };
    let (old_parameters, old_arguments) = scalar_wiring(old_compute);
    let (new_parameters, new_arguments) = scalar_wiring(new_compute);
    assert!(
        new_parameters < old_parameters,
        "selected compute drops dead block parameters: {old_parameters} -> {new_parameters}"
    );
    assert!(
        new_arguments < old_arguments,
        "selected compute drops matching edge arguments: {old_arguments} -> {new_arguments}"
    );
    let operation_kinds = |machine: &terminal_psi::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| operation.kind.clone())
            .collect::<Vec<_>>()
    };
    for old_main in &identity_module.machines {
        if old_main.id == old_compute.id {
            continue;
        }
        let new_main = machine(&selected_module, old_main.id);
        assert_eq!(operation_kinds(new_main), operation_kinds(old_main));
    }
}

#[test]
fn terminal_product_propagates_copies_through_block_parameters() {
    let root = project(
        "terminal-copy-propagation",
        Some(
            r#"
machine build(builder: &mut Build) {
    builder.application("terminal_copy_propagation");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::CopyPropagation);
}
"#,
        ),
    );
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (1)\n\
                 _ -> (2)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    )
    .expect("write forwarded-copy source");
    let request = || {
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    };
    let identity = compiler::compile(request().with_optimization_rollback(
        OptimizationRollback::new([Optimization::CopyPropagation]).unwrap(),
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("rollback executes the identity stage");
    let selected = compiler::compile(request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("selected pre-Terminal pass executes");
    selected.artifact().unwrap().validate().unwrap();
    let identity_module =
        terminal_codec::decode_module(identity.artifact().unwrap().semantic_bytes())
            .expect("identity artifact decodes");
    let selected_module =
        terminal_codec::decode_module(selected.artifact().unwrap().semantic_bytes())
            .expect("selected artifact decodes");
    let compute_id = |module: &terminal_psi::TerminalModule| {
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. } => Some(*callee),
                _ => None,
            })
            .expect("main calls compute")
    };
    fn machine(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::MachineId,
    ) -> &terminal_psi::TerminalMachine {
        module
            .machines
            .iter()
            .find(|machine| machine.id == id)
            .expect("compute machine exists")
    }
    let old_compute = machine(&identity_module, compute_id(&identity_module));
    let new_compute = machine(&selected_module, compute_id(&selected_module));
    let scalar_wiring = |machine: &terminal_psi::TerminalMachine| -> (usize, usize) {
        let parameters = machine
            .blocks
            .iter()
            .map(|block| block.parameters.len())
            .sum();
        let arguments = machine
            .blocks
            .iter()
            .map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump { arguments, .. } => arguments.len(),
                terminal_psi::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true.arguments.len() + when_false.arguments.len(),
                _ => 0,
            })
            .sum();
        (parameters, arguments)
    };
    let (old_parameters, old_arguments) = scalar_wiring(old_compute);
    let (new_parameters, new_arguments) = scalar_wiring(new_compute);
    assert!(
        new_parameters < old_parameters,
        "selected compute collapses forwarded copies: {old_parameters} -> {new_parameters}"
    );
    assert!(
        new_arguments < old_arguments,
        "selected compute drops matching edge arguments: {old_arguments} -> {new_arguments}"
    );
    let dispatch = new_compute
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                terminal_psi::Terminator::Conditional { .. }
            )
        })
        .expect("the equality dispatch survives");
    let equal = dispatch
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::IntegerEqual { left, right } => Some((*left, *right)),
            _ => None,
        })
        .expect("the dispatch condition survives");
    assert_eq!(
        equal,
        (new_compute.parameters[0].id, new_compute.parameters[1].id),
        "the equality reads the machine parameters directly"
    );
    assert_ne!(
        identity.artifact().unwrap().semantic_bytes(),
        selected.artifact().unwrap().semantic_bytes(),
        "copy propagation changes the published semantic bytes"
    );
}

#[test]
fn terminal_product_retains_the_exact_pending_physical_selection() {
    let root = project(
        "selected-terminal-physical-proposal",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer_selected_terminal_physical_proposal");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.optimizations.enable(Optimization::SelectedIncomingU12ExactAddImmediate);
}
"#,
        ),
    );
    let report = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("a physical selection remains pending after Terminal publication");
    let proposal = report
        .terminal_native_realization_proposal()
        .expect("target-constrained Terminal product retains its native proposal");
    let expected = optimization_core::PostTerminalOptimizationSelections::new(
        optimization_core::OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        proposal.post_terminal_optimizations().selections(),
        &expected
    );
    assert_eq!(
        proposal.post_terminal_optimizations().complete_selection(),
        expected.identity()
    );
}

#[test]
fn dependency_build_selection_cannot_enable_root_package_optimization() {
    let root = project("dependency-selection", None);
    std::fs::write(
        root.join("main.omg"),
        "use dep::values;\ndata Main { value: u8; }\n",
    )
    .expect("write package-aware optimizer root");
    let dependency = root.with_file_name(format!(
        "{}-dependency",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 optimizer test root")
    ));
    std::fs::create_dir(&dependency).expect("create optimizer dependency");
    std::fs::write(dependency.join("values.omg"), "pub const VALUE: u8 = 1;\n")
        .expect("write optimizer dependency source");
    std::fs::write(
        dependency.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.optimizations.enable(Optimization::ProofCheckElision);
    builder.optimizations.emit_report();
}
"#,
    )
    .expect("write dependency optimizer build");
    let root_identity = fixture_package_identity(1);
    let dependency_identity = fixture_package_identity(2);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "root", root.clone()),
            PackageSourceBinding::new(dependency_identity, "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "dep",
            dependency_identity,
        )],
    )
    .expect("optimizer package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("dependency build companion must not join root compilation");
    assert!(checked.optimization_selections().is_empty());
    assert_eq!(
        checked.optimization_report_request(),
        OptimizationReportRequest::Suppressed
    );
}

#[test]
fn package_aware_root_build_retains_its_exact_selection() {
    let root = project(
        "package-root-selection",
        Some(
            r#"machine build(builder: &mut Build) {
    builder.application("optimizer_package_root_selection");
    builder.optimizations.enable(Optimization::GlobalValueNumbering);
}
"#,
        ),
    );
    let root_identity = fixture_package_identity(3);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("root-only optimizer package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("root package build selection should check");
    assert_eq!(
        checked.optimization_selections().as_slice(),
        &[Optimization::GlobalValueNumbering]
    );
}

#[test]
fn checked_tree_product_pruning_retains_the_selected_product_root() {
    let root = checked_tree_pruning_project("checked_tree_pruning", true, true);

    let checked = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("the selected checked-tree product compiles");

    // The exact bundled Linux entry contract retains its evaluated Extent
    // predicate machine alongside the authored root.
    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(machine_names, ["Main::main", "no_wrap"]);

    let entry = checked
        .selected_program_entry()
        .expect("the selected target retains its program entry")
        .source_signature()
        .machine_symbol();
    let selection = checked
        .checked_tree_product_selection()
        .expect("checked-tree product pruning retains its selection evidence");
    assert_eq!(selection.roots().machines(), &[entry]);
    let contract_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "no_wrap")
        .expect("the exact Linux entry contract retains its Extent predicate")
        .symbol;
    assert_eq!(
        selection.retained_machines(),
        &[entry, contract_machine],
        "entry selection retains the contract machine; pruning retains the root"
    );
    let pruned_names = selection
        .pruned_machines()
        .iter()
        .map(|symbol| checked.typed.symbols.display_path(*symbol, "::"))
        .collect::<Vec<_>>();
    for pruned in ["Dead::unused", "build"] {
        assert!(
            pruned_names.iter().any(|name| name.as_str() == pruned),
            "pruned: {pruned_names:?}"
        );
    }
    assert_ne!(selection.identity().as_bytes(), [0; 32]);
    assert!(
        checked
            .optimization_selections()
            .contains(Optimization::CheckedTreeProductPruning)
    );
}

#[test]
fn absent_checked_tree_pruning_selection_is_the_identity_boundary() {
    let root = checked_tree_pruning_project("checked_tree_identity", false, true);

    let checked = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect("an unselected checked-tree phase is the identity boundary");

    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    for name in ["Main::main", "Dead::unused", "build"] {
        assert!(machine_names.contains(&name), "retained: {machine_names:?}");
    }
    assert!(checked.checked_tree_product_selection().is_none());
}

#[test]
fn checked_tree_product_pruning_without_a_bound_product_root_rejects() {
    let root = checked_tree_pruning_project("checked_tree_no_roots", true, false);

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect_err("product pruning selected without a bound product root rejects");
    assert!(
        diagnostic_messages(&diagnostics).contains("requires at least one bound product root"),
        "unexpected diagnostics: {}",
        diagnostic_messages(&diagnostics)
    );
}

#[test]
fn checked_tree_product_pruning_cannot_hide_an_invalid_authored_declaration() {
    let root = checked_tree_pruning_project("checked_tree_invalid", true, true);
    std::fs::write(
        root.join("main.omg"),
        "data Main { value: u8; }\nmachine Main::main(&mut self) { }\ndata Dead { value: u8; }\nmachine Dead::unused(&mut self) { self.missing(); }\n",
    )
    .expect("write unreachable invalid authored declaration");

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect_err("checking precedes pruning, so the unreachable invalid machine rejects");
    let messages = diagnostic_messages(&diagnostics);
    assert!(
        messages.contains("missing"),
        "unexpected diagnostics: {messages}"
    );
    assert!(
        !messages.contains("product root"),
        "the rejection must come from checking, not pruning: {messages}"
    );
}

#[test]
fn checked_tree_product_pruning_rollback_restores_the_full_product() {
    let root = checked_tree_pruning_project("checked_tree_rollback", true, true);

    let checked = compile_to_checked(CheckedCompileRequest {
        optimization_rollback: OptimizationRollback::new([Optimization::CheckedTreeProductPruning])
            .expect("the rollback selection is unique"),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("the rolled-back checked-tree phase is the identity boundary");

    let machine_names = checked
        .typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    for name in ["Main::main", "Dead::unused", "build"] {
        assert!(machine_names.contains(&name), "retained: {machine_names:?}");
    }
    assert!(checked.checked_tree_product_selection().is_none());
    // The authored selection remains the retained identity even though the
    // effective selection executed as identity.
    assert!(
        checked
            .optimization_selections()
            .contains(Optimization::CheckedTreeProductPruning)
    );
}
