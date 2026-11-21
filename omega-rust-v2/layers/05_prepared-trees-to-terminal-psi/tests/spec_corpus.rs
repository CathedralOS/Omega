use std::collections::{BTreeMap, BTreeSet};

use tree_handoff_consumer::obligation_report;
#[path = "support/preparation.rs"]
mod preparation;

#[path = "support/control_flow.rs"]
mod control_flow;

use omega_lowering::{
    BlockTerminator, LoweredPackage, OperationKind, OperationResult, PredicateOperator, PredicateStep, ReturnEdge, ScalarLiteral, ScalarType, TypeShape,
    lower_package,
};
use omega_parser::parse_fragment;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use spec_corpus::parser_examples::{fragment_kind, parser_source};

const RESOLUTION: &str = include_str!("../../02_syntax-trees-to-symbol-resolved-trees/tests/spec-outcomes.txt");
const TYPING: &str = include_str!("../../03_symbol-resolved-trees-to-typed-trees/tests/spec-outcomes.txt");
const LOWERING: &str = include_str!("spec-outcomes.txt");

/// These fences demonstrate local statements. Lowering
/// receives packages, so the bare fragments are expected to refuse.
const FRAGMENTS_WITHOUT_PACKAGE_CONTEXT: &[&str] = &["language/data_and_literals.md:435", "layouts/recasts.md:18"];

fn outcomes(rows: &str) -> BTreeMap<&str, &str> {
    rows.lines()
        .map(|line| line.split_once("  ").unwrap())
        .collect()
}

#[test]
fn spec_fragments_have_reviewed_lowering_outcomes() {
    let examples = spec_corpus::published_examples().unwrap();
    let resolution = outcomes(RESOLUTION);
    let typing = outcomes(TYPING);
    let expected_lowering = outcomes(LOWERING);
    let example_count = examples.len();
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_INVENTORY\t{}", obligation_report::KINDS.join(","));
        println!("OMEGA_EXPECTED\tspec\t{example_count}");
    }
    let mut lowered = BTreeMap::new();
    let mut frontend_refusals = 0;
    let mut failures = Vec::new();
    if example_count != resolution.len() {
        failures.push(format!("found {example_count} spec examples and {} resolution records", resolution.len()));
    }
    let mut inspection_failures = 0;
    // One final record per example, completed only after the loop and the snapshot comparison below.
    let mut receipts = Vec::with_capacity(example_count);
    let mut labels = BTreeSet::new();
    for example in examples {
        let label = example.get_label();
        if !labels.insert(label.clone()) {
            failures.push(format!("{label}: duplicate spec example label"));
        }
        // The witnesses use Rust assertions. Preserve each failure and inspect the next independent example.
        let expected_frontend = match resolution.get(label.as_str()) {
            Some(&"accepted") => typing.get(label.as_str()).copied().unwrap_or("<missing typing expectation>").to_owned(),
            Some(reason) => format!("L2 {reason}"),
            None => "<missing resolution expectation>".to_owned(),
        };
        let frontend_recorded = match resolution.get(label.as_str()) {
            Some(&"accepted") => typing.contains_key(label.as_str()),
            Some(_) => true,
            None => false,
        };
        match std::panic::catch_unwind(|| inspect_example(&example)) {
            Ok(FragmentOutcome::Refused { stage, diagnostic, outcome }) => {
                frontend_refusals += 1;
                let frontend = if !frontend_recorded {
                    "absent"
                } else if outcome == expected_frontend {
                    "matched"
                } else {
                    "mismatched"
                };
                let status = if outcome == expected_frontend {
                    "recorded refusal"
                } else {
                    failures.push(format!("{label}: expected {expected_frontend}, observed {stage} · {diagnostic}"));
                    "inspection failed"
                };
                report_case(&label, stage, status, &diagnostic);
                receipts.push(Receipt::new(&label, status, frontend, "none"));
            }
            Ok(FragmentOutcome::Lowering(outcome)) => {
                let frontend = if !frontend_recorded {
                    "absent"
                } else if expected_frontend == "accepted" {
                    "matched"
                } else {
                    "mismatched"
                };
                if expected_frontend != "accepted" {
                    failures.push(format!("{label}: expected {expected_frontend}, reached L5"));
                }
                let status = if outcome.starts_with("L5 open gap:") {
                    "open gap"
                } else if outcome.starts_with("L5 fragment only:") {
                    // Only produced after the machine-wrapped control met its witness.
                    "contextual product inspected"
                } else if outcome.starts_with("L5 context unverified:") {
                    "missing context"
                } else if outcome.starts_with("L5 lowered ") {
                    "executable product inspected"
                } else if outcome.starts_with("L5 produced uninspected ") {
                    failures.push(format!("{label}: executable product has no reviewed semantic witness"));
                    "executable product uninspected"
                } else {
                    "declaration or empty product"
                };
                report_case(&label, "L5", status, &outcome);
                // `inspect_example` returned, so every reviewed witness for this outcome ran and held.
                let semantic = match status {
                    "executable product inspected" | "contextual product inspected" => "witness",
                    "declaration or empty product" => "declarations",
                    "executable product uninspected" => "missing",
                    _ => "none",
                };
                receipts.push(Receipt::new(&label, status, frontend, semantic));
                lowered.insert(label, outcome);
            }
            Err(payload) => {
                inspection_failures += 1;
                let message = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("unexpected panic");
                failures.push(format!("{label}: {message}"));
                report_case(&label, "unattributed", "inspection failed", message);
                receipts.push(Receipt::new(&label, "inspection failed", "unchecked", "panicked"));
            }
        }
    }
    let accepted = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 lowered "))
        .count();
    let schema_only = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 schema only "))
        .count();
    let domain_only = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 domain only "))
        .count();
    let alias_only = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 alias only "))
        .count();
    let empty = lowered
        .values()
        .filter(|outcome| *outcome == "L5 empty product")
        .count();
    let interfaces = lowered
        .values()
        .filter(|s| {
            s.starts_with("L5 trait declarations ")
                || s.starts_with("L5 mathematical declarations ")
                || s.starts_with("L5 data templates ")
                || s.starts_with("L5 machine templates ")
        })
        .count();
    let fragments = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 fragment only:"))
        .count();
    let unverified_context = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 context unverified:"))
        .count();
    let uninspected = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 produced uninspected "))
        .count();
    let gaps = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 open gap:"))
        .count();
    eprintln!(
        "{} fragments. {frontend_refusals} front-end refusals, {fragments} package-context refusals with inspected contextual products, {unverified_context} with unverified context, {gaps} open lowering gaps, {empty} empty products, {alias_only} alias-only products, {interfaces} declaration interfaces, {domain_only} domain-only products, {schema_only} schema-only products, {accepted} inspected executable products and {uninspected} uninspected executable products.",
        example_count,
    );
    let accounted = inspection_failures
        + frontend_refusals
        + fragments
        + unverified_context
        + uninspected
        + gaps
        + empty
        + alias_only
        + interfaces
        + domain_only
        + schema_only
        + accepted;
    if example_count != accounted {
        failures.push(format!("classified {accounted} of {example_count} spec examples"));
    }
    let actual: BTreeMap<_, _> = lowered
        .iter()
        .map(|(label, outcome)| (label.as_str(), outcome.as_str()))
        .collect();
    let snapshot = compare_lowering_outcomes(&expected_lowering, &actual, &mut failures);
    // Every frontend, snapshot and semantic check has now run for every example.
    // An example's receipt states only its own checks, so a failure elsewhere in
    // the corpus does not hide an example that completed them all.
    for receipt in &mut receipts {
        receipt.lowering = snapshot.get(receipt.label.as_str()).copied().unwrap_or("none");
        receipt.report();
    }
    assert!(failures.is_empty(), "unexpected spec example outcomes:\n{}", failures.join("\n"));
}

