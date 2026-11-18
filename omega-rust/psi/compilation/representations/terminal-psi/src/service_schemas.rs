//! Service contracts travel with the checked program that uses them. Native
//! planning reads this normalized surface instead of reopening typed source.

use crate::TerminalIdentityIndex;
use declaration_contracts::service_surface::ServiceSurface;
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

impl TerminalServiceSchema {
    /// Several source consumers can retain the same application. Coalesce only
    /// equal complete surfaces; an existing duplicate or a changed contract is
    /// an inconsistent namespace, not another available implementation.
    pub fn join(existing: &[Self], incoming: &[Self]) -> Result<Vec<Self>, String> {
        let mut rows = existing
            .iter()
            .map(|row| (true, row))
            .chain(incoming.iter().map(|row| (false, row)))
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(_, row)| (row.declaration, row.application));
        let mut joined = Vec::with_capacity(rows.len());
        let mut start = 0;
        while start < rows.len() {
            let key = (rows[start].1.declaration, rows[start].1.application);
            let mut end = start + 1;
            while end < rows.len() && (rows[end].1.declaration, rows[end].1.application) == key {
                end += 1;
            }
            let group = &rows[start..end];
            if key.0.is_absent()
                || key.1.is_absent()
                || group.iter().filter(|(existing, _)| *existing).count() > 1
                || group
                    .iter()
                    .any(|(_, row)| row.surface != group[0].1.surface)
            {
                return Err(
                    "service schema publication conflicts with its exact declaration application"
                        .into(),
                );
            }
            joined.push(group[0].1.clone());
            start = end;
        }
        Ok(joined)
    }
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
    pub composition_mode: declaration_contracts::selection::CompositionMode,
    pub schema_declaration: TerminalIdentityIndex,
    pub schema_application: TerminalIdentityIndex,
    pub provider_declaration: TerminalIdentityIndex,
    pub provider_identity: TerminalIdentityIndex,
    pub provider_application: Option<TerminalIdentityIndex>,
    pub provider_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub provider_custody: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
}

