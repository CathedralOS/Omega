//! Physical geometry and an optional exact deployment identity for layout.
//! Native shape alone cannot distinguish profiles such as Windows and UEFI.

use target::{NativeTarget, TargetProfile};

/// Inputs already selected by the embedding realization owner. A raw native
/// shape can consume invariant plans; a deployment profile can select its exact
/// retained policy row. This type supplies no layout or access authority.
#[derive(Debug, Clone, Copy)]
pub struct LayoutTarget {
    native: NativeTarget,
    profile: Option<TargetProfile>,
}

impl LayoutTarget {
    pub fn for_profile(profile: TargetProfile) -> Result<Self, String> {
        profile.layout_target_facts().ok_or_else(|| {
            format!(
                "target `{}` has no admitted physical layout facts",
                profile.target_name()
            )
        })?;
        let native = profile.native_realization().ok_or_else(|| {
            format!(
                "target `{}` has no native realization",
                profile.target_name()
            )
        })?;
        Ok(Self {
            native,
            profile: Some(profile),
        })
    }

    pub fn native_target(self) -> NativeTarget {
        self.native
    }

    pub(crate) fn into_parts(self) -> (NativeTarget, Option<TargetProfile>) {
        (self.native, self.profile)
    }
}

impl From<NativeTarget> for LayoutTarget {
    fn from(native: NativeTarget) -> Self {
        Self {
            native,
            profile: None,
        }
    }
}
