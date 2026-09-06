use super::super::*;

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees(typed_source(source))
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn receiver_source(receiver: &str, result: &str, body: &str) -> String {
    format!(
        r#"
            data Inner {{ value: u16; values: [u16; 2]; }}
            data Record {{ prefix: u8; value: u16; inner: Inner; }}

            machine Record::exercise({receiver}, replacement: u16) {result} {{
                {body}
            }}
        "#
    )
}

fn reject_source(source: &str, expected: &str) {
    // A synthetic Self type rejection or a missing Terminal plan is not
    // evidence that the checker recognized observation through this receiver.
    let diagnostics = match check_source(source) {
        Ok(_) => panic!("receiver observation must fail checking: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected receiver diagnostic `{expected}`: {diagnostics:#?}\nsource:\n{source}"
    );
}

fn reject_field_observation(body: &str) {
    reject_source(
        &receiver_source("&write self", "", body),
        "reads field `value` from write-only parameter `self`; an eligible record-field path may be replaced as an assignment target, but write-only projection never grants observation",
    );
}

fn reject_index_observation(body: &str) {
    reject_source(
        &receiver_source("&write self", "", body),
        "reads through index projection of write-only parameter `self`; `&write` permits admitted fixed-array element replacement but never observation",
    );
}

#[test]
fn write_only_self_direct_scalar_store_checks() {
    check_source(&receiver_source("&write self", "", "self.value = 17;"))
        .expect("write-only self permits a direct scalar store in a plain record");
}

#[test]
fn write_only_self_nested_scalar_store_checks() {
    check_source(&receiver_source(
        "&write self",
        "",
        "self.inner.value = 17;",
    ))
    .expect("write-only self permits a nested scalar store in plain records");
}

#[test]
fn write_only_self_literal_index_scalar_store_checks() {
    check_source(&receiver_source(
        "&write self",
        "",
        "self.inner.values[1] = 17;",
    ))
    .expect("write-only self permits an in-bounds literal-index scalar store");
}

#[test]
fn write_only_self_scalar_parameter_rhs_checks() {
    for destination in ["self.value", "self.inner.value", "self.inner.values[1]"] {
        let source = receiver_source("&write self", "", &format!("{destination} = replacement;"));
        check_source(&source).unwrap_or_else(|diagnostics| {
            panic!("an independent scalar parameter may supply `{destination}`: {diagnostics:#?}")
        });
    }
}

#[test]
fn write_only_self_direct_prior_read_rejects() {
    reject_field_observation("let prior: u16 = self.value;");
}

#[test]
fn write_only_self_nested_prior_read_rejects() {
    reject_field_observation("let prior: u16 = self.inner.value;");
}

#[test]
fn write_only_self_literal_index_prior_read_rejects() {
    reject_index_observation("let prior: u16 = self.inner.values[1];");
}

#[test]
fn write_only_self_direct_compound_assignment_rejects() {
    reject_field_observation("self.value += 1;");
}

#[test]
fn write_only_self_nested_compound_assignment_rejects() {
    reject_field_observation("self.inner.value += 1;");
}

#[test]
fn write_only_self_literal_index_compound_assignment_rejects() {
    reject_index_observation("self.inner.values[1] += 1;");
}

#[test]
fn write_only_self_direct_read_after_write_rejects() {
    reject_field_observation("self.value = 17; let observed: u16 = self.value;");
}

#[test]
fn write_only_self_nested_read_after_write_rejects() {
    reject_field_observation("self.inner.value = 17; let observed: u16 = self.inner.value;");
}

#[test]
fn write_only_self_literal_index_read_after_write_rejects() {
    reject_index_observation(
        "self.inner.values[1] = 17; let observed: u16 = self.inner.values[1];",
    );
}

#[test]
fn write_only_self_prior_value_cannot_supply_store_rhs() {
    reject_field_observation("self.inner.value = self.value;");
    reject_field_observation("self.value = self.inner.value;");
    reject_index_observation("self.inner.values[0] = self.inner.values[1];");
}

#[test]
fn write_only_self_shared_reborrows_reject() {
    reject_field_observation("let readable: &u16 = &self.value;");
    reject_field_observation("let readable: &u16 = &self.inner.value;");
    reject_index_observation("let readable: &u16 = &self.inner.values[1];");
    reject_source(
        &receiver_source("&write self", "", "let readable: &Record = &self;"),
        "reads write-only parameter `self`; `&write` permits replacement or exact `&write` forwarding, never observation",
    );
}

#[test]
fn write_only_self_mutable_reborrows_reject() {
    for body in [
        "let readable: &mut u16 = &mut self.value;",
        "let readable: &mut u16 = &mut self.inner.value;",
        "let readable: &mut u16 = &mut self.inner.values[1];",
        "let readable: &mut Record = &mut self;",
    ] {
        reject_source(
            &receiver_source("&write self", "", body),
            "widens write-only parameter `self` to `&mut`; forward it explicitly as `&write self` instead",
        );
    }
}

#[test]
fn write_only_self_bare_observation_rejects() {
    reject_source(
        &receiver_source("&write self", "", "self;"),
        "reads write-only parameter `self`; `&write` permits replacement or exact `&write` forwarding, never observation",
    );
}

#[test]
fn write_only_self_return_rejects() {
    reject_source(
        &receiver_source("&write self", "-> Self", "self"),
        "reads write-only parameter `self`; `&write` permits replacement or exact `&write` forwarding, never observation",
    );
}

#[test]
fn write_only_self_observing_receiver_calls_reject() {
    for receiver in ["&self", "&mut self"] {
        let source = format!(
            "{}\nmachine Record::observe({receiver}) -> u16 {{ self.value }}",
            receiver_source("&write self", "", "let observed: u16 = self.observe();"),
        );
        reject_source(
            &source,
            "reads write-only parameter `self`; `&write` permits replacement or exact `&write` forwarding, never observation",
        );
    }
}

#[test]
fn write_only_self_observing_statement_call_rejects() {
    let source = format!(
        "{}\nmachine Record::observe(&self) {{ let prior: u16 = self.value; }}",
        receiver_source("&write self", "", "self.observe();"),
    );
    reject_source(&source, "calls through write-only parameter `self`");
}

#[test]
fn write_only_self_nested_observing_statement_call_rejects() {
    let source = format!(
        "{}\nmachine Inner::observe(&self) {{ let prior: u16 = self.value; }}",
        receiver_source("&write self", "", "self.inner.observe();"),
    );
    reject_source(&source, "calls through write-only parameter `self`");
}

#[test]
fn write_only_self_static_fixed_array_length_checks() {
    check_source(&receiver_source(
        "&write self",
        "",
        "let length: u64 = self.inner.values.len;",
    ))
    .expect("literal fixed-array length is static metadata, not receiver content");
}

#[test]
fn write_only_self_bare_attached_fields_cannot_be_observed() {
    for body in ["let prior: u16 = value;", "let prior: u16 = inner.value;"] {
        reject_source(
            &receiver_source("&write self", "", body),
            "write-only parameter `self`",
        );
    }
}

#[test]
fn write_only_self_bare_attached_fields_pass_access_validation() {
    for body in ["value = 17;", "inner.value = 17;", "inner.values[1] = 17;"] {
        validation::validate_program(&typed_source(&receiver_source("&write self", "", body)))
            .expect("bare fields have the same access rules; later member-selection binding is separate");
    }
    check_source(&receiver_source("&write self", "", "value = 17;"))
        .expect("a direct bare field store completes checking");
}

#[test]
fn write_only_self_state_transfer_cannot_restore_observation() {
    for receiver in ["&self", "&mut self", "self"] {
        reject_source(
            &format!(
                "data Record {{ value: u16; }}
                 machine Record::exercise(&write self) {{
                     transition {{ _ -> observe() }}
                     state observe({receiver}) {{ let prior: u16 = self.value; }}
                 }}"
            ),
            "widens write-only receiver",
        );
    }
}

#[test]
fn write_only_self_state_transfer_preserves_non_observing_access() {
    check_source(
        "data Record { value: u16; }
         machine Record::exercise(&write self) {
             transition { _ -> replace() }
             state replace(&write self) { self.value = 17; }
         }",
    )
    .expect("a state transfer may preserve write-only receiver access");
}

#[test]
fn shared_and_mutable_self_prior_reads_check() {
    for receiver in ["&self", "&mut self"] {
        for value in ["self.value", "self.inner.value", "self.inner.values[1]"] {
            check_source(&receiver_source(
                receiver,
                "",
                &format!("let prior: u16 = {value};"),
            ))
            .unwrap_or_else(|diagnostics| {
                panic!("{receiver} permits observation of `{value}`: {diagnostics:#?}")
            });
        }
    }
}

#[test]
fn mutable_self_read_after_write_checks() {
    for destination in ["self.value", "self.inner.value", "self.inner.values[1]"] {
        check_source(&receiver_source(
            "&mut self",
            "",
            &format!("{destination} = 17; let observed: u16 = {destination};"),
        ))
        .unwrap_or_else(|diagnostics| {
            panic!("mutable self retains read authority after `{destination}` is written: {diagnostics:#?}")
        });
    }
}
