//! Parse captured files into an invocation's source forest. Fresh and retained
//! files enter the same ordered batch before any pages are appended.

mod batch;
mod import_advisories;
mod parsed_files;
pub use import_advisories::FileImportAdvisories;
pub use parsed_files::{
    ParsedSourceBatch, ParsedSourceSummary, RetainedSourceParse, SourceImport, SourceImportTarget,
    SourceItems, SourceSyntax,
};

use artifacts::compile_timings::{CompileTimings, source as stages};
use batch::Layout;
use diagnostics::Diagnostic;
use source_files::{LoadedSource, LoadedSources};
use syntax_trees::{ParsedSources, ParsedUnit, SyntaxTrees};

pub struct SourceInput(pub LoadedSources);

impl SourceInput {
    pub fn append(
        self,
        syntax: &mut SourceSyntax,
        timings: &mut CompileTimings,
    ) -> Result<ParsedSourceBatch, Vec<Diagnostic>> {
        let parsed = self.append_pages(&mut syntax.trees, Layout::Shared, timings)?;
        Ok(syntax.retain_batch(parsed))
    }

    pub fn append_reusing<'unit>(
        self,
        syntax: &mut SourceSyntax,
        timings: &mut CompileTimings,
        mut retained: impl FnMut(&LoadedSource) -> Option<&'unit RetainedSourceParse>,
    ) -> Result<ParsedSourceBatch, Vec<Diagnostic>> {
        let files = self.0.sources.span_or_empty(self.0.batch);
        let units: Vec<_> = files
            .iter()
            .map(|file| {
                retained(file)
                    .map(|record| &record.unit)
                    .filter(|unit| unit.source == file.source)
            })
            .collect();
        let layout = if units.iter().any(Option::is_some) {
            Layout::Reuse(&units)
        } else {
            Layout::Shared
        };
        let parsed = self.append_pages(&mut syntax.trees, layout, timings)?;
        Ok(syntax.retain_batch(parsed))
    }

    fn append_pages(
        self,
        syntax: &mut SyntaxTrees,
        layout: Layout<'_>,
        timings: &mut CompileTimings,
    ) -> Result<ParsedSources, Vec<Diagnostic>> {
        let first_page = u32::try_from(syntax.page_count())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| vec![Diagnostic::error("syntax page identity overflow")])?;
        let (pages, parsed) = self.parse_batch(first_page, layout, timings)?;
        for page in pages {
            syntax.adopt_page(page);
        }
        Ok(parsed)
    }

    /// Capture each file with the roots it owns, ready for later admission.
    pub fn into_parsed_units(
        self,
        timings: &mut CompileTimings,
    ) -> Result<Vec<RetainedSourceParse>, Vec<Diagnostic>> {
        let (pages, mut sources) = self.parse_batch(1, Layout::Independent, timings)?;
        let records = sources.sources.span_mut_or_empty(sources.batch);
        if records.len() != pages.len() {
            return Err(vec![Diagnostic::error(
                "independent parse lost the page of a source",
            )]);
        }
        Ok(pages
            .into_iter()
            .zip(records)
            .map(|(syntax, source)| RetainedSourceParse {
                unit: ParsedUnit {
                    syntax,
                    source: source.source.clone(),
                    root_items: std::mem::take(&mut source.root_items),
                    import_root_range: source.import_root_range.clone(),
                },
            })
            .collect())
    }

    fn parse_batch(
        self,
        first_page: u32,
        layout: Layout<'_>,
        timings: &mut CompileTimings,
    ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
        timings.record_result(stages::PARSE_FILES, || {
            batch::parse(self.0, first_page, layout)
        })
    }
}

#[cfg(test)]
mod reused_sources;
#[cfg(test)]
mod tests;
