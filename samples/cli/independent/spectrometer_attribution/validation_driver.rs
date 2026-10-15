//! Private runtime validator, not an implementation of the spectrum algorithm.
//! Uses the same reviewed fixture inputs and retained native/Terminal products
//! as the repository corpus runner. No production acceptance is persisted.
use compiler::runtime::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeRequest,
};
use compiler::{CompileRequest, RequestedCompileProduct};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "../../../../omega-rust/omega/compilation/driver/tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

fn request(
    root: &Path,
    build: &Path,
    product: RequestedCompileProduct,
    native: bool,
) -> CompileRequest {
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        root,
        Some("linux_x86_64"),
    )
    .unwrap_or_else(|errors| panic!("fixture preparation: {errors:?}"));
    let snapshot = fixture_package_inputs::fixture_build_snapshot(&prepared.root_path)
        .unwrap_or_else(|errors| panic!("build snapshot: {errors:?}"));
    let targets = if native {
        vec![Some(
            target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap(),
        )]
    } else {
        vec![None]
    };
    let mut request = CompileRequest::for_targets(prepared.root_path, build.to_path_buf(), targets)
        .with_requested_product(product);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    if let Some(snapshot) = snapshot {
        request = request.with_build_snapshot(snapshot);
    }
    request
}

fn observe(
    directory: &Path,
    name: &str,
    engine: &str,
    exit: i32,
    stdout: &[u8],
    stderr: &[u8],
    expected_exit: i32,
    expected_stdout: &[u8],
) {
    let path = directory.join(name);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join(format!("{engine}.stdout")), stdout).unwrap();
    fs::write(path.join(format!("{engine}.stderr")), stderr).unwrap();
    fs::write(path.join(format!("{engine}.exit")), format!("{exit}\n")).unwrap();
    assert_eq!(exit, expected_exit, "{name}: {engine} exit");
    assert_eq!(stdout, expected_stdout, "{name}: {engine} stdout");
    assert!(stderr.is_empty(), "{name}: {engine} stderr: {:?}", stderr);
    println!("{name}\t{engine}\texit:{exit}\tstdout:match\tstderr:match");
    std::io::stdout().flush().unwrap();
}

