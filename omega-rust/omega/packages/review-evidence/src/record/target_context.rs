//! Authored deployment scope of a disclosed callable, not provider selection.
//!
//! One activation reviews every authored leaf, including inactive private
//! external supplies. Its selected target therefore cannot stand in for each
//! leaf's authored target. Uncatalogued tags remain valid inert disclosure for
//! pure bodies; a physical binding still needs a catalogued profile.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTargetContext {
    TargetIndependent,
    TargetMember { target: String },
}

impl PackagePolicyTargetContext {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if let Self::TargetMember { target } = self
            && target.is_empty()
        {
            return Err("authored target member has an empty target tag");
        }
        Ok(())
    }
}
