//! Explain the missing name recorded by typing using its own source's import.

use super::PendingBuildImport;
use crate::source_state::TypedTrees;
use source_text::SourceSpan;

impl PendingBuildImport {
    pub fn alias_at_refused_call<'a>(
        imports: &'a [Self],
        typed: &TypedTrees,
        span: SourceSpan,
    ) -> Option<&'a str> {
        let path = typed.deferred_refusals.iter().find_map(|refusal| {
            (refusal.source_span == span)
                .then_some(refusal.unresolved_call_path.as_deref())
                .flatten()
        })?;
        imports.iter().find_map(|import| {
            let mut occurrence = path.split("::");
            if import.source != span.source_id { return None; }
            match &import.target {
                crate::SourceImportTarget::File { root, alias, .. } => {
                    let name = occurrence.next()?.strip_prefix('|')?.strip_suffix('|')?;
                    (name == alias && occurrence.next().is_some()).then_some(root.as_deref()).flatten()
                }
                crate::SourceImportTarget::Namespace(members) => {
                    members.iter().all(|member| occurrence.next() == Some(member.as_str())).then(|| members.first().map(String::as_str)).flatten()
                }
            }
        })
    }
}