impl crate::TerminalModule {
    /// Recognize only a receiver-bundled filesystem capability application.
    /// The descriptive application joins its retained exact declaration before
    /// recognition; neither a familiar name nor successful parsing grants host
    /// access. The host still supplies the filesystem and admits each mechanism.
    pub fn core_filesystem_service(
        &self,
        service: semantic_vocabulary::ServiceId,
    ) -> Result<language_core::core_vocabulary::FilesystemService, String> {
        use application_identity::type_identity::encoding::{CoordinateTag, atom, bare, compound};
        use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
        use language_core::core_vocabulary::{CoreVocabulary, CoreVocabularyCatalog};

        let mut services = self.services.iter().filter(|row| row.id == service);
        let (Some(service), None) = (services.next(), services.next()) else {
            return Err("filesystem service has no unique retained declaration".into());
        };
        if !service.parents.is_empty() {
            return Err("core filesystem capability has unexpected parent services".into());
        }
        let mut schemas = self
            .service_schemas
            .iter()
            .filter(|row| row.application == service.identity);
        let (Some(schema), None) = (schemas.next(), schemas.next()) else {
            return Err("filesystem service has no unique retained schema application".into());
        };
        let schema = self.service_schema(schema.declaration, schema.application)?;
        let declaration = self
            .identities
            .text(schema.declaration)
            .ok_or("filesystem service has no retained exact owner")?;
        let kind = read_core_filesystem_declaration(declaration)?;
        let catalog = CoreVocabularyCatalog::declaration(CoreVocabulary::FilesystemService(kind))
            .ok_or("filesystem service is absent from the core catalog")?;
        let descriptive = compound(
            NameForm::Nominal,
            [
                bare(OwnerForm::Toolchain),
                atom(CoordinateTag::Path, catalog.declaration),
            ],
        );
        let application = compound(
            CoordinateTag::TraitApplication,
            [
                atom(CoordinateTag::Declaration, &descriptive),
                compound(CoordinateTag::Arguments, []),
                compound(CoordinateTag::Lifetimes, []),
            ],
        );
        let surface = &schema.surface;
        let [root] = surface.receiver_applications.as_slice() else {
            return Err("core filesystem capability has unexpected receiver applications".into());
        };
        if self.identities.text(service.identity) != Some(application.as_str())
            || surface.trait_package_identity.is_some()
            || surface.identity_text(surface.trait_name) != catalog.declaration
            || !surface.methods.is_empty()
            || !root.argument_identities.is_empty()
            || !root.lifetime_argument_identities.is_empty()
            || !root.requirement_identities.is_empty()
        {
            return Err(
                "filesystem service differs from its exact core capability contract".into(),
            );
        }
        Ok(kind)
    }

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

fn read_core_filesystem_declaration(
    identity: &str,
) -> Result<language_core::core_vocabulary::FilesystemService, String> {
    use application_identity::type_identity::vocabulary::{OwnerForm, TypeForm};
    use application_identity::type_identity::{
        TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
    };
    use language_core::core_vocabulary::{
        CoreVocabulary, CoreVocabularyCatalog, FilesystemService,
    };

    struct Declaration {
        depth: usize,
        remaining: usize,
        service: Option<FilesystemService>,
    }
    impl TypeIdentityPackageOwnerVisitor for Declaration {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            if self.depth == 128 {
                return Err(TypeIdentityVisitError::ResourceLimitExceeded);
            }
            self.depth += 1;
            Ok(())
        }
        fn leave(&mut self) {
            self.depth -= 1;
        }
        fn reserve(&mut self, bytes: usize) -> Result<(), TypeIdentityVisitError> {
            self.remaining = self
                .remaining
                .checked_sub(bytes)
                .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
            Ok(())
        }
        fn package_owner(&mut self, _: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            Err(TypeIdentityVisitError::UnknownPackage)
        }
        fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn enter_type(&mut self, form: TypeForm) -> Result<(), TypeIdentityVisitError> {
            if form != TypeForm::Nominal {
                return Err(TypeIdentityVisitError::MalformedIdentity);
            }
            Ok(())
        }
        fn nominal_declaration(
            &mut self,
            owner: OwnerForm,
            digest: Option<[u8; 32]>,
            path: &str,
        ) -> Result<(), TypeIdentityVisitError> {
            if owner != OwnerForm::ToolchainSource || self.service.is_some() {
                return Err(TypeIdentityVisitError::MalformedIdentity);
            }
            for service in FilesystemService::ALL {
                let site =
                    CoreVocabularyCatalog::declaration(CoreVocabulary::FilesystemService(service))
                        .ok_or(TypeIdentityVisitError::UnsupportedEmbeddedName)?;
                if site.declaration != path {
                    continue;
                }
                let file = site
                    .file
                    .to_str()
                    .ok_or(TypeIdentityVisitError::UnsupportedEmbeddedName)?;
                let revision =
                    declaration_contracts::source_identity::toolchain_owned_identity_digest(
                        "core",
                        vec![file.to_owned()],
                        site.source,
                    )
                    .map_err(|_| TypeIdentityVisitError::MalformedIdentity)?;
                if digest != Some(revision) {
                    return Err(TypeIdentityVisitError::UnsupportedEmbeddedName);
                }
                self.service = Some(service);
                return Ok(());
            }
            Err(TypeIdentityVisitError::UnsupportedEmbeddedName)
        }
    }
    let mut declaration = Declaration {
        depth: 0,
        remaining: identity.len(),
        service: None,
    };
    visit_type_identity_package_owners(identity, &mut declaration)
        .map_err(|_| "filesystem service is not an exact receiver-bundled core declaration")?;
    declaration
        .service
        .ok_or_else(|| "filesystem service has no exact core declaration".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceDeclaration, TerminalIdentityTableBuilder, TerminalModule};
    use application_identity::type_identity::encoding::{
        CoordinateTag, atom, bare, byte_atom, compound,
    };
    use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
    use declaration_contracts::service_surface::{ServiceMethod, ServiceReceiverApplication};
    use language_core::core_vocabulary::{
        CoreVocabulary, CoreVocabularyCatalog, FilesystemService,
    };
    use semantic_vocabulary::{PackageKeyIdentity, ServiceId};

