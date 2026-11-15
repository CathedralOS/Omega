/*
    Target-neutral source parsing, resolution, typing, and preparation. This
    facade supplies frozen typed input to the lowered-form checker; it produces
    no source-checker judgment or selected product cache.
*/

/*
    Separating target-neutral Psi stage sequencing from Omega package discovery,
    Build host execution, and native code generation ensures every compile-time
    and product consumer traverses the exact same lexical, typing, checking, and
    portable Terminal production rules without circular coordinator dependencies.
*/

/*
    Start at `source_parsing` (below) for admitted source frontiers and snippet
    parsing, `source_compilation` (`SourceCompilationRequest`,
    `SourceContinuation`, and `CompletedSource`) for the resolver/typing
    continuation and Product policy admission, `source_checking`
    (`SourcePreparationRequest` and `FrozenSource`) before lowered-form
    checking. Accepted source publication and artifact retention belong to
    `terminal-production`.
*/

#![forbid(unsafe_code)]

mod product_descriptions;
mod root_bindings;
mod source_checking;
mod source_compilation;
pub(crate) mod source_parsing {
    mod batches {
        use arena::Arena;
        use diagnostics::Diagnostic;
        use rayon::prelude::*;
        use source::{LoadedSource, LoadedSources, SourcePosition};
        use syntax_trees::{ParsedSource, ParsedSources, SyntaxTrees};

        struct ParsedPageSources {
            sources: Vec<ParsedSource>,
            lexical_error: Option<Diagnostic>,
            parse_error: Option<Diagnostic>,
        }

        pub(super) fn parse(
            mut loaded: LoadedSources,
            first_page: u32,
            independent_units: bool,
        ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
            let sources = loaded.sources.span_mut_or_empty(loaded.batch);
            let count = sources.len();
            let desired_pages = if independent_units {
                count.max(1)
            } else {
                count
                    .min(execution_admission::current_workers().saturating_mul(4))
                    .max(1)
            };
            let chunk_size = count.max(1).div_ceil(desired_pages);
            let page_count = count.div_ceil(chunk_size);
            if page_count != 0
                && (first_page == 0
                    || u32::try_from(page_count - 1)
                        .ok()
                        .and_then(|last| first_page.checked_add(last))
                        .is_none())
            {
                return Err(vec![Diagnostic::error(
                    "parser exhausted its source-page directory",
                )]);
            }
            let mut trees: Vec<_> = sources
                .chunks(chunk_size)
                .enumerate()
                .map(|(index, chunk)| {
                    SyntaxTrees::new_with_generation(chunk[0].source_id, first_page + index as u32)
                })
                .collect();
            let mut pages = Vec::with_capacity(if page_count == 1 { 0 } else { page_count });
            let mut single_page_sources = execution_admission::within_budget(|| {
                if let [syntax] = trees.as_mut_slice() {
                    Some(parse_page(sources, syntax))
                } else {
                    sources
                        .par_chunks_mut(chunk_size)
                        .zip_eq(trees.par_iter_mut())
                        .map(|(sources, syntax)| parse_page(sources, syntax))
                        .collect_into_vec(&mut pages);
                    None
                }
            });
            for page in single_page_sources.iter_mut().chain(&mut pages) {
                if let Some(error) = page.lexical_error.take() {
                    return Err(vec![error]);
                }
            }
            for page in single_page_sources.iter_mut().chain(&mut pages) {
                if let Some(error) = page.parse_error.take() {
                    return Err(vec![error]);
                }
            }
            let mut parsed = Arena::with_capacity(count);
            let batch = parsed.insert_many(
                single_page_sources
                    .into_iter()
                    .chain(pages)
                    .flat_map(|page| page.sources),
            );
            Ok((
                trees,
                ParsedSources {
                    sources: parsed,
                    batch,
                },
            ))
        }

        fn parse_page(sources: &mut [LoadedSource], syntax: &mut SyntaxTrees) -> ParsedPageSources {
            let mut page = ParsedPageSources {
                sources: Vec::with_capacity(sources.len()),
                lexical_error: None,
                parse_error: None,
            };
            let mut scratch = tokens::TokenScratch::default();
            for source in sources {
                let tokens = match source_files_to_tokens::Lexer::new(&source.source)
                    .tokenize_reusing(&mut scratch)
                {
                    Ok(tokens) => tokens.retain_source(source.source.clone()),
                    Err(error) => {
                        page.lexical_error =
                            Some(located_error(source, error.span.start, error.message));
                        break;
                    }
                };
                if page.parse_error.is_none() {
                    match tokens_to_syntax_trees::parse(syntax, source.source_id, &tokens) {
                        Ok(roots) => page.sources.push(ParsedSource {
                            source_id: source.source_id,
                            path: std::mem::take(&mut source.path),
                            source: source.source.clone(),
                            origin: source.origin,
                            root_items: roots.items,
                            import_root_range: roots.import_range,
                            scope: source.scope,
                        }),
                        Err(error) => {
                            page.parse_error = Some(located_error(
                                source,
                                error.source_span.span.start,
                                error.message,
                            ))
                        }
                    }
                }
                tokens.recycle(&mut scratch);
            }
            page
        }

