//! Existential field types depend on the particular value that supplied them.
//! These rows keep that relationship for later call, ownership and layout checks.
use crate::expression::ExpressionHandle;
use crate::types::TypeReferenceHandle;
use arena::Handle;
use symbols::SymbolHandle;

pub type PackageOpeningHandle = Handle<PackageOpening>;
impl Default for PackageSubjectRoot {
    fn default() -> Self {
        Self::Binding(SymbolHandle::invalid())
    }
}
fn expression_key(expression: ExpressionHandle) -> (u32, u32) {
    (expression.arena_index(), expression.generation())
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub enum PackageSubjectRoot {
    Binding(SymbolHandle),
    Evaluation(ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub enum PackageSubjectStep {
    Field(SymbolHandle),
    Dereference,
    ConstantIndex(u64),
    IndexBinding(SymbolHandle),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct PackageSubject {
    pub root: PackageSubjectRoot,
    pub steps: Vec<PackageSubjectStep>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct PackageOpening {
    pub declaration: SymbolHandle,
    pub state: SymbolHandle,
    pub subject: PackageSubject,
    pub storage_version: u32,
    /// In declaration order. A witness is an abstract nominal type in this scope.
    pub witnesses: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

/// The type of a field selection inside a particular opening. Its subject and
/// storage version belong to that opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct OpenedPackageField {
    pub expression: ExpressionHandle,
    pub field: SymbolHandle,
    pub opening: PackageOpeningHandle,
    pub result_type: TypeReferenceHandle,
}

impl crate::TypedTrees {
    pub fn package_opening(&self, handle: PackageOpeningHandle) -> &PackageOpening {
        self.package_openings.get(handle)
    }

    pub fn package_projection(&self, expression: ExpressionHandle) -> Option<&OpenedPackageField> {
        self.package_projections
            .binary_search_by_key(&expression_key(expression), |row| {
                expression_key(row.expression)
            })
            .ok()
            .map(|index| &self.package_projections[index])
    }

    pub fn retain_package_projection(
        &mut self,
        row: OpenedPackageField,
    ) -> Result<(), &'static str> {
        match self
            .package_projections
            .binary_search_by_key(&expression_key(row.expression), |old| {
                expression_key(old.expression)
            }) {
            Ok(i) if self.package_projections[i] == row => Ok(()),
            Ok(_) => Err("a shared package projection has incompatible opening scopes"),
            Err(i) => {
                self.package_projections.insert(i, row);
                Ok(())
            }
        }
    }
}
