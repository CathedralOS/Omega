//! One source occurrence's complete Build-output inventory. Package and Core
//! owners share the same carrier and receiving admission, without inventing a
//! package graph node for Toolchain declarations.

use crate::{BuildSourceOwner, PackageBuildHandoff};
use build_declarations::DependencyPurpose;

mod admission;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildHandoffInventory {
    handoffs: Vec<BuildHandoffInstance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildHandoffInstance {
    consumer_scope: DependencyPurpose,
    handoff: PackageBuildHandoff,
}

fn key(
    instance: &BuildHandoffInstance,
) -> (
    DependencyPurpose,
    BuildSourceOwner,
    DependencyPurpose,
    Option<u8>,
    Option<u8>,
) {
    let handoff = &instance.handoff;
    (
        instance.consumer_scope,
        handoff.source_owner(),
        handoff.purpose(),
        handoff
            .target()
            .map(crate::target_profile_tags::target_profile_tag),
        handoff
            .build_execution_profile()
            .map(crate::target_profile_tags::target_profile_tag),
    )
}

impl BuildHandoffInventory {
    pub fn handoffs(&self) -> impl Iterator<Item = (DependencyPurpose, &PackageBuildHandoff)> {
        self.handoffs
            .iter()
            .map(|instance| (instance.consumer_scope, &instance.handoff))
    }

    pub fn insert(
        &mut self,
        consumer_scope: DependencyPurpose,
        handoff: PackageBuildHandoff,
    ) -> Result<(), String> {
        let instance = BuildHandoffInstance {
            consumer_scope,
            handoff,
        };
        match self.handoffs.binary_search_by_key(&key(&instance), key) {
            Ok(index) if self.handoffs[index] == instance => Ok(()),
            Ok(_) => Err(
                "Build handoff inventory has different outputs for one admitted source occurrence"
                    .into(),
            ),
            Err(index) => {
                self.handoffs.insert(index, instance);
                Ok(())
            }
        }
    }

    pub fn find(
        &self,
        consumer_scope: DependencyPurpose,
        owner: BuildSourceOwner,
        purpose: DependencyPurpose,
        target: Option<target::TargetProfile>,
        execution: Option<target::TargetProfile>,
    ) -> Option<&PackageBuildHandoff> {
        let key = (
            consumer_scope,
            owner,
            purpose,
            target.map(crate::target_profile_tags::target_profile_tag),
            execution.map(crate::target_profile_tags::target_profile_tag),
        );
        self.handoffs
            .binary_search_by_key(&key, self::key)
            .ok()
            .map(|index| &self.handoffs[index].handoff)
    }
}
