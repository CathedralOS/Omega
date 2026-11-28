//! Service contracts travel with the checked program that uses them. Native
//! planning reads this normalized surface instead of reopening typed source.

use crate::TerminalIdentityIndex;
use service_surface::{ServiceSurface, ServiceSynchronousInvocation};
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

/// A supplier explicitly publishes this closed operation record. Its members
/// name checked candidates, not every candidate used by their implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProviderExport {
    pub schema_declaration: TerminalIdentityIndex,
    pub schema_application: TerminalIdentityIndex,
    pub provider_declaration: TerminalIdentityIndex,
    pub provider_identity: TerminalIdentityIndex,
    pub members: Vec<ProviderExportMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderExportMember {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub machine: semantic_vocabulary::MachineId,
}

impl TerminalProviderExport {
    /// A candidate may be an internal dependency. Only the members of this
    /// declared record, proved for this schema and this provider, are exported.
    pub fn check_members(&self, module: &crate::TerminalModule) -> Result<(), &'static str> {
        self.check_member_contracts(module, |_, _| Ok(()))
    }

    /// Apply the receiver's contract check to the boundary and public method
    /// matched by the same checked provider and closed requirement identity.
    pub fn check_member_contracts(
        &self,
        module: &crate::TerminalModule,
        mut check: impl FnMut(
            semantic_vocabulary::BoundaryMachineId,
            &service_surface::ServiceMethod,
        ) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let schema = module
            .service_schema(self.schema_declaration, self.schema_application)
            .map_err(|_| "export does not name a closed schema")?;
        if [self.provider_declaration, self.provider_identity]
            .iter()
            .any(|&id| module.identities.text(id).is_none_or(str::is_empty))
        {
            return Err("export does not identify its provider");
        }
        if self.members.len() != schema.surface.methods.len()
            || !self
                .members
                .windows(2)
                .all(|w| w[0].boundary < w[1].boundary)
        {
            return Err("export must provide one checked member per requirement");
        }
        let mut provided = Vec::with_capacity(self.members.len());
        let candidates = &module.interfaces.provider_candidates;
        for member in &self.members {
            let start = candidates.partition_point(|row| row.boundary < member.boundary);
            let mut matched = candidates[start..]
                .iter()
                .take_while(|row| row.boundary == member.boundary)
                .filter(|row| {
                    row.candidate == member.machine
                        && row.provider_identity == self.provider_identity
                        && row.provider_declaration.identity() == self.provider_declaration
                });
            let (Some(row), None) = (matched.next(), matched.next()) else {
                return Err("exported member is not this provider's checked candidate");
            };
            let requirement = module
                .identities
                .text(row.source_requirement_identity)
                .ok_or("unknown exported requirement")?;
            let receiving = if row.declaration_application.is_absent() {
                None
            } else {
                Some(
                    module
                        .identities
                        .text(row.declaration_application)
                        .ok_or("unknown exported application")?,
                )
            };
            if !schema
                .surface
                .matches_receiving_application(requirement, receiving)
            {
                return Err("exported member belongs to a different schema application");
            }
            provided.push((requirement, member.boundary));
        }
        provided.sort_unstable();
        let mut required = schema.surface.methods.iter().collect::<Vec<_>>();
        required.sort_unstable_by_key(|method| {
            schema.surface.identity_text(method.requirement_identity)
        });
        for ((identity, boundary), method) in provided.into_iter().zip(required) {
            if identity != schema.surface.identity_text(method.requirement_identity) {
                return Err("exported members differ from the closed requirement record");
            }
            check(boundary, method)?;
        }
        Ok(())
    }
}

