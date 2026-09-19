use crate::lower_typed_trees;
use crate::tests::termination::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
    symbol_of_checked,
};

#[test]
fn direct_crash_fallthrough_projects_immutable_entry_snapshots() {
    for (guard, route) in [
        ("flag", "!flag"),
        ("flag || (identity(current) && false)", "!flag"),
        ("!(flag && identity(current))", "flag"),
        ("true == (flag || identity(current))", "!flag"),
        ("false == (flag && identity(current))", "flag"),
    ] {
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}

            machine value(flag: bool, input: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap
                {route}
            {{
                let mut current: bool = input;
                let saved: bool = current;
                current = identity(!current);
                transition {{ {guard} -> saved }}
                crash Trap;
            }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed)
            .unwrap_or_else(|diagnostics| panic!("guard {guard}: {diagnostics:?}"));
        let plan = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, "value"))
            .expect("contract plan");
        let [site] = plan.crash.checked_sites() else {
            panic!("one source crash site");
        };
        assert_eq!(site.guard_covering_buckets().len(), 1, "guard {guard}");
        assert!(!site.path_guard_conjuncts().is_empty(), "guard {guard}");
    }
}

#[test]
fn direct_crash_fallthrough_does_not_project_unproven_boolean_operands() {
    for (guard, route) in [
        ("flag && identity(input)", "!flag"),
        ("!(flag || identity(input))", "flag"),
        ("identity(flag)", "!flag"),
    ] {
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool, input: bool) -> bool
            crashes Trap
                {route}
            {{ transition {{ {guard} -> true }} crash Trap; }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let diagnostics = lower_typed_trees(typed).expect_err("route is not implied");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("machine `value` has an uncovered Trap crash")
            }),
            "guard {guard}: {diagnostics:?}"
        );
    }
}

#[test]
fn direct_crash_fallthrough_does_not_confuse_state_and_entry_parameters() {
    let source = r#"
        machine value(flag: bool) -> bool
        crashes Trap
            !flag
        {
            transition { _ -> next(!flag) }
            state next(flag: bool) -> bool {
                transition { flag -> true }
                crash Trap;
            }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("distinct entry and state snapshots");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("machine `value` has an uncovered Trap crash")
        }),
        "{diagnostics:?}"
    );
}

fn check_fallthrough_coverage(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

fn assert_covered_site(source: &str) {
    let checked = check_fallthrough_coverage(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    let plan = checked
        .facts
        .contract_plans
        .for_machine(symbol_of_checked(&checked, "value"))
        .expect("contract plan");
    let [site] = plan.crash.checked_sites() else {
        panic!("one source crash site: {source}");
    };
    assert_eq!(site.guard_covering_buckets().len(), 1, "{source}");
}

fn assert_uncovered_site(source: &str) {
    let diagnostics = check_fallthrough_coverage(source)
        .expect_err("a current-storage read cannot claim the published entry route");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("machine `value` has an uncovered Trap crash")
        }),
        "{source}: {diagnostics:?}"
    );
}

#[test]
fn direct_crash_fallthrough_projects_pristine_mutable_snapshots() {
    // A `mut` parameter read still denotes its bound entry operand while no
    // statement before the guarded edge could have written it, and a write
    // after the edge cannot unmake the entry fact the unselected arm
    // established. A write before the guard ends that provenance: the
    // fallthrough then reads current storage, not the invocation snapshot.
    for (body, covered) in [
        ("transition { flag -> true } crash Trap;", true),
        (
            // The later arm's own guard borrows `flag` exclusively and may
            // overwrite it, but that evaluation runs after the earlier arm
            // established `!flag`; provenance is resolved where the read ran.
            "transition { flag -> true } transition { touch(&mut flag) -> true } crash Trap;",
            true,
        ),
        (
            "flag = true; transition { flag -> true } crash Trap;",
            false,
        ),
        (
            "let saved: bool = flag; flag = true; transition { flag -> true } crash Trap;",
            false,
        ),
    ] {
        let source = format!(
            "machine touch(slot: &mut bool) -> bool {{ slot = true; slot }}
             machine value(mut flag: bool) -> bool
             crashes Trap !flag
             {{ {body} }}"
        );
        if covered {
            assert_covered_site(&source);
        } else {
            assert_uncovered_site(&source);
        }
    }
}

