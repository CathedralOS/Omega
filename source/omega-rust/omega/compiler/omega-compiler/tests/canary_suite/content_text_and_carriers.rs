use super::*;

#[test]
fn unary_negation_exit_canary_runs() {
    let canary = pass_canary("operators/unary_negation_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-unary-negation-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unary negation canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unary negation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unary negation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected negative literals and unary negation to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 B1c / GAP #2 (value-call arg descriptor): a string literal passed to a
// `&[u8] in Utf8` parameter materializes a correct `{ptr, len}` slice descriptor
// so the callee's `text.len` reads the literal's byte length 5 (exit 70). The
// value-call mechanism and arg passing already worked; the gap was the
// param-slice `.len` value-source read in the value-call splice path.
#[test]
fn utf8_literal_len_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_literal_len_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-literal-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 literal len canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 literal len canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 literal len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a string literal passed to a `&[u8] in Utf8` param to read len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66: the literal-grant mechanism is GENERAL -- not hardcoded to the `Utf8`
// domain name. A USER domain `[u8]::Ascii` with a DIFFERENT byte-predicate fact
// (`ascii_only(self)`) grants an ASCII string literal its domain, discharging the
// param's `in Ascii` requirement; under the old `name == "Utf8"` hardcode this
// would have failed. The literal also flows as a real `&[u8]` view, so
// `measure("hi")` reads len 2 and exits 70.
#[test]
fn user_domain_literal_grant_canary_runs() {
    let canary = pass_canary("domains/user_domain_literal_grant");
    let build_dir =
        std::env::temp_dir().join(format!("omega-user-domain-grant-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("user-domain literal grant canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("user-domain literal grant canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("user-domain literal grant canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an ASCII literal to be granted a user `[u8]::Ascii` domain via its \
         `ascii_only(self)` fact and read len 2 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn bodyless_domain_declaration_spellings_canary_runs() {
    let canary = pass_canary("domains/bodyless_domain_declarations_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-bodyless-domains-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("equivalent bodyless-domain spellings should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bodyless-domain declaration canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bodyless-domain declaration canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected bodyless-domain declaration canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn authorized_route_establishment_canaries() {
    let pass = pass_canary("domains/bodyless_owner_establishment");
    let build_dir =
        std::env::temp_dir().join(format!("omega-bodyless-owner-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    production_compile(CanaryCompileSpec {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("a carrier-owner checked machine should establish its bodyless result");
    let evidence = fs::read_to_string(build_dir.join("05_qualification_evidence.json"))
        .expect("qualification-evidence artifact");
    assert!(evidence.contains("\"origin\": \"authorized_route_establishment\""));
    assert!(evidence.contains("\"program_point\": \"call_ensures\""));
    let _ = fs::remove_dir_all(&build_dir);

    for name in [
        "domains/bodyless_nonowner_establishment",
        "domains/bodyful_owner_establishment_bypass",
    ] {
        let diagnostics = compile_canary_without_output(&fail_canary(name))
            .expect_err("unauthorized establishment must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
            "{name} rejected differently: {diagnostics:#?}"
        );
    }
}

#[test]
fn extent_root_provider_adapter_compiles() {
    let canary = pass_canary("core/extent_root_provider_adapter");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("the core Extent projection should survive checked lowering");
    let granted = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().rsplit("::").next() == Some("Granted"))
        .expect("core Extent::Granted domain");
    let [psi_typed_trees::domain::ProofFact::Expression(predicate)] = checked.proof_facts(granted)
    else {
        panic!("Extent::Granted should require exactly one no-wrap predicate");
    };
    let psi_typed_trees::expression::ExpressionNode::Call(no_wrap) =
        checked.expression_table.expression(*predicate)
    else {
        panic!("Extent::Granted predicate should be a call");
    };
    assert_eq!(no_wrap.target.as_str(), "no_wrap");
    assert!(
        no_wrap.target_symbol.is_valid(),
        "no_wrap target should resolve: {no_wrap:?}"
    );
    let no_wrap_machine = checked
        .machines()
        .iter()
        .find(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == no_wrap.target_symbol)
        })
        .expect("no_wrap call target should resolve to its owning machine");
    assert_eq!(
        checked.symbols.symbol_source_origin(no_wrap_machine.symbol),
        Some(psi_source::SourceOrigin::Toolchain),
        "the target-bound predicate must be the compiler-provided declaration"
    );
    assert_eq!(
        granted.establishment_routes.len(),
        2,
        "Extent::Granted must retain the provider-result and program-entry routes"
    );
    assert!(granted.establishment_routes.iter().all(|route| matches!(
        route,
        psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
    )));

    let plan = checked
        .facts
        .qualifications
        .content
        .plans
        .iter()
        .find(|plan| {
            matches!(
                &plan.algebra,
                ContentAlgebraIdentity::IntervalSet { coordinate_space }
                    if coordinate_space == "named(name(Nat))"
            )
        })
        .expect("Extent::Granted should publish its Nat-coordinate interval set");
    let ContentProjectionExpression::IntervalSet { members } = &plan.expression else {
        panic!("Extent::Granted must normalize to an interval set");
    };
    let [member] = members.as_slice() else {
        panic!("Extent::Granted must normalize to one interval-set member");
    };
    assert!(matches!(
        member.start(),
        ContentScalarExpression::RuntimeScalarEmbedding(path)
            if matches!(path.as_slice(), [field] if field.name == "base")
    ));
    assert!(matches!(
        member.end(),
        ContentScalarExpression::Arithmetic {
            operator: ContentArithmeticOperator::Add,
            left,
            right,
        } if matches!(left.as_ref(), ContentScalarExpression::RuntimeScalarEmbedding(path)
            if matches!(path.as_slice(), [field] if field.name == "base"))
            && matches!(right.as_ref(), ContentScalarExpression::RuntimeScalarEmbedding(path)
                if matches!(path.as_slice(), [field] if field.name == "length"))
    ));
    assert_ne!(plan.report_fingerprint, 0);

    let build_dir = std::env::temp_dir().join(format!("omega-extent-root-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect(
        "a checked adapter selected through the owner-authored Extent provider \
         requirement should originate and forward one admitted Granted root",
    );
    let evidence = fs::read_to_string(build_dir.join("05_qualification_evidence.json"))
        .expect("Extent qualification-evidence artifact");
    assert!(evidence.contains("\"origin\": \"admitted_receipt\""));
    assert!(evidence.contains("\"source\": \"ExtentRootProvider\""));
    assert!(evidence.contains("\"requirement\": \"ExtentRootProvider::grant\""));
    assert!(
        evidence.contains("\"receipt_identity\": \"0x"),
        "the build grant must attach the selected provider-plan receipt:\n{evidence}"
    );
    let outcomes = fs::read_to_string(build_dir.join("05_claim_outcomes.json"))
        .expect("Extent claim-outcome and content-projection artifact");
    assert!(outcomes.contains("\"content_projections\""));
    assert!(outcomes.contains("\"domain\": \"Extent::Granted\""));
    assert!(outcomes.contains("\"kind\": \"interval_set\""));
    assert!(outcomes.contains("\"members\": ["));
    assert!(outcomes.contains("\"coordinate_space\": \"named(name(Nat))\""));
    assert!(outcomes.contains("\"kind\": \"runtime_scalar_embedding\""));
    assert!(outcomes.contains("\"path\": [\"base\"]"));
    assert!(outcomes.contains("\"path\": [\"length\"]"));
    assert!(outcomes.contains("\"operator\": \"add\""));
    assert!(outcomes.contains("\"report_fingerprint\": \"0x"));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn content_conservation_contract_is_normalized_and_reported() {
    let canary = pass_canary("core/content_conservation_contract");
    let source = fs::read_to_string(canary.join("main.omg")).expect("content canary source");
    assert!(source.contains("old(&whole)"));
    assert!(!source.contains("entry(&whole)"));
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("the exact old/separate conservation contract should check");
    let [plan] = checked
        .facts
        .qualifications
        .content
        .conservation_plans
        .as_slice()
    else {
        panic!("one normalized conservation equation should be retained");
    };
    assert_eq!(
        plan.owner_kind,
        ContentConservationOwnerKind::TraitRequirement
    );
    assert!(matches!(
        &plan.algebra,
        ContentAlgebraIdentity::CountedQuantity { unit }
            if unit == "named(name(ByteUnit))"
    ));
    let ContentConservationTerm::Projection { subject, .. } = plan.equation.left() else {
        panic!("callable-entry projection should canonicalize before separated outputs");
    };
    assert_eq!(subject.version, ContentPlaceVersion::Entry);
    assert!(matches!(
        subject.root,
        ContentPlaceRoot::Parameter { position: 0, .. }
    ));
    assert!(matches!(
        plan.equation.right(),
        ContentConservationTerm::Separate(outputs) if outputs.len() == 2
    ));
    assert_ne!(plan.report_fingerprint, 0);

    let build_dir =
        std::env::temp_dir().join(format!("omega-content-conservation-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("the normalized conservation contract should emit its proof/debug artifact");
    let outcomes = fs::read_to_string(build_dir.join("05_claim_outcomes.json"))
        .expect("content-conservation artifact");
    assert!(outcomes.contains("\"content_conservation\""));
    assert!(outcomes.contains("\"owner_kind\": \"trait_requirement\""));
    assert!(outcomes.contains("\"version\": \"entry\""));
    assert!(outcomes.contains("\"kind\": \"separate\""));
    assert!(outcomes.contains("\"kind\": \"result\""));
    assert!(outcomes.contains("\"report_fingerprint\": \"0x"));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn carry_permission_provider_adapter_compiles_with_exact_artifacts() {
    let canary = pass_canary("core/carry_permission_provider_adapter");
    let build_dir = std::env::temp_dir().join(format!("omega-carry-claim-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect(
        "the selected owner-authorized requirement should admit its exact carry \
         permission and allow that claim to cross suspension",
    );

    let evidence = fs::read_to_string(build_dir.join("05_qualification_evidence.json"))
        .expect("carry qualification-evidence artifact");
    assert!(evidence.contains("\"domain\": \"Carry::AcrossSuspend\""));
    assert!(evidence.contains("\"origin\": \"admitted_receipt\""));
    assert!(evidence.contains("\"source\": \"ClaimProvider\""));
    assert!(evidence.contains("\"requirement\": \"ClaimProvider::grant\""));
    assert!(
        evidence.contains("\"receipt_identity\": \"0x"),
        "the carry permission must retain its admitted provider receipt:\n{evidence}"
    );

    let carry = fs::read_to_string(build_dir.join("05_carry_manifest.json"))
        .expect("carry policy artifact");
    assert!(carry.contains("\"machine\": \"Harness::forward\""));
    assert!(carry.contains("\"storage\": \"local\""));
    assert!(carry.contains(
        "\"effective\": {\"suspension\": \"allowed\", \"cpu\": \"same\", \
         \"thread\": \"same\", \"address\": \"stable\"}"
    ));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn empty_domain_explicit_as_qualifies_vacuously() {
    let canary = pass_canary("domains/vacuous_domain_qualification");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("an empty domain should accept explicit compiler-derived qualification");
    let uses = &checked.facts.qualifications.vacuous_uses;
    assert_eq!(uses.len(), 3, "every explicit `as` use must be retained");
    assert_eq!(uses[0].domain, uses[1].domain);
    assert_ne!(uses[1].domain, uses[2].domain);
    let evidence = omega_visualizations::qualification_evidence_manifest_json(
        &checked,
        checked.selected_provider_plans(),
    );
    assert!(evidence.contains("\"vacuous_qualification_uses\": ["));
    assert!(evidence.contains("\"origin\": \"vacuous_qualification\""));
    assert!(evidence.contains("\"domain\": \"i64::Km\""));
    assert!(evidence.contains("\"domain\": \"i64::Distance\""));
    assert!(!evidence.contains("\"satisfier\""));
}

#[test]
fn user_authored_predicate_machine_compiles() {
    let canary = pass_canary("domains/user_authored_predicate_machine");
    compile_canary_without_output(&canary)
        .expect("domain `requires` may call an ordinary user-authored predicate machine");
}

#[test]
fn boundary_qualification_evidence_names_exact_requirement() {
    let pass = pass_canary("capabilities/derives_authority_via_boundary");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-boundary-qualification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    production_compile(CanaryCompileSpec {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("an exact boundary-result qualification should compile");

    let evidence = fs::read_to_string(build_dir.join("05_qualification_evidence.json"))
        .expect("qualification-evidence artifact");
    assert!(evidence.contains("\"origin\": \"admitted_receipt\""));
    assert!(evidence.contains("\"source\": \"Filesystem\""));
    assert!(evidence.contains("\"requirement\": \"Filesystem::open\""));
    assert!(evidence.contains("\"requirement_identity\": \"named-callable(path(Filesystem::open)"));
    let _ = fs::remove_dir_all(&build_dir);
}

// #66 GAP #4 (slice-`.len`-to-field write): a `&[u8] in Utf8` PARAM is a runtime
// `{ptr, len}` descriptor in a frame slot, so `self.result = text.len` reads the
// descriptor's len field (NOT a compile-time constant -- that is GAP #2). This
// already lowers via the `<root>.len`-over-a-descriptor-slot value-source; the
// canary pins it end-to-end. `store("hello")` records 5; the caller guards == 5.
#[test]
fn utf8_param_len_field_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_param_len_field_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-param-len-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 param len field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 param len field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 param len field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.result = text.len` of a `&[u8] in Utf8` param to record len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 GAP #1 (regular-call arg descriptor) + GAP #3 (sub-state requires flow): a
// string literal passed to a `&[u8] in Utf8` param in a REGULAR statement call
// materializes a correct `{ptr, len}` descriptor (String/slice share the fat
// layout, so the existing String-literal frame-slot writer populates the slot),
// and the callee's synthesized `requires text in [u8]::Utf8` is assumed once on
// entry -- NOT re-imposed at the internal `true -> ok() _ -> nope()` sub-state
// dispatch. `check("hello")` sees len 5 and exits 70.
#[test]
fn utf8_regular_call_len_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_regular_call_len_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-utf8-regular-call-len-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 regular call len canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 regular call len canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 regular call len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a regular call passing a string literal to a `&[u8] in Utf8` param to read len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 (`&[u8] in Utf8 == "literal"` content compare): a domained-slice-view value
// `==` a string literal lowers through the SAME TextEquals leaf String uses, NOT a
// scalar compare of the descriptor's pointer words. `classify("quit")` matches and
// exits 70; the interpreter agrees (differential). Before this, the guard fell to
// the generic scalar path: native compared the descriptor's POINTER words and took
// the wrong arm, silently diverging from the interpreter's content equality.
#[test]
fn utf8_equals_literal_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_equals_literal_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-equals-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 equals literal canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 equals literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 equals literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8] in Utf8 == \"quit\"` content equality to match and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 (`&[u8] in Utf8 == &[u8] in Utf8` content compare): comparing two
// domained-slice views lowers through the TextEquals content leaf. `cmp("Gate",
// "Gate")` matches and exits 70; the interpreter agrees. Before this, the generic
// scalar path tried to load the 16-byte descriptor as a runtime operand (the
// encoder rejects it) and compared only the pointer words.
#[test]
fn utf8_equals_view_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_equals_view_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-equals-view-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 equals view canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 equals view canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 equals view canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8] in Utf8 == &[u8] in Utf8` content equality to match and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 read-narrowing: a DECLARED-domain field (`out: &[u8] in Utf8`) carries its
// `in Utf8` fact when READ. `self.out = "Gate"` (write-enforced) then
// `self.check(self.out)` passes the field read to a `&[u8] in Utf8` parameter --
// which only discharges because the read carries the field's declared domain.
// `check` guards `text == "Gate"` and exits 70; the interpreter agrees. Before the
// read-narrowing fix this rejected with "cannot prove requires contract ...
// self.out in [u8]::Utf8".
#[test]
fn utf8_field_read_carries_domain_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_field_read_carries_domain_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-utf8-field-read-domain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf8 field-read domain canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("utf8 field-read domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("utf8 field-read domain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the declared-domain field read `self.out` to carry `in Utf8` so \
         `self.check(self.out)` discharges and `text == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 soundness-gate COMPLETENESS: an empty-violating domain (`[u8]::NonEmpty`,
// `non_empty(self)`) gets NO machine-entry field-invariant (the empty/ZII value
// violates it -- see fail canary domain_field_read_no_write_unproven), but after
// an ENFORCED write the re-established fact still flows to a read. `self.f = "x"`
// stores a literal accepted by the write-enforcement construction-grant
// (non-empty bytes); the subsequent `self.check(self.f)` read carries the
// re-established `in NonEmpty` and discharges the `&[u8] in NonEmpty` parameter.
// `check` guards `text == "x"` and exits 73; the interpreter agrees.
#[test]
fn domain_field_write_then_read_exit_canary_runs() {
    let canary = pass_canary("domains/domain_field_write_then_read_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-domain-field-write-then-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("domain field write-then-read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("domain field write-then-read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("domain field write-then-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected the enforced write `self.f = \"x\"` to re-establish `in NonEmpty` so the \
         read `self.check(self.f)` discharges and `text == \"x\"` exits 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` bounded byte carrier, end to end on native: the
// literal write materializes into the carrier's `{len, bytes}` inline storage
// (the carrier OWNS its bytes -- not a {ptr,len} descriptor aliasing rodata),
// and the `==` guard reads it back with carrier addressing (len @ 0, bytes @
// pointer_size) and content-compares. `self.label == "Gate"` matches -> exit 70.
// `[u8; 8]` is the 16-byte case (8 + 8 == the string descriptor size) that the
// String text-write pass would otherwise claim as a descriptor.
#[test]
fn runtime_bounded_carrier_write_read_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_write_read_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-write-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded carrier write-read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier write-read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier write-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the owned `[u8; 8] in Utf8` carrier to write `\"Gate\"` into its inline \
         {{len, bytes}} storage and read it back so `self.label == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier `.len` as a marshaled host-call argument:
// `self.message = "ALERT " + self.label` builds a length-10 carrier, and
// `exit_process(self.message.len)` reads the carrier's length word (at the
// carrier's own offset 0, not a fat-slice descriptor's `+pointer_size`) -> exit 10.
// `.len` already resolved in guards; this exercises it in value/argument position.
#[test]
fn runtime_bounded_carrier_length_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_length_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-length-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded carrier length canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier length canary should run");

    assert_eq!(
        output.status.code(),
        Some(10),
        "expected `exit_process(self.message.len)` to read the carrier's length word \
         (\"ALERT temp\" = 10), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier `.len` as a MUTATION-WRITE value:
// `self.count = self.message.len` reads the length-10 carrier's length word into a
// plain i32 field (a 4-byte read narrowing exactly into the i32 target), then exits
// the field -> 10. Covers the mutation value-operand consumer of the shared
// resolver's carrier-`.len` resolution (the host-call consumer is _length_exit).
#[test]
fn runtime_bounded_carrier_length_field_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_length_field_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-length-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded carrier length field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier length field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier length field canary should run");

    assert_eq!(
        output.status.code(),
        Some(10),
        "expected `self.count = self.message.len` to store the carrier length (10) \
         into the i32 field, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier byte indexing in guard subjects:
// `message[i]` reads the byte at `base + pointer_size + i` (content after the
// length word, u8 elements). The compound guard `message[0] == 'A' &&
// message[2] == 'E'` reads two bytes of "ALERT"; both hold -> ok arm exits 70.
// (Indexing in guards is the parsing workhorse; the separate bounded-carrier
// widening canary covers using the indexed byte as an explicitly widened value.)
#[test]
fn runtime_bounded_carrier_byte_index_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_byte_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-byte-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded carrier byte index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier byte index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier byte index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compound byte-index guard `message[0]=='A' && message[2]=='E'` \
         to hold and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bounded_carrier_byte_widen_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_byte_widen_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-byte-widen-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded carrier indexed-byte widening canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier indexed-byte widening should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier indexed-byte widening canary should run");

    assert_eq!(
        output.status.code(),
        Some(65),
        "expected `self.message[0] as i32` to zero-extend ASCII A (65), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_indexed_read_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_read_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-indexed-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("carrier indexed read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier indexed read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.text[self.i]` (runtime index on a [u8;N] carrier) to read 'a'/'c' past the len prefix and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_number_to_decimal_exit_canary_runs() {
    // Numeric output (itoa): build n=12345 at runtime, render it to the decimal text
    // "12345" via divide/modulo + computed carrier byte writes, and assert the
    // carrier equals it. A round-trip proving printable numbers, a serious-app need.
    let canary = pass_canary("text/runtime_number_to_decimal_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-number-to-decimal-{}", std::process::id()));

    let host_scratch = scratch.join("host");
    compile_rooted_canary_for_native_host(&canary, host_scratch.clone())
        .expect("number-to-decimal canary should compile");

    let output = Command::new(host_scratch.join(executable_name()))
        .output()
        .expect("number-to-decimal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected integer->decimal-text round-trip to produce \"12345\" and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    // The native run above pins AArch64 on Apple Silicon. Also emit the same
    // operation through the x86-64 encoder and relocation path so the indexed
    // destination and converted source stay portable.
    let x64_scratch = scratch.join("linux-x64");
    compile_rooted_canary_for_target(&canary, x64_scratch.clone(), "linux_x86_64")
        .expect("number-to-decimal canary should cross-compile for linux_x64");
    let elf = fs::read(x64_scratch.join("omega-program"))
        .expect("number-to-decimal linux_x64 ELF emitted");
    assert_eq!(&elf[..4], b"\x7fELF");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_decimal_to_number_exit_canary_runs() {
    // Numeric input (atoi): parse the decimal text "12345" into the integer 12345 via
    // carrier byte reads + accumulation, and assert it. The read-side complement of
    // the itoa canary -- carrier reads used as arithmetic operands.
    let canary = pass_canary("text/runtime_decimal_to_number_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-decimal-to-number-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("decimal-to-number canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("decimal-to-number canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("decimal-to-number canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected decimal-text->integer parse of \"12345\" to yield 12345 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_indexed_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_write_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-carrier-indexed-write-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier indexed write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier indexed write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier indexed write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.out[self.i] = self.ch` (runtime index on a [u8;N] carrier, runtime value) to write bytes past the len prefix and read 2 back at index 2 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_indexed_read_operand_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_read_operand_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-carrier-read-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier indexed-read-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier indexed-read-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier indexed-read-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a carrier indexed read in operand position (`sum + self.text[self.i] as u32`, temp typed `u8` not `u8 in Utf8`) to sum 'ABCD' to 266 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_cipher_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_cipher_exit");
    let scratch = std::env::temp_dir().join(format!("omega-carrier-cipher-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier cipher canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier cipher canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier cipher canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a Caesar-cipher loop (read `text[i]` in an expression, Wrapping-shift, write `out[i]`) to map \"HELLO\" to \"KHOOR\" and read it back (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_indexed_const_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_const_write_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-carrier-const-write-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier const-write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier const-write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier const-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a CONSTANT byte written into a carrier at a RUNTIME index (`self.out[self.i] = 88`) to respect the index at both ends (out[0] and out[3]) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_len_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_len_guard_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-carrier-len-guard-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier len guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier len guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier len guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a carrier `.len` guard to actually evaluate (len==3 true, len==9 false; exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_fnv_loop_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_fnv_loop_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-carrier-fnv-loop-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier fnv loop canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier fnv loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier fnv loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected FNV-1a over a carrier string (`.len`-bounded loop + byte reads) to hash 'abc' to 11 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mandelbrot_render_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mandelbrot_render_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-mandelbrot-render-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mandelbrot render canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("mandelbrot render canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the 40x18 Mandelbrot renderer (f64 escape-time over a 1D carrier framebuffer) to produce 140 in-set pixels and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_crc32_exit_canary_runs() {
    // CRC-32 (the ZIP/PNG/Ethernet checksum): polynomial division over GF(2), bit by bit
    // with shifts + XOR (reflected poly 0xEDB88320, init/final 0xFFFFFFFF). CRC-32("abc")
    // is 891568578, verified against zlib -> exit 70.
    let canary = pass_canary("text/runtime_crc32_exit");
    let scratch = std::env::temp_dir().join(format!("omega-crc32-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("crc32 canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("crc32 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("crc32 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected CRC-32(\"abc\") == 891568578 (exit 70); got {:?} -- a shift/XOR or u32 regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_base64_encode_exit_canary_runs() {
    // Base64 encoding: three input bytes regrouped into four 6-bit values (shifts + masks +
    // OR), each indexing the 64-char alphabet. "Man" -> "TWFu", all four bytes checked ->
    // exit 70.
    let canary = pass_canary("text/runtime_base64_encode_exit");
    let scratch = std::env::temp_dir().join(format!("omega-base64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("base64 encode canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("base64 encode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("base64 encode canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected base64(\"Man\") == \"TWFu\" (exit 70); got {:?} -- a bit-op regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_run_length_encode_exit_canary_runs() {
    // Run-length encoding (compression): scan counting consecutive equal bytes, emit
    // byte+count at each run boundary and at the end (shared emit dispatched by a mode
    // field). "aaabbbbcc" -> "a3b4c2", six output bytes checked -> exit 70.
    let canary = pass_canary("text/runtime_run_length_encode_exit");
    let scratch = std::env::temp_dir().join(format!("omega-rle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("run-length encode canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("run-length encode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run-length encode canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected RLE of \"aaabbbbcc\" to be \"a3b4c2\" (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_binary_format_exit_canary_runs() {
    // Format a number as an 8-bit binary string: `(n >> (7-i)) & 1` per bit (runtime shift
    // amount + bitwise AND in value position), written to a carrier. 42 -> "00101010",
    // all eight bytes checked -> exit 70.
    let canary = pass_canary("text/runtime_binary_format_exit");
    let scratch = std::env::temp_dir().join(format!("omega-binary-format-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("binary format canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("binary format canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("binary format canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 42 to format as binary \"00101010\" (exit 70); got {:?} -- a shift/bitwise regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_substring_search_exit_canary_runs() {
    // Naive substring search (find a needle in a haystack): nested loop, carrier byte
    // comparison, the index guarded against `.len` directly. "world" in "hello world"
    // rejects i=0..5 and matches at i=6 -> exit 70.
    let canary = pass_canary("text/runtime_substring_search_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-substring-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("substring search canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("substring search canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("substring search canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected substring search to find \"world\" at position 6 (exit 70); got {:?} (a non-70 code is the wrong position)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_string_palindrome_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_palindrome_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-string-palindrome-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("string palindrome canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("string palindrome canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("string palindrome canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a two-pointer string palindrome check -- text[i] proved via the relational chain (i <= j < len), bytes compared through local temps -- to detect 'ABCBA' (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_carrier_itoa_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_itoa_exit");
    let scratch = std::env::temp_dir().join(format!("omega-carrier-itoa-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier itoa canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("carrier itoa canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `itoa` (computed digit chars written into a carrier) to render 150 as \"150\" and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier byte WRITE `self.buffer[i] = <byte>`: the byte
// stores inline at `base + pointer_size + i`. Both a byte literal (`buffer[0] = 67`
// = 'C') and a u8 field (`buffer[1] = self.ch` = 'D') work; from "AB" the writes
// yield "CD" -> `==` exits 70.
#[test]
fn runtime_carrier_byte_write_width_coercion_canary_runs() {
    // A carrier byte WRITE of a COMPUTED value coerces to the u8 byte width
    // (`buffer[0] = a+b` with a+b=300 stores the low byte 44), matching native.
    // The carrier (Value::Str) path is separate from the array element_cell path.
    // exit 71 = the byte was not the low-byte 44.
    let canary = pass_canary("text/runtime_carrier_byte_write_width_coercion");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("carrier byte-write coercion canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (carrier byte write coerces to u8), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-carrier-coerce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("carrier byte-write coercion canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier byte-write coercion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("carrier byte-write coercion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected carrier byte write to coerce to u8 low byte (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bounded_carrier_byte_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_byte_write_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-byte-write-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier byte write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier byte write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier byte write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexed byte writes (literal 'C' + u8-field 'D') to turn \"AB\" into \
         \"CD\" so `==` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A runtime slice `.len` (descriptor read of a slice PARAM, not a folded
// fixed-array constant) narrows into an i32 field: `self.count = s.len` where
// `s: &[i32]` -> exit 5 for a 5-element view. The length value is 32-bit, so its
// low 4-byte word lowers into the i32 target (an 8-byte read does not) -- the same
// width convention as carrier `.len`.
#[test]
fn runtime_slice_length_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_slice_length_field_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-slice-length-field-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("slice length field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("slice length field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("slice length field canary should run");

    assert_eq!(
        output.status.code(),
        Some(5),
        "expected `self.count = s.len` to store the slice param's length (5) into \
         the i32 field, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The RUNTIME-index guarded element increment: both the operand hoist
// (`__hoist_N + 1`) and the frontend-hoisted boolean guard subject are
// DE-HOISTED through their states' call-free initializers, so
// `tallies[self.k] < 16` proves `tallies[self.k] += 1`. -> 1.
