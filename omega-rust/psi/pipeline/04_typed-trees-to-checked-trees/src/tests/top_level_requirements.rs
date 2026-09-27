use crate::tests::front_end::checked_program_result;

fn checked_source(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
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

fn diagnostic_text(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
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
fn top_level_requirement_external_leaf_satisfier_states_no_contract_of_its_own() {
    // A compiler-intrinsic (or other `via`) satisfier is an external leaf:
    // the requirement's contract stays the public contract and the binding's
    // sealed catalog carries the realization, exactly as for a boundary
    // operator satisfier. Only a checked body refines the contract.
    let source = r#"
        pub data Negation [copy] {}
        pub boundary requirement Negation::flip(value: f32) -> f32
        ensures result == result;

        pub data NegationProvider {}
        machine NegationProvider::flip32(value: f32) -> f32
            satisfies Negation::flip
            via ForeignBinding::CompilerIntrinsic;
    "#;
    checked_source(source).expect("an external leaf satisfier restates no guarantee");
    let checked_body = source.replace(
        "via ForeignBinding::CompilerIntrinsic;",
        "{ transition { _ -> (value) } }",
    );
    let diagnostics = checked_source(&checked_body)
        .expect_err("a checked body without the guarantee is not a refinement");
    let text = diagnostic_text(&diagnostics);
    assert!(
        text.contains("Negation::flip") && text.contains("conservative refinement"),
        "unexpected diagnostics: {text}"
    );
}

#[test]
fn undischarged_linear_self_rejects_like_an_ordinary_parameter() {
    // Normal completion consumes neither a linear receiver nor an affine one:
    // an owned `self` that reaches a checked body's exit undischarged rejects
    // with the same diagnostic as the equivalent ordinary parameter.
    let sources = [
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
    "#,
        r#"
        data Receipt [linear] { code: i32; }
        machine ack(receipt: Receipt) {}
    "#,
    ];
    for source in sources {
        let text = diagnostic_text(
            &checked_source(source).expect_err("undischarged linear custody rejects at scope exit"),
        );
        assert!(
            text.contains("reaches scope exit without being consumed or transferred"),
            "unexpected diagnostics: {text}"
        );
    }
}