fn compare_lowering_outcomes<'a>(
    expected_lowering: &BTreeMap<&'a str, &str>,
    actual: &BTreeMap<&'a str, &str>,
    failures: &mut Vec<String>,
) -> BTreeMap<&'a str, &'static str> {
    // The result of each snapshot comparison, by example label.
    let mut snapshot: BTreeMap<&str, &'static str> = BTreeMap::new();
    for (label, recorded) in expected_lowering {
        let check = match actual.get(label) {
            Some(observed) if observed == recorded => "matched",
            Some(_) => "mismatched",
            None => "extra",
        };
        if check != "matched" {
            failures.push(format!("{label}: expected {recorded}, observed {:?}", actual.get(label)));
        }
        snapshot.insert(*label, check);
    }
    for (label, outcome) in actual {
        if !expected_lowering.contains_key(label) {
            failures.push(format!("{label}: unrecorded outcome {outcome}"));
            snapshot.insert(*label, "absent");
        }
    }
    snapshot
}

#[test]
fn a_recorded_gap_requires_the_same_diagnostic_at_the_same_example() {
    let expected = outcomes("a.md:10  L5 open gap: unsupported carrier | reader");
    let original = expected["a.md:10"];
    let mut failures = Vec::new();
    let snapshot = compare_lowering_outcomes(&expected, &expected, &mut failures);
    assert_eq!(snapshot["a.md:10"], "matched");
    assert!(failures.is_empty());

    let changed = BTreeMap::from([("a.md:10", "L5 open gap: different reason | reader")]);
    let snapshot = compare_lowering_outcomes(&expected, &changed, &mut failures);
    assert_eq!(snapshot["a.md:10"], "mismatched");
    assert_eq!(failures.len(), 1);

    failures.clear();
    let moved = BTreeMap::from([("b.md:20", original)]);
    let snapshot = compare_lowering_outcomes(&expected, &moved, &mut failures);
    assert_eq!(snapshot["a.md:10"], "extra");
    assert_eq!(snapshot["b.md:20"], "absent");
    assert_eq!(failures.len(), 2);
}

/// The final status of one example after all of its checks ran.
struct Receipt {
    label: String,
    /// The status of its `OMEGA_CASE` row.
    status: &'static str,
    /// `matched`, `mismatched` or `absent` against the L2/L3 records; `unchecked` after a panic.
    frontend: &'static str,
    /// `matched`, `mismatched` or `absent` for an L5 outcome; `extra` for a lowering record
    /// of an example without one; `none` when neither exists.
    lowering: &'static str,
    /// `witness`, `declarations`, `missing` (an unwitnessed executable product), `none` or `panicked`.
    semantic: &'static str,
}

impl Receipt {
    fn new(
        label: &str,
        status: &'static str,
        frontend: &'static str,
        semantic: &'static str,
    ) -> Self {
        Self { label: label.to_owned(), status, frontend, lowering: "none", semantic }
    }

    /// Mirrors `receipt_verdict` in tools/spec-corpus/lowering_evidence.py. Only `passed` is a success.
    fn verdict(&self) -> &'static str {
        if self.semantic == "panicked" {
            "panicked"
        } else if self.frontend == "mismatched" || self.lowering == "mismatched" {
            "mismatched expectation"
        } else if self.frontend == "absent" || self.lowering == "absent" {
            "absent expectation"
        } else if self.lowering == "extra" {
            "extra expectation"
        } else if self.semantic == "missing" {
            "failed"
        } else if matches!(self.status, "open gap" | "missing context") {
            "known gap"
        } else if self.status == "recorded refusal" {
            "recorded refusal"
        } else {
            "passed"
        }
    }

    fn report(&self) {
        if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            println!(
                "OMEGA_L5_SPEC_RECEIPT\t{}\t{}\t{}\tfrontend={},lowering={},semantic={}",
                self.label,
                self.status,
                self.verdict(),
                self.frontend,
                self.lowering,
                self.semantic
            );
        }
    }
}

