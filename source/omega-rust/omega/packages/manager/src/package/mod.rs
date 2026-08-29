//! Package-authored names and source-qualified nominal identity.

mod identity;

pub use identity::{AliasName, PackageKey, PackageName};

#[cfg(test)]
mod tests;
