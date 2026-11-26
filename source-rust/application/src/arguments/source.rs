//! Select a single-file syntax check or a source rewrite.
use super::{Arguments, Invocation, UsageError};

const PARSE: &str = "usage: omega source parse [--] <file>";
const SOURCE: &str = "usage: omega source parse [--] <file>\n       omega source rewrite <ExactRule>... [--check] <root.omg>";
const REWRITE: &str = "usage: omega source rewrite <ExactRule>... [--check] <root.omg>\nSource rewriting is specified and not implemented by this compiler.";

pub(super) fn parse(arguments: &mut Arguments) -> Result<Invocation, UsageError> {
    let action = arguments.next_argument();
    match action.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("parse") => parse_file(arguments),
        Some("rewrite")
            if arguments
                .remaining
                .iter()
                .any(|arg| arg == "--help" || arg == "-h") =>
        {
            Ok(Invocation::Help(REWRITE))
        }
        Some("rewrite") => {
            Err(format!("omega source rewrite is not implemented\n{REWRITE}").into())
        }
        Some("-h" | "--help") => Ok(Invocation::Help(SOURCE)),
        Some(other) => Err(format!("unknown source command `{other}`\n{SOURCE}").into()),
        None => Err(format!("missing source command\n{SOURCE}").into()),
    }
}

fn parse_file(arguments: &mut Arguments) -> Result<Invocation, UsageError> {
    let usage = || UsageError {
        message: PARSE.to_owned(),
        exit: 2,
    };
    let first = arguments.next_argument().ok_or_else(usage)?;
    if first == "--" {
        let path = arguments.next_value().ok_or_else(usage)?;
        if arguments.next_value().is_some() {
            return Err(usage());
        }
        return Ok(Invocation::ParseSource(path.into()));
    }
    if first == "--help" || first == "-h" {
        if arguments.next_argument().is_some() {
            return Err(usage());
        }
        return Ok(Invocation::Help(PARSE));
    }
    if first.as_encoded_bytes().starts_with(b"-") || arguments.next_argument().is_some() {
        return Err(usage());
    }
    Ok(Invocation::ParseSource(first.into()))
}
