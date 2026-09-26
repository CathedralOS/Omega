use std::ffi::OsString;

pub(crate) struct SourceArguments {
    pub(crate) source_kind: String,
    pub(crate) locator: String,
    pub(crate) rev: Option<String>,
}

pub(super) fn parse_source_arguments(
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