        fn located_error(source: &LoadedSource, start: usize, message: String) -> Diagnostic {
            let position = SourcePosition::of(&source.source, start);
            Diagnostic::error(format!(
                "{}:{}:{}: {}",
                source.path.display(),
                position.line,
                position.column,
                message
            ))
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use source::SourceId;
            use std::sync::Arc;

            fn loaded(texts: &[&str]) -> LoadedSources {
                texts
                    .iter()
                    .enumerate()
                    .map(|(index, text)| LoadedSource {
                        source_id: SourceId(index),
                        path: format!("source-{index}.omg").into(),
                        source: Arc::from(*text),
                        ..Default::default()
                    })
                    .collect()
            }

            #[test]
            fn fused_batch_preserves_lexical_error_priority_and_source_order() {
                for texts in [
                    vec!["machine", "\"unterminated"],
                    vec!["machine", "data Good {}", "\"unterminated"],
                ] {
                    let input = loaded(&texts);
                    let expected = source_files_to_tokens::lex_sources(input.clone()).unwrap_err();
                    let actual = parse(input, 1, false).unwrap_err();
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                }
                let mut same_page = loaded(&["machine", "\"unterminated"]);
                let mut syntax = SyntaxTrees::new_with_generation(SourceId(0), 1);
                let page = parse_page(
                    same_page.sources.span_mut_or_empty(same_page.batch),
                    &mut syntax,
                );
                assert!(page.lexical_error.is_some());
                assert!(page.parse_error.is_some());
                let input = loaded(&["machine", "data"]);
                let lexed = source_files_to_tokens::lex_sources(input.clone()).unwrap();
                let expected = tokens_to_syntax_trees::parse_source_pages_from(lexed, 1)
                    .err()
                    .unwrap();
                let actual = parse(input, 1, false).unwrap_err();
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            }

            #[test]
            fn fused_batch_matches_page_identity_metadata_and_owned_literals() {
                let texts = (0..97)
                    .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
                    .collect::<Vec<_>>();
                let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
                let lexed = source_files_to_tokens::lex_sources(input.clone()).unwrap();
                let (expected_pages, expected_sources) =
                    tokens_to_syntax_trees::parse_source_pages_from(lexed, 5)
                        .unwrap()
                        .into_parts();
                let (actual_pages, actual_sources) = parse(input, 5, false).unwrap();
                assert_eq!(
                    format!("{actual_sources:?}"),
                    format!("{expected_sources:?}")
                );
                assert_eq!(actual_pages.len(), expected_pages.len());
                for (actual, expected) in actual_pages.iter().zip(&expected_pages) {
                    assert_eq!(actual.snapshot(), expected.snapshot());
                }
                let units = parse(loaded(&["data First {}", "data Second {}"]), 1, true)
                    .unwrap()
                    .0;
                assert_eq!(units.len(), 2);
                assert_eq!(units[0].root_item_handles()[0].generation(), 1);
                assert_eq!(units[1].root_item_handles()[0].generation(), 2);
            }
        }
    }

    use crate::{PsiCompilationObserver, PsiCompilationStage};
    use arena::Arena;
    use diagnostics::Diagnostic;
    use source::{DependencyScope, LoadedSource, LoadedSources, SourceContents, SourceFileSystem};
    use std::path::PathBuf;
    use syntax_trees::{ParsedSource, ParsedSources, ParsedUnit, SyntaxTrees};

    pub fn load_source_files(
        frontier: Vec<(PathBuf, DependencyScope)>,
        filesystem: &SourceFileSystem,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
        observer.record_result(PsiCompilationStage::SourceParsing, || {
            source_files_to_tokens::load_sources(frontier, filesystem)
        })
    }

    pub struct SourceInput(pub LoadedSources);

