//! Which laws an open-index algebra selection records
//! ([contracts.md](../../../../../wiki/spec/proofs/contracts.md#licensed-normalization)).
//!
//! Reordering operands consumes commutativity and reassociating a chain
//! consumes associativity; "one does not imply another". A selection is
//! therefore recorded as soon as the carrier trait declares either law, and
//! it carries both answers so each consumer can consult the law for the
//! rewrite it is about to perform. With neither law no selection is recorded
//! at all -- the operation stays usable, the rewrites stay unauthorized.
//!
//! What each recorded law then licenses is pinned where the rewrites happen,
//! in `typed-trees`' `type_identity` tests.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

/// The law slots a case declares on the carrier trait.
#[derive(Clone, Copy)]
struct Laws {
    commutativity: bool,
    associativity: bool,
}

const NEITHER: Laws = Laws {
    commutativity: false,
    associativity: false,
};
const COMMUTATIVITY_ONLY: Laws = Laws {
    commutativity: true,
    associativity: false,
};
const ASSOCIATIVITY_ONLY: Laws = Laws {
    commutativity: false,
    associativity: true,
};
const BOTH: Laws = Laws {
    commutativity: true,
    associativity: true,
};

/// Normalizes one authored index expression under a carrier trait declaring
/// exactly `laws`, and reports the `(commutativity, associativity)` pair
/// recorded for every operation the selection covered.
fn recorded_laws(laws: Laws, index_expression: &str) -> Vec<(bool, bool)> {
    let law_requirements = format!(
        "{}{}",
        if laws.commutativity {
            "
            machine add_comm(a: Operand, b: Operand)
            ensures add(a, b) == add(b, a);"
        } else {
            ""
        },
        if laws.associativity {
            "
            machine add_assoc(a: Operand, b: Operand, c: Operand)
            ensures add(add(a, b), c) == add(a, add(b, c));"
        } else {
            ""
        },
    );
    let law_satisfiers = format!(
        "{}{}",
        if laws.commutativity {
            "
        machine plus_index_comm(a: u64, b: u64) -> u64
        satisfies IndexAlgebra<u64>::add_comm as Canonical
        requires plus_index(a, b) == plus_index(b, a)
        ensures plus_index(a, b) == plus_index(b, a)
        { 0 }"
        } else {
            ""
        },
        if laws.associativity {
            "
        machine plus_index_assoc(a: u64, b: u64, c: u64) -> u64
        satisfies IndexAlgebra<u64>::add_assoc as Canonical
        requires plus_index(plus_index(a, b), c) == plus_index(a, plus_index(b, c))
        ensures plus_index(plus_index(a, b), c) == plus_index(a, plus_index(b, c))
        { 0 }"
        } else {
            ""
        },
    );
    let source = format!(
        r#"
        domain<T, const I: u64> T::Indexed<I>;

        trait IndexAlgebra<Operand> {{
            operator + add(a: Operand, b: Operand) -> Operand;{law_requirements}
        }}

        machine plus_index(a: u64, b: u64) -> u64
        satisfies IndexAlgebra<u64>::add as Canonical
        {{ 0 }}
{law_satisfiers}

        boundary machine admitted<T, const A: u64, const B: u64, const C: u64, Canonical: T satisfies IndexAlgebra<u64>>()
            -> i64 in Indexed<{index_expression}>;
        "#
    );

    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let mut program: TypedTrees = lower_symbol_resolved_trees(&resolved).expect("lower");
    validation::normalize_open_index_expressions(&mut program)
        .expect("a selected index algebra should normalize");

    program
        .open_index_normalizations
        .iter()
        .flat_map(|normalization| &normalization.operations)
        .map(|operation| {
            (
                operation.commutativity_licensed,
                operation.associativity_licensed,
            )
        })
        .collect()
}

#[test]
fn a_commutativity_only_carrier_still_selects_its_algebra() {
    assert_eq!(
        recorded_laws(COMMUTATIVITY_ONLY, "A + B"),
        vec![(true, false)],
        "a declared commutativity law selects the algebra on its own, and the \
         absent associativity is recorded as absent rather than assumed"
    );
}

#[test]
fn an_associativity_only_carrier_still_selects_its_algebra() {
    assert_eq!(
        recorded_laws(ASSOCIATIVITY_ONLY, "(A + B) + C"),
        vec![(false, true); 2],
        "a declared associativity law selects the algebra on its own, for the \
         whole chain, with the absent commutativity recorded as absent"
    );
}

#[test]
fn a_carrier_declaring_both_laws_records_both() {
    assert_eq!(recorded_laws(BOTH, "(A + B) + C"), vec![(true, true); 2]);
}

#[test]
fn a_carrier_declaring_no_law_selects_no_algebra() {
    assert!(
        recorded_laws(NEITHER, "(A + B) + C").is_empty(),
        "the bound supplies the operation, but with no law slot there is \
         nothing to license a rewrite, so no selection is recorded"
    );
}