enum FragmentOutcome {
    Refused { stage: &'static str, diagnostic: String, outcome: String },
    Lowering(String),
}

fn report_case(
    label: &str,
    stage: &str,
    status: &str,
    detail: &str,
) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_CASE\tspec\t{label}\t{stage}\t{status}\t{}", spec_corpus::diagnostic_field(detail));
    }
}

fn source_diagnostic(text: &str, package: &str, file: &str, span: omega_lexer::SourceSpan, message: &str) -> String {
    if package != "example" || file != "example.omg" {
        return format!("{package}/{file}: {message} (bytes {}..{})", span.get_start_offset(), span.get_end_offset());
    }
    let pos = omega_lexer::LineIndex::new(text.as_bytes()).get_position(span.get_start_offset());
    format!("{file}:{}:{}: {message} (bytes {}..{})", pos.get_line(), pos.get_column(), span.get_start_offset(), span.get_end_offset())
}

fn report_lowering_location(label: &str, input: &str, text: &str, span: omega_lexer::SourceSpan) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        let pos = omega_lexer::LineIndex::new(text.as_bytes()).get_position(span.get_start_offset());
        println!("OMEGA_SPEC_LOCATION\t{label}\t{input}\t{}\t{}\t{}\t{}",
                 pos.get_line(), pos.get_column(), span.get_start_offset(), span.get_end_offset());
    }
}

