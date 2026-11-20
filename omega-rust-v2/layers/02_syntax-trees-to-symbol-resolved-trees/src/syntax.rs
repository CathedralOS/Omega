use crate::{NameError, StageSource};
use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;

pub(crate) use omega_resolved::IndexedSource as File;

pub(crate) fn index_source<'text>(
    ordinal: usize,
    input: &StageSource<'text, '_>,
) -> Result<File<'text>, NameError> {
    let source = &input.source;
    let bad = || NameError::new(input.package, source.logical_path, SourceSpan::new(0, 0), "inconsistent parsed source");
    let syntax = source.tree.index(source.text, source.tokens).ok_or_else(bad)?;
    let namespace = source
        .logical_path
        .rsplit_once('/')
        .map_or(String::new(), |(dir, _)| dir.replace('/', "\\"));
    Ok(File::new(ordinal, input.package, source.logical_path, namespace, syntax))
}

pub(crate) fn arm_has_subject(
    file: &File<'_>,
    arm: usize,
) -> bool {
    let dispatch = file.nodes[arm].parent.expect("arm belongs to dispatch");
    file.nodes[file.nodes[dispatch].children[0]].kind != NodeKind::Arm
}