#[test]
fn direct_crash_fallthrough_projects_entry_field_snapshots() {
    // The same provenance law covers field projections: an immutable record
    // parameter's member read was never a scalar snapshot, and a mutable
    // record keeps `flag.enabled` across a write confined to `flag.other`.
    for (mutable, prefix, covered) in [
        ("", "", true),
        ("mut ", "flag.other = true;", true),
        ("mut ", "flag.enabled = true;", false),
    ] {
        let source = format!(
            "data Flag {{ enabled: bool; other: bool; }}
             machine value({mutable}flag: Flag) -> bool
             crashes Trap !flag.enabled
             {{
                 {prefix}
                 transition {{ flag.enabled -> true }}
                 crash Trap;
             }}"
        );
        if covered {
            assert_covered_site(&source);
        } else {
            assert_uncovered_site(&source);
        }
    }
}

#[test]
fn direct_crash_fallthrough_projects_uniform_state_arrivals() {
    // Every named edge into `next` binds its `flag` to the entry operand, so
    // its fallthrough still speaks for the published entry route.
    assert_covered_site(
        "machine value(flag: bool) -> bool
         crashes Trap !flag
         {
             transition { _ -> next(flag) }
             state next(flag: bool) -> bool {
                 transition { flag -> true }
                 crash Trap;
             }
         }",
    );
    // A mutable state parameter keeps the same snapshot under the same rule:
    // one arrival binding it to the entry operand, no write before the read.
    assert_covered_site(
        "machine value(flag: bool) -> bool
         crashes Trap !flag
         {
             transition { _ -> next(flag) }
             state next(mut flag: bool) -> bool {
                 transition { flag -> true }
                 crash Trap;
             }
         }",
    );
    // A second arrival binding a different operand breaks the uniformity the
    // first case relied on: `flag` inside `next` is no longer the entry one.
    assert_uncovered_site(
        "machine value(flag: bool, other: bool) -> bool
         crashes Trap !flag
         {
             transition other { true -> next(flag) _ -> next(other) }
             state next(flag: bool) -> bool {
                 transition { flag -> true }
                 crash Trap;
             }
         }",
    );
}

