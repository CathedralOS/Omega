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

/// Both legacy entry programs use a qualified main and the retired mutable binding and borrow spellings.
const FREE_MAIN: &[(&str, &str)] = &[
    ("machine Main::main(main: @mut Main)", "machine main(main: @mutable Main)"),
    ("machine pass(main: @mut Main)", "machine pass(main: @mutable Main)"),
    ("machine fail(main: @mut Main)", "machine fail(main: @mutable Main)"),
    ("let mut entry_storage: Main;", "let mutable entry_storage: Main;"),
    ("    Main::main(@mut entry_storage);", "    main(@mutable entry_storage);"),
];
/// The retired callback binder and its `requires machine` bound, written as a `const` binder
/// of a callable alias that keeps the bound's premise.
const RETIRED_BINDER: &str = "machine apply<machine Observe>(kept: bool) -> bool
requires machine Observe(kept: bool) -> bool
requires premise(kept);
requires premise(kept)
";
const CONST_BINDER: &str = "data Observer = machine(kept: bool) -> bool
requires premise(kept);

machine apply<const Observe: Observer>(kept: bool) -> bool
requires premise(kept)
";
/// V1's crash statement has no V2 form, so the unreached `bad` arm becomes empty.
const NO_CRASH_STATEMENT: &[(&str, &str)] = &[
    ("machine main() crashes Trap {", "machine main() {"),
    ("    machine bad() { crash Trap; }", "    machine bad() {}"),
];

/// Applies each replacement to exactly one occurrence.
fn replace_once(
    id: &str,
    text: &str,
    edits: &[(&str, &str)],
) -> String {
    edits.iter().fold(text.to_owned(), |text, (old, new)| {
        assert_eq!(text.matches(old).count(), 1, "{id}: {old:?}");
        text.replacen(old, new, 1)
    })
}

/// Each migrated twin changes only the named retired spellings. Every other byte comes from
/// the pinned history, so the twin measures the program the sample wrote.
#[test]
#[ignore = "reads the pinned sample revision from repository history"]
fn migrated_samples_change_only_retired_spellings() {
    let migrations: [(&str, Vec<(&str, &str)>); 4] = [
        ("cross-width-v2", [FREE_MAIN, &[("    hits: i32 in Wrapping;", "    hits: i32;")]].concat()),
        ("rational-landing-v2", [FREE_MAIN, &[("data Main { hits: i32 in Wrapping; }", "data Main { hits: i32; }")]].concat()),
        ("abstract-premise-v2", [&[(RETIRED_BINDER, CONST_BINDER)], NO_CRASH_STATEMENT].concat()),
        ("false-abstract-premise-v2", [&[(RETIRED_BINDER, CONST_BINDER)], NO_CRASH_STATEMENT].concat()),
    ];
    for (id, edits) in &migrations {
        let case = cases::CASES.iter().find(|case| case.id == *id).unwrap();
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
        assert_eq!(std::str::from_utf8(migrated).unwrap(), replace_once(id, &historical, edits), "{id}");
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

fn lower_text(source: &str) -> Result<omega_psi::LoweredPackage, pipeline::Refusal> {
    pipeline::lower(&[("main.omg", source.as_bytes().to_vec())])
}

/// The value types of one operation's operands, resolved through the package's type table.
fn operand_shapes(
    package: &omega_psi::LoweredPackage,
    machine: &omega_psi::LoweredMachine,
    operands: &[omega_psi::ValueId],
) -> Vec<Option<omega_psi::TypeShape>> {
    operands
        .iter()
        .map(|value| package.types.get(machine.body.values[value.index()].ty).copied())
        .collect()
}

/// The lawful cross-width twin lowers each comparison on its two fields at their own widths.
/// Putting back only the unimported `Wrapping` refuses at that name, and changing one field's
/// carrier keeps Psi but loses the cross-width witness.
#[test]
fn the_lawful_cross_width_twin_compares_fields_at_their_own_carriers() {
    use omega_psi::{IntegerObligation, IntegerPrimitive, OperationKind, ScalarType, TypeShape};
    let source = include_str!("fixtures/cross_width_compare/main.omg");
    let product = lower_text(source).expect("the lawful twin must lower");
    // Producer: `main` holds exactly the three comparisons, each exact, owing nothing, and
    // each joining two different integer carriers without a conversion between them.
    let main = product
        .machines
        .iter()
        .find(|m| m.declaration.path == "main")
        .unwrap();
    let comparisons: Vec<_> = main
        .body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Integer { operation, operands }
                if matches!(operation.primitive, IntegerPrimitive::NotEqual | IntegerPrimitive::LessThan | IntegerPrimitive::Equal) =>
            {
                Some((operation, operand_shapes(&product, main, operands)))
            }
            _ => None,
        })
        .collect();
    let int = |signed, width| Some(TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width }));
    assert_eq!(
        comparisons
            .iter()
            .map(|(operation, shapes)| (operation.primitive, operation.obligations.as_slice(), shapes.clone()))
            .collect::<Vec<_>>(),
        [
            (IntegerPrimitive::NotEqual, &[] as &[IntegerObligation], vec![int(true, 8), int(true, 32)]),
            (IntegerPrimitive::LessThan, &[], vec![int(true, 16), int(true, 64)]),
            (IntegerPrimitive::Equal, &[], vec![int(false, 8), int(true, 8)]),
        ]
    );
    // Consumer: the Psi-only reader names the field behind each operand.
    assert!(cases::Check::Comparisons.matches(&product));

    let qualified = replace_once("cross-width-v2", source, &[("    hits: i32;", "    hits: i32 in Wrapping;")]);
    assert_eq!(lower_text(&qualified).unwrap_err().to_string(), "L2\tmain.omg:17:18\tunresolved name Wrapping");

    let narrowed = replace_once("cross-width-v2", source, &[("    tiny: u8;", "    tiny: i8;")]);
    let product = lower_text(&narrowed).expect("an i8 field still lowers");
    assert!(!cases::Check::Comparisons.matches(&product), "the witness must read the u8 carrier");
}

