use omega_psi::{DeclarationKind, PredicateStep, Site, SourceSpan};

pub fn site(file: &str, source: &str, text: &str) -> Site {
    let span = super::sources::unique_span(source, text);
    Site { package: "app".into(), file: file.into(), span: SourceSpan::new(span.start, span.end) }
}

/// Build the expected obligation from the authorized interface and authored
/// occurrence, independently of the lowering return and its diagnostic sites.
pub fn read(file: &str, source: &str, text: &str) -> PredicateStep {
    let read = omega_toolchain::core_interface().collection(omega_toolchain::CollectionPrimitive::Read);
    PredicateStep::FixedArrayRead {
        callee: read.try_map_kind(&|k| (*k == omega_syntax::NodeKind::Machine).then_some(DeclarationKind::Machine)).unwrap(),
        site: site(file, source, text),
    }
}