#[test]
fn checked_call_fallthrough_projects_unselected_arm_facts() {
    // The callee's surviving route names its own parameter while the caller's
    // published ceiling names the negated entry operand. Only the fallthrough
    // fact `!flag` — established when the earlier arm was not selected —
    // activates the published guard as a path consequence over the call.
    let source = r#"
        machine risky(flag: bool) -> bool
        crashes Trap
            flag
        { flag }

        machine value(flag: bool) -> bool
        crashes Trap
            !flag
        {
            transition { flag -> true }
            transition { risky(flag) -> true }
            crash Trap;
        }
    "#;
    let checked =
        check_fallthrough_coverage(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    let plan = checked
        .facts
        .contract_plans
        .for_machine(symbol_of_checked(&checked, "value"))
        .expect("contract plan");
    let [call] = plan.crash.checked_calls() else {
        panic!("one checked call");
    };
    let [bucket] = call.surviving_buckets() else {
        panic!("the guarded route survives operand substitution");
    };
    assert_eq!(bucket.cause(), checked_trees::CrashCause::Trap);
    assert!(
        !call.path_guard_consequences().is_empty(),
        "the unselected arm contributes its entry-term fact at the call"
    );
}

#[test]
fn checked_call_fallthrough_rejects_written_entry_claims() {
    // A write before the guarded edge ends the parameter's entry provenance:
    // the fallthrough then reads current storage, no `!flag` entry fact
    // survives, and the surviving `flag` route stays uncovered.
    let source = r#"
        machine risky(flag: bool) -> bool
        crashes Trap
            flag
        { flag }

        machine value(mut flag: bool) -> bool
        crashes Trap
            !flag
        {
            flag = true;
            transition { flag -> true }
            transition { risky(flag) -> true }
            crash Trap;
        }
    "#;
    let diagnostics = check_fallthrough_coverage(source)
        .expect_err("a current-storage read cannot claim the published entry route");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("call from `value` to `risky`")
                && diagnostic.message.contains("uncovered Trap crash route")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn direct_crash_fallthrough_does_not_substitute_local_aliases() {
    // `saved` holds the entry operand's value, but its own spelling claims no
    // parameter position: the published route names `flag`, and no proven
    // leaf identity equals that claim.
    assert_uncovered_site(
        "machine value(flag: bool) -> bool
         crashes Trap !flag
         {
             let saved: bool = flag;
             transition { saved -> true }
             crash Trap;
         }",
    );
}

#[test]
fn crash_bucket_identity_includes_cause_routes_and_unconditional_presence() {
    let source = r#"
    machine baseline() {}
    machine trap_activation(flag: bool)
    crashes Trap
        flag
    {}
    machine trap_domain(flag: bool)
    crashes Trap
        flag
    {}
    machine abort_activation(flag: bool)
    crashes Abort
        flag
    {}
    machine unconditional_abort()
    crashes Abort
    {}
    machine explicit_true_abort()
    crashes Abort
        true
    {}
    machine grouped(first: bool, second: bool)
    crashes Trap
        first
        second
    {}
    machine split(first: bool, second: bool)
    crashes Trap
        first
    crashes Trap
        second
    {}
    machine reordered(first: bool, second: bool)
    crashes Trap
        second
        first
    {}
    machine duplicated(first: bool, second: bool)
    crashes Trap
        first
        second
        first
    {}
    machine unconditional_with_guard(flag: bool)
    crashes Abort
        flag
    crashes Abort
    {}
    machine unconditional_only(flag: bool)
    crashes Abort
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let fingerprint = |name: &str| {
        let symbol = symbol_of_checked(&checked, name);
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
            .report_fingerprint
    };

    assert_ne!(fingerprint("baseline"), fingerprint("unconditional_abort"));
    assert_eq!(
        fingerprint("unconditional_abort"),
        fingerprint("explicit_true_abort")
    );
    assert_eq!(fingerprint("trap_activation"), fingerprint("trap_domain"));
    assert_ne!(
        fingerprint("trap_activation"),
        fingerprint("abort_activation")
    );
    assert_eq!(fingerprint("grouped"), fingerprint("split"));
    assert_eq!(fingerprint("grouped"), fingerprint("reordered"));
    assert_eq!(fingerprint("grouped"), fingerprint("duplicated"));
    assert_eq!(
        fingerprint("unconditional_with_guard"),
        fingerprint("unconditional_only")
    );

    let crash = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
            .crash
            .clone()
    };
    assert_eq!(crash("grouped"), crash("split"));
    assert_eq!(crash("grouped"), crash("reordered"));
    assert_eq!(crash("grouped"), crash("duplicated"));
    assert_eq!(crash("unconditional_abort"), crash("explicit_true_abort"));
    let grouped = crash("grouped");
    assert_eq!(
        grouped.interface(),
        checked_trees::CrashInterface::PublishedCeiling
    );
    assert_eq!(grouped.published().len(), 1);
    assert_eq!(
        grouped.published()[0].cause(),
        checked_trees::CrashCause::Trap
    );
    assert_eq!(grouped.published()[0].alternative_guards().len(), 2);
    assert!(!grouped.published()[0].is_unconditional());
}

#[test]
fn empty_record_equality_retains_existing_boolean_constant_carriers() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Empty {}
    EmptyEquatable: Empty satisfies Equatable;

    machine equal(left: Empty, right: Empty)
    crashes Abort
        left == right
    {}

    machine not_equal(left: Empty, right: Empty)
    crashes Abort
        left != right
    {}

    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let scalar = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned()
    };

    assert_eq!(
        scalar("equal"),
        Some(checked_trees::CheckedBooleanExpression::Constant(true))
    );
    assert_eq!(
        scalar("not_equal"),
        Some(checked_trees::CheckedBooleanExpression::Not(Box::new(
            checked_trees::CheckedBooleanExpression::Constant(true)
        )))
    );
}

