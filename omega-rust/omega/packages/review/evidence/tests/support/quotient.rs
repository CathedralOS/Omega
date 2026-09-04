pub(crate) fn public_quotient_source(
    carrier: &str,
    relation: &str,
    evidence: &str,
    reverse_relation: bool,
) -> String {
    let (
        relation_body,
        symmetric_requires,
        symmetric_ensures,
        transitive_requires,
        transitive_ensures,
    ) = if reverse_relation {
        ("b == a", "b == a", "a == b", "b == a\n    c == b", "c == a")
    } else {
        ("a == b", "a == b", "b == a", "a == b\n    b == c", "a == c")
    };
    format!(
        r#"use omega::language::core::relation;

pub data {carrier} {{
    case Zero;
    case Next(previous: {carrier});
}}

pub proposition {relation}(a: {carrier}, b: {carrier}) = {relation_body};

machine equivalent_reflexive(a: {carrier})
ensures a == a
{{
}}

machine equivalent_symmetric(a: {carrier}, b: {carrier})
requires {symmetric_requires}
ensures {symmetric_ensures}
{{
}}

machine equivalent_transitive(a: {carrier}, b: {carrier}, c: {carrier})
requires
    {transitive_requires}
ensures {transitive_ensures}
{{
}}

{evidence}: satisfies Equivalence<{carrier}, {relation}> {{
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}}

pub data EquivalenceClass = {carrier} % {relation}
where {relation} satisfies
    Equivalence<{carrier}, {relation}>
    as {evidence};
"#,
    )
}
