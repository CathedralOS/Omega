//! Service contracts travel with the checked program that uses them. Native
//! planning reads this normalized surface instead of reopening typed source.

use crate::TerminalIdentityIndex;
use language_semantics::service_surface::ServiceSurface;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSealedNativeIntrinsic {
    Hosted(language_core::sealed_intrinsics::HostedOperation),
    FixedImage(language_core::sealed_intrinsics::FixedImageOperation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalServiceSchema {
    pub declaration: TerminalIdentityIndex,
    pub application: TerminalIdentityIndex,
    pub surface: Arc<ServiceSurface>,
}

/// The entry source is selected before checking, when its captured bytes are
/// available. This commitment uses the installed physical contract's existing
/// digest recipe; it is descriptive and grants no package permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProgramEntryContract {
    pub target: String,
    pub entry: semantic_vocabulary::MachineId,
    pub schema_declaration: TerminalIdentityIndex,
    pub schema_application: TerminalIdentityIndex,
    pub physical_source_digest: [u8; 32],
}

/// Build precedence has already selected this source provider declaration.
/// Its nominal owner and its semantic implementation identity stay distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProviderSelection {
    pub target: String,
    pub composition_mode: language_semantics::declaration_selection::CompositionMode,
    pub schema_declaration: TerminalIdentityIndex,
    pub schema_application: TerminalIdentityIndex,
    pub provider_declaration: TerminalIdentityIndex,
    pub provider_identity: TerminalIdentityIndex,
    pub provider_application: Option<TerminalIdentityIndex>,
    pub provider_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub provider_custody: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
}

impl crate::TerminalModule {
    pub fn service_schema(
        &self,
        declaration: TerminalIdentityIndex,
        application: TerminalIdentityIndex,
    ) -> Result<&TerminalServiceSchema, String> {
        if declaration.is_absent() || application.is_absent() {
            return Err("service schema selection has an absent identity".into());
        }
        let mut rows = self
            .service_schemas
            .iter()
            .filter(|row| row.declaration == declaration && row.application == application);
        let (Some(row), None) = (rows.next(), rows.next()) else {
            return Err(
                "service schema selection does not name one exact declaration application".into(),
            );
        };
        let expected = self
            .identities
            .text(application)
            .filter(|identity| !identity.is_empty())
            .ok_or("service schema application is absent from the module identity table")?;
        if self.identities.text(declaration).is_none_or(str::is_empty)
            || row.surface.root_application_identity() != Some(expected)
        {
            return Err("service schema differs from its canonical declaration application".into());
        }
        row.surface.validate_receiver_applications()?;
        for method in &row.surface.methods {
            let identities = &row.surface.identities;
            if method.name.is_empty()
                || method.requirement_owner.is_empty()
                || identities.text(method.requirement_identity).is_empty()
                || (method.has_result && identities.text(method.result_type_identity).is_empty())
                || method.parameter_count != method.parameter_type_identities.len()
                || method
                    .parameter_type_identities
                    .iter()
                    .any(|id| identities.text(*id).is_empty())
                || method.entry_claims.iter().any(|claim| {
                    claim.parameter_index >= method.parameter_count
                        || identities.text(claim.carrier_identity).is_empty()
                })
                || method.termination_premises.iter().any(|premise| {
                    premise
                        .establishment_routes
                        .iter()
                        .any(|route| identities.text(route.requirement_identity).is_empty())
                })
            {
                return Err("service schema contains an incomplete method contract".into());
            }
        }
        Ok(row)
    }

    pub fn program_entry_contract(
        &self,
        target: &str,
    ) -> Result<&TerminalProgramEntryContract, String> {
        let mut rows = self
            .program_entry_contracts
            .iter()
            .filter(|row| row.target == target);
        let (Some(row), None) = (rows.next(), rows.next()) else {
            return Err(format!(
                "target `{target}` has no unique checked entry contract"
            ));
        };
        if row.physical_source_digest == [0; 32] {
            return Err("entry contract has no captured physical source commitment".into());
        }
        if self
            .machine(row.entry)
            .and_then(|machine| machine.declaration.as_ref())
            .is_none()
        {
            return Err("entry contract names no actual checked machine".into());
        }
        self.service_schema(row.schema_declaration, row.schema_application)?;
        Ok(row)
    }

    pub fn provider_selection(
        &self,
        target: &str,
        schema_declaration: TerminalIdentityIndex,
        schema_application: TerminalIdentityIndex,
    ) -> Result<&TerminalProviderSelection, String> {
        let mut rows = self.provider_selections.iter().filter(|row| {
            row.target == target
                && row.schema_declaration == schema_declaration
                && row.schema_application == schema_application
        });
        let (Some(row), None) = (rows.next(), rows.next()) else {
            return Err(format!(
                "target `{target}` has no unique selected provider for this schema application"
            ));
        };
        self.service_schema(schema_declaration, schema_application)?;
        for identity in [row.provider_declaration, row.provider_identity]
            .into_iter()
            .chain(row.provider_application)
        {
            if self.identities.text(identity).is_none_or(str::is_empty) {
                return Err("selected provider lost its complete canonical source identity".into());
            }
        }
        use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
        if !matches!((row.provider_custody, row.provider_package),
            (DeclarationCustody::Package(owner), Some(package)) if owner == package)
            && !matches!(
                (row.provider_custody, row.provider_package),
                (
                    DeclarationCustody::Toolchain | DeclarationCustody::ToolchainContract,
                    None
                )
            )
        {
            return Err("selected provider lost its exact package custody".into());
        }
        Ok(row)
    }
}
