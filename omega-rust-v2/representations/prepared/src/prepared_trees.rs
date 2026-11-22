use crate::{PreparationDependencies, PreparationResourceReceipt, PreparedFloat, PreparedValue, TargetObservation};
use num_bigint::BigInt;
use omega_resolved::{DeclarationId, TreeNodeId};
use omega_typed::{SpecializedIndexValue, TypeError, TypedTrees};
use std::collections::HashMap;
use std::sync::Arc;

/// A node number belongs to its defining forest. Library and package file
/// ordinals can overlap when a library clause is instantiated in the package.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct PreparedComparisons {
    pub package: HashMap<TreeNodeId, bool>,
    pub library: HashMap<TreeNodeId, bool>,
}

/// A selected policy awaiting evaluation. Its typed carrier names the subject;
/// a ready schema holds the actual core record to pass to the plan machine.
#[derive(Debug)]
pub struct PendingAccessPlan<'a> {
    pub selection: &'a omega_typed::AccessPolicySelection,
    pub carrier: &'a omega_typed::ValueType,
    pub schema: &'a PolicySchemaInput,
    pub pending_construction: Option<&'a PolicyConstruction>,
    checked_construction: bool,
}

impl PendingAccessPlan<'_> {
    /// Constructor correspondence is diagnostic progress. Policy execution and
    /// validation of the returned plan still belong to this pending demand.
    pub fn construction_checked(&self) -> bool {
        self.checked_construction
    }
}

/// Identifies one frozen occurrence even across independent preparations of the
/// same source and package. Keeping the construction alive prevents address reuse.
#[derive(Debug)]
pub struct PolicyConstructionOccurrence {
    domain: DeclarationId,
    construction: Arc<PolicyConstruction>,
}

impl PolicyConstructionOccurrence {
    pub fn domain(&self) -> &DeclarationId {
        &self.domain
    }
}

/// Frozen arguments and the outstanding construction of one selected invocation.
/// The Build owner and subject revision travel with the particular schema value.
#[derive(Debug)]
pub struct PolicyInvocation<'a> {
    pub selection: &'a omega_typed::AccessPolicySelection,
    pub package_key: &'a [u8; 32],
    pub schema: &'a PreparedValue,
    pub subject_revision: &'a [u8],
    pub construction: &'a PolicyConstruction,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PolicyPreparation {
    pub package_key: Option<[u8; 32]>,
    pub schema: PolicySchemaInput,
    pending_construction: Option<Arc<PolicyConstruction>>,
    checked_construction: bool,
}

impl PolicyPreparation {
    pub fn pending(package_key: Option<[u8; 32]>, schema: PolicySchemaInput, construction: Option<PolicyConstruction>) -> Self {
        Self { package_key, schema, pending_construction: construction.map(Arc::new), checked_construction: false }
    }
}

/// The checked body reached this data construction in its defining forest.
/// Written inputs have values. Each omission still owes the zero of its own
/// carrier, including establishment if that zero has a default domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyConstruction {
    pub machine: omega_typed::MachineApplication,
    /// Occurrence in the defining machine's own forest.
    pub node: TreeNodeId,
    pub site: omega_typed::Site,
    pub carrier: omega_typed::ValueType,
    pub fields: Vec<(DeclarationId, PolicyFieldInput)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyFieldInput {
    Written(PreparedValue),
    Zero(omega_typed::ValueType),
}

/// Progress constructing the one ordinary argument of a selected policy machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicySchemaInput {
    NeedsPackageOwner,
    UnsupportedSubject,
    Ready {
        argument: PreparedValue,
        subject_revision: Vec<u8>,
    },
}

/// Typed trees whose demanded closed indices have been evaluated and materialized.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedTrees<'text> {
    typed: TypedTrees<'text>,
    policies: HashMap<DeclarationId, PolicyPreparation>,
    constants: HashMap<DeclarationId, PreparedValue>,
    open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    specialized_indices: Vec<SpecializedIndexValue>,
    target_observations: Vec<TargetObservation>,
    resource_receipt: Option<PreparationResourceReceipt>,
    dependencies: PreparationDependencies,
    exact_comparisons: PreparedComparisons,
    /// Sorted by node, then format.
    landed_floats: Vec<(TreeNodeId, PreparedFloat)>,
}

impl<'text> PreparedTrees<'text> {
    /// Freeze the results from a completed preparation round with the typed
    /// package they describe.
    pub fn from_preparation(
        typed: TypedTrees<'text>,
        policies: HashMap<DeclarationId, PolicyPreparation>,
        constants: HashMap<DeclarationId, PreparedValue>,
        open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
        specialized_indices: Vec<SpecializedIndexValue>,
        target_observations: Vec<TargetObservation>,
        resource_receipt: Option<PreparationResourceReceipt>,
        dependencies: PreparationDependencies,
        exact_comparisons: PreparedComparisons,
        mut landed_floats: Vec<(TreeNodeId, PreparedFloat)>,
    ) -> Self {
        landed_floats.sort_unstable_by_key(|(node, value)| (*node, value.format() == omega_target_contract::FloatFormat::Binary64));
        landed_floats.dedup();
        Self {
            typed,
            policies,
            constants,
            open_target_constants,
            specialized_indices,
            target_observations,
            resource_receipt,
            dependencies,
            landed_floats,
            exact_comparisons,
        }
    }

