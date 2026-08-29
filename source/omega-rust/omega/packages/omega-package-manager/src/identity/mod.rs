//! Package-authored names and source-qualified nominal identity.

mod names;

pub use names::{AliasName, PackageKey, PackageName};

#[cfg(test)]
mod tests;
