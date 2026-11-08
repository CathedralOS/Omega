/*
    An authored declaration retained across independently allocated source contexts.

    This transfers an already selected declaration, never visibility, checking
    evidence or execution authority. Invocation owners must separately admit the
    destination source context. No numeric handle or display name crosses it.
*/

use crate::{SymbolHandle, SymbolKind, SymbolTable};
use source::{SourceFile, Span};

/*
    The symbols of one table that carry an authored source span, looked up by
    that span. Rejoining retained provider selections asks, for each
    selection, which symbols sit at an anchor's exact span; scanning the whole
    table per question made that quadratic in selections times symbols.

    GUARDED: a symbol's span is decided by the symbol arena and the name arena,
    so every lookup compares the table's address and both arenas' revisions and
    lengths with the ones the rows were built from, and rebuilds when any
    differ. Rows keep arena order within one span, the order of a table scan.
*/
#[derive(Debug, Default)]
pub struct SourceSpanCandidates {
    built_for: Option<(usize, u64, usize, u64, usize)>,
    rows: Vec<(usize, usize, SymbolHandle)>,
}

impl SourceSpanCandidates {
    /*
        Every symbol of `symbols` whose authored source span is exactly `span`
        (in any source), in arena order.
    */
    pub fn at_span(&mut self, symbols: &SymbolTable, span: Span) -> Vec<SymbolHandle> {
        let nodes = symbols.symbols().nodes();
        let names = symbols.names();
        let key = (
            std::ptr::from_ref(symbols) as usize,
            nodes.revision(),
            nodes.len(),
            names.revision(),
            names.len(),
        );
        if self.built_for != Some(key) {
            self.rows = nodes
                .iter()
                .filter_map(|(symbol, _)| {
                    let span = symbols.symbol_source_span(symbol)?.span;
                    Some((span.start, span.end, symbol))
                })
                .collect();
            self.rows.sort_by_key(|&(start, end, _)| (start, end));
            self.built_for = Some(key);
        }
        let first = self
            .rows
            .partition_point(|&(start, end, _)| (start, end) < (span.start, span.end));
        self.rows[first..]
            .iter()
            .take_while(|&&(start, end, _)| (start, end) == (span.start, span.end))
            .map(|&(_, _, symbol)| symbol)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedDeclaration {
    source: SourceFile,
    span: Span,
    kind: SymbolKind,
}

impl RetainedDeclaration {
    /*
        Recover an inert declaration anchor from retained source custody.
        This establishes only a well-formed coordinate. The receiving compiler
        must still admit the source and rejoin its checked declaration.
    */
    pub fn from_retained_parts(
        mut source: SourceFile,
        span: Span,
        kind: SymbolKind,
    ) -> Result<Self, &'static str> {
        if span.start >= span.end || source.source.get(span.start..span.end).is_none() {
            return Err("retained declaration has an invalid authored source coordinate");
        }
        source.source_id = source::SourceId(0);
        Ok(Self { source, span, kind })
    }

    /* The immutable source custody captured when this declaration was selected. */
    pub fn source(&self) -> &SourceFile {
        &self.source
    }

    /* The original declaration anchor, independent of any later source number. */
    pub const fn span(&self) -> Span {
        self.span
    }

    /* The declaration role retained beside its source anchor. */
    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn capture(symbols: &SymbolTable, selected: SymbolHandle) -> Result<Self, &'static str> {
        if !selected.is_valid() {
            return Err("selected declaration has no symbol");
        }
        let span = symbols
            .symbol_source_span(selected)
            .ok_or("selected declaration has no authored source coordinate")?;
        let source = symbols
            .source_file(span)
            .ok_or("selected declaration lost its source custody")?;
        Ok(Self {
            source: source.clone(),
            span: span.span,
            kind: symbols.get(selected).kind,
        })
    }