    /// The bits of the anonymous numeric expression that typing landed at
    /// `node` in `format`, rounded once to nearest, ties to even. `None` when
    /// no such landing exists or two landings there disagree.
    pub fn landed_float(
        &self,
        node: TreeNodeId,
        format: omega_target_contract::FloatFormat,
    ) -> Option<PreparedFloat> {
        let start = self.landed_floats.partition_point(|(at, _)| *at < node);
        let mut matches = self.landed_floats[start..]
            .iter()
            .take_while(|(at, _)| *at == node)
            .filter(|(_, value)| value.format() == format)
            .map(|(_, value)| *value);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    /// The truth of a selected comparison between two wholly anonymous numbers.
    pub fn exact_comparison(
        &self,
        tree: &omega_typed::TypedTree<'_>,
        node: usize,
    ) -> Option<bool> {
        let facts = if std::ptr::eq(tree.forest(), &self.typed) {
            &self.exact_comparisons.package
        } else if self
            .typed
            .library
            .is_some_and(|library| std::ptr::eq(tree.forest(), library))
        {
            &self.exact_comparisons.library
        } else {
            return None;
        };
        facts.get(&tree.resolved().file.node_id(node)).copied()
    }

    /// The policy this package still owes for the given domain. An authored
    /// constructor or an issuance argument cannot discharge this demand.
    pub fn pending_access_plan(&self, domain: &DeclarationId) -> Option<PendingAccessPlan<'_>> {
        let selection = self.typed.access_policies.iter().find(|policy| &policy.domain == domain)?;
        let carrier = self.typed.domain_carriers.get(domain)?;
        let progress = self.policies.get(domain);
        Some(PendingAccessPlan {
            selection,
            carrier,
            schema: progress.map_or(&PolicySchemaInput::UnsupportedSubject, |p| &p.schema),
            pending_construction: progress.and_then(|p| p.pending_construction.as_deref()),
            checked_construction: progress.is_some_and(|p| p.checked_construction),
        })
    }

    /// The closed inputs L4 actually prepared for this domain's constructor.
    /// A missing owner, unsupported schema, or unevaluated input leaves the
    /// demand pending without supplying a construction to the checker.
    pub fn policy_invocation(&self, domain: &DeclarationId) -> Option<PolicyInvocation<'_>> {
        let progress = self.policies.get(domain)?;
        let PolicySchemaInput::Ready { argument, subject_revision } = &progress.schema else {
            return None;
        };
        let selection = self.typed.access_policies.iter().find(|policy| &policy.domain == domain)?;
        Some(PolicyInvocation {
            selection,
            package_key: progress.package_key.as_ref()?,
            schema: argument,
            subject_revision,
            construction: progress.pending_construction.as_deref()?,
        })
    }

    pub fn policy_construction_occurrence(&self, domain: &DeclarationId) -> Option<PolicyConstructionOccurrence> {
        self.policy_invocation(domain)?;
        Some(PolicyConstructionOccurrence {
            domain: domain.clone(),
            construction: self.policies.get(domain)?.pending_construction.clone()?,
        })
    }

    /// L4 publishes a comparison against the observed input of the checker.
    /// A receipt from another preparation cannot advance this occurrence, even
    /// when the program and schema bytes are identical. This records only the
    /// comparison, never an evaluated plan or an authorization decision.
    pub fn record_policy_construction_comparison(
        &mut self,
        occurrence: PolicyConstructionOccurrence,
        package_key: &[u8; 32],
        subject_revision: &[u8],
        image: &[u8],
        selection_site: &omega_typed::Site,
        construction_site: &omega_typed::Site,
    ) -> Result<(), &'static str> {
        let invocation = self.policy_invocation(&occurrence.domain).ok_or("policy construction is not pending")?;
        let PreparedValue::Record { fields, .. } = invocation.schema else { return Err("policy schema has no record") };
        let [PreparedValue::SharedBytes(bytes)] = fields.as_ref() else { return Err("policy schema has no byte view") };
        if !std::ptr::eq(invocation.construction, occurrence.construction.as_ref())
            || invocation.package_key != package_key || invocation.subject_revision != subject_revision || bytes.as_ref() != image
            || &invocation.selection.site != selection_site || &invocation.construction.site != construction_site
        {
            return Err("checked construction belongs to different policy inputs");
        }
        self.policies.get_mut(&occurrence.domain).expect("matched pending policy").checked_construction = true;
        Ok(())
    }

    pub fn typed(&self) -> &TypedTrees<'text> {
        &self.typed
    }

    /// The frozen value of a local integer declaration.
    pub fn integer_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&BigInt> {
        match self.constant(declaration)? {
            PreparedValue::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&PreparedValue> {
        self.constants.get(declaration)
    }

    pub fn dependencies(&self) -> &PreparationDependencies {
        &self.dependencies
    }

    pub fn resource_receipt(&self) -> Option<&PreparationResourceReceipt> {
        self.resource_receipt.as_ref()
    }

    /// Identify the live target dependency of an open constant. The original
    /// initializer remains a typed recipe in the typed stage.
    pub fn open_target_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&TypeError> {
        self.open_target_constants.get(declaration).map(Arc::as_ref)
    }

    /// Target inputs used anywhere in this prepared package, including inputs
    /// that selected an overload during an earlier typing pass.
    pub fn target_observations(&self) -> &[TargetObservation] {
        &self.target_observations
    }

    /// Each answer identifies both the source operation and the owner arguments
    /// that closed it. One generic field can have a different value at each use.
    pub fn specialized_indices(&self) -> &[SpecializedIndexValue] {
        &self.specialized_indices
    }

    /// Extract the typed stage, dropping preparation results owned by this stage.
    pub fn into_typed(self) -> TypedTrees<'text> {
        self.typed
    }
}