fn inspect_example(example: &spec_corpus::SpecExample) -> FragmentOutcome {
    let label = example.get_label();
    let text = parser_source(&example.source_text);
    // `stage` is the reported token (`L3-resumed` after L4 answers); the recorded
    // outcome names the owning layer, so resumed typing still matches L3.
    let refusal = |stage, package: &str, file: &str, span: omega_lexer::SourceSpan, message: &str| {
        let owner = preparation::owner(stage);
        let mut outcome = format!("{owner} {message}");
        if owner == "L3" && package == "example" && file == "example.omg" {
            let occurrence = span.slice(text.as_bytes()).and_then(|b| std::str::from_utf8(b).ok());
            outcome.push_str(" | ");
            outcome.push_str(occurrence.and_then(|s| s.lines().next()).unwrap_or(""));
        }
        FragmentOutcome::Refused { stage, diagnostic: source_diagnostic(&text, package, file, span, message), outcome }
    };
    let tokens = match omega_lexer::lex(text.as_bytes()) {
        Ok(tokens) => tokens,
        Err(e) => return refusal("L0", "example", "example.omg", e.get_span(), e.get_kind().get_message()),
    };
    let tree = match parse_fragment(text.as_bytes(), &tokens, fragment_kind(&label)) {
        Ok(tree) => tree,
        Err(e) => return refusal("L1", "example", "example.omg", e.get_span(), &e.get_kind().get_message(e.get_expected())),
    };
    let files = [StageSource {
        package: "example",
        source: ParsedSource {
            logical_path: "example.omg",
            text: &text,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let package = match resolve_source_package(&files) {
        Err(e) => return refusal("L2", &e.package, &e.file, e.span, &e.message),
        Ok(package) => package,
    };
    let prepared = match preparation::prepare(&package) {
        Err((stage, e)) => return refusal(stage, &e.package, &e.file, e.span, &e.message),
        Ok(prepared) => prepared,
    };
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        for (kind, count) in obligation_report::counts(&prepared) {
            println!("OMEGA_OBLIGATION\t{label}\t{kind}\t{count}");
        }
    }
    let outcome = match lower_package(&prepared) {
        Err(error) => {
            assert_eq!(error.site.file, "example.omg", "{label}");
            report_lowering_location(&label, "fragment", &text, error.site.span);
            let occurrence = std::str::from_utf8(error.site.span.slice(text.as_bytes()).unwrap()).unwrap();
            let occurrence = occurrence.lines().next().unwrap_or("");
            // A listed fence is excused only by a positive control: its statements,
            // enclosed in a machine after any preceding declarations, must lower to
            // a product that carries the fence's values.
            let listed = label == "language/data_and_literals.md:90" || FRAGMENTS_WITHOUT_PACKAGE_CONTEXT.contains(&label.as_str());
            match contextual_witness(&label) {
                Some(witness) if listed && error.message == "unsupported Psi declaration: Let" => {
                    let start = error.site.span.get_start_offset();
                    let prefix = "machine build() { ";
                    let wrapped = format!("{}{prefix}{} }}", &text[..start], &text[start..]);
                    match lower_wrapped(&wrapped) {
                        Ok(lowered) => match witness(&lowered) {
                            Ok(()) => format!("L5 fragment only: {} | {occurrence}", error.message),
                            Err(reason) => format!("L5 open gap: machine-wrapped fragment lowered without its witness: {reason} | {occurrence}"),
                        },
                        Err(WrappedStop::Lowering { message, occurrence, span }) => {
                            let lo = span.get_start_offset();
                            let hi = span.get_end_offset();
                            if lo >= start + prefix.len() && hi <= text.len() + prefix.len() {
                                report_lowering_location(&label, "context", &text, omega_lexer::SourceSpan::new(lo - prefix.len(), hi - prefix.len()));
                            } else if hi <= start {
                                report_lowering_location(&label, "context", &text, span);
                            } else {
                                report_lowering_location(&label, "generated", &wrapped, span);
                            }
                            format!("L5 open gap: machine-wrapped fragment refused: {message} | {occurrence}")
                        }
                        Err(WrappedStop::Frontend { stage, message }) => {
                            format!("L5 context unverified: machine-wrapped fragment stopped at {stage}: {message} | {occurrence}")
                        }
                    }
                }
                _ => format!("L5 open gap: {} | {occurrence}", error.message),
            }
        }
        Ok(package) if package.machines.is_empty() && !package.machine_templates.is_empty() => {
            assert!(package.machines.is_empty(), "{label}");
            check_machine_templates(&package, &label);
            format!(
                "L5 machine templates {}",
                package
                    .machine_templates
                    .iter()
                    .map(|m| m.declaration.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Ok(package) if package.machines.is_empty() && !package.traits.is_empty() => {
            assert!(
                package.data.is_empty()
                    && package.domains.is_empty()
                    && package.aliases.is_empty()
                    && package.domain_aliases.is_empty()
                    && package.mathematics.is_empty(),
                "{label}"
            );
            if label == "language/generics.md:510" {
                let scan = &package.traits[0].requirements[0];
                let widths = [16u32, 32, 64].map(|n| vec![ScalarLiteral::Integer(n.into())]);
                assert_eq!(scan.family.as_deref(), Some(widths.as_slice()));
                assert_eq!(package.types.get(scan.parameter_types[0].carrier()), Some(&TypeShape::SharedByteView));
                assert_eq!(
                    package.types.get(scan.value_indices[0]),
                    Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 32 }))
                );
            }
            format!(
                "L5 trait declarations {}",
                package
                    .traits
                    .iter()
                    .map(|t| t.declaration.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Ok(package) if package.machines.is_empty() && !package.mathematics.is_empty() => {
            assert!(
                package.data.is_empty() && package.domains.is_empty() && package.aliases.is_empty() && package.domain_aliases.is_empty(),
                "{label}"
            );
            format!(
                "L5 mathematical declarations {}",
                package
                    .mathematics
                    .iter()
                    .map(|m| m.declaration.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Ok(package) if package.machines.is_empty() && !package.data_templates.is_empty() => {
            let templates = package
                .data_templates
                .iter()
                .map(|t| t.declaration.path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let mut summary = format!("L5 data templates {templates}");
            if !package.data.is_empty() {
                summary.push_str(&format!(" | schemas {}", schema_names(&package)));
            }
            with_declaration_names(summary, &package)
        }
        Ok(package) if package.machines.is_empty() && package.data.is_empty() && !package.domains.is_empty() => {
            check_domain_declaration(&package, &label);
            with_alias_names(format!("L5 domain only {}", domain_names(&package)), &package)
        }
        Ok(package) if package.machines.is_empty() && package.data.is_empty() => {
            if package.aliases.is_empty() && package.domain_aliases.is_empty() {
                "L5 empty product".into()
            } else {
                format!("L5 alias only {}", alias_names(&package))
            }
        }
        Ok(package) if package.machines.is_empty() => with_declaration_names(format!("L5 schema only {}", schema_names(&package)), &package),
        Ok(package) => {
            for machine in &package.machines {
                control_flow::check_operation_ownership(&machine.body).unwrap_or_else(|failure| panic!("{label}: {failure}"));
            }
            // Reachability and names do not witness behaviour. Only a reviewed
            // semantic witness earns `L5 lowered`.
            let witness: Option<fn(&LoweredPackage)> = match label.as_str() {
                "build/provider_selection.md:305" => Some(check_nanosecond_units),
                "language/domains.md:572" => Some(check_previous_distance),
                "proofs/contracts.md:826" => Some(check_required_discharge_inputs),
                "language/effects.md:641" => Some(check_closed_callback),
                "language/machines.md:33" => Some(check_explicit_mutable_argument),
                "language/patterns.md:50" => Some(check_held_sum_dispatch),
                _ => None,
            };
            if let Some(witness) = witness {
                witness(&package);
            }
            let inspected = witness.is_some();
            let prefix = if inspected { "L5 lowered" } else { "L5 produced uninspected" };
            let machines = package
                .machines
                .iter()
                .map(|machine| machine.declaration.path.as_str())
                .collect::<Vec<_>>();
            if package.data.is_empty() {
                with_declaration_names(format!("{prefix} {}", machines.join(", ")), &package)
            } else {
                with_declaration_names(format!("{prefix} {} | schemas {}", machines.join(", "), schema_names(&package)), &package)
            }
        }
    };
    FragmentOutcome::Lowering(outcome)
}

fn with_declaration_names(
    outcome: String,
    package: &LoweredPackage,
) -> String {
    let outcome = with_alias_names(outcome, package);
    let outcome = if package.machines.is_empty() || package.machine_templates.is_empty() {
        outcome
    } else {
        format!(
            "{outcome} | machine templates {}",
            package
                .machine_templates
                .iter()
                .map(|m| m.declaration.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    if package.domains.is_empty() {
        outcome
    } else {
        format!("{outcome} | domains {}", domain_names(package))
    }
}

fn with_alias_names(
    outcome: String,
    package: &LoweredPackage,
) -> String {
    if package.aliases.is_empty() && package.domain_aliases.is_empty() {
        outcome
    } else {
        format!("{outcome} | aliases {}", alias_names(package))
    }
}

fn alias_names(package: &LoweredPackage) -> String {
    package
        .aliases
        .iter()
        .map(|alias| alias.declaration.path.as_str())
        .chain(
            package
                .domain_aliases
                .iter()
                .map(|alias| alias.declaration.path.as_str()),
        )
        .collect::<Vec<_>>()
        .join(", ")
}

fn domain_names(package: &LoweredPackage) -> String {
    package
        .domains
        .iter()
        .map(|domain| domain.declaration.path.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn check_domain_declaration(
    package: &LoweredPackage,
    label: &str,
) {
    use PredicateOperator::{And, GreaterOrEqual, GreaterThan, LessOrEqual, LessThan};
    use PredicateStep::{Apply, Integer, SelfValue};

    let (signed, width, expected) = match label {
        "language/domains.md:116" => (
            false,
            64,
            [
                SelfValue,
                Integer(1.into()),
                Apply(GreaterOrEqual),
                SelfValue,
                Integer(8.into()),
                Apply(LessOrEqual),
                Apply(And),
            ],
        ),
        "language/domains.md:495" => (
            true,
            32,
            [
                SelfValue,
                Integer(0.into()),
                Apply(GreaterThan),
                SelfValue,
                Integer(0.into()),
                Apply(LessThan),
                Apply(And),
            ],
        ),
        _ => return,
    };
    let [domain] = package.domains.as_slice() else {
        panic!("{label}: expected exactly one domain declaration");
    };
    assert_eq!(
        package.types.get(domain.carrier),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width })),
        "{label}",
    );
    let [requirement] = domain.requires.as_slice() else {
        panic!("{label}: expected exactly one domain predicate");
    };
    assert_eq!(requirement.predicate, expected, "{label}");
}

fn schema_names(package: &LoweredPackage) -> String {
    package
        .data
        .iter()
        .map(|data| data.declaration.path.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn check_nanosecond_units(package: &LoweredPackage) {
    let [machine] = package.machines.as_slice() else {
        panic!("expected exactly one machine");
    };
    assert_eq!(
        package.types.get(machine.result_type.unwrap()),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 })),
    );
    let [operation] = machine.body.operations.as_slice() else {
        panic!("expected exactly one operation");
    };
    assert_eq!(operation.kind, OperationKind::Constant(ScalarLiteral::Integer(1_000_000_000u64.into())));
    let OperationResult::Scalar(value) = operation.result else {
        panic!("expected a scalar constant result");
    };
    let [block] = machine.body.blocks.as_slice() else {
        panic!("expected exactly one block");
    };
    assert!(matches!(
        block.terminator,
        BlockTerminator::Return { result: ReturnEdge::Scalar(returned), .. } if returned == value
    ));
}

fn check_previous_distance(package: &LoweredPackage) {
    use omega_lowering::{ArithmeticPolicy, IntegerObligation, IntegerPrimitive, Parameter, QualificationDirection, ScalarQualificationChange};
    let machine = &package.machines[0];
    let [entry] = machine.contract.requires.as_slice() else {
        panic!("expected the distance invariant")
    };
    assert_eq!(
        entry.predicate,
        [
            PredicateStep::Formal(0),
            PredicateStep::Integer(0.into()),
            PredicateStep::Apply(PredicateOperator::GreaterThan)
        ]
    );
    let body = &machine.body;
    let [Parameter::Scalar(distance)] = body.parameters[..] else {
        panic!("expected the distance input")
    };
    let [initial, plain, finish] = body.blocks.as_slice() else {
        panic!("expected both cast edges")
    };
    let [one, subtract] = body.operations.as_slice() else {
        panic!("expected a literal and subtraction")
    };
    assert_eq!(one.kind, OperationKind::Constant(ScalarLiteral::Integer(1.into())));
    let (OperationResult::Scalar(unit), OperationResult::Scalar(difference)) = (one.result, subtract.result) else {
        panic!("expected scalar operands")
    };
    let OperationKind::Integer { operation, operands } = &subtract.kind else {
        panic!("expected integer subtraction")
    };
    assert_eq!((operation.primitive, operation.policy), (IntegerPrimitive::Subtract, ArithmeticPolicy::Exact));
    assert_eq!(operation.obligations, [IntegerObligation::ResultFitsCarrier]);
    assert_eq!(operands, &[plain.scalar_parameters[0], unit]);
    for (from, to, arg, direction) in [
        (initial, plain, distance, QualificationDirection::Erase),
        (plain, finish, difference, QualificationDirection::Add),
    ] {
        let BlockTerminator::Jump(edge) = &from.terminator else {
            panic!("expected a cast edge")
        };
        assert_eq!(edge.target, to.id);
        assert_eq!(edge.arguments, [arg]);
        assert_eq!(edge.scalar_qualification_changes, [ScalarQualificationChange { argument_index: 0, direction }]);
    }
    assert!(matches!(finish.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if value == finish.scalar_parameters[0]));
    let memberships = |value: omega_lowering::ValueId| match package.types.get(body.values[value.index()].ty).unwrap() {
        TypeShape::QualifiedScalar { domains, .. } => package
            .types
            .domain_set(*domains)
            .unwrap()
            .iter()
            .map(|d| d.path.as_str())
            .collect::<Vec<_>>(),
        TypeShape::PrimitiveScalar(_) => vec![],
        _ => panic!("expected a scalar type"),
    };
    assert_eq!(memberships(distance), ["i32::Km", "i32::Positive"]);
    assert!(memberships(plain.scalar_parameters[0]).is_empty());
    assert_eq!(memberships(finish.scalar_parameters[0]), ["i32::Km"]);
}

fn check_required_discharge_inputs(package: &LoweredPackage) {
    use PredicateStep::{Apply, ByteSequenceLength, Formal, Integer};
    use omega_lowering::{IntegerObligation, IntegerPrimitive, Parameter};
    let [area, get] = package.machines.as_slice() else {
        panic!("expected area and get")
    };
    let [small_count, small_size] = area.contract.requires.as_slice() else {
        panic!("expected both product bounds")
    };
    assert_eq!(
        small_count.predicate,
        [
            Formal(0),
            Integer(1000.into()),
            Apply(PredicateOperator::LessOrEqual)
        ]
    );
    assert_eq!(
        small_size.predicate,
        [
            Formal(1),
            Integer(1000.into()),
            Apply(PredicateOperator::LessOrEqual)
        ]
    );
    let [product] = area.body.operations.as_slice() else {
        panic!("expected one multiplication")
    };
    let [Parameter::Scalar(count), Parameter::Scalar(size)] = area.body.parameters[..] else {
        panic!("expected count and size")
    };
    assert!(
        matches!(&product.kind, OperationKind::Integer{operation,operands} if operation.primitive == IntegerPrimitive::Multiply && operation.obligations == [IntegerObligation::ResultFitsCarrier] && operands == &[count,size])
    );
    let [index_bound, length_bound] = get.contract.requires.as_slice() else {
        panic!("expected two ordered premises")
    };
    assert_eq!(index_bound.predicate, [Formal(1), Formal(2), Apply(PredicateOperator::LessThan)]);
    assert_eq!(
        length_bound.predicate,
        [
            Formal(2),
            Formal(0),
            ByteSequenceLength,
            Apply(PredicateOperator::LessOrEqual)
        ]
    );
    let [Parameter::Structural(items), Parameter::Scalar(i), _] = get.body.parameters[..] else {
        panic!("expected view and index")
    };
    assert_eq!(package.types.get(get.body.places[items.index()].ty), Some(&TypeShape::SharedByteView));
    let [measure, read] = get.body.operations.as_slice() else {
        panic!("expected a length observation followed by a read")
    };
    assert_eq!(measure.kind, OperationKind::ByteSequenceLength { source: items });
    let OperationKind::ByteSequenceRead {
        source,
        index,
        length,
        obligation,
    } = read.kind
    else {
        panic!("expected byte read")
    };
    assert_eq!((source, index), (items, i));
    assert_eq!(measure.result, OperationResult::Scalar(length));
    assert_eq!(get.body.values[length.index()].ty, get.parameter_types[1]);
    assert_eq!(get.body.diagnostics.bounds.len(), 1);
    assert_eq!(obligation.get(), 1);
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(byte),
        ..
    } = get.body.blocks[0].terminator
    else {
        panic!("expected returned byte")
    };
    assert_eq!(read.result, OperationResult::Scalar(byte));
}

fn check_closed_callback(package: &LoweredPackage) {
    let [template] = package.machine_templates.as_slice() else {
        panic!("expected public template alongside its instance")
    };
    assert_eq!(template.declaration.path, "apply");
    assert_eq!(template.body[0].target, omega_lowering::TemplateCallTarget::StaticBinder(0));
    assert!(template.body[0].acknowledgements.suspend);
    assert_eq!(template.callback_pause, [(0, omega_lowering::PauseEffects { suspend: true, block: false })]);
    assert!(!template.fixed_pause.suspend);
    use omega_lowering::{CallArgument, Parameter, PauseEffects, StaticArgument};
    let [example, local, apply] = package.machines.as_slice() else {
        panic!("expected a caller, callback, and specialized wrapper")
    };
    assert_eq!(apply.contract.pause_bounds.suspend, Some(true));
    assert_eq!(apply.contract.realized_pause, PauseEffects::default());
    let [StaticArgument::Callable(selected)] = apply.arguments.as_slice() else {
        panic!("missing static callable argument")
    };
    assert_eq!(selected.machine, local.id);
    assert!(selected.pause.suspend && !selected.pause.block);
    assert_eq!(selected.parameters, local.parameter_types);
    assert_eq!(selected.result, local.result_type);
    let [Parameter::Scalar(input)] = apply.body.parameters[..] else {
        panic!("expected value parameter")
    };
    let [inside] = apply.body.operations.as_slice() else {
        panic!("expected a direct invocation of local")
    };
    assert!(
        matches!(&inside.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==local.id && arguments==&[CallArgument::Scalar(input)])
    );
    let [literal, invoke] = example.body.operations.as_slice() else {
        panic!("expected a literal and wrapper invocation")
    };
    assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(42.into())));
    let OperationResult::Scalar(value) = literal.result else {
        panic!("expected the scalar 42")
    };
    assert!(
        matches!(&invoke.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==apply.id && arguments==&[CallArgument::Scalar(value)])
    );
}

fn check_machine_templates(
    package: &LoweredPackage,
    label: &str,
) {
    use omega_lowering::{MachineTemplateBinder, TemplateCallTarget, TypeExpressionStep};
    let first = &package.machine_templates[0];
    let callback = |m: &omega_lowering::MachineTemplate| {
        m.parameters
            .iter()
            .position(|b| matches!(b, MachineTemplateBinder::Callable { .. }))
            .unwrap()
    };
    let MachineTemplateBinder::Callable { signature: bound, requirement } = &first.parameters[callback(first)] else {
        unreachable!()
    };
    assert_eq!(first.body.len(), 1);
    assert_eq!(first.body[0].target, TemplateCallTarget::StaticBinder(callback(first)));
    assert_eq!(first.invokes, [callback(first)]);
    assert_eq!(first.result.is_some(), first.signature.result.is_some());
    match label {
        "language/effects.md:576" => {
            assert_eq!(first.body[0].operands, [0]);
            assert_eq!(first.result, Some(0));
            assert!(requirement.is_none());
            assert_eq!(
                bound.ensures[0].predicate,
                [
                    PredicateStep::Result,
                    PredicateStep::Formal(0),
                    PredicateStep::Apply(PredicateOperator::GreaterOrEqual)
                ]
            );
            assert_eq!(
                first.signature.ensures[0].predicate,
                [
                    PredicateStep::Formal(0),
                    PredicateStep::Result,
                    PredicateStep::CallbackGuarantee { binder: 0, operand_count: 2 }
                ]
            );
        }
        "language/state_contracts.md:74" => {
            assert!(first.body[0].operands.is_empty());
            assert!(bound.terminates);
            assert!(!first.signature.terminates);
            assert_eq!(first.signature.crash_ceiling, Some(vec![omega_typed::CrashCause::Trap]));
            assert_eq!(bound.crash_ceiling, None);
        }
        "language/generics.md:767" => {
            let member = &package.traits[0].dependent_requirements[0];
            assert_eq!(package.traits[0].parameters.len(), 1);
            assert_eq!(member.declaration.path, "ElementStep::apply");
            assert!(member.signature.terminates);
            let wrapper = &package.machine_templates[1];
            assert_eq!(wrapper.body[0].target, TemplateCallTarget::Declaration(first.declaration.clone()));
            assert_eq!(wrapper.body[0].operands, [0]);
            for (index, arg) in wrapper.body[0].arguments.iter().enumerate() {
                assert_eq!(
                    arg.0,
                    [TypeExpressionStep::GenericParameter {
                        owner: wrapper.declaration.clone(),
                        index
                    }]
                );
            }
            for m in [first, wrapper] {
                let MachineTemplateBinder::Callable {
                    signature,
                    requirement: Some(req),
                } = &m.parameters[1]
                else {
                    panic!("missing normalized nominal callback")
                };
                assert!(signature.terminates && m.signature.terminates);
                assert_eq!(req.declaration, member.declaration);
                assert_eq!(
                    req.arguments[0].0,
                    [TypeExpressionStep::GenericParameter {
                        owner: m.declaration.clone(),
                        index: 0
                    }]
                );
                assert_eq!(m.signature.parameters, signature.parameters);
                assert_eq!(
                    signature.parameters[0].0,
                    [
                        TypeExpressionStep::GenericParameter {
                            owner: m.declaration.clone(),
                            index: 0
                        },
                        TypeExpressionStep::Reference(omega_lowering::TemplateAccess::Mut)
                    ]
                );
            }
        }
        other => panic!("unreviewed template package {other}"),
    }
}

/// Where lowering of a machine-wrapped fragment stopped.
enum WrappedStop {
    /// The synthesized context did not reach L5; the fence's context is unverified.
    Frontend { stage: &'static str, message: String },
    /// L5 refused complete statements inside a machine: an open lowering gap.
    Lowering { message: String, occurrence: String, span: omega_lexer::SourceSpan },
}

fn lower_wrapped(wrapped: &str) -> Result<LoweredPackage, WrappedStop> {
    let frontend = |stage: &'static str, message: String| WrappedStop::Frontend { stage, message };
    let tokens = omega_lexer::lex(wrapped.as_bytes()).map_err(|e| frontend("L0", e.get_kind().get_message().to_owned()))?;
    let syntax = omega_parser::parse_file(wrapped.as_bytes(), &tokens).map_err(|e| frontend("L1", e.get_kind().get_message(e.get_expected())))?;
    let files = [StageSource {
        package: "example",
        source: ParsedSource {
            logical_path: "example.omg",
            text: wrapped,
            tokens: &tokens,
            tree: &syntax,
        },
    }];
    let package = resolve_source_package(&files).map_err(|e| frontend("L2", e.message))?;
    // The recorded contextual outcome names the owning layer.
    let prepared = preparation::prepare(&package).map_err(|(stage, e)| frontend(preparation::owner(stage), e.message))?;
    lower_package(&prepared).map_err(|error| WrappedStop::Lowering {
        span: error.site.span,
        occurrence: error
            .site
            .span
            .slice(wrapped.as_bytes())
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|occurrence| occurrence.lines().next())
            .unwrap_or("")
            .to_owned(),
        message: error.message,
    })
}

/// The reviewed product of each statement fence once enclosed in a machine.
fn contextual_witness(label: &str) -> Option<fn(&LoweredPackage) -> Result<(), String>> {
    match label {
        "language/data_and_literals.md:90" => Some(check_nested_case_local),
        "language/data_and_literals.md:435" => Some(check_radix_locals),
        "layouts/recasts.md:18" => Some(check_float_view_recast),
        _ => None,
    }
}

fn only_machine(package: &LoweredPackage) -> Result<&omega_lowering::LoweredMachine, String> {
    match package.machines.as_slice() {
        [machine] if machine.declaration.path == "build" => {
            control_flow::check_operation_ownership(&machine.body).map_err(str::to_owned)?;
            Ok(machine)
        }
        machines => Err(format!("expected only the wrapping machine, found {} machines", machines.len())),
    }
}

fn scalar_type<'a>(
    package: &'a LoweredPackage,
    body: &omega_lowering::MachineBody,
    result: OperationResult,
) -> Option<&'a TypeShape> {
    match result {
        OperationResult::Scalar(value) => package.types.get(body.values[value.index()].ty),
        _ => None,
    }
}

/// `let body: Envelope<u32>::Body = Envelope::<u32>::Body::Value { value: 7 };`
fn check_nested_case_local(package: &LoweredPackage) -> Result<(), String> {
    let body = &only_machine(package)?.body;
    let [input, construct] = body.operations.as_slice() else {
        return Err(format!("expected the payload and its case, found {} operations", body.operations.len()));
    };
    if input.kind != OperationKind::Constant(ScalarLiteral::Integer(7.into())) {
        return Err(format!("expected payload 7, found {:?}", input.kind));
    }
    if scalar_type(package, body, input.result) != Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 32 })) {
        return Err("payload is not the applied u32".into());
    }
    let OperationKind::EstablishCase { case, fields } = &construct.kind else {
        return Err(format!("expected case construction, found {:?}", construct.kind));
    };
    let [field] = fields.as_slice() else {
        return Err("expected one payload field".into());
    };
    if case.path != "Envelope::Body::Value"
        || field.field.path != "Envelope::Body::Value::value"
        || !matches!(field.operand, omega_lowering::ValueOperand::Scalar(value) if OperationResult::Scalar(value) == input.result)
    {
        return Err(format!("case {} does not install the payload in its value field", case.path));
    }
    let OperationResult::Structural(place) = construct.result else {
        return Err("expected an owned case".into());
    };
    let carrier = body.places[place.index()].ty;
    if !package
        .data
        .iter()
        .any(|schema| schema.declaration.path == "Envelope::Body" && schema.carrier == carrier && schema.type_arguments.len() == 1)
    {
        return Err("case carrier is not the applied nested Envelope<u32>::Body".into());
    }
    Ok(())
}

/// `let mask: u16 = 0xFFFFu16; let byte_divisor: u32 = 0x1_0000u32;`
fn check_radix_locals(package: &LoweredPackage) -> Result<(), String> {
    let body = &only_machine(package)?.body;
    let [mask, divisor] = body.operations.as_slice() else {
        return Err(format!("expected two constants, found {} operations", body.operations.len()));
    };
    for (operation, value, width) in [(mask, 0xFFFFu32, 16), (divisor, 0x1_0000u32, 32)] {
        if operation.kind != OperationKind::Constant(ScalarLiteral::Integer(value.into())) {
            return Err(format!("expected {value}, found {:?}", operation.kind));
        }
        if scalar_type(package, body, operation.result) != Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width })) {
            return Err(format!("{value} is not a u{width}"));
        }
    }
    Ok(())
}

