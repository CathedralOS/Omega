use super::*;

fn checked_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    lower_typed_trees(typed)
}

fn completion_program(provider_parameter: &str, provider_clauses: &str) -> String {
    format!(
        r#"
        boundary trait MachineControl {{}}
        boundary trait PortIo {{}}
        boundary trait FilesystemHost {{}}

        pub data InterruptAcknowledgement [copy] {{
            token: u64;
        }}
        pub domain InterruptAcknowledgement::Pending;

        pub data OtherAcknowledgement [copy] {{
            token: u64;
        }}

        pub boundary requirement InterruptAcknowledgement::complete(self)
        reaches <= MachineControl + PortIo
        requires
            self in InterruptAcknowledgement::Pending;

        machine LapicCompletion::complete({provider_parameter})
        satisfies InterruptAcknowledgement::complete
        {provider_clauses}
        {{
        }}
        "#,
    )
}

fn restore_program(provider_parameter: &str, provider_reach: &str) -> String {
    format!(
        r#"
        boundary trait MachineControl {{}}
        boundary trait PortIo {{}}

        pub data InterruptMaskGuard [copy] {{
            token: u64;
        }}
        pub domain InterruptMaskGuard::Active;

        pub boundary requirement InterruptMaskGuard::restore(self)
        reaches MachineControl
        requires
            self in InterruptMaskGuard::Active;

        machine CpuMask::restore({provider_parameter})
        satisfies InterruptMaskGuard::restore
        reaches {provider_reach}
        {{
        }}
        "#,
    )
}

fn restore_call_program(caller_parameter: &str) -> String {
    format!(
        r#"
        boundary trait MachineControl {{}}

        pub data InterruptMaskGuard [copy] {{
            token: u64;
        }}
        pub domain InterruptMaskGuard::Active;

        pub boundary requirement InterruptMaskGuard::restore(self)
        reaches MachineControl
        requires
            self in InterruptMaskGuard::Active;

        machine misuse({caller_parameter})
        reaches MachineControl
        {{
            guard.restore();
        }}
        "#,
    )
}

fn diagnostic_text(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn top_level_requirement_maps_self_to_one_explicit_exact_carrier_parameter() {
    let checked = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending",
        "reaches MachineControl",
    ))
    .expect("the checked provider should refine the exact top-level requirement");

    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "LapicCompletion::complete")
        .expect("checked completion provider");
    let [conformance] = checked.typed.machine_trait_conformances(provider) else {
        panic!("one exact top-level requirement edge")
    };
    let requirement = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == conformance.requirement_symbol)
        .expect("exact retained top-level requirement");
    assert_eq!(
        requirement.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    assert_eq!(conformance.symbol, requirement.symbol);
    assert_eq!(conformance.requirement_symbol, requirement.symbol);
}

#[test]
fn fixed_reach_restore_requirement_rejects_a_call_without_active_custody() {
    let diagnostics = checked_source(&restore_call_program("guard: InterruptMaskGuard"))
        .expect_err("restore requires exact Active custody at the call site");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("call restore") && text.contains("InterruptMaskGuard::Active"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn fixed_reach_restore_requirement_rejects_provider_reach_drift() {
    let diagnostics = checked_source(&restore_program(
        "guard: InterruptMaskGuard in Active",
        "PortIo",
    ))
    .expect_err("the restore provider must stay within the fixed MachineControl reach");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptMaskGuard::restore")
            && text.contains("PortIo")
            && text.contains("reach"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_a_different_explicit_carrier_type() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: OtherAcknowledgement",
        "reaches MachineControl",
    ))
    .expect_err("the requirement receiver binds the exact declared carrier");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete")
            && text.contains("OtherAcknowledgement"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_an_extra_provider_parameter() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending, extra: u64",
        "reaches MachineControl",
    ))
    .expect_err("the provider entry must preserve exact requirement arity");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete")
            && text.contains("parameter")
            && text.contains("2"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_provider_reach_outside_its_bound() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending",
        "reaches FilesystemHost",
    ))
    .expect_err("provider reach must remain within the requirement upper bound");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("FilesystemHost")
            && text.contains("InterruptAcknowledgement::complete")
            && text.contains("reach"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_provider_suspension_beyond_its_ceiling() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending",
        "reaches MachineControl\nsuspends;",
    ))
    .expect_err("provider suspension must be published by the requirement");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete") && text.contains("suspend"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_a_stronger_provider_precondition() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending",
        "reaches MachineControl\nrequires acknowledgement.token == 0;",
    ))
    .expect_err("a provider cannot add a precondition absent from the requirement");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete")
            && text.contains("requires")
            && text.contains("conservative refinement"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_a_missing_provider_guarantee() {
    let source = r#"
        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub domain InterruptAcknowledgement::Pending;

        pub boundary requirement InterruptAcknowledgement::complete(self)
        ensures
            self in InterruptAcknowledgement::Pending;

        machine LapicCompletion::complete(
            acknowledgement: InterruptAcknowledgement in Pending
        )
        satisfies InterruptAcknowledgement::complete
        {
        }
    "#;
    let diagnostics = checked_source(source)
        .expect_err("a provider must publish every guarantee owned by the requirement");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete")
            && text.contains("ensures")
            && text.contains("conservative refinement"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_a_provider_crash_outside_its_ceiling() {
    let diagnostics = checked_source(&completion_program(
        "acknowledgement: InterruptAcknowledgement in Pending",
        "reaches MachineControl\ncrashes Abort",
    ))
    .expect_err("a provider cannot add a crash route absent from the requirement");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("InterruptAcknowledgement::complete")
            && text.contains("crashes")
            && text.contains("not contained"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_rejects_a_different_result_type() {
    let source = r#"
        pub data Buffer [copy] { value: u64; }
        pub boundary requirement Buffer::read(buffer: Buffer) -> u64;

        machine SoftwareBuffer::read(buffer: Buffer) -> i64
        satisfies Buffer::read
        {
            0i64
        }
    "#;
    let diagnostics = checked_source(source)
        .expect_err("the provider result must preserve the exact requirement result");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("Buffer::read") && text.contains("returns") && text.contains("u64"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn top_level_requirement_preserves_a_generic_static_telescope() {
    let source = r#"
        pub data Buffer [copy] { value: u64; }
        pub boundary requirement Buffer::identity<T>(buffer: Buffer, value: T) -> T;

        machine SoftwareBuffer::identity<T>(buffer: Buffer, value: T) -> T
        satisfies Buffer::identity
        {
            value
        }
    "#;
    checked_source(source)
        .expect("the provider should preserve the requirement's generic static telescope");
}
