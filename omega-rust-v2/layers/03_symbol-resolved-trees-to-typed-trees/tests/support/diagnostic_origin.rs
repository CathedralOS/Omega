use omega_typer::TypeError;

pub fn report_return(
    error: &TypeError,
    fixture: &str,
    expected: Option<(&str, &str, usize, usize, &str)>,
) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        let origin = error.diagnostic_origin();
        let field = spec_corpus::diagnostic_field;
        let phase = if error.waiting_for_static_value() { "pending" } else { "refused" };
        let (tag, expectation) = match expected {
            Some((package, file, start, end, reason)) => (
                "OMEGA_TYPING_RETURN_CHECKED",
                format!("\t{}\t{}\t{start}\t{end}\t{}", field(package), field(file), field(reason)),
            ),
            None => ("OMEGA_TYPING_RETURN", String::new()),
        };
        println!(
            "{tag}\t{}\t{}\t{}\t{}\t{phase}\t{}\t{}\t{}\t{}\t{}\t{}{expectation}",
            field(fixture),
            field(&origin.file().replace('\\', "/")),
            origin.line(),
            origin.column(),
            field(&error.package),
            field(&error.file),
            error.span.get_start_offset(),
            error.span.get_end_offset(),
            error.message.len(),
            field(&error.message)
        );
    }
}