// Read-only inspection of a sealed Check product; grants no selection authority.
fn verify_value_equality_selection(root: &Path, evidence: &Path) {
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    )
    .unwrap_or_else(|errors| panic!("identity fixture preparation: {errors:?}"));
    let snapshot = fixture_package_inputs::fixture_build_snapshot(&prepared.root_path)
        .unwrap_or_else(|errors| panic!("identity build snapshot: {errors:?}"));
    let mut request =
        compiler::CheckedCompileRequest::new(&prepared.root_path, Some("linux_x86_64"));
    request.package_inputs = prepared.package_inputs;
    request.build_snapshot = snapshot;
    request.build_dir = Some(evidence.join("selection_check"));
    let checked = compiler::CompilationSession::default()
        .check(request)
        .result
        .unwrap_or_else(|failure| panic!("identity checked product: {failure:?}"))
        .into_program();
    let program = &checked.typed;
    let machines = program
        .machines()
        .iter()
        .filter(|machine| {
            program.symbols.display_path(machine.symbol, "::")
                == "spectrometer_equality_control::same"
        })
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        panic!("one exact same declaration required");
    };
    let state = program
        .machine_states(machine)
        .first()
        .expect("same entry state");
    let body = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    let [typed_trees::statement::StatementNode::Expression(body_expression)] = body else {
        panic!("same must have one reachable expression body");
    };
    let parameters = program.state_parameters(state);
    assert_eq!(parameters.len(), 2);
    assert!(
        parameters
            .iter()
            .all(|parameter| program.display_type_reference(parameter.type_reference) == "f64")
    );
    let operators = program
        .all_operators()
        .chain(program.machine_token_bindings())
        .filter(|operator| {
            program.operator_path_string(operator.name) == "Float::equal"
                && program.operator_parameters(operator).len() == 2
                && program
                    .operator_parameters(operator)
                    .iter()
                    .all(|parameter| {
                        program.display_type_reference(parameter.type_reference) == "f64"
                    })
                && program.display_type_reference(operator.return_type) == "bool"
        })
        .collect::<Vec<_>>();
    let [expected] = operators.as_slice() else {
        panic!("one exact Float::equal declaration required");
    };
    assert!(
        expected.is_public,
        "Float::equal is the independently nameable public declaration"
    );
    let target_machine = program
        .machine(expected.symbol)
        .expect("Float::equal boundary machine declaration");
    assert!(target_machine.is_public);
    let expected_span = program
        .symbols
        .symbol_source_span(expected.symbol)
        .expect("authored Float::equal source");
    let expected_file = program
        .symbols
        .source_file(expected_span)
        .expect("retained Float::equal source file");
    assert!(expected_file.path.ends_with("core/float_operations.omg"));
    let expression = *body_expression;
    let body_span = program.expression_table.source_span(expression);
    let owner_span = program
        .symbols
        .symbol_source_span(machine.symbol)
        .expect("authored same source identity");
    assert_eq!(body_span.source_id, owner_span.source_id);
    let mut observed = Vec::new();
    let mut occurrences = std::collections::HashSet::new();
    {
        let binary = program
            .expression_table
            .binary_node(expression)
            .expect("same exact binary body");
        assert_eq!(
            binary.operator,
            typed_trees::expression::BinaryOperator::Equal
        );
        let left = program
            .expression_table
            .name_path_node(binary.left)
            .expect("same left parameter");
        let right = program
            .expression_table
            .name_path_node(binary.right)
            .expect("same right parameter");
        assert_eq!(left.symbol, parameters[0].symbol);
        assert_eq!(right.symbol, parameters[1].symbol);
        for occurrence in checked.authored_selection_occurrences(expression) {
            let selection = checked
                .authored_declaration_selections()
                .get(occurrence)
                .expect("retained exact authored occurrence");
            assert_eq!(
                selection.kind(),
                typed_trees::AuthoredDeclarationSelectionKind::Operator
            );
            assert_eq!(selection.source_span(), body_span);
            occurrences.insert(occurrence);
            let target = selection.target();
            let resolved = target
                .as_resolved()
                .expect("ordinary equality retains declaration identity");
            assert_eq!(resolved.selected_symbol(), expected.symbol);
            observed.push(format!("occurrence={occurrence:?} operator={:?} span={:?} target={:?} declaration=Float::equal", binary.operator, selection.source_span(), resolved.selected_symbol()));
        }
    }
    assert_eq!(
        occurrences.len(),
        1,
        "one distinct authored equality occurrence in exact same entry body"
    );
    let text = observed.join("\n") + "\n";
    fs::create_dir_all(evidence).unwrap();
    fs::write(evidence.join("selection-identity.txt"), &text).unwrap();
    print!("{text}");
}

