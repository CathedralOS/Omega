//! A regression may pin the diagnostic of a valid source while its Psi handoff is missing.
pub fn report(
    label: &str,
    required: &str,
    source: &str,
    error: &omega_lowering::LoweringError,
) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_none() {
        return;
    }
    let span = error
        .site
        .span
        .slice(source.as_bytes())
        .expect("diagnostic belongs to this source");
    let detail = format!(
        "{required}. Observed {} at {}: {}",
        error.message,
        error.site.file,
        String::from_utf8_lossy(span)
    );
    println!("OMEGA_CASE\thandoff\t{label}\tL5\topen gap\t{}", detail.replace(['\r', '\n', '\t'], " "));
    let site = &error.site;
    let start = site.span.get_start_offset();
    let pos = omega_lexer::LineIndex::new(source.as_bytes()).get_position(start);
    println!(
        "OMEGA_GAP_LOCATION\t{label}\t{}\t{}\t{start}\t{}\t{}\t{}\t{}",
        site.package,
        site.file,
        site.span.get_end_offset(),
        pos.get_line(),
        pos.get_column(),
        String::from_utf8_lossy(span).escape_debug()
    );
}
