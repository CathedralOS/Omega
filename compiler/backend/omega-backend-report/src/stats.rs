use super::format;

use crate::BackendReportInput;
use crate::identity::count_backend_string_storage;

pub(super) fn write_backend_phase_timings(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    let timings = backend_plan.phase_timings;

    output.push_str("## Backend Subphases\n");
    if timings.is_empty() {
        output.push_str("none\n\n");
        return;
    }

    let total_microseconds = timings
        .iter()
        .map(|timing| timing.microseconds)
        .sum::<u128>();
    let total_allocations = timings
        .iter()
        .map(|timing| timing.allocations.allocation_calls)
        .sum::<u64>();
    let total_allocated_bytes = timings
        .iter()
        .map(|timing| timing.allocations.allocated_bytes)
        .sum::<u64>();
    let phase_width = timings
        .iter()
        .map(|timing| timing.phase.len())
        .chain(std::iter::once("subphase".len()))
        .max()
        .unwrap_or("subphase".len());
    let duration_width = timings
        .iter()
        .map(|timing| format::duration(timing.microseconds).len())
        .chain(std::iter::once("time".len()))
        .max()
        .unwrap_or("time".len());
    let alloc_width = timings
        .iter()
        .map(|timing| format::integer(u128::from(timing.allocations.allocation_calls)).len())
        .chain(std::iter::once("allocs".len()))
        .max()
        .unwrap_or("allocs".len());
    let allocated_width = timings
        .iter()
        .map(|timing| format::bytes(timing.allocations.allocated_bytes).len())
        .chain(std::iter::once("allocated".len()))
        .max()
        .unwrap_or("allocated".len());

    output.push_str(&format!(
        "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>alloc_width$}  {:>allocated_width$}\n",
        "subphase", "time", "share", "allocs", "allocated"
    ));
    output.push_str(&format!(
        "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<alloc_width$}  {:-<allocated_width$}\n",
        "", "", "", "", ""
    ));
    for timing in timings {
        output.push_str(&format!(
            "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>alloc_width$}  {:>allocated_width$}\n",
            timing.phase,
            format::duration(timing.microseconds),
            format::percentage(timing.microseconds, total_microseconds),
            format::integer(u128::from(timing.allocations.allocation_calls)),
            format::bytes(timing.allocations.allocated_bytes)
        ));
    }
    output.push_str(&format!(
        "{:-<phase_width$}  {:-<duration_width$}  {:-<7}  {:-<alloc_width$}  {:-<allocated_width$}\n",
        "", "", "", "", ""
    ));
    output.push_str(&format!(
        "{:<phase_width$}  {:>duration_width$}  {:>7}  {:>alloc_width$}  {:>allocated_width$}\n\n",
        "total",
        format::duration(total_microseconds),
        "100.00%",
        format::integer(u128::from(total_allocations)),
        format::bytes(total_allocated_bytes)
    ));
}

pub(super) fn write_backend_string_storage(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    let storage = count_backend_string_storage(backend_plan);

    output.push_str("## Backend String Storage\n");
    output.push_str("This counts `String` fields still carried by backend planning structures.\n");
    output.push_str("Identity strings are compiler debt; payload and generated symbols are expected later-stage text.\n\n");

    let rows = [
        (
            "identity",
            storage.identity_strings,
            storage.identity_bytes,
            "machine/state/name strings still used as identity",
        ),
        (
            "payload",
            storage.payload_strings,
            storage.payload_bytes,
            "program text literals copied into backend structures",
        ),
        (
            "generated symbols",
            storage.generated_symbol_strings,
            storage.generated_symbol_bytes,
            "labels, object symbols, and section names",
        ),
        (
            "report",
            storage.report_strings,
            storage.report_bytes,
            "diagnostic/report-only strings",
        ),
        (
            "total",
            storage.total_strings(),
            storage.total_bytes(),
            "all counted backend strings",
        ),
    ];
    let category_width = rows
        .iter()
        .map(|(category, _, _, _)| category.len())
        .chain(std::iter::once("category".len()))
        .max()
        .unwrap_or("category".len());
    let count_width = rows
        .iter()
        .map(|(_, count, _, _)| format::integer(*count as u128).len())
        .chain(std::iter::once("strings".len()))
        .max()
        .unwrap_or("strings".len());
    let bytes_width = rows
        .iter()
        .map(|(_, _, bytes, _)| format::bytes(*bytes as u64).len())
        .chain(std::iter::once("bytes".len()))
        .max()
        .unwrap_or("bytes".len());

    output.push_str(&format!(
        "{:<category_width$}  {:>count_width$}  {:>bytes_width$}  note\n",
        "category", "strings", "bytes"
    ));
    output.push_str(&format!(
        "{:-<category_width$}  {:-<count_width$}  {:-<bytes_width$}  {:-<4}\n",
        "", "", "", ""
    ));
    for (category, count, bytes, note) in rows {
        output.push_str(&format!(
            "{:<category_width$}  {:>count_width$}  {:>bytes_width$}  {}\n",
            category,
            format::integer(count as u128),
            format::bytes(bytes as u64),
            note
        ));
    }
    output.push('\n');
}
