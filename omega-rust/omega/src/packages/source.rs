use crate::arguments::SourceArguments;

pub(crate) fn run(arguments: SourceArguments) {
    let adapter = match package_manager::operations::SourceAdapter::parse(&arguments.source_kind) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("invalid source adapter: {error:?}");
            std::process::exit(2);
        }
    };
    let storage = match package_source::SourceResolverStorage::for_current_user() {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("cannot open private source resolver storage: {error}");
            std::process::exit(1);
        }
    };
    match package_manager::operations::inspect_package_source_locator(
        adapter,
        arguments.locator,
        arguments.rev,
        &storage,
        package_source::LocalSourceLimits::default(),
    ) {
        Ok(report) => print!("{}", report.to_text()),
        Err(error) => {
            eprintln!("cannot inspect package source: {error:?}");
            std::process::exit(1);
        }
    }
}