    impl SourceInput {
        pub fn append(
            self,
            syntax: &mut SyntaxTrees,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<ParsedSources, Vec<Diagnostic>> {
            let first_page = u32::try_from(syntax.page_count())
                .ok()
                .and_then(|identity| identity.checked_add(1))
                .ok_or_else(|| vec![Diagnostic::error("syntax page identity overflow")])?;
            let (pages, sources) = self.parse_batch(first_page, false, observer)?;
            for page in pages {
                syntax.adopt_page(page);
            }
            Ok(sources)
        }

        pub fn into_units(
            self,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<Vec<SyntaxTrees>, Vec<Diagnostic>> {
            self.parse_batch(1, true, observer).map(|(pages, _)| pages)
        }

        /// Parses each source into its own standalone page and retains it
        /// with its root items, so a later admission of the same bytes can
        /// adopt the parse through `append_reusing`.
        pub fn into_parsed_units(
            self,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<Vec<ParsedUnit>, Vec<Diagnostic>> {
            let (pages, mut sources) = self.parse_batch(1, true, observer)?;
            let parsed = sources.sources.span_mut_or_empty(sources.batch);
            if parsed.len() != pages.len() {
                return Err(vec![Diagnostic::error(
                    "independent parse lost the page of a source",
                )]);
            }
            Ok(pages
                .into_iter()
                .zip(parsed)
                .map(|(syntax, source)| ParsedUnit {
                    syntax,
                    source: source.source.clone(),
                    root_items: std::mem::take(&mut source.root_items),
                    import_root_range: source.import_root_range.clone(),
                })
                .collect())
        }

        /// Appends these admitted sources to `syntax` as `append` does, except
        /// that a source for which `retained` supplies the parse of its exact
        /// bytes adopts that parse instead of being lexed and parsed again.
        /// The result lists every source in input order either way.
        pub fn append_reusing<'unit>(
            self,
            syntax: &mut SyntaxTrees,
            observer: &mut impl PsiCompilationObserver,
            mut retained: impl FnMut(&LoadedSource) -> Option<&'unit ParsedUnit>,
        ) -> Result<ParsedSources, Vec<Diagnostic>> {
            let loaded = self.0.sources.span_or_empty(self.0.batch);
            let units = loaded
                .iter()
                .map(|source| retained(source).filter(|unit| unit.source == source.source))
                .collect::<Vec<_>>();
            let fresh = Self(
                loaded
                    .iter()
                    .zip(&units)
                    .filter(|(_, unit)| unit.is_none())
                    .map(|(source, _)| source.clone())
                    .collect(),
            )
            .append(syntax, observer)?;
            let mut fresh = fresh.sources.span_or_empty(fresh.batch).iter().cloned();
            let mut sources = Arena::with_capacity(loaded.len());
            let mut ordered = Vec::with_capacity(loaded.len());
            for (source, unit) in loaded.iter().zip(units) {
                ordered.push(match unit {
                    Some(unit) => ParsedSource {
                        source_id: source.source_id,
                        path: source.path.clone(),
                        source: source.source.clone(),
                        origin: source.origin,
                        root_items: syntax
                            .adopt_unit(unit, source.source_id)
                            .map_err(|message| vec![Diagnostic::error(message)])?,
                        import_root_range: unit.import_root_range.clone(),
                        scope: source.scope,
                    },
                    None => fresh
                        .next()
                        .ok_or_else(|| vec![Diagnostic::error("parser lost an admitted source")])?,
                });
            }
            let batch = sources.insert_many(ordered);
            Ok(ParsedSources { sources, batch })
        }

        fn parse_batch(
            self,
            first_page: u32,
            independent_units: bool,
            observer: &mut impl PsiCompilationObserver,
        ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
            observer.record_result(PsiCompilationStage::SourceParsing, || {
                batches::parse(self.0, first_page, independent_units)
            })
        }
    }

    #[cfg(test)]
    mod reuse_tests {
        use super::*;
        use crate::UnmeasuredProduction;
        use source::SourceId;
        use std::sync::Arc;

        fn core_sources() -> Vec<(PathBuf, Arc<str>)> {
            let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../source/library/core");
            let mut paths = std::fs::read_dir(&core)
                .expect("bundled core is readable")
                .map(|entry| entry.expect("core entry").path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
                .collect::<Vec<_>>();
            paths.sort();
            paths
                .into_iter()
                .map(|path| {
                    let text = std::fs::read_to_string(&path).expect("core source is UTF-8");
                    (path, Arc::from(text))
                })
                .collect()
        }

        fn loaded(source_id: usize, path: &std::path::Path, source: &Arc<str>) -> LoadedSource {
            LoadedSource {
                source_id: SourceId(source_id),
                path: path.to_path_buf(),
                source: source.clone(),
                ..Default::default()
            }
        }

        // Discovery parses every unit under private coordinates; admission
        // gives each file a different source ID and a later page. The adopted
        // parse must be exactly the forest and roots a fresh parse of the
        // same bytes would have produced there, for every core construct.
        #[test]
        fn adopted_discovery_parse_equals_a_fresh_parse_at_admitted_coordinates() {
            let sources = core_sources();
            assert!(sources.len() > 20, "core sources were found");
            let units = SourceInput(
                sources
                    .iter()
                    .enumerate()
                    .map(|(index, (path, text))| loaded(usize::MAX - index, path, text))
                    .collect(),
            )
            .into_parsed_units(&mut UnmeasuredProduction)
            .expect("core parses");
            let prefix = Arc::from("data Prefix {}");
            let mut fresh = SyntaxTrees::new(SourceId(500));
            let mut adopted = SyntaxTrees::new(SourceId(500));
            for forest in [&mut fresh, &mut adopted] {
                SourceInput(
                    [loaded(500, std::path::Path::new("prefix.omg"), &prefix)]
                        .into_iter()
                        .collect(),
                )
                .append(forest, &mut UnmeasuredProduction)
                .unwrap();
            }
            for (index, (path, text)) in sources.iter().enumerate().rev() {
                let admitted = || -> LoadedSources {
                    [loaded(1000 + index, path, text)].into_iter().collect()
                };
                let expected = SourceInput(admitted())
                    .append(&mut fresh, &mut UnmeasuredProduction)
                    .unwrap();
                let mut offered = 0;
                let actual = SourceInput(admitted())
                    .append_reusing(&mut adopted, &mut UnmeasuredProduction, |source| {
                        offered += 1;
                        assert_eq!(source.path, *path);
                        Some(&units[index])
                    })
                    .unwrap();
                assert_eq!(offered, 1);
                assert_eq!(actual, expected, "{}", path.display());
            }
            assert_eq!(adopted, fresh);
            assert_eq!(adopted.page_count(), fresh.page_count());
        }

        // A retained parse of different bytes is never adopted: the file
        // parses afresh, so a changed capture cannot reuse stale syntax.
        #[test]
        fn retained_parse_of_other_bytes_is_not_adopted() {
            let path = std::path::Path::new("unit.omg");
            let old: Arc<str> = Arc::from("data Old {}");
            let new: Arc<str> = Arc::from("data New { value: u8 }");
            let units = SourceInput([loaded(usize::MAX, path, &old)].into_iter().collect())
                .into_parsed_units(&mut UnmeasuredProduction)
                .unwrap();
            let mut fresh = SyntaxTrees::new(SourceId(3));
            let mut reused = SyntaxTrees::new(SourceId(3));
            let expected = SourceInput([loaded(3, path, &new)].into_iter().collect())
                .append(&mut fresh, &mut UnmeasuredProduction)
                .unwrap();
            let actual = SourceInput([loaded(3, path, &new)].into_iter().collect())
                .append_reusing(&mut reused, &mut UnmeasuredProduction, |_| Some(&units[0]))
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(reused, fresh);
        }
    }

    pub struct ParsedText<'text> {
        pub tokens: tokens::TokenStream<'text>,
        pub syntax: SyntaxTrees,
    }

    #[derive(Debug)]
    pub enum SourceParseError {
        Lex { message: String },
        Parse { message: String },
    }

    pub fn parse_text(text: &str) -> Result<ParsedText<'_>, SourceParseError> {
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .map_err(|error| SourceParseError::Lex {
                message: error.message,
            })?;
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).map_err(|error| {
            SourceParseError::Parse {
                message: error.message,
            }
        })?;
        Ok(ParsedText { tokens, syntax })
    }
}

