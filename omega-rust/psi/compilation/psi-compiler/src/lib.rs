/*
    Target-neutral Psi source compilation facade: source-frontier parsing,
    resolver and typing continuations across Build execution, whole-program and
    library-slice checking, and invocation-shared portable Terminal production.

    We load and parse admitted source frontiers into arena-backed syntax pages,
    drive symbol resolution and seeded typing through `SourceContinuation` and
    `CompletedSource`, admit post-join Product policies, check typed programs
    against fresh invocation authority or matching retained judgments, and
    re-export `psi-terminal-production` for checked-to-Terminal lowering.
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
    (`SourcePreparationRequest` and `PreparedSource`) before lowered-form
    checking, and `portable_productions` (`PortableProductions`) for sharing
    identical portable Terminal productions within an invocation.
*/

#![forbid(unsafe_code)]

pub(crate) mod portable_productions {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use psi_terminal_production::{
        PortableTerminalProgram, TerminalMachineSelection, TerminalProgramRoots,
    };
    use std::sync::{Arc, Mutex, OnceLock};
    use symbols::SymbolHandle;

    type Production = Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>>;

    pub struct PortableProductionInputs<'request> {
        pub program: &'request Arc<CheckedTrees>,
        pub machine: TerminalMachineSelection<'request>,
        pub roots: TerminalProgramRoots<'request>,
        pub exact_toolchain_sources: &'request [(source::SourceId, [u8; 32])],
        pub optimizations: &'request optimization::PsiOptimizationSelections,
    }

    struct ProductionKey {
        program: Arc<CheckedTrees>,
        entry: Option<SymbolHandle>,
        machines: Vec<SymbolHandle>,
        boundaries: Vec<SymbolHandle>,
        exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
        optimizations: optimization::PsiOptimizationSelections,
    }

    impl ProductionKey {
        fn new(inputs: &PortableProductionInputs<'_>) -> Option<Self> {
            let entry = match inputs.machine {
                TerminalMachineSelection::Symbol(symbol) => Some(symbol),
                TerminalMachineSelection::Declarations => None,
                _ => return None,
            };
            Some(Self {
                program: Arc::clone(inputs.program),
                entry,
                machines: inputs.roots.machines.to_vec(),
                boundaries: inputs.roots.boundaries.to_vec(),
                exact_toolchain_sources: inputs.exact_toolchain_sources.to_vec(),
                optimizations: inputs.optimizations.clone(),
            })
        }

        fn matches(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.program, &other.program)
                && self.entry == other.entry
                && self.machines == other.machines
                && self.boundaries == other.boundaries
                && self.exact_toolchain_sources == other.exact_toolchain_sources
                && self.optimizations == other.optimizations
        }
    }

    #[derive(Default)]
    pub struct PortableProductions {
        entries: Mutex<Vec<(ProductionKey, Arc<OnceLock<Production>>)>>,
    }

    impl std::fmt::Debug for PortableProductions {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let entries = self.entries.lock().map_or(0, |entries| entries.len());
            formatter
                .debug_struct("PortableProductions")
                .field("entries", &entries)
                .finish()
        }
    }

    impl PortableProductions {
        pub fn produce(
            &self,
            inputs: PortableProductionInputs<'_>,
            produce: impl FnOnce() -> Result<PortableTerminalProgram, Vec<Diagnostic>>,
        ) -> Production {
            let Some(key) = ProductionKey::new(&inputs) else {
                return produce().map(Arc::new);
            };
            let slot = {
                let mut entries = self
                    .entries
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match entries.iter().find(|(known, _)| known.matches(&key)) {
                    Some((_, slot)) => Arc::clone(slot),
                    None => {
                        let slot = Arc::new(OnceLock::new());
                        entries.push((key, Arc::clone(&slot)));
                        slot
                    }
                }
            };
            slot.get_or_init(|| produce().map(Arc::new)).clone()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn inputs<'a>(
            program: &'a Arc<CheckedTrees>,
            machine: TerminalMachineSelection<'a>,
            machines: &'a [SymbolHandle],
            sources: &'a [(source::SourceId, [u8; 32])],
            optimizations: &'a optimization::PsiOptimizationSelections,
        ) -> PortableProductionInputs<'a> {
            PortableProductionInputs {
                program,
                machine,
                roots: TerminalProgramRoots {
                    machines,
                    boundaries: &[],
                },
                exact_toolchain_sources: sources,
                optimizations,
            }
        }

        fn run(
            productions: &PortableProductions,
            inputs: PortableProductionInputs<'_>,
            runs: &mut usize,
        ) {
            let result = productions.produce(inputs, || {
                *runs += 1;
                Err(vec![Diagnostic::error("refused")])
            });
            assert_eq!(result.unwrap_err()[0].message, "refused");
        }

        #[test]
        fn equal_inputs_produce_once_and_any_difference_produces_again() {
            let productions = PortableProductions::default();
            let program = Arc::new(CheckedTrees::default());
            let other_program = Arc::new(CheckedTrees::default());
            let optimizations = optimization::PsiOptimizationSelections::default();
            let root = [SymbolHandle::default()];
            let sources = [(source::SourceId(0), [0; 32])];
            let other_sources = [(source::SourceId(1), [0; 32])];
            let entry = TerminalMachineSelection::Declarations;
            let mut runs = 0;

            run(
                &productions,
                inputs(&program, entry, &[], &sources, &optimizations),
                &mut runs,
            );
            run(
                &productions,
                inputs(&program, entry, &[], &sources, &optimizations),
                &mut runs,
            );
            assert_eq!(runs, 1, "equal inputs share one production");

            run(
                &productions,
                inputs(&other_program, entry, &[], &sources, &optimizations),
                &mut runs,
            );
            run(
                &productions,
                inputs(&program, entry, &root, &sources, &optimizations),
                &mut runs,
            );
            run(
                &productions,
                inputs(&program, entry, &[], &other_sources, &optimizations),
                &mut runs,
            );
            let symbol = TerminalMachineSelection::Symbol(SymbolHandle::default());
            run(
                &productions,
                inputs(&program, symbol, &[], &sources, &optimizations),
                &mut runs,
            );
            assert_eq!(runs, 5, "each differing input produces its own code");
        }

        #[test]
        fn named_inspection_is_never_shared() {
            let productions = PortableProductions::default();
            let program = Arc::new(CheckedTrees::default());
            let optimizations = optimization::PsiOptimizationSelections::default();
            let named = TerminalMachineSelection::Name("main");
            let mut runs = 0;
            run(
                &productions,
                inputs(&program, named, &[], &[], &optimizations),
                &mut runs,
            );
            run(
                &productions,
                inputs(&program, named, &[], &[], &optimizations),
                &mut runs,
            );
            assert_eq!(runs, 2);
        }
    }
}
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
                    .min(
                        tokens_to_syntax_trees::parser::parse_worker_count()
                            .max(1)
                            .saturating_mul(4),
                    )
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
            let mut single_page_sources =
                tokens_to_syntax_trees::parser::with_parse_workers(|| {
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
    use diagnostics::Diagnostic;
    use source::{DependencyScope, LoadedSources, SourceContents, SourceFileSystem};
    use std::path::PathBuf;
    use syntax_trees::{ParsedSources, SyntaxTrees};

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

pub use portable_productions::{PortableProductionInputs, PortableProductions};
pub use source_checking::{
    PreparedSource, ProductRepresentationInput, SourceAdmission, SourcePreparationRequest,
};
pub use source_parsing::{
    ParsedText, SourceInput, SourceParseError, load_source_files, parse_text,
};
pub use validation::{dependency_body_read_totals, dependency_entailment_totals};

pub use source_compilation::{
    AdmittedProductPolicy, BuildSourceInput, CompletedSource, PreparedSource, ProductPolicyRequest,
    SourceCompilationRequest, SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use psi_terminal_production::{
    BuildProviderProductionRequest, PortableTerminalProgram, PreparedExternalProviderDeclarations,
    PreparedProviderInventory, PreparedTerminalRoots, ProducedBuildProvider,
    ProducedTerminalApplication, ProducedTerminalProgram, ProgramEntryTerminalReceiptError,
    PsiCompilationObserver, PsiCompilationStage, TerminalAdmissionRoot, TerminalApplicationCustody,
    TerminalArtifactProductionError, TerminalBuildBindingCall, TerminalBuildBindingOperation,
    TerminalBuildBindings, TerminalBuildFilesystem, TerminalBuildFilesystemCall,
    TerminalBuildFilesystemOperation, TerminalBuildNativeLocator, TerminalBuildProviderRequirement,
    TerminalBuildQueryCall, TerminalBuildQueryOperation, TerminalBuildRequestCall,
    TerminalBuildRequestOperation, TerminalBuildRequests, TerminalBuildSelectionRequest,
    TerminalBuildValueCase, TerminalBuildValueField, TerminalBuildValueShape,
    TerminalCompositionRoots, TerminalMachineSelection, TerminalProductionCustody,
    TerminalProductionRequest, TerminalProgramRoots, UnmeasuredProduction,
    lower_task_activation_demands,
};
