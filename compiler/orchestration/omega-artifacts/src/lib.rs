use std::fs;
use std::path::{Path, PathBuf};

use omega_checked_trees::{CheckedTrees, machine::Machine, platform::Platform};
use omega_core::allocations::AllocationDelta;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::NativeTarget;

// Foundational report/plan data types live in `omega-backend-report-types` so the
// backend report passes can depend on them downward. Re-exported here so existing
// `omega_artifacts::{EmissionPlan, BackendSurfaceReport, ...}` paths keep working.
pub use omega_backend_report_types::{
    BackendEntryPoint, BackendMachineSurface, BackendPlatformSurface, BackendSurfaceReport,
    EmissionBlocker, EmissionPlan, emission_blocker,
};

pub struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    pub fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        let root = build_dir.to_path_buf();
        fs::create_dir_all(&root).map_err(|error| {
            Diagnostic::error(format!(
                "failed to create artifact directory {}: {error}",
                root.display()
            ))
        })?;

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_text(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        let path = self.root.join(file_name);
        let temp_path = temp_path_for(&path);
        let _ = fs::remove_file(&temp_path);
        fs::write(&temp_path, contents).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write temporary artifact {}: {error}",
                temp_path.display()
            ))
        })?;
        fs::rename(&temp_path, &path).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            Diagnostic::error(format!(
                "failed to install artifact {}: {error}",
                path.display()
            ))
        })
    }

    pub fn write_html_report(
        &self,
        file_name: &str,
        title: &str,
        contents: &str,
    ) -> Result<(), Diagnostic> {
        self.write_text(file_name, &html_report(title, contents))
    }

    pub fn write_bytes(&self, file_name: &str, bytes: &[u8]) -> Result<PathBuf, Diagnostic> {
        let path = self.root.join(file_name);
        fs::write(&path, bytes).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write artifact {}: {error}",
                path.display()
            ))
        })?;

        Ok(path)
    }

    pub fn remove_files<'a>(
        &self,
        file_names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), Diagnostic> {
        for file_name in file_names {
            let path = self.root.join(file_name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Diagnostic::error(format!(
                        "failed to remove stale artifact {}: {error}",
                        path.display()
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn write_sources(&self, source_artifact: &SourceLoadArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        let total_bytes = source_artifact
            .files
            .iter()
            .map(|file| file.byte_count)
            .sum::<usize>();
        let total_lines = source_artifact
            .files
            .iter()
            .map(|file| file.line_count)
            .sum::<usize>();
        let total_non_empty_lines = source_artifact
            .files
            .iter()
            .map(|file| file.non_empty_line_count)
            .sum::<usize>();

        output.push_str("# Omega Source Load\n\n");
        output.push_str("## Totals\n");
        output.push_str(&format!("files: {}\n", source_artifact.files.len()));
        output.push_str(&format!("items: {}\n", source_artifact.item_count));
        output.push_str(&format!("bytes: {}\n", format_bytes(total_bytes as u64)));
        output.push_str(&format!("lines: {}\n", total_lines));
        output.push_str(&format!("non-empty lines: {}\n\n", total_non_empty_lines));

        output.push_str("## Files\n");
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "id", "bytes", "lines", "code", "items", "item range", "path"
        ));
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "--", "-----", "-----", "----", "-----", "----------", "----"
        ));

        for file in &source_artifact.files {
            write_source_file_artifact(&mut output, file);
        }

        self.write_html_report("01_sources.html", "sources", &output)
    }

    pub fn write_ast(&self, ast_artifact: &AstArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega AST\n\n");
        output.push_str(&format!("files: {}\n", ast_artifact.file_count));
        output.push_str(&format!("items: {}\n\n", ast_artifact.item_count));

        output.push_str("## Identity Storage\n");
        output.push_str(&format!(
            "owned identifier strings: {}\n",
            ast_artifact.identity.owned_identifier_strings
        ));
        output.push_str(&format!(
            "identifiers: {}\n",
            ast_artifact.identity.identifiers
        ));
        output.push_str(&format!(
            "source identifiers: {}\n",
            ast_artifact.identity.source_identifiers
        ));
        output.push_str(&format!(
            "generated identifiers: {}\n",
            ast_artifact.identity.generated_identifiers
        ));
        output.push_str(&format!(
            "path members: {}\n",
            ast_artifact.identity.path_members
        ));
        output.push_str(&format!(
            "string literals: {}\n",
            ast_artifact.identity.string_literals
        ));
        output.push_str(&format!(
            "float literals: {}\n",
            ast_artifact.identity.float_literals
        ));
        output.push_str(&format!(
            "source float literals: {}\n",
            ast_artifact.identity.source_float_literals
        ));
        output.push_str(&format!(
            "generated float literals: {}\n\n",
            ast_artifact.identity.generated_float_literals
        ));

        for file in &ast_artifact.files {
            write_ast_file_artifact(&mut output, file);
        }

        self.write_html_report("02_ast.html", "ast", &output)
    }

    pub fn write_emission_plan(&self, emission_plan: &EmissionPlan) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Emission Plan\n\n");
        output.push_str(&format!("image format: {:?}\n", emission_plan.image_format));
        output.push_str(&format!("entry symbol: {}\n", emission_plan.entry_symbol));
        output.push_str(&format!("sections: {}\n", emission_plan.sections));
        output.push_str(&format!("symbols: {}\n", emission_plan.symbols));
        output.push_str(&format!("host bindings: {}\n", emission_plan.host_bindings));
        output.push_str(&format!("host calls: {}\n", emission_plan.host_calls));
        output.push_str(&format!("data bytes: {}\n", emission_plan.data_bytes));
        output.push_str(&format!(
            "selected instructions: {}\n",
            emission_plan.selected_instructions
        ));
        output.push_str(&format!(
            "instruction operands: {}\n",
            emission_plan.instruction_operands
        ));
        output.push_str(&format!(
            "machine code bytes: {}\n",
            emission_plan.machine_code_bytes
        ));
        output.push_str(&format!(
            "encoded machine bytes: {}\n",
            emission_plan.encoded_machine_bytes
        ));
        output.push_str(&format!("relocations: {}\n", emission_plan.relocations));
        output.push_str(&format!("blockers: {}\n\n", emission_plan.blockers.len()));

        if emission_plan.blockers.is_empty() {
            output.push_str("status: ready to emit\n");
        } else {
            output.push_str("status: blocked before byte emission\n\n");
            output.push_str("## Blockers\n");

            for (_, blocker) in emission_plan.blockers.iter() {
                output.push_str(&format!("- {}: {}\n", blocker.stage, blocker.reason));
            }
        }

        self.write_text("12_emission.txt", &output)
    }

    pub fn write_emitted_native_output(
        &self,
        emitted_output: &EmittedImageOutput,
    ) -> Result<PathBuf, Diagnostic> {
        let output_path = self.write_bytes(&emitted_output.file_name, &emitted_output.bytes)?;

        let mut output = String::new();
        output.push_str("# Omega Emitted Native Output\n\n");
        output.push_str(&format!("path: {}\n", output_path.display()));
        output.push_str(&format!("format: {}\n", emitted_output.format));
        output.push_str(&format!("kind: {:?}\n", emitted_output.kind));
        output.push_str(&format!("bytes: {}\n", emitted_output.bytes.len()));
        output.push_str(&format!("text bytes: {}\n", emitted_output.text_bytes));
        output.push_str(&format!("data bytes: {}\n", emitted_output.data_bytes));
        output.push_str(&format!("bss bytes: {}\n", emitted_output.bss_bytes));
        output.push_str(&format!("symbols: {}\n", emitted_output.symbols));
        output.push_str(&format!("relocations: {}\n", emitted_output.relocations));
        output.push_str(&format!(
            "final image symbols: {}\n",
            emitted_output.final_image_symbols
        ));
        output.push_str(&format!(
            "final image imports: {}\n",
            emitted_output.final_image_imports
        ));
        output.push_str(&format!(
            "final image relocations: {}\n",
            emitted_output.final_image_relocations
        ));

        self.write_html_report("13_emitted_output.html", "emitted_output", &output)?;

        Ok(output_path)
    }

    pub fn remove_stale_native_output(&self) -> Result<(), Diagnostic> {
        self.remove_files([
            "omega-program",
            "12_emitted_output.txt",
            "12_emitted_output.html",
            "13_emitted_output.txt",
            "13_emitted_output.html",
            "13_finalization.txt",
            "13_finalization.html",
            "14_finalization.txt",
            "14_finalization.html",
        ])
    }

    pub fn write_executable_finalization_report(
        &self,
        finalization: &ExecutableFinalization,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Executable Finalization\n\n");
        output.push_str(&format!("status: {:?}\n", finalization.status));
        output.push_str(&format!(
            "output: {}\n",
            finalization.executable_path.display()
        ));
        if finalization.command.is_empty() {
            output.push_str("command: none\n");
        } else {
            output.push_str(&format!("command: {}\n", finalization.command.join(" ")));
        }
        if !finalization.stdout.is_empty() {
            output.push_str("\n## stdout\n");
            output.push_str(&finalization.stdout);
            output.push('\n');
        }
        if !finalization.stderr.is_empty() {
            output.push_str("\n## stderr\n");
            output.push_str(&finalization.stderr);
            output.push('\n');
        }
        if finalization.status == ExecutableFinalizationStatus::AlreadyExecutable {
            output.push_str(
                "\nno finalization command was needed; the backend emitted an executable image directly.\n",
            );
        }

        self.write_html_report("14_finalization.html", "finalization", &output)
    }

    pub fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Phase Timings\n\n");

        let total_microseconds = timings
            .iter()
            .map(|timing| timing.microseconds)
            .sum::<u128>();
        let total_allocation_calls = timings
            .iter()
            .map(|timing| timing.allocations.allocation_calls)
            .sum::<u64>();
        let total_deallocation_calls = timings
            .iter()
            .map(|timing| timing.allocations.deallocation_calls)
            .sum::<u64>();
        let total_allocated_bytes = timings
            .iter()
            .map(|timing| timing.allocations.allocated_bytes)
            .sum::<u64>();
        let total_deallocated_bytes = timings
            .iter()
            .map(|timing| timing.allocations.deallocated_bytes)
            .sum::<u64>();
        let total_net_live_bytes =
            i128::from(total_allocated_bytes) - i128::from(total_deallocated_bytes);
        let slowest = timings.iter().max_by_key(|timing| timing.microseconds);
        let allocation_heaviest = timings
            .iter()
            .max_by_key(|timing| timing.allocations.allocated_bytes);
        let average_microseconds = if timings.is_empty() {
            0
        } else {
            total_microseconds / timings.len() as u128
        };

        output.push_str("## Summary\n\n");
        output.push_str(&format!("phase count: {}\n", timings.len()));
        output.push_str(&format!(
            "total measured: {}\n",
            format_duration(total_microseconds)
        ));
        output.push_str(&format!(
            "average phase: {}\n",
            format_duration(average_microseconds)
        ));
        output.push_str(&format!(
            "allocation calls: {} alloc / {} free\n",
            format_integer(u128::from(total_allocation_calls)),
            format_integer(u128::from(total_deallocation_calls))
        ));
        output.push_str(&format!(
            "allocated bytes: {} allocated / {} freed / {} net\n",
            format_bytes(total_allocated_bytes),
            format_bytes(total_deallocated_bytes),
            format_signed_bytes(total_net_live_bytes)
        ));
        if let Some(slowest) = slowest {
            output.push_str(&format!(
                "slowest phase: {} ({}, {})\n",
                slowest.phase,
                format_duration(slowest.microseconds),
                format_percentage(slowest.microseconds, total_microseconds)
            ));
        }
        if let Some(allocation_heaviest) = allocation_heaviest {
            output.push_str(&format!(
                "allocation-heaviest phase: {} ({} in {} allocation calls)\n",
                allocation_heaviest.phase,
                format_bytes(allocation_heaviest.allocations.allocated_bytes),
                format_integer(u128::from(allocation_heaviest.allocations.allocation_calls))
            ));
        }

        output.push_str("\n## Phases\n\n");
        let phase_width = timings
            .iter()
            .map(|timing| timing.phase.len())
            .max()
            .unwrap_or("phase".len())
            .max("phase".len());
        let duration_width = timings
            .iter()
            .map(|timing| format_duration(timing.microseconds).len())
            .chain(std::iter::once("time".len()))
            .max()
            .unwrap_or("time".len());
        let raw_width = timings
            .iter()
            .map(|timing| format!("{} us", format_integer(timing.microseconds)).len())
            .chain(std::iter::once("raw".len()))
            .max()
            .unwrap_or("raw".len());
        let alloc_calls_width = timings
            .iter()
            .map(|timing| format_integer(u128::from(timing.allocations.allocation_calls)).len())
            .chain(std::iter::once("allocs".len()))
            .max()
            .unwrap_or("allocs".len());
        let allocated_width = timings
            .iter()
            .map(|timing| format_bytes(timing.allocations.allocated_bytes).len())
            .chain(std::iter::once("allocated".len()))
            .max()
            .unwrap_or("allocated".len());
        let net_width = timings
            .iter()
            .map(|timing| format_signed_bytes(timing.allocations.net_live_bytes()).len())
            .chain(std::iter::once("net".len()))
            .max()
            .unwrap_or("net".len());

        output.push_str(&format!(
            "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
            "phase", "time", "share", "raw", "allocs", "allocated", "net"
        ));
        output.push_str(&format!(
            "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<raw_width$}  {:-<alloc_calls_width$}  {:-<allocated_width$}  {:-<net_width$}\n",
            "", "", "", "", "", "", ""
        ));
        for timing in timings {
            output.push_str(&format!(
                "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
                timing.phase,
                format_duration(timing.microseconds),
                format_percentage(timing.microseconds, total_microseconds),
                format!("{} us", format_integer(timing.microseconds)),
                format_integer(u128::from(timing.allocations.allocation_calls)),
                format_bytes(timing.allocations.allocated_bytes),
                format_signed_bytes(timing.allocations.net_live_bytes()),
            ));
        }
        output.push_str(&format!(
            "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<raw_width$}  {:-<alloc_calls_width$}  {:-<allocated_width$}  {:-<net_width$}\n",
            "", "", "", "", "", "", ""
        ));
        output.push_str(&format!(
            "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>raw_width$}  {:>alloc_calls_width$}  {:>allocated_width$}  {:>net_width$}\n",
            "total",
            format_duration(total_microseconds),
            "100.00%",
            format!("{} us", format_integer(total_microseconds)),
            format_integer(u128::from(total_allocation_calls)),
            format_bytes(total_allocated_bytes),
            format_signed_bytes(total_net_live_bytes),
        ));

        self.write_html_report("00_timings.html", "phase_timings", &output)
    }

    pub fn write_wire_protocol_report(
        &self,
        wire_report: &WireProtocolReport,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Wire Protocols\n\n");
        output.push_str(&format!("wire data schemas: {}\n", wire_report.schemas.len()));

        for schema in &wire_report.schemas {
            output.push_str(&format!("\n## wire data {}\n", schema.name));
            output.push_str(&format!(
                "encoding: {}\n",
                schema.encoding.as_deref().unwrap_or("(default)")
            ));
            output.push_str(&format!("current era: {}\n", schema.current_era));

            push_wire_field_table(&mut output, &schema.fields, &schema.reserved);

            for version in &schema.versions {
                output.push_str(&format!(
                    "\n### version {} (era {})\n",
                    version.name, version.era
                ));
                push_wire_field_table(&mut output, &version.fields, &version.reserved);

                output.push_str(&format!(
                    "\n### compatibility {} -> {}\n",
                    version.name, version.successor
                ));
                push_wire_verdicts(&mut output, "compatible", &version.verdicts.compatible);
                push_wire_verdicts(
                    &mut output,
                    "requires migration",
                    &version.verdicts.requires_migration,
                );
                push_wire_verdicts(&mut output, "reserved", &version.verdicts.reserved);
                push_wire_verdicts(&mut output, "incompatible", &version.verdicts.incompatible);
            }
        }

        self.write_text("04_wire_protocols.txt", &output)
    }

    pub fn write_boundary_report(
        &self,
        boundary_report: &BoundaryReport,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Boundary\n\n");
        output.push_str(&format!("targets: {}\n", boundary_report.targets.len()));
        output.push_str(&format!(
            "boundary contracts: {}\n",
            boundary_report.contracts.len()
        ));
        output.push_str(&format!(
            "unchecked policies: {}\n",
            boundary_report.unchecked_policies.len()
        ));
        output.push_str(&format!(
            "boundary providers: {}\n\n",
            boundary_report.providers.len()
        ));

        output.push_str("## Boundary Providers\n");
        if boundary_report.providers.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, provider) in boundary_report.providers.iter() {
                let contract = provider.contract_ref.as_deref().unwrap_or("none");
                let authority = if provider.authority_effects.is_empty() {
                    "none".to_owned()
                } else {
                    provider.authority_effects.join(", ")
                };
                let targets = if provider.target_applicability.is_empty() {
                    "all".to_owned()
                } else {
                    provider.target_applicability.join(", ")
                };
                output.push_str(&format!(
                    "- provider `{}` [{}] contract `{}` authority {{{}}} targets {{{}}} origin `{}`\n",
                    provider.name,
                    provider.category,
                    contract,
                    authority,
                    targets,
                    provider.origin_package,
                ));
            }
        }
        output.push('\n');

        output.push_str("## Targets\n");
        if boundary_report.targets.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, target) in boundary_report.targets.iter() {
                output.push_str(&format!(
                    "- target `{}` host `{}` settings {} checked boundaries {} unchecked boundaries {}\n",
                    target.name,
                    target.host_provider,
                    target.host_settings,
                    target.checked_boundaries,
                    target.unchecked_boundaries
                ));
            }
        }

        output.push_str("\n## Boundary Contracts\n");
        if boundary_report.contracts.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, contract) in boundary_report.contracts.iter() {
                output.push_str(&format!(
                    "- {}.{} boundary `{}` requires {} ensures {}\n",
                    contract.capability,
                    contract.state,
                    contract.boundary,
                    contract.requires_count,
                    contract.ensures_count
                ));
            }
        }

        output.push_str("\n## Unchecked Policies\n");
        if boundary_report.unchecked_policies.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, policy) in boundary_report.unchecked_policies.iter() {
                output.push_str(&format!(
                    "- target `{}` boundary unchecked `{}`\n",
                    policy.target, policy.name
                ));
            }
        }

        output.push_str("\n## Capability Blast Radius\n");
        if boundary_report.capability_blast_radius.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, radius) in boundary_report.capability_blast_radius.iter() {
                let provider = if radius.approved_provider {
                    "approved provider"
                } else {
                    "in-package provider"
                };
                let effects = if radius.authority_effects.is_empty() {
                    "none".to_owned()
                } else {
                    radius.authority_effects.join(", ")
                };
                output.push_str(&format!(
                    "- capability `{}` [{}] authority {{{}}} uses {} acquires {} returns {} stores {} derives {}\n",
                    radius.capability,
                    provider,
                    effects,
                    radius.uses,
                    radius.acquires,
                    radius.returns,
                    radius.stores,
                    radius.derives,
                ));
                for propagated in &radius.propagated_flows {
                    output.push_str(&format!("  - {propagated}\n"));
                }
            }
        }

        self.write_html_report("10_boundary.html", "boundary", &output)
    }
}