    /*
        Rejoin the exact authored occurrence among the destination's declarations.
        Source IDs and arena handles may change; source bytes, location, stratum,
        origin and checked dependency scope may not. An anonymous source may gain
        its final package owner, but an existing owner cannot be replaced.
    */
    pub fn rejoin(
        &self,
        symbols: &SymbolTable,
        candidates: impl IntoIterator<Item = SymbolHandle>,
    ) -> Result<SymbolHandle, &'static str> {
        let mut selected = SymbolHandle::invalid();
        for candidate in candidates {
            if !candidate.is_valid() || symbols.get(candidate).kind != self.kind {
                continue;
            }
            let Some(span) = symbols.symbol_source_span(candidate) else {
                continue;
            };
            if span.span != self.span {
                continue;
            }
            let Some(source) = symbols.source_file(span) else {
                continue;
            };
            if source.path != self.source.path
                || source.package_root != self.source.package_root
                || source.origin != self.source.origin
                || source.resolution_stratum != self.source.resolution_stratum
                || source.dependency_scope != self.source.dependency_scope
                || self
                    .source
                    .package_identity
                    .is_some_and(|owner| source.package_identity != Some(owner))
                || source.source != self.source.source
            {
                continue;
            }
            if selected.is_valid() && selected != candidate {
                return Err("retained declaration is ambiguous in the admitted source context");
            }
            selected = candidate;
        }
        if !selected.is_valid() {
            return Err("retained declaration is absent or changed in the admitted source context");
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolNameRef, SymbolTableBuilder};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn program(
        text: &str,
        owner: Option<PackageKeyIdentity>,
        scope: DependencyScope,
        prefix: bool,
    ) -> (SymbolTable, Vec<SymbolHandle>) {
        let mut sources = SourceMap::default();
        if prefix {
            sources.add(PathBuf::from("unrelated.omg"), "unrelated".to_owned());
        }
        let source = sources
            .add_checked_instance(
                PathBuf::from("package/main.omg"),
                text.to_owned(),
                PathBuf::from("package"),
                owner,
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                scope,
            )
            .source_id;
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        if prefix {
            symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("unrelated"));
        }
        let declarations = [Span::new(0, 6), Span::new(7, 13)].map(|span| {
            symbols.insert_root(
                SymbolKind::Machine,
                SymbolNameRef::Source(SourceSpan::new(source, span)),
            )
        });
        (symbols.finish(), declarations.to_vec())
    }

    #[test]
    fn retained_declaration_rejoins_source_identity_not_arena_or_file_number() {
        let (original, declarations) =
            program("launch launch", None, DependencyScope::Product, false);
        let selected = RetainedDeclaration::capture(&original, declarations[1]).unwrap();
        let owner = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let (destination, declarations) =
            program("launch launch", Some(owner), DependencyScope::Product, true);
        assert_eq!(
            selected
                .rejoin(&destination, declarations.iter().copied())
                .unwrap(),
            declarations[1]
        );
        assert!(selected.rejoin(&destination, [declarations[0]]).is_err());
    }

    #[test]
    fn retained_declaration_refuses_changed_source_or_checked_context() {
        let owner = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let (original, declarations) = program(
            "launch launch",
            Some(owner),
            DependencyScope::Product,
            false,
        );
        let selected = RetainedDeclaration::capture(&original, declarations[1]).unwrap();
        for (text, owner, scope) in [
            (
                "launch launch // changed",
                Some(owner),
                DependencyScope::Product,
            ),
            ("launch launch", None, DependencyScope::Product),
            (
                "launch launch",
                Some(PackageKeyIdentity::from_digest([8; 32]).unwrap()),
                DependencyScope::Product,
            ),
            ("launch launch", Some(owner), DependencyScope::Build),
        ] {
            let (destination, declarations) = program(text, owner, scope, false);
            assert!(selected.rejoin(&destination, declarations).is_err());
        }
    }

    #[test]
    fn retained_declaration_refuses_missing_source_and_ambiguous_occurrences() {
        let (original, declarations) =
            program("launch launch", None, DependencyScope::Product, false);
        assert!(RetainedDeclaration::capture(&original, SymbolHandle::invalid()).is_err());
        let selected = RetainedDeclaration::capture(&original, declarations[0]).unwrap();
        let mut symbols = SymbolTableBuilder::with_sources(original.source_map());
        let span = original.symbol_source_span(declarations[0]).unwrap();
        let candidates =
            [0, 1].map(|_| symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Source(span)));
        let symbols = symbols.finish();
        assert_eq!(
            selected.rejoin(&symbols, candidates).unwrap_err(),
            "retained declaration is ambiguous in the admitted source context"
        );
    }
}
