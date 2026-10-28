//! One source occurrence's complete Build-output inventory. Package and Core
//! owners share the same carrier and receiving admission, without inventing a
//! package graph node for Toolchain declarations.

use crate::{BuildSourceOwner, PackageBuildHandoff};
use build_declarations::DependencyPurpose;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildHandoffInventory {
    handoffs: Vec<PackageBuildHandoff>,
}

fn key(handoff: &PackageBuildHandoff) -> (BuildSourceOwner, DependencyPurpose, u8, Option<u8>) {
    (
        handoff.source_owner(),
        handoff.purpose(),
        crate::target_profile_tags::target_profile_tag(handoff.target()),
        handoff
            .build_execution_profile()
            .map(crate::target_profile_tags::target_profile_tag),
    )
}

impl BuildHandoffInventory {
    pub fn handoffs(&self) -> &[PackageBuildHandoff] {
        &self.handoffs
    }

    pub fn insert(&mut self, handoff: PackageBuildHandoff) -> Result<(), String> {
        match self.handoffs.binary_search_by_key(&key(&handoff), key) {
            Ok(index) if self.handoffs[index] == handoff => Ok(()),
            Ok(_) => Err(
                "Build handoff inventory has different outputs for one admitted source occurrence"
                    .into(),
            ),
            Err(index) => {
                self.handoffs.insert(index, handoff);
                Ok(())
            }
        }
    }

    pub fn find(
        &self,
        owner: BuildSourceOwner,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        execution: Option<target::TargetProfile>,
    ) -> Option<&PackageBuildHandoff> {
        let key = (
            owner,
            purpose,
            crate::target_profile_tags::target_profile_tag(target),
            execution.map(crate::target_profile_tags::target_profile_tag),
        );
        self.handoffs
            .binary_search_by_key(&key, self::key)
            .ok()
            .map(|index| &self.handoffs[index])
    }
}
