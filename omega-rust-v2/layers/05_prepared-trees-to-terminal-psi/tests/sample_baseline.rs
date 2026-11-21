//! Compatibility observations use pinned Git objects so editing a sample cannot change the result.
#[path = "sample_baseline/cases.rs"]
mod cases;
#[path = "support/control_flow.rs"]
mod control_flow;
#[path = "sample_baseline/pipeline.rs"]
mod pipeline;

use omega_lexer::SourceSpan;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

const REVISION: &str = "36d45157895e1674fcdc01222fdee0faa1b68082";

#[test]
#[ignore = "reads the pinned sample revision from repository history"]
fn fixed_sample_configurations_through_l5() {
    let mut git = Command::new("git")
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start git");
    let mut requests = git.stdin.take().unwrap();
    let mut replies = BufReader::new(git.stdout.take().unwrap());
    let mut blob = |directory: &str, file: &str| {
        let object = format!("{REVISION}:samples/cli/{directory}/{file}");
        writeln!(requests, "{object}").unwrap();
        let mut header = String::new();
        replies.read_line(&mut header).unwrap();
        let fields: Vec<_> = header.split_whitespace().collect();
        assert!(matches!(&fields[..], [_, "blob", _]), "cannot read {object}: {header}");
        let mut bytes = vec![0; fields[2].parse().unwrap()];
        replies.read_exact(&mut bytes).unwrap();
        let mut separator = [0];
        replies.read_exact(&mut separator).unwrap();
        assert_eq!(separator, [b'\n']);
        bytes
    };
    let expected: Vec<_> = include_str!("sample_baseline/observations.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(cases::CASES.len(), expected.len(), "review the fixed cohort size when changing cases");
    let mut changes = Vec::new();
    let report = std::env::var_os("OMEGA_LAYER_REPORT").is_some();
    if report {
        println!("OMEGA_EXPECTED\tsample\t{}", cases::CASES.len());
        println!("OMEGA_SOURCE\thistorical_samples\t{REVISION}");
    }
    for (case, previous) in cases::CASES.iter().zip(expected) {
        let build = blob(case.directory, "build.omg");
        case.confirm_configuration(std::str::from_utf8(&build).unwrap());
        let files: Vec<_> = match case.migrated {
            Some(sources) => sources
                .iter()
                .map(|&(path, bytes)| (path, bytes.to_vec()))
                .collect(),
            None => case
                .files
                .iter()
                .map(|&path| (path, blob(case.directory, path)))
                .collect(),
        };
        // The root's Build recipe registers each dependency; its own recipe names the package.
        let mut sources: Vec<(&str, &str, Vec<u8>)> = files
            .into_iter()
            .map(|(path, bytes)| (pipeline::ROOT, path, bytes))
            .collect();
        let mut admitted = Vec::new();
        for dependency in case.dependencies {
            let recipe = blob(dependency.directory, "build.omg");
            assert!(
                std::str::from_utf8(&recipe)
                    .unwrap()
                    .contains(dependency.configuration),
                "{}: dependency package declaration absent from pinned build.omg",
                case.id
            );
            sources.extend(
                dependency
                    .files
                    .iter()
                    .map(|&path| (dependency.package, path, blob(dependency.directory, path))),
            );
            // A negative control keeps the package's sources and omits or retargets only its edge.
            admitted.extend(dependency.admitted());
        }
        let sources: Vec<_> = sources
            .iter()
            .map(|(package, path, bytes)| (*package, *path, bytes.as_slice()))
            .collect();
        let observation = case.expectation.observe_admitted(&sources, &admitted);
        let actual = &observation.actual;
        // Every refusal is reported with its location, whether or not the case expects it.
        if let cases::Actual::Refused(refusal) = actual {
            println!("{}", refusal.marker(case.id));
        }
        // The actual stopping stage, or L5 when Psi was produced.
        let stage = actual.stage();
        if let Some(failure) = observation.mismatch {
            if report {
                let status = match actual {
                    cases::Actual::Product { .. } => "unexpected Psi product",
                    cases::Actual::Refused(_) => "inspection failed",
                };
                println!(
                    "OMEGA_CASE\tsample\t{}\t{stage}\t{status}\t{}",
                    case.id,
                    failure.replace(['\r', '\n', '\t'], " ")
                );
            }
            changes.push(format!("{}: {failure}", case.id));
            continue;
        }
        let outcome = actual.row();
        let row = match actual {
            cases::Actual::Refused(refusal) => refusal.baseline_row(case.id),
            _ => format!("{}\t{outcome}", case.id),
        };
        println!("{row}");
        if report {
            let (_, detail) = outcome
                .split_once('\t')
                .expect("each observation names its stage");
            let status = match (&case.expectation, actual) {
                (cases::Expectation::MustRefuse(_), _) => "intended rejection verified",
                (cases::Expectation::ExpectCheckerRefusal(_), cases::Actual::Product { .. }) => "expected L6 refusal unverified",
                (_, cases::Actual::Product { .. }) => "executable product inspected",
                _ => "compatibility blocked",
            };
            println!(
                "OMEGA_CASE\tsample\t{}\t{stage}\t{status}\t{}",
                case.id,
                detail.replace(['\r', '\n', '\t'], " ")
            );
        }
        if row != previous {
            changes.push(format!("previous: {previous}\n current: {row}"));
        }
    }
    drop(requests);
    assert!(git.wait().unwrap().success());
    assert!(changes.is_empty(), "review changes before updating the baseline:\n{}", changes.join("\n"));
}

fn current_import_files(main: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("main.omg", main.to_vec()),
        (cases::IMPORT_A.0, cases::IMPORT_A.1.to_vec()),
        (cases::IMPORT_B.0, cases::IMPORT_B.1.to_vec()),
    ]
}