    fn id() -> ServiceId {
        ServiceId::new(1).unwrap()
    }

    fn nominal(owner: String, path: &str) -> String {
        compound(NameForm::Nominal, [owner, atom(CoordinateTag::Path, path)])
    }

    fn application(path: &str, arguments: Vec<String>, lifetimes: Vec<String>) -> String {
        compound(
            CoordinateTag::TraitApplication,
            [
                atom(
                    CoordinateTag::Declaration,
                    &nominal(bare(OwnerForm::Toolchain), path),
                ),
                compound(CoordinateTag::Arguments, arguments),
                compound(CoordinateTag::Lifetimes, lifetimes),
            ],
        )
    }

    fn revision(namespace: &str, file: &str) -> [u8; 32] {
        let site = CoreVocabularyCatalog::declaration(CoreVocabulary::FilesystemService(
            FilesystemService::MetadataQuery,
        ))
        .unwrap();
        declaration_contracts::source_identity::toolchain_owned_identity_digest(
            namespace,
            vec![file.to_owned()],
            site.source,
        )
        .unwrap()
    }

    fn fixture(kind: FilesystemService) -> TerminalModule {
        let site =
            CoreVocabularyCatalog::declaration(CoreVocabulary::FilesystemService(kind)).unwrap();
        let mut identities = TerminalIdentityTableBuilder::default();
        let declaration = identities.intern(&nominal(
            byte_atom(
                OwnerForm::ToolchainSource,
                &revision("core", "filesystem_authority.omg"),
            ),
            site.declaration,
        ));
        let application_text = application(site.declaration, vec![], vec![]);
        let application = identities.intern(&application_text);
        let mut surface = ServiceSurface::default();
        surface.trait_name = surface.identities.intern(site.declaration);
        surface
            .receiver_applications
            .push(ServiceReceiverApplication {
                application: surface.identities.intern(&application_text),
                trait_name: surface.trait_name,
                trait_package_identity: None,
                argument_identities: vec![],
                lifetime_argument_identities: vec![],
                requirement_identities: vec![],
            });
        let mut module = TerminalModule::declarations();
        module.identities = identities.build();
        module.services.push(ServiceDeclaration {
            id: id(),
            identity: application,
            parents: vec![],
        });
        module.service_schemas.push(TerminalServiceSchema {
            declaration,
            application,
            surface: Arc::new(surface),
        });
        module
    }

    fn intern(module: &mut TerminalModule, text: &str) -> TerminalIdentityIndex {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let identity = identities.intern(text);
        module.identities = identities.build();
        identity
    }

    fn replace_application(module: &mut TerminalModule, text: &str) {
        let identity = intern(module, text);
        module.services[0].identity = identity;
        module.service_schemas[0].application = identity;
        let surface = Arc::make_mut(&mut module.service_schemas[0].surface);
        surface.receiver_applications[0].application = surface.identities.intern(text);
    }

    #[test]
    fn canonical_filesystem_services_join_exact_receiver_bundled_declarations() {
        for kind in FilesystemService::ALL {
            assert_eq!(fixture(kind).core_filesystem_service(id()), Ok(kind));
        }
    }

