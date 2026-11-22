//! Most published examples mix declarations and statements. The fences listed here
//! demonstrate standalone clauses. The parser corpus test checks each label.
use omega_parser::FragmentKind;

pub const EXAMPLE_SHAPES: &[(&str, FragmentKind)] = &[
    ("language/generics.md:312", FragmentKind::Clauses),
    ("language/termination.md:35", FragmentKind::Clauses),
    ("resources/content_custody.md:129", FragmentKind::Clauses),
];

pub fn fragment_kind(label: &str) -> FragmentKind {
    EXAMPLE_SHAPES
        .iter()
        .find(|(shaped_label, _)| *shaped_label == label)
        .map_or(FragmentKind::Statements, |&(_, kind)| kind)
}

/// Keep source offsets when a spec example leaves out the contents of a body.
pub fn parser_source(source: &str) -> String {
    source.replace("{ ... }", "{     }")
}