/// A migrated twin replaces only the retired `Main::main` owner with the free
/// machine `main` and its call. Every other byte comes from the pinned history.
#[test]
#[ignore = "reads the pinned sample revision from repository history"]
fn migrated_samples_change_only_the_retired_machine_owner() {
    for id in ["cross-width-v2", "rational-landing-v2"] {
        let case = cases::CASES.iter().find(|case| case.id == id).unwrap();
        let Some(&[("main.omg", migrated)]) = case.migrated else {
            panic!("{id}: expected one migrated main.omg");
        };
        let object = format!("{REVISION}:samples/cli/{}/main.omg", case.directory);
        let output = Command::new("git")
            .args(["cat-file", "blob", &object])
            .output()
            .expect("start git");
        assert!(output.status.success(), "cannot read {object}");
        let historical = String::from_utf8(output.stdout).unwrap();
        let (owner, call) = ("machine Main::main(", "    Main::main(@mut entry_storage);");
        assert_eq!((historical.matches(owner).count(), historical.matches(call).count()), (1, 1), "{id}");
        let expected = historical
            .replace(owner, "machine main(")
            .replace(call, "    main(@mut entry_storage);");
        assert_eq!(std::str::from_utf8(migrated).unwrap(), expected, "{id}");
    }
}

#[test]
fn directory_import_and_its_missing_import_twin() {
    let main = include_bytes!("fixtures/direct_file_imports/main.omg");
    let twin = include_bytes!("fixtures/direct_file_imports/missing_import.omg");
    let main = std::str::from_utf8(main).unwrap().replace("\r\n", "\n");
    let twin = std::str::from_utf8(twin).unwrap().replace("\r\n", "\n");
    assert_eq!(main.replace("use m\\A;\n", ""), twin);
    let product = pipeline::lower(&current_import_files(main.as_bytes())).expect("the valid import must lower");
    assert!(
        cases::Check::NestedField.matches(&product),
        "expected the return value read from B.value then A.value"
    );
    let refusal = pipeline::lower(&current_import_files(twin.as_bytes())).unwrap_err();
    assert_eq!(refusal.to_string(), cases::MISSING_IMPORT);
}

/// The lawful twin of `import-from-dependency`: it imports the declaration instead of a file.
fn dependency_import_sources(main: &'static [u8]) -> [(&'static str, &'static str, &'static [u8]); 3] {
    let [kit] = cases::KIT else { unreachable!() };
    [
        (pipeline::ROOT, "main.omg", main),
        (kit.package, "main.omg", include_bytes!("fixtures/dependency_import/dep/main.omg")),
        (kit.package, "other.omg", include_bytes!("fixtures/dependency_import/dep/other.omg")),
    ]
}