    #[test]
    fn filesystem_services_refuse_foreign_owners_and_changed_source_coordinates() {
        let exact = revision("core", "filesystem_authority.omg");
        let mut changed = exact;
        changed[0] ^= 1;
        let path = "FilesystemMetadataQuery";
        for declaration in [
            nominal(byte_atom(OwnerForm::Package, &exact), path),
            nominal(bare(OwnerForm::Toolchain), path),
            nominal(byte_atom(OwnerForm::ToolchainSource, &changed), path),
            nominal(
                byte_atom(OwnerForm::ToolchainSource, &exact),
                "other::FilesystemMetadataQuery",
            ),
            nominal(
                byte_atom(
                    OwnerForm::ToolchainSource,
                    &revision("foreign", "filesystem_authority.omg"),
                ),
                path,
            ),
            nominal(
                byte_atom(OwnerForm::ToolchainSource, &revision("core", "other.omg")),
                path,
            ),
        ] {
            let mut module = fixture(FilesystemService::MetadataQuery);
            module.service_schemas[0].declaration = intern(&mut module, &declaration);
            assert!(
                module.core_filesystem_service(id()).is_err(),
                "accepted {declaration}"
            );
        }
    }

    #[test]
    fn filesystem_services_refuse_changed_applications_and_malformed_identities() {
        let path = "FilesystemMetadataQuery";
        let exact = application(path, vec![], vec![]);
        for application in [
            application(path, vec!["compiler-type(atom(u8))".into()], vec![]),
            application(path, vec![], vec!["lifetime(other)".into()]),
            application("other::FilesystemMetadataQuery", vec![], vec![]),
            exact[..exact.len() - 1].to_owned(),
            format!("{exact}trailing"),
            "toolchain::FilesystemMetadataQuery".into(),
        ] {
            let mut module = fixture(FilesystemService::MetadataQuery);
            replace_application(&mut module, &application);
            assert!(
                module.core_filesystem_service(id()).is_err(),
                "accepted {application}"
            );
        }
        let module = fixture(FilesystemService::MetadataQuery);
        let exact = module
            .identities
            .text(module.service_schemas[0].declaration)
            .unwrap();
        for declaration in [
            exact[..exact.len() - 1].to_owned(),
            format!("{exact}trailing"),
            "nominal(".into(),
        ] {
            let mut module = fixture(FilesystemService::MetadataQuery);
            module.service_schemas[0].declaration = intern(&mut module, &declaration);
            assert!(
                module.core_filesystem_service(id()).is_err(),
                "accepted {declaration}"
            );
        }
    }

    #[test]
    fn filesystem_services_refuse_missing_duplicate_or_expanded_contract_metadata() {
        for case in 0..15 {
            let mut module = fixture(FilesystemService::MetadataQuery);
            match case {
                0 => module.services.clear(),
                1 => module.services.push(module.services[0].clone()),
                2 => module.services[0].parents.push(id()),
                3 => module.services[0].parents.push(ServiceId::new(99).unwrap()),
                4 => module.service_schemas.clear(),
                5 => module
                    .service_schemas
                    .push(module.service_schemas[0].clone()),
                6 => {
                    let mut other = module.service_schemas[0].clone();
                    other.declaration = intern(&mut module, "nominal(toolchain-owner,path(Other))");
                    module.service_schemas.push(other);
                }
                case => {
                    let surface = Arc::make_mut(&mut module.service_schemas[0].surface);
                    let foreign = surface.identities.intern("Other");
                    match case {
                        7 => {
                            surface.trait_package_identity =
                                PackageKeyIdentity::from_digest([1; 32])
                        }
                        8 => surface.trait_name = foreign,
                        9 => surface.receiver_applications[0].application = foreign,
                        10 => surface
                            .receiver_applications
                            .push(surface.receiver_applications[0].clone()),
                        11 => surface.receiver_applications[0]
                            .argument_identities
                            .push(foreign),
                        12 => surface.receiver_applications[0]
                            .lifetime_argument_identities
                            .push(foreign),
                        13 => surface.receiver_applications[0]
                            .requirement_identities
                            .push(foreign),
                        14 => surface.methods.push(ServiceMethod::default()),
                        _ => unreachable!(),
                    }
                }
            }
            assert!(
                module.core_filesystem_service(id()).is_err(),
                "accepted malformed case {case}"
            );
        }
    }
}
