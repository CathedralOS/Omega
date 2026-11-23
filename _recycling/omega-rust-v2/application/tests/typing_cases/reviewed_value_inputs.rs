use super::{Case, check};

fn twin(
    rule: &str,
    file: &str,
    good: &str,
    bad: &str,
    occurrence: &str,
    reason: &str,
) {
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[(file, good)],
        bad: &[(file, bad)],
        file,
        occurrence,
        reason,
        rule,
    });
}

// expressions.md, Range endpoints: the omitted end needs the live length.
// An authored bracket machine takes two endpoints; B supplies no extent.
#[test]
fn collection_access_index_operand_missing_end() {
    let good = "data B{n:u8;} machine [..] between(b:@B,start:u64,end:u64)->u8{b.n}
        machine f(b:B)->u8{b[0..2]}";
    twin(
        "Collection access",
        "range.omg",
        good,
        &good.replace("0..2", "0.."),
        "0..",
        "open range needs a collection with a known live extent",
    );
}

// mathematical_bindings.md: a citation in a field initializer must admit
// every input under its exact binder. L3 leaves the guarantee as an obligation.
fn citation(parameters: &str) {
    let source = |params: &str| {
        format!(
            r"let reflexive(x:u8):core\Strict<0> = x==x;
        machine lemma({params}) ensures y==y {{}}
        data Bundle{{evidence:(x:u8)->reflexive(x);}}
        machine bundle()->Bundle{{Bundle{{evidence:lemma}}}}"
        )
    };
    twin(
        "Mathematical terms and assembly",
        "citation.omg",
        &source("y:u8"),
        &source(parameters),
        "lemma",
        "mathematical citation has incompatible parameters",
    );
}

#[test]
fn mathematical_terms_mathematical_citation_exhausted_arrow() {
    citation("y:u8,z:u8");
}

#[test]
fn mathematical_terms_mathematical_citation_wrong_input_carrier() {
    citation("y:u16");
}

// patterns.md, Data and case decomposition: even identical fields belong
// to their declaring record. This tests the record head, not a sum case.
#[test]
fn patterns_and_places_bind_pattern_at_wrong_record_head() {
    let good = "data D{flag:bool;} data E{flag:bool;}
        machine f(d:@D)->bool{match d {D{flag:true}->true,_->false,}}";
    twin(
        "Patterns and places",
        "record.omg",
        good,
        &good.replace("D{flag:true}", "E{flag:true}"),
        "E",
        "pattern does not match the subject type",
    );
}

// patterns.md, Irrefutable data bindings: the unknown u8 need not equal 3.
// The initializer is present in both twins (not the uninitialized-let check).
#[test]
fn patterns_and_places_check_binding_refutable_initialized_let() {
    let good = "machine pick(n:u8)->u8{let (a, b) = (5u8, n); a}";
    twin(
        "Patterns and places",
        "binding.omg",
        good,
        &good.replace("(a, b)", "(a, 3u8)"),
        "(a, 3u8)",
        "let requires an irrefutable pattern",
    );
}

// ownership.md: a move transfers a value. Unit completion of a resultless
// call is valid as a statement, but provides no value to transfer.
#[test]
fn patterns_and_places_type_operand_node_move_resultless_call() {
    let good = "machine nothing(){} machine f(){nothing();}";
    twin(
        "Patterns and places",
        "move.omg",
        good,
        &good.replace("{nothing();}", "{move nothing();}"),
        "move nothing()",
        "move requires a value",
    );
}