#[test]
fn a_build_admitted_alias_imports_a_dependency_declaration_and_its_absence_refuses() {
    use omega_psi::{BlockTerminator, CallTarget, OperationKind, OperationResult, ReturnEdge, ScalarLiteral};
    let [kit] = cases::KIT else { unreachable!() };
    let edge = kit.admitted().expect("the registered edge");
    let (_, main) = cases::DEPENDENCY_IMPORT;
    let sources = dependency_import_sources(main);
    let product = pipeline::lower_admitted(&sources, &[edge]).expect("the admitted alias must lower");

    // Producer: entry's returned value is a call whose target id is the machine declared as
    // mod_kernel's other_v. No other package declares a machine with that path.
    let declared = |path: &str| {
        product
            .machines
            .iter()
            .filter(|m| m.declaration.path == path)
            .collect::<Vec<_>>()
    };
    let ([entry], [callee]) = (&declared("entry")[..], &declared("other_v")[..]) else {
        panic!("expected one entry and one other_v machine");
    };
    assert_eq!(
        (entry.declaration.package.as_str(), callee.declaration.package.as_str()),
        (pipeline::ROOT, kit.package)
    );
    let [block] = &entry.body.blocks[..] else { panic!("expected one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = block.terminator
    else {
        panic!("expected a scalar return");
    };
    let call = entry
        .body
        .operations
        .iter()
        .find(|op| op.result == OperationResult::Scalar(returned))
        .expect("an operation produces the returned value");
    spec_corpus::observe_handoff!(
        psi "psi-output/dependency-call",
        assert!(matches!(&call.kind, OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } if *id == callee.id && arguments.is_empty())),
    );

    // Independent consumer: the Psi-only client follows the same return edge to its callee's package and constant.
    let seven = ScalarLiteral::Integer(7.into());
    let expected = psi_handoff_consumer::ReturnedCall {
        package: kit.package,
        path: "other_v",
        arguments: 0,
        returns: Some(&seven),
    };
    assert_eq!(psi_handoff_consumer::returned_call(&product, pipeline::ROOT, "entry"), Ok(expected));
    assert!(cases::Check::DependencyCall.matches(&product));

    // A changed callee still crosses the edge, but it reaches mod_kernel's v, so the call witness rejects it.
    let other_callee = dependency_import_sources(b"use kit\\v;\nmachine entry() -> i32 {\n    v()\n}\n");
    let changed = pipeline::lower_admitted(&other_callee, &[edge]).expect("importing v must lower");
    let one = ScalarLiteral::Integer(1.into());
    assert_eq!(
        psi_handoff_consumer::returned_call(&changed, pipeline::ROOT, "entry"),
        Ok(psi_handoff_consumer::ReturnedCall {
            package: kit.package,
            path: "v",
            arguments: 0,
            returns: Some(&one),
        })
    );
    assert!(!cases::Check::DependencyCall.matches(&changed));

    // Negative twins keep every source byte and change only the edge: omitted, or retargeted to the toolchain.
    for admitted in [cases::KIT_WITHOUT_EDGE, cases::KIT_RETARGETED] {
        let edges: Vec<_> = admitted.iter().filter_map(cases::Admitted::admitted).collect();
        let refusal = pipeline::lower_admitted(&sources, &edges).unwrap_err();
        assert_eq!(refusal.to_string(), cases::UNAVAILABLE_DECLARATION_IMPORT);
    }
    let retargeted = cases::KIT_RETARGETED[0].admitted().map(|edge| edge.package);
    assert_eq!(retargeted, Some(omega_toolchain::TOOLCHAIN_PACKAGE));

    // The historical program imports the file name. The admitted edge leaves `kit\other` unbound.
    let historical = b"use kit\\other;\nmachine entry() -> i32 {\n    other_v()\n}\n";
    let refusal = pipeline::lower_admitted(&dependency_import_sources(historical), &[edge]).unwrap_err();
    assert_eq!(refusal.to_string(), "L2\tmain.omg:1:5\tunavailable import kit\\other");
}

#[test]
fn refusal_locations_use_only_bytes_from_the_named_package() {
    let sources: [(&str, &str, &[u8]); 3] = [
        ("root", "main.omg", b"ab\tc%\r\n-"),
        ("dep", "main.omg", b"\n\nxyz"),
        ("bin", "x.omg", &[b'a', 0xFF, b'%']),
    ];
    let at = |package: &str, file: &str, start: usize, end: usize| {
        pipeline::Refusal::locate("L3", package, file, SourceSpan::new(start, end), "why\t100%\nmore".into(), &sources)
    };
    let reason = "why%09100%25%0Amore";
    let observed = [
        // The dependency's file shares the root's name; its own lines and bytes locate it.
        (at("dep", "main.omg", 2, 5), "dep\tmain.omg\t2\t5\t3\t1\txyz"),
        (at("root", "main.omg", 1, 7), "root\tmain.omg\t1\t7\t1\t2\tb%09c%25%0D%0A"),
        (at("root", "main.omg", 7, 8), "root\tmain.omg\t7\t8\t2\t1\t%2D"),
        // Local empty and end-of-file spans have a position and an empty occurrence.
        (at("root", "main.omg", 0, 0), "root\tmain.omg\t0\t0\t1\t1\t"),
        (at("dep", "main.omg", 5, 5), "dep\tmain.omg\t5\t5\t3\t4\t"),
        // A span leaving the file keeps its start position only.
        (at("dep", "main.omg", 4, 9), "dep\tmain.omg\t4\t9\t3\t3\t-"),
        (at("dep", "main.omg", 9, 9), "dep\tmain.omg\t9\t9\t-\t-\t-"),
        // Another package's file of the same name is unavailable, not the root's.
        (at("@omega/toolchain", "main.omg", 2, 5), "@omega/toolchain\tmain.omg\t2\t5\t-\t-\t-"),
        (at("bin", "x.omg", 0, 3), "bin\tx.omg\t0\t3\t1\t1\ta%FF%25"),
    ];
    for (refusal, fields) in &observed {
        assert_eq!(refusal.marker("label"), format!("OMEGA_SAMPLE_REFUSAL\tlabel\tL3\t{fields}\t{reason}"));
        assert_eq!(refusal.reason, "why\t100%\nmore");
    }
    assert_eq!(observed[1].0.to_string(), "L3\tmain.omg:1:2\twhy\t100%\nmore");
    assert_eq!(observed[0].0.to_string(), "L3\t<dep>/main.omg:3:1\twhy\t100%\nmore");
    assert_eq!(observed[7].0.to_string(), "L3\t<@omega/toolchain>/main.omg (bytes 2..5)\twhy\t100%\nmore");
}

#[test]
fn a_dependency_refusal_keeps_its_package_when_file_names_repeat() {
    let root = b"public data A { value: i32; }\n";
    let dependency = b"\n\nmachine result() -> i32 {\n    missing\n}\n";
    let refusal = pipeline::lower_packages(&[("root", "main.omg", root), ("dep", "main.omg", dependency)]).unwrap_err();
    assert_eq!((refusal.stage, refusal.package.as_str(), refusal.file.as_str()), ("L2", "dep", "main.omg"), "{refusal}");
    assert!(refusal.reason.starts_with("unresolved name missing"), "{refusal}");
    let (start, end) = (refusal.span.get_start_offset(), refusal.span.get_end_offset());
    // Count the dependency's bytes independently of the line index.
    let prefix = &dependency[..start];
    let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
    let column = start - prefix.iter().rposition(|byte| *byte == b'\n').map_or(0, |newline| newline + 1) + 1;
    assert_eq!(refusal.position, Some((line, column)));
    assert_eq!(line, 4, "the root file of the same name has one line");
    assert_eq!(refusal.occurrence.as_deref(), Some(&dependency[start..end]));
    assert!(dependency[start..end].windows(7).any(|window| window == b"missing"));
    assert!(refusal.to_string().starts_with(&format!("L2\t<dep>/main.omg:4:{column}\tunresolved name missing")));
    assert!(
        refusal
            .marker("shared-name")
            .starts_with(&format!("OMEGA_SAMPLE_REFUSAL\tshared-name\tL2\tdep\tmain.omg\t{start}\t{end}\t4\t{column}\t"))
    );
}

#[test]
fn a_required_layout_without_a_target_reaches_preparation() {
    let source = br"use omega\language\core; const VALUE:u64=core\size_of::<u16>(); machine consume(bytes:[u8; VALUE]){}";
    let refusal = pipeline::lower_packages(&[("root", "main.omg", source)]).unwrap_err();
    assert_eq!(refusal.stage, "L4", "{refusal}");
    assert_eq!(refusal.occurrence.as_deref(), Some(br"core\size_of::<u16>()".as_slice()));
    assert_eq!(refusal.reason, "static evaluation cannot admit this invocation: the layout query requires a selected target");
}

#[test]
fn mismatched_expectations_report_where_compilation_actually_stopped() {
    let read = |bytes: &[u8]| std::str::from_utf8(bytes).unwrap().replace("\r\n", "\n");
    let twin = read(include_bytes!("fixtures/direct_file_imports/missing_import.omg"));
    let main = read(include_bytes!("fixtures/direct_file_imports/main.omg"));
    let refused = current_import_files(twin.as_bytes());
    let lowered = current_import_files(main.as_bytes());

    let expected_psi = cases::Expectation::MustLower(cases::Check::NestedField).observe(&refused);
    let cases::Actual::Refused(refusal) = &expected_psi.actual else {
        panic!("the twin must stop before Psi");
    };
    assert_eq!(expected_psi.actual.stage(), "L2");
    assert_eq!(refusal.to_string(), cases::MISSING_IMPORT);
    assert_eq!(
        (refusal.package.as_str(), refusal.file.as_str(), refusal.position, refusal.reason.as_str()),
        (pipeline::ROOT, "main.omg", Some((4, 30)), "unresolved name A")
    );
    let (start, end) = (refusal.span.get_start_offset(), refusal.span.get_end_offset());
    assert_eq!(refusal.occurrence.as_deref(), Some(&twin.as_bytes()[start..end]));
    assert!(twin.as_bytes()[start..].starts_with(b"A"));
    let marker = refusal.marker("missing-import-v2");
    assert!(marker.starts_with(&format!("OMEGA_SAMPLE_REFUSAL\tmissing-import-v2\tL2\troot\tmain.omg\t{start}\t{end}\t4\t30\t")));
    assert!(marker.ends_with("\tunresolved name A"));
    assert_eq!(expected_psi.mismatch, Some(format!("expected Psi, got {}", cases::MISSING_IMPORT)));

    let wrong = "L2\tmain.omg:1:1\tunresolved name B";
    let wrong_refusal = cases::Expectation::MustRefuse(wrong).observe(&refused);
    assert_eq!(wrong_refusal.actual, expected_psi.actual);
    assert_eq!(wrong_refusal.mismatch, Some(format!("expected {wrong}, got {}", cases::MISSING_IMPORT)));
    assert_eq!(cases::Expectation::MustRefuse(cases::MISSING_IMPORT).observe(&refused).mismatch, None);

    let unexpected_psi = cases::Expectation::MustRefuse(cases::MISSING_IMPORT).observe(&lowered);
    assert_eq!((unexpected_psi.actual.stage(), &unexpected_psi.actual), ("L5", &cases::Actual::Product { witness: None }));
    assert_eq!(unexpected_psi.mismatch, Some(format!("produced Psi; expected {}", cases::MISSING_IMPORT)));

    let other_behavior = cases::Expectation::MustLower(cases::Check::EmptyMain).observe(&lowered);
    assert_eq!((other_behavior.actual.stage(), &other_behavior.actual), ("L5", &cases::Actual::Product { witness: Some(false) }));
    assert_eq!(other_behavior.mismatch.as_deref(), Some("produced Psi without the required behavior"));
}

#[test]
fn an_unqualified_counter_preserves_the_two_samples_arithmetic_witnesses() {
    for (source, witness) in [
        (include_str!("fixtures/cross_width_compare/main.omg"), cases::Check::Comparisons),
        (include_str!("fixtures/rational_arg_landing/main.omg"), cases::Check::IdentityCalls),
    ] {
        // Both pinned sources stop at the same unresolved policy name. Vary only
        // the counter's qualification to test what each program reaches after lookup.
        let qualified = "hits: i32 in Wrapping;";
        assert_eq!(source.matches(qualified).count(), 1);
        let control = source.replacen(qualified, "hits: i32;", 1);
        let product = pipeline::lower(&[("main.omg", control.into_bytes())]).unwrap();
        assert!(witness.matches(&product));
    }
}

#[test]
fn a_passing_main_witness_cannot_hide_an_unowned_operation_in_another_machine() {
    let files = [("main.omg", b"machine main(){} machine helper()->u8{7u8}".to_vec())];
    let mut product = pipeline::lower(&files).unwrap();
    let expectation = cases::Expectation::MustLower(cases::Check::EmptyMain);
    assert!(expectation.observe(&files).mismatch.is_none());
    let helper = product.machines.iter_mut().find(|m| m.declaration.path == "helper").unwrap();
    assert_eq!(helper.body.operations.len(), 1);
    helper.body.blocks[0].operations = 0..0;

    assert!(cases::Check::EmptyMain.matches(&product));
    let observed = expectation.inspect_lowering(Ok(product));
    assert_eq!(observed.actual.stage(), "L5");
    assert_eq!(observed.mismatch.as_deref(), Some("machine helper: operation belongs to no reachable block"));
}