/// `let view: @f32 = @bits as @f32; let value: f32 = $view;`
/// Psi has no float scalar and its only recast is between equal-width
/// integers, so no lowered product can yet carry this fence.
fn check_float_view_recast(package: &LoweredPackage) -> Result<(), String> {
    only_machine(package)?;
    Err("Psi has no reviewed witness for a u32-to-f32 shared view".into())
}

/// `clear(@mut counter)` writes zero into the caller's own `Counter::value`.
fn check_explicit_mutable_argument(package: &LoweredPackage) {
    use omega_lowering::{CallArgument, CallTarget, CleanupAction, Parameter, PlaceProjection, ReferenceAccess};
    let machine = |path: &str| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == path)
            .unwrap_or_else(|| panic!("missing machine {path}"))
    };
    let (clear, example) = (machine("clear"), machine("example"));
    assert_eq!(package.machines.len(), 2);
    assert!(matches!(
        package.types.get(clear.parameter_types[0]),
        Some(TypeShape::NominalReference { referent, access: ReferenceAccess::Mutable })
            if package.data[referent.get() - 1].declaration.path == "Counter"
    ));
    let [Parameter::Structural(counter)] = clear.body.parameters[..] else {
        panic!("expected the caller's referent")
    };
    let [zero, store] = clear.body.operations.as_slice() else {
        panic!("expected the zero and its store")
    };
    assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    assert_eq!(
        scalar_type(package, &clear.body, zero.result),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 }))
    );
    assert!(matches!(
        &store.kind,
        OperationKind::StructuralScalarFieldStore { destination, field, value }
            if destination.root == counter
                && destination.path == [PlaceProjection::Referent]
                && field.path == "Counter::value"
                && OperationResult::Scalar(*value) == zero.result
    ));
    let [establish, call] = example.body.operations.as_slice() else {
        panic!("expected the local and the call")
    };
    assert_eq!(establish.kind, OperationKind::EstablishZeroedStructural);
    let OperationResult::Structural(home) = establish.result else {
        panic!("expected the owned local")
    };
    assert_eq!(example.body.mutable_homes, [home]);
    assert!(matches!(
        &call.kind,
        OperationKind::Call { callee: CallTarget::Machine(callee), arguments, .. }
            if *callee == clear.id
                && matches!(
                    arguments.as_slice(),
                    [CallArgument::Borrow { place, access: ReferenceAccess::Mutable }] if place.root == home && place.path.is_empty()
                )
    ));
    assert!(matches!(
        &example.body.blocks[0].terminator,
        BlockTerminator::Return { result: ReturnEdge::Unit, cleanup, .. } if cleanup == &[CleanupAction::DiscardRoot(home)]
    ));
}

