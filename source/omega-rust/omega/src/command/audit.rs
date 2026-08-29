use std::ffi::OsString;

const SOURCE_USAGE: &str = "usage: omega audit source --kind <local|git> <locator> [--rev <rev>]";

pub(super) fn run(arguments: impl Iterator<Item = OsString>) {
    let mut arguments = arguments;
    let Some(subcommand) = arguments.next() else {
        usage();
        std::process::exit(2);
    };
    if subcommand == "source" {
        source(arguments);
        return;
    }
    eprintln!("unknown audit command `{}`", subcommand.to_string_lossy());
    usage();
    std::process::exit(2);
}

fn usage() {
    eprintln!("{SOURCE_USAGE}");
}

fn source(arguments: impl Iterator<Item = OsString>) {
    warn_unhardened_source_resolver();
    let Some(arguments) = parse_source_arguments(arguments) else {
        eprintln!("{SOURCE_USAGE}");
        std::process::exit(2);
    };
    let adapter =
        match omega_package_manager::operations::SourceAdapter::parse(&arguments.source_kind) {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("invalid source adapter: {error:?}");
                std::process::exit(2);
            }
        };
    let storage = match omega_package_source::SourceResolverStorage::for_current_user() {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("cannot open private source resolver storage: {error}");
            std::process::exit(1);
        }
    };
    match omega_package_manager::operations::audit_package_source_locator(
        adapter,
        arguments.locator,
        arguments.rev,
        &storage,
        omega_package_source::LocalSourceLimits::default(),
    ) {
        Ok(report) => print!("{}", report.to_text()),
        Err(error) => {
            eprintln!("cannot audit package source: {error:?}");
            std::process::exit(1);
        }
    }
}

fn warn_unhardened_source_resolver() {
    eprintln!(
        "warning: source audit is diagnostic and non-admitting; strict native \
         confinement on every platform, TLS/SSH credential custody, aggregate \
         CPU/memory/process/object-store accounting, and an accepted source \
         receipt remain unavailable"
    );
}

struct SourceArguments {
    source_kind: String,
    locator: String,
    rev: Option<String>,
}

fn parse_source_arguments(
    mut arguments: impl Iterator<Item = OsString>,
) -> Option<SourceArguments> {
    let mut locator = None;
    let mut source_kind = None;
    let mut rev = None;
    while let Some(argument) = arguments.next() {
        if argument == "--kind" {
            if source_kind.is_some() {
                return None;
            }
            source_kind = arguments.next().and_then(|value| value.into_string().ok());
            source_kind.as_ref()?;
            continue;
        }
        if argument == "--rev" {
            if rev.is_some() {
                return None;
            }
            rev = arguments.next().and_then(|value| value.into_string().ok());
            rev.as_ref()?;
            continue;
        }
        if locator.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        locator = Some(argument.into_string().ok()?);
    }
    Some(SourceArguments {
        source_kind: source_kind?,
        locator: locator?,
        rev,
    })
}
