//! Immutable representation facts supplied by the target owner to policy
//! evaluation. A profile name is a retained coordinate, never host authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct LayoutTargetFacts {
    pub profile: String,
    pub pointer_size: u64,
    pub pointer_alignment: u64,
    pub case_tag_size: u64,
    pub case_tag_alignment: u64,
    pub little_endian: bool,
}
impl LayoutTargetFacts {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.profile.is_empty()
            || self.pointer_size == 0
            || !self.pointer_alignment.is_power_of_two()
            || !self.pointer_size.is_multiple_of(self.pointer_alignment)
            || self.case_tag_size == 0
            || !self.case_tag_alignment.is_power_of_two()
            || !self.case_tag_size.is_multiple_of(self.case_tag_alignment)
        {
            return Err("layout target facts have no complete immutable representation profile");
        }
        Ok(())
    }
}