fn main() {
    let root = PathBuf::from(env::args_os().nth(1).expect("application directory"));
    if env::args_os().any(|argument| argument == "--selection-identity") {
        verify_value_equality_selection(&root, &root.join("build/selection_identity"));
        return;
    }
    let terminal_only = env::args_os().any(|argument| argument == "--terminal-only");
    let fixture = env::args_os()
        .nth(2)
        .is_some_and(|argument| argument == "--fixture");
    let evidence = root.join(if terminal_only {
        "build/terminal_matrix"
    } else {
        "build/runtime_matrix"
    });
    fs::create_dir_all(&evidence).unwrap();
    let root_source = root.join("main.omg");
    let check = compiler::compile(request(
        &root_source,
        &evidence.join("check"),
        RequestedCompileProduct::Check,
        false,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|errors| panic!("Check product: {errors:?}"));
    drop(check);
    fs::write(evidence.join("check.txt"), "Check product succeeded\n").unwrap();
    println!("check\tpassed");
    let report = compiler::compile(request(
        &root_source,
        &evidence.join("native"),
        if terminal_only {
            RequestedCompileProduct::TerminalArtifact
        } else {
            RequestedCompileProduct::NativeArtifact
        },
        true,
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|errors| panic!("native product: {errors:?}"));
    let terminal = if terminal_only {
        report.artifact().expect("retained Terminal artifact")
    } else {
        let artifact = report
            .retained_native_artifact()
            .expect("retained native image");
        image_emission::verify_retained_target_operations(artifact.object()).unwrap();
        artifact.psi_artifact()
    };
    fs::write(evidence.join("program.terminal"), terminal.to_bytes()).unwrap();
    let manifest = if fixture {
        format!(
            "fixture\t{}",
            fs::read_to_string(root.join("expected_native_exit.txt"))
                .unwrap()
                .trim()
        )
    } else {
        fs::read_to_string(root.join("build/runtime-cases.tsv")).unwrap()
    };
    let cases = manifest
        .lines()
        .map(|line| {
            let (name, exit) = line.split_once('\t').expect("name and expected exit");
            assert!(
                name.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            );
            (name.to_owned(), exit.parse::<i32>().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cases.len(),
        if fixture { 1 } else { 32 },
        "complete requested runtime matrix"
    );
    let input_path = |name: &str| {
        if fixture {
            root.join("input.txt")
        } else {
            root.join(format!("data/{name}.input"))
        }
    };
    let expected_path = |name: &str| {
        if fixture {
            root.join("expected_stdout.txt")
        } else {
            root.join(format!("data/{name}.expected"))
        }
    };
    for (name, expected_exit) in &cases {
        let input = fs::read(input_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("input: {error}")
            }
        });
        let expected_stdout = fs::read(expected_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("stdout oracle: {error}")
            }
        });
        let result = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact: terminal,
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            target: Some("linux_x86_64"),
            fuel_allowance: 1_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(std::io::Cursor::new(input))),
            clock: Some(TerminalClockSnapshot::capture_host_epoch().unwrap()),
        })
        .unwrap_or_else(|error| panic!("{name}: Terminal admission: {error:?}"));
        let exit = match result.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => panic!("{name}: Terminal outcome: {other:?}"),
        };
        observe(
            &evidence,
            name,
            "Terminal",
            exit,
            &result.stdout,
            &result.stderr,
            *expected_exit,
            &expected_stdout,
        );
    }
    if terminal_only {
        println!(
            "matrix\tpassed\t{} inputs\t{} Terminal executions",
            cases.len(),
            cases.len()
        );
        return;
    }
    let published =
        compilation_products::publication::publish_compilation(report, &evidence.join("native"))
            .unwrap();
    let executable = published
        .checked_native_executable_path()
        .expect("published executable");
    fs::copy(executable, evidence.join("spectrometer.native")).unwrap();
    for (name, expected_exit) in &cases {
        let input = match fs::File::open(input_path(name)) {
            Ok(file) => Stdio::from(file),
            Err(error) if fixture && error.kind() == std::io::ErrorKind::NotFound => Stdio::null(),
            Err(error) => panic!("input: {error}"),
        };
        let expected_stdout = fs::read(expected_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("stdout oracle: {error}")
            }
        });
        let output = Command::new(executable)
            .current_dir(&evidence)
            .stdin(input)
            .output()
            .unwrap();
        let exit = output
            .status
            .code()
            .expect("normal native process termination");
        observe(
            &evidence,
            name,
            "native",
            exit,
            &output.stdout,
            &output.stderr,
            *expected_exit,
            &expected_stdout,
        );
    }
    println!(
        "matrix\tpassed\t{} inputs\t{} executions",
        cases.len(),
        cases.len() * 2
    );
}