/// The lawful rational-landing twin passes 7, the typed `i32` arithmetic and 7 to `identity`.
/// Only the typed expression owes carrier obligations. Putting back the unimported `Wrapping`
/// refuses at that name, and folding the typed argument loses the landing witness.
#[test]
fn the_lawful_rational_landing_twin_keeps_typed_arithmetic_at_its_carrier() {
    use omega_psi::{CallTarget, IntegerObligation, IntegerPrimitive, OperationKind};
    let source = include_str!("fixtures/rational_arg_landing/main.omg");
    let product = lower_text(source).expect("the lawful twin must lower");
    // Producer: the only integer arithmetic in `main` is the typed division and multiplication,
    // with the checker's carrier questions on each. The two anonymous expressions arrive folded.
    let main = product
        .machines
        .iter()
        .find(|m| m.declaration.path == "main")
        .unwrap();
    let identity = product
        .machines
        .iter()
        .find(|m| m.declaration.path == "identity")
        .unwrap();
    let arithmetic: Vec<_> = main
        .body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::Integer { operation, .. } if matches!(operation.primitive, IntegerPrimitive::Divide | IntegerPrimitive::Multiply) => {
                Some((operation.primitive, operation.obligations.as_slice()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        arithmetic,
        [
            (IntegerPrimitive::Divide, &[IntegerObligation::NonzeroDivisor, IntegerObligation::QuotientFitsCarrier] as &[_]),
            (IntegerPrimitive::Multiply, &[IntegerObligation::ResultFitsCarrier]),
        ]
    );
    let calls = main
        .body
        .operations
        .iter()
        .filter(|op| matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } if *id == identity.id && arguments.len() == 1))
        .count();
    assert_eq!(calls, 3);
    // Consumer: the Psi-only reader follows each argument to its defining operation.
    assert!(cases::Check::IdentityCalls.matches(&product));

    let qualified = replace_once("rational-landing-v2", source, &[("data Main { hits: i32; }", "data Main { hits: i32 in Wrapping; }")]);
    assert_eq!(lower_text(&qualified).unwrap_err().to_string(), "L2\tmain.omg:23:26\tunresolved name Wrapping");

    let folded = replace_once("rational-landing-v2", source, &[("let typed: i32 = identity(7i32 / 2 * 2);", "let typed: i32 = identity((7 / 2 * 2) as i32);")]);
    let product = lower_text(&folded).expect("an anonymous argument still lowers");
    assert!(!cases::Check::IdentityCalls.matches(&product), "the witness must see the typed arithmetic");
}