#[test]
fn erased_record_equality_is_not_mistaken_for_empty_record_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Erased { proof [erased]: bool; }
    ErasedEquatable: Erased satisfies Equatable;

    machine erased_equal(left: Erased, right: Erased)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    // Since 80ca19bb1a synthesized `==` skips `[erased]` fields, so a record
    // whose every field is erased has no runtime value to compare. Typing
    // rejects it explicitly instead of letting the zero-member-record rule
    // compare it as vacuous truth; no later crash-route check is reached.
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("erased semantic fields must not be treated as an empty record");
    assert!(
        diagnostic
            .message
            .contains("every field is `[erased]`, so the record has no runtime value to compare"),
        "{diagnostic:#?}"
    );
}

#[test]
fn address_field_equality_stays_outside_structural_crash_predicates() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Addressed { pointer: addr; }
    AddressedEquatable: Addressed satisfies Equatable;

    machine whole_equal(left: Addressed, right: Addressed)
    crashes Abort
        left == right
    {}

    machine field_equal(left: Addressed, right: Addressed)
    crashes Abort
        left.pointer == right.pointer
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    for name in ["whole_equal", "field_equal"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        assert!(
            predicate.scalar_expression().is_none(),
            "{name} must not retain addr as a fixed-integer structural term"
        );
    }
}

#[test]
fn ieee_float_fields_retain_atomic_structural_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Samples { narrow: f32; wide: f64; }
    SamplesEquatable: Samples satisfies Equatable;

    machine whole(left: Samples, right: Samples)
    crashes Abort
        left == right
    {}

    machine whole_not_equal(left: Samples, right: Samples)
    crashes Abort
        left != right
    {}

    machine narrow(left: Samples, right: Samples)
    crashes Abort
        left.narrow == right.narrow
    {}

    machine wide(left: Samples, right: Samples)
    crashes Abort
        left.wide == right.wide
    {}

    machine narrow_not_equal(left: Samples, right: Samples)
    crashes Abort
        left.narrow != right.narrow
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let scalar = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned().expect("scalar term")
    };
    let format = |expression: &checked_trees::CheckedBooleanExpression| match expression {
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { primitive_type, .. } => {
            Some(*primitive_type)
        }
        _ => None,
    };

    assert_eq!(
        format(&scalar("narrow")),
        Some(typed_trees::types::PrimitiveType::F32)
    );
    assert_eq!(
        format(&scalar("wide")),
        Some(typed_trees::types::PrimitiveType::F64)
    );
    assert!(matches!(
        scalar("narrow_not_equal"),
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison {
            kind: checked_trees::CheckedIeeeFloatComparisonKind::NotEqual,
            primitive_type: typed_trees::types::PrimitiveType::F32,
            ..
        }
    ));
    let checked_trees::CheckedBooleanExpression::And { left, right } = scalar("whole") else {
        panic!("two-field float record equality is one conjunction")
    };
    assert!(matches!(
        scalar("whole_not_equal"),
        checked_trees::CheckedBooleanExpression::Not(operand)
            if matches!(*operand, checked_trees::CheckedBooleanExpression::And { .. })
    ));
    let formats = [format(&left), format(&right)];
    assert!(formats.contains(&Some(typed_trees::types::PrimitiveType::F32)));
    assert!(formats.contains(&Some(typed_trees::types::PrimitiveType::F64)));
}

