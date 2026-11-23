//! A selected policy unit and the frozen source occurrence it lowers.
use crate::{LoweredMachine, LoweredPackage, LoweringError, PolicySchemaValue};
use omega_prepared::{PolicyConstructionOccurrence, PreparedTrees, PreparedValue};
use omega_resolved::DeclarationId;
use omega_typed::{AccessPolicySelection, Site};
use std::sync::Arc;

/// One frozen invocation of a selected policy in one prepared package.
#[derive(Debug)]
pub struct FrozenPolicyInvocation {
    /// Identifies the construction frozen by this preparation.
    occurrence: PolicyConstructionOccurrence,
    /// The schema argument, with the backing the prepared package froze.
    schema: PolicySchemaValue,
    selection: Site,
}

fn schema_backing(schema: &PreparedValue) -> Option<&Arc<[u8]>> {
    let PreparedValue::Record { fields, .. } = schema else { return None };
    match fields.as_ref() {
        [PreparedValue::SharedBytes(backing)] => Some(backing),
        _ => None,
    }
}

impl FrozenPolicyInvocation {
    pub fn occurrence(&self) -> &PolicyConstructionOccurrence {
        &self.occurrence
    }

    pub fn selection_site(&self) -> &Site {
        &self.selection
    }

    /// The invocation `prepared` froze for `selection`, when it has one.
    fn of(
        prepared: &PreparedTrees<'_>,
        selection: &AccessPolicySelection,
    ) -> Result<Option<Arc<Self>>, LoweringError> {
        let (Some(invocation), Some(occurrence)) = (
            prepared.policy_invocation(&selection.domain),
            prepared.policy_construction_occurrence(&selection.domain),
        ) else {
            return Ok(None);
        };
        Ok(Some(Arc::new(Self {
            occurrence,
            schema: super::policy_construction::schema_value(invocation.schema, &selection.site)?,
            selection: selection.site.clone(),
        })))
    }

    /// Whether `prepared` is the package that froze this invocation. Another
    /// preparation of the same source has an equal construction and equal
    /// schema bytes in other allocations, and is a different package.
    pub fn belongs_to(
        &self,
        prepared: &PreparedTrees<'_>,
    ) -> bool {
        prepared
            .policy_invocation(self.occurrence.domain())
            .is_some_and(|current| {
                self.occurrence.matches(&current) && schema_backing(current.schema).is_some_and(|backing| Arc::ptr_eq(backing, &self.schema.storage))
            })
    }
}

/// The closed unit of one selected policy: the selected `plan` body and every
/// body its calls reach. It dereferences to the [`LoweredPackage`] for readers
/// and the checker. Nothing can replace or edit that package, so an execution
/// receipt that names it by address ran exactly what lowering produced.
#[derive(Debug)]
pub struct PolicyUnit {
    package: LoweredPackage,
    /// Position of the selected closed body in `package.machines`.
    plan: usize,
    selection: Site,
    invocation: Option<Arc<FrozenPolicyInvocation>>,
}

impl std::ops::Deref for PolicyUnit {
    type Target = LoweredPackage;
    fn deref(&self) -> &LoweredPackage {
        &self.package
    }
}

impl PolicyUnit {
    pub(crate) fn new(
        prepared: &PreparedTrees<'_>,
        selection: &AccessPolicySelection,
        package: LoweredPackage,
        plan: usize,
    ) -> Result<Self, LoweringError> {
        Ok(Self {
            package,
            plan,
            selection: selection.site.clone(),
            invocation: FrozenPolicyInvocation::of(prepared, selection)?,
        })
    }

    pub fn package(&self) -> &LoweredPackage {
        &self.package
    }

    /// Consume the source-bound unit to obtain standalone Psi for checking or mutation.
    pub fn into_package(self) -> LoweredPackage {
        self.package
    }

    /// The selected policy's closed body, which is the entry to execute.
    pub fn plan(&self) -> &LoweredMachine {
        &self.package.machines[self.plan]
    }

    /// The domain whose policy this unit runs, once preparation froze its invocation.
    pub fn domain(&self) -> Option<&DeclarationId> {
        self.invocation
            .as_ref()
            .map(|invocation| invocation.occurrence.domain())
    }

    /// The schema argument of the frozen invocation, including its original backing.
    pub fn schema(&self) -> Option<&PolicySchemaValue> {
        self.invocation.as_ref().map(|invocation| &invocation.schema)
    }

    pub fn invocation(&self) -> Option<&Arc<FrozenPolicyInvocation>> {
        self.invocation.as_ref()
    }

    pub fn selection_site(&self) -> &Site {
        &self.selection
    }
}
