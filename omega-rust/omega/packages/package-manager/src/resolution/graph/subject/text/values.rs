// We charge typed field conversions against the recovery budget and validate
// package names and aliases before calling their constructors so invalid tokens
// are rejected without extra error-formatting allocations.

use super::Error;
use super::framing::Reader;
use crate::declarations::{AliasName, PackageName};
use package_source::SourceRelativePath;

impl Reader<'_> {
    pub(super) fn package_name(&mut self, maximum: usize) -> Result<PackageName, Error> {
        let value = self.string(maximum)?;
        if !build_declarations::ProjectName::is_valid(&value) {
            return Err(Error::new("invalid text package name"));
        }
        build_declarations::ProjectName::parse(value)
            .map(PackageName::from)
            .map_err(|_| Error::new("invalid text package name"))
    }

    pub(super) fn alias(&mut self, maximum: usize) -> Result<AliasName, Error> {
        let value = self.string(maximum)?;
        if !AliasName::is_valid(&value) {
            return Err(Error::new("invalid text alias"));
        }
        AliasName::parse(value).map_err(|_| Error::new("invalid text alias"))
    }

    pub(super) fn relative_path(&mut self, maximum: usize) -> Result<SourceRelativePath, Error> {
        let value = self.string(maximum)?;
        self.budget.charge(value.len())?;
        SourceRelativePath::parse(&value).map_err(|_| Error::new("invalid text relative path"))
    }

    pub(super) fn hex_string(&mut self, maximum: usize) -> Result<String, Error> {
        let value = self.string(maximum)?;
        self.budget.charge(value.len() / 2)?;
        Ok(value)
    }
}