/// The lawful premise twins differ only in `premise`'s literal, and both lower. In each, `main`
/// calls the `apply<witness>` instance on `true` and owes its premise. The instance's call through
/// `Observe` executes `witness` and owes the `Observer` alias premise. `witness` keeps its own
/// premise for the refinement. L5 judges neither premise; the negative's refusal belongs to L6.
#[test]
fn the_lawful_premise_twins_publish_the_callback_target_and_both_premises() {
    use omega_psi::{CallArgument, CallRequirement, CallSite, CallTarget, CallbackInvocation, OperationKind, Parameter, StaticArgument};
    let read = |(_, bytes): (&str, &[u8])| std::str::from_utf8(bytes).unwrap().to_owned();
    let (positive, negative) = (read(cases::ABSTRACT_PREMISE_V2), read(cases::FALSE_ABSTRACT_PREMISE_V2));
    assert_eq!(
        replace_once("false-abstract-premise-v2", &positive, &[("    true\n}\n\nmachine hold", "    false\n}\n\nmachine hold")]),
        negative
    );
    for (source, value) in [(&positive, true), (&negative, false)] {
        let product = lower_text(source).expect("the lawful premise twin lowers");
        let named = |path: &str| {
            product
                .machines
                .iter()
                .find(|m| m.declaration.path == path)
                .unwrap()
        };
        let (main, apply, witness) = (named("main"), named("apply"), named("witness"));
        // Producer: the selection names witness and carries the alias clause.
        let StaticArgument::Callable(selection) = &apply.arguments[0] else {
            panic!("apply is instantiated with a callable selection")
        };
        assert_eq!(selection.machine, witness.id);
        let ([bound], [own], [instance]) = (&selection.requires[..], &witness.contract.requires[..], &apply.contract.requires[..]) else {
            panic!("expected one alias, one witness and one apply clause")
        };
        let at = |clause: &omega_psi::ContractClause| (clause.site.span.get_start_offset(), clause.site.span.slice(source.as_bytes()));
        let clause = Some(&b"requires premise(kept)"[..]);
        let after = |prefix: &str| source.find(prefix).unwrap() + prefix.len();
        assert_eq!(at(bound), (after("data Observer = machine(kept: bool) -> bool\n"), clause));
        assert_eq!(at(own), (after("machine witness(kept: bool) -> bool\n"), clause));
        assert_eq!(at(instance), (after("machine apply<const Observe: Observer>(kept: bool) -> bool\n"), clause));
        // Observe(kept) is a direct call to witness on apply's formal, owing the alias clause.
        let [Parameter::Scalar(kept)] = apply.body.parameters[..] else { panic!("one formal") };
        let [call] = apply.body.operations.as_slice() else { panic!("one operation") };
        let OperationKind::Call { callee, arguments, requirements } = &call.kind else { panic!("the callback call") };
        let site = CallSite::Operation(call.id);
        assert_eq!(
            (*callee, arguments.as_slice(), requirements.as_slice()),
            (CallTarget::Machine(witness.id), &[CallArgument::Scalar(kept)][..], &[CallRequirement { site, clause: 0 }][..])
        );
        assert_eq!(apply.body.callback_invocations, [CallbackInvocation { call: site, binder: 0 }]);
        // main's call to the instance owes the instance's own clause; it lists no invocation.
        let into_apply: Vec<_> = main
            .body
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::Call { callee: CallTarget::Machine(id), requirements, .. } if *id == apply.id => Some((op.id, requirements.as_slice())),
                _ => None,
            })
            .collect();
        let [(id, owed)] = into_apply[..] else { panic!("main calls the instance once") };
        assert_eq!(owed, [CallRequirement { site: CallSite::Operation(id), clause: 0 }]);
        assert!(main.body.callback_invocations.is_empty());
        // Consumer: the Psi-only reading agrees, and only the premise literal separates the twins.
        spec_corpus::observe_handoff!(psi "psi-output/sample-callback-premise",
            assert!(cases::Check::Premise(value).matches(&product)));
        assert!(!cases::Check::Premise(!value).matches(&product));
    }

    // One fact at a time on the positive. Each still lowers; the consumer sees only that fact change.
    let reading = |source: &str| {
        let product = lower_text(source).unwrap_or_else(|refusal| panic!("{refusal}"));
        let apply = product.machines.iter().find(|m| m.declaration.path == "apply").unwrap();
        let rows = psi_handoff_consumer::callback_premises(&product, apply).expect("well-formed callback listing");
        let [row] = rows.as_slice() else { panic!("one callback invocation") };
        let reading = (row.selected.declaration.path.clone(), row.owed.len(), row.provided.len());
        (reading, cases::callback_premise(&product))
    };
    assert_eq!(reading(&positive), (("witness".into(), 1, 1), Ok(())));
    let target = replace_once("abstract-premise-v2", &positive, &[("apply::<witness>(kept)", "apply::<premise>(kept)")]);
    assert_eq!(reading(&target).0, ("premise".into(), 1, 0));
    let open = replace_once(
        "abstract-premise-v2",
        &positive,
        &[("data Observer = machine(kept: bool) -> bool\nrequires premise(kept);\n", "data Observer = machine(kept: bool) -> bool;\n")],
    );
    assert_eq!(reading(&open).0, ("witness".into(), 0, 1));
    let unpremised = replace_once(
        "abstract-premise-v2",
        &positive,
        &[("machine witness(kept: bool) -> bool\nrequires premise(kept)\n", "machine witness(kept: bool) -> bool\n")],
    );
    assert_eq!(reading(&unpremised).0, ("witness".into(), 1, 0));
    for changed in [&target, &open, &unpremised] {
        assert!(reading(changed).1.is_err(), "the witness must reject the changed fact");
    }

    let retired = replace_once("abstract-premise-v2", &positive, &[(CONST_BINDER, RETIRED_BINDER)]);
    let refusal = lower_text(&retired).unwrap_err();
    assert_eq!(
        (refusal.stage, refusal.reason.as_str()),
        ("L3", "logical hypothesis cannot promise an observed Type result"),
        "{refusal}"
    );
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