fn html_report(title: &str, contents: &str) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>");
    html.push_str(&escape_html(title));
    html.push_str("</title>\n<style>\n");
    html.push_str(REPORT_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<aside>\n<h1>");
    html.push_str(&escape_html(title));
    html.push_str("</h1>\n");
    push_report_nav(&mut html);
    html.push_str("</aside>\n<main><pre>");
    html.push_str(&escape_html(contents));
    html.push_str("</pre></main>\n</body>\n</html>\n");
    html
}

fn push_report_nav(html: &mut String) {
    html.push_str("<nav class=\"phase-nav\" aria-label=\"Pipeline stages\"><a target=\"_top\" href=\"00_pipeline.html\">Index</a>");
    for (number, label, id) in REPORT_LINKS {
        html.push_str("<a target=\"_top\" href=\"00_pipeline.html#");
        html.push_str(&escape_html(id));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span> ");
        html.push_str(&escape_html(label));
        html.push_str("</a>");
    }
    html.push_str("</nav>\n");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const REPORT_LINKS: &[(&str, &str, &str)] = &[
    ("00", "Timings", "timings"),
    ("02", "Syntax", "syntax"),
    ("03", "Symbols", "symbols"),
    ("04", "Typed", "typed"),
    ("05", "Checked", "checked"),
    ("06", "State Graph", "state-graph"),
    ("07", "Control Flow", "control-flow"),
    ("08", "Abstract Operations", "abstract-operations"),
    ("09", "Target Operations", "target-operations"),
    (
        "10",
        "Assigned Target Operations",
        "assigned-target-operations",
    ),
    ("11", "Machine Instructions", "machine-instructions"),
    ("12", "Emission", "emission"),
];

const REPORT_STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
}
* { box-sizing: border-box; }
body {
  min-height: 100vh;
  margin: 0;
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
  color: var(--text);
  display: grid;
  grid-template-columns: minmax(280px, 22vw) 1fr;
  font: 14px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
aside {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  min-height: 100vh;
  padding: 18px;
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
.phase-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.phase-nav a {
  border: 1px solid #303d50;
  border-radius: 999px;
  color: #d8e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 7px 9px;
  text-decoration: none;
}
.phase-nav a:hover { background: #263247; border-color: #8ab4ff; }
.phase-nav span { color: var(--muted); }
main {
  min-width: 0;
  overflow: auto;
  padding: 28px;
}
pre {
  background: rgba(13, 17, 23, 0.82);
  border: 1px solid #283343;
  border-radius: 18px;
  color: #d8e2ef;
  line-height: 1.45;
  margin: 0;
  min-height: calc(100vh - 56px);
  overflow: auto;
  padding: 24px;
  white-space: pre;
}
"#;

fn temp_path_for(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-artifact"),
        std::process::id()
    ))
}

fn write_source_file_artifact(output: &mut String, file: &SourceFileArtifact) {
    let item_range = if file.item_count == 0 {
        String::from("-")
    } else {
        format!("{}..{}", file.first_item, file.first_item + file.item_count)
    };

    output.push_str(&format!(
        "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
        file.id,
        format_bytes(file.byte_count as u64),
        file.line_count,
        file.non_empty_line_count,
        file.item_count,
        item_range,
        file.path.display()
    ));
}

fn write_ast_file_artifact(output: &mut String, file: &AstFileArtifact) {
    output.push_str(&format!("## {}\n", file.path.display()));
    output.push_str(&format!(
        "tables: statements {} transition_targets {} expressions {} type_references {} type_constraints {}\n",
        file.statement_count,
        file.transition_target_count,
        file.expression_count,
        file.type_reference_count,
        file.type_constraint_count
    ));

    if !file.item_range_valid {
        output.push_str("invalid item range\n\n");
        return;
    }

    if file.item_summaries.is_empty() {
        output.push_str("items: none\n\n");
        return;
    }

    for (index, summary) in file.item_summaries.iter().enumerate() {
        output.push_str(&format!(
            "- item {}: {}\n",
            file.first_item + index,
            summary
        ));
    }

    output.push('\n');
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn format_duration(microseconds: u128) -> String {
    if microseconds >= 1_000_000 {
        format!("{:.3} s", microseconds as f64 / 1_000_000.0)
    } else {
        format!("{:.3} ms", microseconds as f64 / 1_000.0)
    }
}

fn format_percentage(part: u128, total: u128) -> String {
    if total == 0 {
        return "0.00%".to_owned();
    }

    format!("{:.2}%", part as f64 * 100.0 / total as f64)
}

fn format_integer(value: u128) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(character);
    }
    output
}

