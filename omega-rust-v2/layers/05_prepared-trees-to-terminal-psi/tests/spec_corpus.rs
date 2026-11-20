use std::collections::BTreeMap;

#[path = "support/control_flow.rs"]
mod control_flow;

use omega_lowering::{
    BlockTerminator, LoweredPackage, OperationKind, OperationResult, PredicateOperator, PredicateStep, ReturnEdge, ScalarLiteral, ScalarType, TypeShape,
    lower_package,
};
use omega_parser::parse_fragment;
use omega_preparation::prepare_source_package;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use spec_corpus::parser_examples::{fragment_kind, parser_source};

const RESOLUTION: &str = include_str!("../../02_syntax-trees-to-symbol-resolved-trees/fixtures/spec-outcomes.txt");
const TYPING: &str = include_str!("../../03_symbol-resolved-trees-to-typed-trees/fixtures/spec-outcomes.txt");
const LOWERING: &str = include_str!("../fixtures/spec-outcomes.txt");

/// These fences demonstrate a generic parameter or local statements. Lowering
/// receives packages, so the bare fragments are expected to refuse.
const FRAGMENTS_WITHOUT_PACKAGE_CONTEXT: &[&str] = &[
    "language/data_and_literals.md:435",
    "language/state_contracts.md:86",
    "layouts/recasts.md:18",
];

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
    assert_eq!(examples.len(), resolution.len());
    let mut lowered = BTreeMap::new();
    let mut resolution_refusals = 0;
    let mut preparation_refusals = 0;
    for example in examples {
        let label = example.get_label();
        let text = parser_source(&example.source_text);
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap_or_else(|error| panic!("{label}: {error:?}"));
        let tree = parse_fragment(text.as_bytes(), &tokens, fragment_kind(&label)).unwrap_or_else(|error| panic!("{label}: {error:?}"));
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
            Err(error) => {
                assert_eq!(error.message, resolution[label.as_str()], "{label}");
                resolution_refusals += 1;
                continue;
            }
            Ok(package) => {
                assert_eq!(resolution[label.as_str()], "accepted", "{label}");
                package
            }
        };
        let prepared = match prepare_source_package(&package) {
            Err(error) => {
                assert_eq!(error.file, "example.omg", "{label}");
                let occurrence = std::str::from_utf8(error.span.slice(text.as_bytes()).unwrap()).unwrap();
                let diagnostic = format!("L3 {} | {}", error.message, occurrence.lines().next().unwrap_or(""));
                assert_eq!(diagnostic, typing[label.as_str()], "{label}");
                preparation_refusals += 1;
                continue;
            }
            Ok(prepared) => {
                assert_eq!(typing[label.as_str()], "accepted", "{label}");
                prepared
            }
        };
        let outcome = match lower_package(&prepared) {
            Err(error) => {
                assert_eq!(error.site.file, "example.omg", "{label}");
                let occurrence = std::str::from_utf8(error.site.span.slice(text.as_bytes()).unwrap()).unwrap();
                let nested_data_local = label == "language/data_and_literals.md:90" && error.message == "unsupported Psi declaration: Let";
                if nested_data_local {
                    let start = error.site.span.get_start_offset();
                    let wrapped = format!("{}machine build() {{ {} }}", &text[..start], &text[start..]);
                    let tokens = omega_lexer::lex(wrapped.as_bytes()).unwrap();
                    let syntax = omega_parser::parse_file(wrapped.as_bytes(), &tokens).unwrap();
                    let files = [StageSource {
                        package: "example",
                        source: ParsedSource {
                            logical_path: "example.omg",
                            text: &wrapped,
                            tokens: &tokens,
                            tree: &syntax,
                        },
                    }];
                    let package = resolve_source_package(&files).unwrap();
                    let prepared = prepare_source_package(&package).unwrap();
                    let lowered = lower_package(&prepared).unwrap();
                    assert_eq!(lowered.machines.len(), 1);
                    assert!(
                        lowered
                            .data
                            .iter()
                            .any(|schema| schema.declaration.path == "Envelope::Body" && schema.type_arguments.len() == 1)
                    );
                    control_flow::check_operation_ownership(&lowered.machines[0].body).unwrap();
                }
                let classification = if nested_data_local || FRAGMENTS_WITHOUT_PACKAGE_CONTEXT.contains(&label.as_str()) {
                    "fragment only"
                } else {
                    "open gap"
                };
                format!("L5 {classification}: {} | {}", error.message, occurrence.lines().next().unwrap_or(""))
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
                if label == "language/generics.md:506" {
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
                if label == "build/provider_selection.md:305" {
                    check_nanosecond_units(&package);
                } else if label == "language/domains.md:572" {
                    check_previous_distance(&package);
                } else if label == "proofs/contracts.md:826" {
                    check_required_discharge_inputs(&package);
                } else if label == "language/effects.md:636" {
                    check_closed_callback(&package);
                }
                let machines = package
                    .machines
                    .iter()
                    .map(|machine| machine.declaration.path.as_str())
                    .collect::<Vec<_>>();
                if package.data.is_empty() {
                    with_declaration_names(format!("L5 lowered {}", machines.join(", ")), &package)
                } else {
                    with_declaration_names(format!("L5 lowered {} | schemas {}", machines.join(", "), schema_names(&package)), &package)
                }
            }
        };
        lowered.insert(label, outcome);
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
    let gaps = lowered
        .values()
        .filter(|outcome| outcome.starts_with("L5 open gap:"))
        .count();
    eprintln!(
        "{} fragments. {resolution_refusals} resolution refusals, {preparation_refusals} typing/preparation refusals, {fragments} package-context refusals, {gaps} open lowering gaps, {empty} empty products, {alias_only} alias-only products, {interfaces} declaration interfaces, {domain_only} domain-only products, {schema_only} schema-only products and {accepted} executable lowered products.",
        resolution.len(),
    );
    assert_eq!(
        resolution.len(),
        resolution_refusals + preparation_refusals + fragments + gaps + empty + alias_only + interfaces + domain_only + schema_only + accepted
    );
    let actual: BTreeMap<_, _> = lowered
        .iter()
        .map(|(label, outcome)| (label.as_str(), outcome.as_str()))
        .collect();
    let actual_rows = lowered
        .iter()
        .map(|(label, outcome)| format!("{label}  {outcome}"))
        .collect::<Vec<_>>();
    assert!(actual == expected_lowering, "reviewed L5 outcomes differ:\n{}", actual_rows.join("\n"));
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
    assert!(matches!(&inside.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==local.id && arguments==&[CallArgument::Scalar(input)]));
    let [literal, invoke] = example.body.operations.as_slice() else {
        panic!("expected a literal and wrapper invocation")
    };
    assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(42.into())));
    let OperationResult::Scalar(value) = literal.result else {
        panic!("expected the scalar 42")
    };
    assert!(matches!(&invoke.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==apply.id && arguments==&[CallArgument::Scalar(value)]));
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
        "language/effects.md:571" => {
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
        "language/generics.md:761" => {
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