/// `inspect` borrows `command.op` itself, and only `GetUser` reaches `true`.
/// Exchanging the arms keeps every name and type but fails the edge binding.
fn check_held_sum_dispatch(package: &LoweredPackage) {
    use omega_lowering::{CallArgument, CallTarget, Parameter, PlaceProjection, ReferenceAccess};
    let machine = |path: &str| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == path)
            .unwrap_or_else(|| panic!("missing machine {path}"))
    };
    let (inspect, targets) = (machine("inspect"), machine("targets_one_user"));
    assert_eq!(package.machines.len(), 2);
    let [Parameter::Structural(command)] = inspect.body.parameters[..] else {
        panic!("expected the borrowed record")
    };
    let [call] = inspect.body.operations.as_slice() else {
        panic!("expected one call without a snapshot")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::Call { callee: CallTarget::Machine(callee), arguments, .. }
            if *callee == targets.id
                && matches!(
                    arguments.as_slice(),
                    [CallArgument::Borrow { place, access: ReferenceAccess::Shared }]
                        if place.root == command
                            && matches!(place.path.as_slice(), [PlaceProjection::Referent, PlaceProjection::Field(field)] if field.path == "Command::op")
                )
    ));
    let returns = inspect
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Return { result, .. } => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(returns.as_slice(), [ReturnEdge::Scalar(value)] if call.result == OperationResult::Scalar(*value)),
        "inspect must return the callee's verdict"
    );
    let [Parameter::Structural(op)] = targets.body.parameters[..] else {
        panic!("expected the borrowed sum")
    };
    assert!(matches!(
        package.types.get(targets.parameter_types[0]),
        Some(TypeShape::NominalReference { referent, access: ReferenceAccess::Shared })
            if package.data[referent.get() - 1].declaration.path == "Command::Operation"
    ));
    let [observation, yes, no] = targets.body.operations.as_slice() else {
        panic!("expected one case test and both arm results")
    };
    assert!(matches!(
        &observation.kind,
        OperationKind::StructuralCaseMembership { source, case }
            if source.root == op && source.path == [PlaceProjection::Referent] && case.path == "Command::Operation::GetUser"
    ));
    assert_eq!(yes.kind, OperationKind::Constant(ScalarLiteral::Boolean(true)));
    assert_eq!(no.kind, OperationKind::Constant(ScalarLiteral::Boolean(false)));
    let BlockTerminator::Conditional { condition, when_true, when_false } = &targets.body.blocks[0].terminator else {
        panic!("expected case dispatch")
    };
    assert_eq!(observation.result, OperationResult::Scalar(*condition));
    for (edge, arm) in [(when_true, yes), (when_false, no)] {
        assert!(matches!(
            targets.body.blocks[edge.target.get() - 1].terminator,
            BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if arm.result == OperationResult::Scalar(value)
        ));
    }
}