/// Build precedence has already selected this source provider declaration.
/// Its nominal owner and its semantic implementation identity stay distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProviderSelection {
    pub target: String,
    pub composition_mode: semantic_vocabulary::source_selections::CompositionMode,
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
        use language_core::bundled_files::{BundledFile, core_revision};
        use language_core::core_vocabulary::CoreVocabulary;
        let service = self.service(service).ok_or("missing filesystem service")?;
        match self.bundled_marker_service(
            service,
            BundledFile::CoreFilesystemAuthority,
            &core_revision::FILESYSTEM_AUTHORITY,
            None,
        )? {
            CoreVocabulary::FilesystemService(kind) => Ok(kind),
            _ => Err("filesystem service is absent from the core catalog".into()),
        }
    }

    /// An unused source declaration has no retained service application. When
    /// present, verify the full installed marker and its parent before classifying it.
    pub fn retained_core_unsafe_service(
        &self,
        atom: language_core::core_vocabulary::UnsafeAtom,
    ) -> Result<Option<&crate::ServiceDeclaration>, String> {
        use language_core::core_vocabulary::{CoreVocabulary, CoreVocabularyCatalog};
        let declaration = CoreVocabularyCatalog::declaration(CoreVocabulary::UnsafeAtom(atom))
            .ok_or("missing Core unsafe declaration")?;
        let expected = marker_application(declaration.declaration);
        if self
            .interfaces
            .services
            .iter()
            .any(|row| self.identities.text(row.identity) == Some(expected.as_str()))
        {
            self.core_unsafe_service(atom).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Resolve the installed atom and its authentic Unsafe parent, including
    /// their exact toolchain source revision in this artifact's namespace.
    pub fn core_unsafe_service(
        &self,
        atom: language_core::core_vocabulary::UnsafeAtom,
    ) -> Result<&crate::ServiceDeclaration, String> {
        use language_core::bundled_files::{BundledFile, core_revision};
        use language_core::core_vocabulary::{CoreVocabulary, CoreVocabularyCatalog};
        let resolve = |role, parent| {
            let name = CoreVocabularyCatalog::declaration(role)
                .ok_or("missing Core unsafe declaration")?
                .declaration;
            let expected = marker_application(name);
            let mut matches = self
                .interfaces
                .services
                .iter()
                .filter(|row| self.identities.text(row.identity) == Some(expected.as_str()));
            let (Some(service), None) = (matches.next(), matches.next()) else {
                return Err(format!(
                    "native borrow requires one installed Core service for {name}"
                ));
            };
            let actual = self.bundled_marker_service(
                service,
                BundledFile::CoreUnsafe,
                &core_revision::UNSAFE,
                parent,
            )?;
            if actual != role {
                return Err(format!("installed service differs from Core {name}"));
            }
            Ok(service)
        };
        let parent = resolve(CoreVocabulary::UnsafeService, None)?;
        resolve(CoreVocabulary::UnsafeAtom(atom), Some(parent))
    }

    fn bundled_marker_service(
        &self,
        service: &crate::ServiceDeclaration,
        file: language_core::bundled_files::BundledFile,
        revision: &[u8; 32],
        parent: Option<&crate::ServiceDeclaration>,
    ) -> Result<language_core::core_vocabulary::CoreVocabulary, String> {
        use application_identity::type_identity::encoding::{
            CoordinateTag, atom, byte_atom, compound,
        };
        use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
        use language_core::core_vocabulary::CoreVocabularyCatalog;
        let mut declarations = self
            .interfaces
            .services
            .iter()
            .filter(|row| row.id == service.id);
        if declarations.next() != Some(service) || declarations.next().is_some() {
            return Err("Core service has no unique retained declaration".into());
        }
        let mut schemas = self
            .interfaces
            .service_schemas
            .iter()
            .filter(|row| row.application == service.identity);
        let (Some(schema), None) = (schemas.next(), schemas.next()) else {
            return Err("Core service has no unique retained schema application".into());
        };
        let surface = &self
            .service_schema(schema.declaration, schema.application)?
            .surface;
        let name = surface.identity_text(surface.trait_name);
        let role = CoreVocabularyCatalog::classify_bundled(file, name)
            .ok_or("service is absent from the expected Core file")?;
        let declaration = compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, revision),
                atom(CoordinateTag::Path, name),
            ],
        );
        let application = marker_application(name);
        if self.identities.text(schema.declaration) != Some(declaration.as_str())
            || self.identities.text(service.identity) != Some(application.as_str())
            || surface.trait_package_identity.is_some()
            || !surface.methods.is_empty()
            || surface.receiver_applications.iter().any(|row| {
                row.trait_package_identity.is_some()
                    || !row.argument_identities.is_empty()
                    || !row.lifetime_argument_identities.is_empty()
                    || !row.requirement_identities.is_empty()
            })
        {
            return Err(
                "service differs from its exact Core marker declaration or application".into(),
            );
        }
        match (
            parent,
            service.parents.as_slice(),
            surface.receiver_applications.as_slice(),
        ) {
            (None, [], [_]) => {}
            (Some(parent), [id], [_, ancestor])
                if *id == parent.id
                    && self.identities.text(parent.identity)
                        == Some(surface.identity_text(ancestor.application))
                    && self.identities.text(parent.identity)
                        == Some(
                            marker_application(surface.identity_text(ancestor.trait_name)).as_str(),
                        ) => {}
            _ => return Err("Core service differs from its exact parent declaration".into()),
        }
        Ok(role)
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
            .interfaces.service_schemas
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
            service_surface::ServiceDomainMembership::validate_entries(
                &method.domain_memberships,
                method.parameter_count,
                method.has_result,
            )?;
            ServiceSynchronousInvocation::validate_entries(
                &method.synchronous_invocations,
                method.parameter_count,
            )?;
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
            .interfaces.program_entry_contracts
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
        let mut rows = self.interfaces.provider_selections.iter().filter(|row| {
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

fn marker_application(name: &str) -> String {
    use application_identity::type_identity::encoding::{CoordinateTag, atom, bare, compound};
    use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
    let declared = compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(CoordinateTag::Path, name)],
    );
    compound(
        CoordinateTag::TraitApplication,
        [
            atom(CoordinateTag::Declaration, &declared),
            compound(CoordinateTag::Arguments, []),
            compound(CoordinateTag::Lifetimes, []),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceDeclaration, TerminalIdentityTableBuilder, TerminalModule};
    use application_identity::type_identity::encoding::{
        CoordinateTag, atom, bare, byte_atom, compound,
    };
    use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
    use language_core::core_vocabulary::{
        CoreVocabulary, CoreVocabularyCatalog, FilesystemService,
    };
    use semantic_vocabulary::{PackageKeyIdentity, ServiceId};
    use service_surface::{ServiceMethod, ServiceReceiverApplication};

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
        source_provenance::toolchain_owned_identity_digest(
            namespace,
            vec![file.to_owned()],
            include_bytes!("../../../../../source/library/core/filesystem_authority.omg"),
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
        module.interfaces.services.push(ServiceDeclaration {
            id: id(),
            identity: application,
            parents: vec![],
        });
        module.interfaces.service_schemas.push(TerminalServiceSchema {
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
        module.interfaces.services[0].identity = identity;
        module.interfaces.service_schemas[0].application = identity;
        let surface = Arc::make_mut(&mut module.interfaces.service_schemas[0].surface);
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
            module.interfaces.service_schemas[0].declaration = intern(&mut module, &declaration);
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
            .text(module.interfaces.service_schemas[0].declaration)
            .unwrap();
        for declaration in [
            exact[..exact.len() - 1].to_owned(),
            format!("{exact}trailing"),
            "nominal(".into(),
        ] {
            let mut module = fixture(FilesystemService::MetadataQuery);
            module.interfaces.service_schemas[0].declaration = intern(&mut module, &declaration);
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
                0 => module.interfaces.services.clear(),
                1 => module.interfaces.services.push(module.interfaces.services[0].clone()),
                2 => module.interfaces.services[0].parents.push(id()),
                3 => module.interfaces.services[0].parents.push(ServiceId::new(99).unwrap()),
                4 => module.interfaces.service_schemas.clear(),
                5 => module
                    .interfaces.service_schemas
                    .push(module.interfaces.service_schemas[0].clone()),
                6 => {
                    let mut other = module.interfaces.service_schemas[0].clone();
                    other.declaration = intern(&mut module, "nominal(toolchain-owner,path(Other))");
                    module.interfaces.service_schemas.push(other);
                }
                case => {
                    let surface = Arc::make_mut(&mut module.interfaces.service_schemas[0].surface);
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