#[test]
fn byte_sequence_fields_retain_atomic_content_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    domain [u8]::Utf8
    requires
        valid_utf8(self);
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    data Borrowed { active: bool; text: &[u8] in Utf8; }
    BorrowedEquatable: Borrowed satisfies Equatable;
    data Bounded { active: bool; text: [u8; 8] in Utf8; }
    BoundedEquatable: Bounded satisfies Equatable;

    machine borrowed(left: Borrowed, right: Borrowed)
    crashes Abort
        left == right
    {}

    machine bounded(left: Bounded, right: Bounded)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    for name in ["borrowed", "bounded"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        let checked_trees::CheckedBooleanExpression::And { left, right } = predicate
            .scalar_expression()
            .expect("whole-record equality remains a checked expression")
        else {
            panic!("{name} should compare its Boolean and byte-sequence fields")
        };
        assert!(
            matches!(
                left.as_ref(),
                checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
            ) || matches!(
                right.as_ref(),
                checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
            ),
            "{name} should retain byte content equality as one atomic leaf"
        );
        assert!(
            matches!(
                left.as_ref(),
                checked_trees::CheckedBooleanExpression::Equal { .. }
            ) || matches!(
                right.as_ref(),
                checked_trees::CheckedBooleanExpression::Equal { .. }
            ),
            "{name} should retain its scalar sibling independently"
        );
    }
}

#[test]
fn payloadless_sum_equality_retains_closed_case_roster() {
    let source = r#"
    data Mode {
        case Off;
        case On;
    }

    machine equal(left: Mode, right: Mode)
    crashes Abort
        left == right
    {}

    machine not_equal(left: Mode, right: Mode)
    crashes Abort
        left != right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let expression = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned().expect("scalar term")
    };

    assert!(matches!(
        expression("equal"),
        checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { cases, .. }
            if cases == ["Off", "On"]
    ));
    assert!(matches!(
        expression("not_equal"),
        checked_trees::CheckedBooleanExpression::Not(operand)
            if matches!(operand.as_ref(),
                checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { cases, .. }
                    if cases.len() == 2 && cases[0] == "Off" && cases[1] == "On")
    ));
}

#[test]
fn nested_payload_bearing_sum_equality_retains_record_case_payload_paths() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression,
        CheckedStructuralPredicatePathSegment as Path,
    };

    fn collect_paths(
        expression: &CheckedBooleanExpression,
        membership_paths: &mut Vec<Vec<Path>>,
        payload_paths: &mut Vec<Vec<Path>>,
    ) {
        match expression {
            CheckedBooleanExpression::ErasedParameter { .. } => {}
            CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
                membership_paths.push(subject.path.clone());
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let CheckedScalarExpression::StructuralParameterField { path, .. } = operand
                    {
                        payload_paths.push(path.clone());
                    }
                }
            }
            CheckedBooleanExpression::Not(operand) => {
                collect_paths(operand, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                collect_paths(left, membership_paths, payload_paths);
                collect_paths(right, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {}
        }
    }

    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Envelope { active: bool; message: Message; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    machine equal(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let contract = checked
        .facts
        .contract_plans
        .for_machine(symbol_of_checked(&checked, "equal"))
        .expect("contract plan");
    let [bucket] = contract.crash.published() else {
        panic!("equal should publish one crash bucket")
    };
    let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards() else {
        panic!("equal should publish one predicate")
    };
    let expression = predicate
        .scalar_expression()
        .expect("nested sum equality remains a checked expression");
    let mut membership_paths = Vec::new();
    let mut payload_paths = Vec::new();
    collect_paths(expression, &mut membership_paths, &mut payload_paths);

    assert_eq!(membership_paths.len(), 4);
    assert!(
        membership_paths
            .iter()
            .all(|path| { path == &[Path::Field("message".to_owned())] })
    );
    assert_eq!(payload_paths.len(), 2);
    assert!(payload_paths.iter().all(|path| {
        path == &[
            Path::Field("message".to_owned()),
            Path::Case("Data".to_owned()),
            Path::Field("value".to_owned()),
        ]
    }));
}

#[test]
fn payload_sum_equality_expands_acyclic_nested_records_with_exact_paths() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression,
        CheckedStructuralPredicatePathSegment as Path,
    };

    fn collect_leaf_paths(expression: &CheckedBooleanExpression, paths: &mut Vec<Vec<Path>>) {
        match expression {
            CheckedBooleanExpression::ErasedParameter { .. } => {}
            CheckedBooleanExpression::StructuralParameterField { path, .. } => {
                paths.push(path.clone());
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let CheckedScalarExpression::StructuralParameterField { path, .. } = operand
                    {
                        paths.push(path.clone());
                    }
                }
            }
            CheckedBooleanExpression::Not(operand) => collect_leaf_paths(operand, paths),
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                collect_leaf_paths(left, paths);
                collect_leaf_paths(right, paths);
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
        }
    }

    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Counter { count: i32; }
    CounterEquatable: Counter satisfies Equatable;

    data Metrics { active: bool; counter: Counter; }
    MetricsEquatable: Metrics satisfies Equatable;

    data Message {
        case Empty;
        case Data(metrics: Metrics);
    }
    MessageEquatable: Message satisfies Equatable;

    machine equal(left: Message, right: Message)
    crashes Abort
        left == right
    {}

    machine different(left: Message, right: Message)
    crashes Abort
        left != right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    for name in ["equal", "different"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        let mut paths = Vec::new();
        collect_leaf_paths(
            predicate
                .scalar_expression()
                .expect("nested payload record equality remains checked"),
            &mut paths,
        );
        assert_eq!(paths.len(), 4, "{name} retains both roots for two leaves");
        assert!(paths.iter().all(|path| match path.as_slice() {
            [Path::Case(case), Path::Field(payload), Path::Field(leaf)] => {
                case == "Data" && payload == "metrics" && leaf == "active"
            }
            [
                Path::Case(case),
                Path::Field(payload),
                Path::Field(record),
                Path::Field(leaf),
            ] => {
                case == "Data" && payload == "metrics" && record == "counter" && leaf == "count"
            }
            _ => false,
        }));
    }
}