fn format_signed_bytes(bytes: i128) -> String {
    if bytes < 0 {
        format!("-{}", format_bytes(bytes.unsigned_abs() as u64))
    } else {
        format_bytes(bytes as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceLoadArtifact {
    pub item_count: usize,
    pub files: Vec<SourceFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFileArtifact {
    pub id: usize,
    pub path: PathBuf,
    pub first_item: usize,
    pub item_count: usize,
    pub byte_count: usize,
    pub line_count: usize,
    pub non_empty_line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstArtifact {
    pub file_count: usize,
    pub item_count: usize,
    pub identity: AstIdentityArtifact,
    pub files: Vec<AstFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstIdentityArtifact {
    pub owned_identifier_strings: usize,
    pub identifiers: usize,
    pub source_identifiers: usize,
    pub generated_identifiers: usize,
    pub path_members: usize,
    pub string_literals: usize,
    pub float_literals: usize,
    pub source_float_literals: usize,
    pub generated_float_literals: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstFileArtifact {
    pub path: PathBuf,
    pub first_item: usize,
    pub statement_count: usize,
    pub transition_target_count: usize,
    pub expression_count: usize,
    pub type_reference_count: usize,
    pub type_constraint_count: usize,
    pub item_summaries: Vec<String>,
    pub item_range_valid: bool,
}

fn push_wire_field_table(output: &mut String, fields: &[WireFieldReportEntry], reserved: &[i64]) {
    output.push_str("fields:\n");
    if fields.is_empty() {
        output.push_str("  none\n");
    } else {
        for field in fields {
            output.push_str(&format!(
                "  {} {} {}\n",
                field.number, field.name, field.type_display
            ));
        }
    }

    if !reserved.is_empty() {
        output.push_str(&format!(
            "reserved: {}\n",
            reserved
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn push_wire_verdicts(output: &mut String, label: &str, verdicts: &[String]) {
    output.push_str(&format!("{label}:\n"));
    if verdicts.is_empty() {
        output.push_str("  none\n");
    } else {
        for verdict in verdicts {
            output.push_str(&format!("  {verdict}\n"));
        }
    }
}

/// Compatibility report for `wire data` protocol schemas (chapter 20): field
/// tables, retired numbers, declared version eras, and per-era verdicts along
/// the VERSION CHAIN (each era against its successor; the newest era against
/// the current schema body). Built from typed trees by the compiler pipeline;
/// this crate only owns the artifact shape and rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProtocolReport {
    pub schemas: Vec<WireSchemaReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireSchemaReportEntry {
    pub name: String,
    pub encoding: Option<String>,
    /// The era discriminator the CURRENT body encodes (decision 10): the
    /// number of declared version blocks (0 for an unversioned schema).
    pub current_era: u64,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<i64>,
    pub versions: Vec<WireVersionReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireFieldReportEntry {
    pub number: i64,
    pub name: String,
    pub type_display: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireVersionReportEntry {
    pub name: String,
    /// The era discriminator payloads of this declared version carry: its
    /// zero-based position in the declaration-ordered version chain.
    pub era: u64,
    /// The next era in the version chain this era's verdicts compare against:
    /// the following declared version, or `current` for the newest era.
    pub successor: String,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<i64>,
    pub verdicts: WireCompatibilityVerdicts,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityVerdicts {
    pub compatible: Vec<String>,
    /// Cross-era type changes on a stable field number: legal evolution (the
    /// era discriminator selects the old decode table), surfaced as a report
    /// verdict instead of a compile error.
    pub requires_migration: Vec<String>,
    pub reserved: Vec<String>,
    pub incompatible: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryReport {
    pub targets: Arena<BoundaryTarget>,
    pub contracts: Arena<BoundaryContract>,
    pub unchecked_policies: Arena<UncheckedBoundaryPolicy>,
    pub capability_blast_radius: Arena<CapabilityBlastRadius>,
    pub providers: Arena<BoundaryProviderEntry>,
}

/// One registered boundary primitive provider (frozen Wave 0 decision #4):
/// the governing contract, the authority effects it carries, and the targets
/// it applies to, sourced from the boundary operator(s) bound to it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryProviderEntry {
    pub name: String,
    pub category: String,
    /// The contract governing the provider, when a bound operator declares one.
    pub contract_ref: Option<String>,
    pub authority_effects: Vec<String>,
    /// Targets the provider applies to; empty means all targets.
    pub target_applicability: Vec<String>,
    pub origin_package: String,
}

/// Theoretical blast radius for a single boundary capability: the host-authority
/// effects it can mint, whether it is an approved provider edge or an in-package
/// (application-minted) provider, and the authority-flow verbs it participates in
/// (chapter 18, "Capabilities And Authority Flow").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityBlastRadius {
    pub capability: String,
    pub authority_effects: Vec<String>,
    pub approved_provider: bool,
    pub uses: usize,
    pub returns: usize,
    pub acquires: usize,
    pub stores: usize,
    pub derives: usize,
    /// Authority-flow verbs that reached a state through a nested helper call
    /// rather than a direct boundary call, rendered as provenance lines such as
    /// `Main::main acquires via Vault::expose`.
    pub propagated_flows: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryTarget {
    pub name: String,
    pub host_provider: String,
    pub host_settings: usize,
    pub checked_boundaries: usize,
    pub unchecked_boundaries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryContract {
    pub capability: String,
    pub state: String,
    pub boundary: String,
    pub requires_count: usize,
    pub ensures_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UncheckedBoundaryPolicy {
    pub target: String,
    pub name: String,
}

pub fn build_backend_surface_report(program: &CheckedTrees) -> BackendSurfaceReport {
    let mut report = BackendSurfaceReport::default();

    for machine in program.machines() {
        collect_machine(&mut report, program, machine);
    }

    for platform in program.platforms() {
        collect_platform(&mut report, program, platform);
    }

    report
}

fn collect_machine(report: &mut BackendSurfaceReport, program: &CheckedTrees, machine: &Machine) {
    report.machines.insert(BackendMachineSurface {
        name: machine.name.to_string(),
        contained_objects: program.machine_contained_objects(machine).len(),
        owned_data: program.machine_owned_data(machine).len(),
        states: program.machine_states(machine).len(),
    });

    if machine.name.as_str() == "Main::main"
        && program
            .machine_states(machine)
            .iter()
            .any(|state| state.name.as_str() == "main")
    {
        report.entry_points.insert(BackendEntryPoint {
            machine: "Main::main".to_owned(),
            state: "main".to_owned(),
        });
    } else if machine.name.as_str() == "main"
        && program
            .machine_states(machine)
            .iter()
            .any(|state| state.name.as_str() == "entry")
    {
        report.entry_points.insert(BackendEntryPoint {
            machine: "main".to_owned(),
            state: "entry".to_owned(),
        });
    }
}

fn collect_platform(
    report: &mut BackendSurfaceReport,
    program: &CheckedTrees,
    platform: &Platform,
) {
    report.platforms.insert(BackendPlatformSurface {
        name: platform.name.to_string(),
        states: program.platform_state_signatures(platform).len(),
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableFinalization {
    pub executable_path: PathBuf,
    pub status: ExecutableFinalizationStatus,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableFinalizationStatus {
    AlreadyExecutable,
}

pub fn finalize_emitted_image_output(
    target: NativeTarget,
    emitted_output: &EmittedImageOutput,
    output_path: &Path,
) -> Result<ExecutableFinalization, Diagnostic> {
    if emitted_output.kind == ImageOutputKind::DirectExecutable {
        mark_executable_if_needed(output_path)?;
        return Ok(ExecutableFinalization {
            executable_path: output_path.to_path_buf(),
            status: ExecutableFinalizationStatus::AlreadyExecutable,
            command: Vec::new(),
            stdout: "native output is already an executable image".to_owned(),
            stderr: String::new(),
        });
    }

    Err(Diagnostic::error(format!(
        "native output `{}` is {:?} for {:?}; Omega does not invoke external linkers, so this target must emit a direct executable image",
        emitted_output.format, emitted_output.kind, target
    )))
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to read executable permissions {}: {error}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark executable {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &Path) -> Result<(), Diagnostic> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_checked_trees::CheckedTrees;
    use omega_checked_trees::machine::Machine;
    use omega_checked_trees::name::Identifier;
    use omega_checked_trees::platform::Platform;
    use omega_checked_trees::signature::StateSignature;
    use omega_checked_trees::state::State;
    use omega_core::symbols::SymbolHandle;

    use super::build_backend_surface_report;

    #[test]
    fn collects_entry_machine_and_platforms() {
        let mut program = CheckedTrees::default();
        let mut platform = Platform {
            symbol: SymbolHandle::default(),
            name: Identifier::generated("Console"),
            states: Default::default(),
        };
        program.typed.push_platform_state_signature(
            &mut platform,
            StateSignature {
                symbol: SymbolHandle::default(),
                name: Identifier::generated("write_line"),
                is_default: false,
                parameters: Default::default(),
                return_type: Default::default(),
                ..Default::default()
            },
        );
        program.typed.push_platform(platform);
        let mut machine = Machine {
            symbol: SymbolHandle::default(),
            name: Identifier::generated("main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: SymbolHandle::default(),
                name: Identifier::generated("entry"),
                parameters: Default::default(),
                return_type: Default::default(),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);

        let report = build_backend_surface_report(&program);

        assert_eq!(report.entry_points.len(), 1);
        assert_eq!(report.platforms.len(), 1);
        assert_eq!(report.machines.len(), 1);
    }
}
