/*
Completed independent component products and their `OMGCMP01` retention codec.

Package review consumes serialized component descriptions, while executable
composition also requires the exact canonical Terminal artifact and completed
native proposal that produced them. `IndependentComponentProduct` couples those
outputs before their source continuation leaves the compiler invocation, and
`SelectedIndependentComponent` binds a verified component to its receiving
provider plan without reopening the dependency's source or Build.
*/

/*
`encode_retained` and `recover_retained` serialize and recover a coupled
component through `component_description` and
`terminal_psi_to_abstract_operations::recover_native_derivation_inputs`,
rejecting any record that exceeds `MAX_BYTES` (128 MiB) or attempts to carry
application installation assumptions.
*/

use package_compilation::IndependentComponentDescription;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::{
    RetainedComponentNativeInputs, RetainedTerminalArtifact,
};

const MAGIC: &[u8; 8] = b"OMGCMP01";
const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIndependentComponent {
    input: RetainedComponentNativeInputs,
    plan: effects::provider_plan::ProviderPlan,
    receiving_identities: Arc<terminal_psi::TerminalIdentityTable>,
    accepted_package_permissions:
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
}

impl SelectedIndependentComponent {
    pub fn join(
        component: Arc<component_description::VerifiedComponent>,
        product: &IndependentComponentProduct,
        plan: effects::provider_plan::ProviderPlan,
        selecting_declaration: &str,
        receiving_identities: Arc<terminal_psi::TerminalIdentityTable>,
        applications: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<Self, String> {
        let input = RetainedComponentNativeInputs::join(
            component,
            Arc::clone(&product.retained),
            &plan,
            selecting_declaration,
            applications,
        )?;
        Ok(Self {
            input,
            plan,
            receiving_identities,
            accepted_package_permissions: product.accepted_package_permissions.clone(),
        })
    }

    pub fn input(
        &self,
    ) -> Result<terminal_psi_to_abstract_operations::ComponentNativeInputs<'_>, String> {
        self.input.borrow()
    }

    pub fn plan(&self) -> &effects::provider_plan::ProviderPlan {
        &self.plan
    }

    pub fn receiving_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.receiving_identities
    }

    pub fn accepted_package_permissions(
        &self,
    ) -> &terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy
    {
        &self.accepted_package_permissions
    }
}

#[derive(Debug, Clone)]
pub struct IndependentComponentProduct {
    description: IndependentComponentDescription,
    retained: Arc<RetainedTerminalArtifact>,
    accepted_package_permissions:
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    native_derivation_inputs: Option<Result<Arc<[u8]>, String>>,
}

impl PartialEq for IndependentComponentProduct {
    fn eq(&self, other: &Self) -> bool {
        self.description == other.description
            && self.retained == other.retained
            && self.accepted_package_permissions == other.accepted_package_permissions
    }
}

impl Eq for IndependentComponentProduct {}

impl IndependentComponentProduct {
    pub fn new(
        description: IndependentComponentDescription,
        retained: RetainedTerminalArtifact,
        accepted_package_permissions: terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    ) -> Result<Self, &'static str> {
        if description.expected_subject() != retained.artifact().view().identity() {
            return Err("component description and retained product name different subjects");
        }
        Ok(Self {
            description,
            retained: Arc::new(retained),
            accepted_package_permissions,
            native_derivation_inputs: None,
        })
    }

    pub const fn description(&self) -> &IndependentComponentDescription {
        &self.description
    }

    pub fn with_native_derivation_inputs(mut self, inputs: Result<Vec<u8>, String>) -> Self {
        self.native_derivation_inputs = Some(inputs.map(Arc::from));
        self
    }

    pub const fn accepted_package_permissions(
        &self,
    ) -> &terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy
    {
        &self.accepted_package_permissions
    }

    pub fn retained(&self) -> &RetainedTerminalArtifact {
        &self.retained
    }

    pub fn encode_retained(&self) -> Result<Option<Vec<u8>>, String> {
        let Some(inputs) = &self.native_derivation_inputs else {
            return Ok(None);
        };
        let inputs = inputs.as_ref().map_err(Clone::clone)?;
        let description = self.description.description();
        let size = MAGIC.len() + 32 + 16;
        let size = size
            .checked_add(description.len())
            .and_then(|size| size.checked_add(inputs.len()))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or("component retention record exceeds its byte bound")?;
        let mut bytes = Vec::with_capacity(size);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&self.description.package().digest());
        bytes.extend_from_slice(&(description.len() as u64).to_le_bytes());
        bytes.extend_from_slice(description);
        bytes.extend_from_slice(&(inputs.len() as u64).to_le_bytes());
        bytes.extend_from_slice(inputs);
        Ok(Some(bytes))
    }

    pub fn recover_retained(package: PackageKeyIdentity, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("component retention record exceeds its byte bound".to_owned());
        }
        let mut reader = Reader { bytes, position: 0 };
        if reader.take(MAGIC.len())? != MAGIC || reader.take(32)? != package.digest() {
            return Err("component retention schema or package identity differs".to_owned());
        }
        let description_bytes = reader.section()?;
        let native_inputs = reader.section()?;
        if reader.position != bytes.len() {
            return Err("component retention has trailing bytes".to_owned());
        }
        let (_, artifact) =
            component_description::decode_component_description_with_artifact(description_bytes)
                .map_err(|reason| format!("retained component description: {reason:?}"))?;
        let description = IndependentComponentDescription::new(
            package,
            artifact.view().identity(),
            description_bytes.to_vec(),
        );
        let (retained, permissions, installation_assumptions) =
            terminal_psi_to_abstract_operations::recover_native_derivation_inputs(
                Arc::new(artifact),
                native_inputs,
            )
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
        if !installation_assumptions.is_empty() {
            return Err(
                "component inputs cannot accept application installation conditions".into(),
            );
        }
        Self::new(description, retained, permissions)
            .map_err(str::to_owned)
            .map(|product| product.with_native_derivation_inputs(Ok(native_inputs.to_vec())))
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> Reader<'bytes> {
    fn take(&mut self, length: usize) -> Result<&'bytes [u8], String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or("component retention length overflow")?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or("truncated component retention record")?;
        self.position = end;
        Ok(slice)
    }

    fn section(&mut self) -> Result<&'bytes [u8], String> {
        let length = u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|reason| format!("component retention section width: {reason}"))?,
        );
        self.take(
            usize::try_from(length)
                .map_err(|reason| format!("component retention section length: {reason}"))?,
        )
    }
}