pub use source_checking::{
    FrozenSource, ProductRepresentationInput, SourceAdmission, SourcePreparationFailure,
    SourcePreparationRequest,
};
pub use source_parsing::{
    ParsedText, SourceInput, SourceParseError, load_source_files, parse_text,
};

pub use source_compilation::{
    AdmittedProductPolicy, BuildSourceInput, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceCompilationRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use terminal_production::{
    AcceptedTerminalSource, BuildCallSite, CallableBoundaryImplementation, ConstantRead,
    DeclaredBoundaryRequirement, PortableTerminalProgram, PreparedTerminalRoots,
    ProducedBuildProvider, ProducedTerminalApplication, ProducedTerminalProgram,
    ProgramEntryTerminalReceiptError, PsiCompilationObserver, PsiCompilationStage,
    TerminalAdmissionRoot, TerminalApplicationCustody, TerminalArtifactProductionError,
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
    TerminalBuildNativeLocator, TerminalBuildProviderRequirement, TerminalBuildQueryCall,
    TerminalBuildQueryOperation, TerminalBuildRequestCall, TerminalBuildRequestOperation,
    TerminalBuildRequests, TerminalBuildSelectionRequest, TerminalBuildValueCase,
    TerminalBuildValueField, TerminalBuildValueShape, TerminalCallingPolicySelections,
    TerminalCompositionRoots, TerminalMachineSelection, TerminalProductionCustody,
    TerminalProgramEntryContractSelection, TerminalProgramRoots, TerminalProviderSelectionInput,
    TerminalProviderSelections, TerminalSourceCompilation, TerminalSourceSupply,
    TerminalSuppliedRequirement, UnmeasuredProduction,
};