#[test]
fn payload_sum_equality_expands_acyclic_nested_sums_with_exact_paths() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression,
        CheckedStructuralPredicatePathSegment as Path,
    };

    fn collect_paths(
        expression: &CheckedBooleanExpression,
        membership_paths: &mut Vec<Vec<Path>>,
        payload_paths: &mut Vec<Vec<Path>>,
    ) {
        match expression {
            CheckedBooleanExpression::ErasedParameter { .. } => {}
            CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
                membership_paths.push(subject.path.clone());
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let CheckedScalarExpression::StructuralParameterField { path, .. } = operand
                    {
                        payload_paths.push(path.clone());
                    }
                }
            }
            CheckedBooleanExpression::Not(operand) => {
                collect_paths(operand, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                collect_paths(left, membership_paths, payload_paths);
                collect_paths(right, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {}
        }
    }

    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Detail {
        case Missing;
        case Count(value: i32);
    }
    DetailEquatable: Detail satisfies Equatable;

    data Message {
        case Empty;
        case Data(detail: Detail);
    }
    MessageEquatable: Message satisfies Equatable;

    machine equal(left: Message, right: Message)
    crashes Abort
        left == right
    {}

    machine different(left: Message, right: Message)
    crashes Abort
        left != right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    for name in ["equal", "different"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        let mut membership_paths = Vec::new();
        let mut payload_paths = Vec::new();
        collect_paths(
            predicate
                .scalar_expression()
                .expect("nested payload-sum equality remains checked"),
            &mut membership_paths,
            &mut payload_paths,
        );

        assert_eq!(membership_paths.len(), 8, "{name} retains both sum levels");
        assert_eq!(
            membership_paths
                .iter()
                .filter(|path| path.is_empty())
                .count(),
            4,
            "{name} retains both roots for both outer cases"
        );
        assert_eq!(
            membership_paths
                .iter()
                .filter(|path| {
                    path.as_slice()
                        == [
                            Path::Case("Data".to_owned()),
                            Path::Field("detail".to_owned()),
                        ]
                })
                .count(),
            4,
            "{name} retains both roots for both nested cases"
        );
        assert_eq!(payload_paths.len(), 2, "{name} retains both integer roots");
        assert!(payload_paths.iter().all(|path| {
            path.as_slice()
                == [
                    Path::Case("Data".to_owned()),
                    Path::Field("detail".to_owned()),
                    Path::Case("Count".to_owned()),
                    Path::Field("value".to_owned()),
                ]
        }));
    }
}
