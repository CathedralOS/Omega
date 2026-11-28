//! Completed artifacts and the receipts issued when their primary outputs are installed.

use crate::{
    DynamicElfPublicationReceipt, ExecutablePublicationReceipt, NativePackagePublicationReceipt,
    RetainedDynamicElfArtifact, RetainedNativeArtifact, TerminalRuntimeProvisions,
};
use native_artifact::{DynamicElfNativeArtifactIdentity, NativeArtifactIdentity};
use std::path::Path;
use target::NativeApplicationMetadata;
use terminal_codec::CanonicalTerminalArtifact;
use verified_optimization_input::RetainedTerminalArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedNativeProgramIdentity {
    Direct(NativeArtifactIdentity),
    DynamicElf(DynamicElfNativeArtifactIdentity),
}

impl std::fmt::Display for RetainedNativeProgramIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct(identity) => std::fmt::Display::fmt(identity, formatter),
            Self::DynamicElf(identity) => std::fmt::Display::fmt(identity, formatter),
        }
    }
}

pub struct RetainedNativeProgram<'artifact> {
    artifact: &'artifact CanonicalTerminalArtifact,
    identity: RetainedNativeProgramIdentity,
}

impl<'artifact> RetainedNativeProgram<'artifact> {
    pub fn psi_artifact(&self) -> &'artifact CanonicalTerminalArtifact {
        self.artifact
    }

    pub fn identity(&self) -> RetainedNativeProgramIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
}

#[derive(Debug)]
struct TerminalProduct {
    artifact: RetainedTerminalArtifact,
    runtime_provisions: TerminalRuntimeProvisions,
}

/// A native image (direct or dynamic ELF) retained before publication.
#[derive(Debug)]
struct NativeProduct<Artifact> {
    artifact: Artifact,
    runtime_provisions: Option<TerminalRuntimeProvisions>,
    application: NativeApplicationMetadata,
}

/// A published native image. Direct images keep their macOS .app package
/// receipt (flat publications retain None); dynamic ELF images have none (`()`).
#[derive(Debug)]
struct PublishedNativeProduct<Receipt, Package = Option<NativePackagePublicationReceipt>> {
    runtime_provisions: Option<TerminalRuntimeProvisions>,
    executable: Receipt,
    package: Package,
    application: NativeApplicationMetadata,
}

impl<Artifact> NativeProduct<Artifact> {
    fn published<Receipt, Package>(
        self,
        executable: Receipt,
        package: Package,
    ) -> Box<PublishedNativeProduct<Receipt, Package>> {
        Box::new(PublishedNativeProduct {
            runtime_provisions: self.runtime_provisions,
            executable,
            package,
            application: self.application,
        })
    }
}

#[derive(Debug)]
enum CompileProduct {
    TerminalArtifact(Box<TerminalProduct>),
    PublishedPsi(std::sync::Arc<CanonicalTerminalArtifact>),
    RetainedNativeArtifact(Box<NativeProduct<RetainedNativeArtifact>>),
    NativeExecutable(Box<PublishedNativeProduct<ExecutablePublicationReceipt>>),
    RetainedDynamicElfArtifact(Box<NativeProduct<RetainedDynamicElfArtifact>>),
    DynamicElfExecutable(Box<PublishedNativeProduct<DynamicElfPublicationReceipt, ()>>),
}

impl CompileProduct {
    const fn kind(&self) -> CompileOutputKind {
        match self {
            Self::TerminalArtifact(_) | Self::PublishedPsi(_) => {
                CompileOutputKind::TerminalArtifact
            }
            Self::RetainedNativeArtifact(_) | Self::RetainedDynamicElfArtifact(_) => {
                CompileOutputKind::RetainedNativeArtifact
            }
            Self::NativeExecutable(_) | Self::DynamicElfExecutable(_) => {
                CompileOutputKind::NativeExecutable
            }
        }
    }

    fn application(&self) -> Option<&NativeApplicationMetadata> {
        match self {
            Self::RetainedNativeArtifact(product) => Some(&product.application),
            Self::NativeExecutable(product) => Some(&product.application),
            Self::RetainedDynamicElfArtifact(product) => Some(&product.application),
            Self::DynamicElfExecutable(product) => Some(&product.application),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct CompletedProduct {
    product: CompileProduct,
    wrote_output: bool,
}

impl CompletedProduct {
    /// Publication accepts the completed image and its admitted runtime as a single input.
    ///
    /// ```compile_fail,E0308
    /// fn detach(image: native_artifact::RequestedNativeArtifact, app: target::NativeApplicationMetadata) {
    ///     let _ = product_publication::CompletedProduct::from_native(image, app);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// fn replace(product: product_publication::CompletedProduct, runtime: product_publication::TerminalRuntimeProvisions) {
    ///     let _ = product.with_runtime_provisions(Some(runtime));
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// use product_publication::RetainedNativeArtifact;
    /// let _ = RetainedNativeArtifact::from_emitted_parts;
    /// ```
    pub fn from_native(
        image: native_artifact::FinishedNativeImage,
        application: NativeApplicationMetadata,
    ) -> Self {
        let (artifact, runtime_provisions) = image.into_parts();
        let product = match artifact {
            native_artifact::RequestedNativeArtifact::Direct(artifact) => {
                CompileProduct::RetainedNativeArtifact(Box::new(NativeProduct {
                    artifact: artifact.into_retained(),
                    runtime_provisions,
                    application,
                }))
            }
            native_artifact::RequestedNativeArtifact::DynamicElf(artifact) => {
                CompileProduct::RetainedDynamicElfArtifact(Box::new(NativeProduct {
                    artifact: artifact.into_retained(),
                    runtime_provisions,
                    application,
                }))
            }
        };
        Self {
            product,
            wrote_output: false,
        }
    }

    pub fn from_published_psi(
        artifact: std::sync::Arc<CanonicalTerminalArtifact>,
    ) -> Result<Self, &'static str> {
        if artifact.publication_metadata().is_none() {
            return Err("saving a program requires its checked publication");
        }
        Ok(Self {
            product: CompileProduct::PublishedPsi(artifact),
            wrote_output: false,
        })
    }
    pub fn from_runtime(program: native_artifact::AdmittedRuntimeProgram) -> Self {
        let (artifact, runtime_provisions) = program.into_parts();
        Self {
            product: CompileProduct::TerminalArtifact(Box::new(TerminalProduct {
                artifact,
                runtime_provisions,
            })),
            wrote_output: false,
        }
    }

    pub const fn output_kind(&self) -> CompileOutputKind {
        self.product.kind()
    }

    pub const fn retained_native_artifact(&self) -> Option<&RetainedNativeArtifact> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => Some(&product.artifact),
            _ => None,
        }
    }

    pub fn retained_native_program(&self) -> Option<RetainedNativeProgram<'_>> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => Some(RetainedNativeProgram {
                artifact: product.artifact.psi_artifact(),
                identity: RetainedNativeProgramIdentity::Direct(product.artifact.identity()),
            }),
            CompileProduct::RetainedDynamicElfArtifact(product) => Some(RetainedNativeProgram {
                artifact: product.artifact.psi_artifact(),
                identity: RetainedNativeProgramIdentity::DynamicElf(product.artifact.identity()),
            }),
            _ => None,
        }
    }

    pub const fn retained_dynamic_elf_artifact(&self) -> Option<&RetainedDynamicElfArtifact> {
        match &self.product {
            CompileProduct::RetainedDynamicElfArtifact(product) => Some(&product.artifact),
            _ => None,
        }
    }

    fn record_component_publications(
        &mut self,
        receipts: Vec<native_artifact::NativeComponentPublicationReceipt>,
    ) -> Result<(), String> {
        let runtime = match &mut self.product {
            CompileProduct::RetainedDynamicElfArtifact(product) => {
                product.runtime_provisions.as_mut()
            }
            CompileProduct::RetainedNativeArtifact(product)
                if matches!(
                    product.artifact.target().object_format,
                    target::ObjectFormat::MachO | target::ObjectFormat::Coff
                ) =>
            {
                product.runtime_provisions.as_mut()
            }
            _ => return Err(
                "component files require an admitted ELF, Mach-O, or Windows PE process publication contract"
                    .into(),
            ),
        }
        .ok_or("component publication lost its exact runtime product roster")?;
        runtime
            .record_component_publications(receipts)
            .map_err(str::to_owned)
    }

    pub fn validate_component_product_inputs(&self) -> Result<(), String> {
        let object = self
            .retained_native_artifact()
            .map(|artifact| artifact.object())
            .or_else(|| {
                self.retained_dynamic_elf_artifact()
                    .map(|artifact| artifact.object())
            });
        match object {
            Some(object) => {
                TerminalRuntimeProvisions::validate_root_object(object, self.runtime_provisions())
                    .map_err(str::to_owned)
            }
            None => self.runtime_provisions().map_or(Ok(()), |runtime| {
                runtime
                    .validate_component_product_roster()
                    .map_err(str::to_owned)
            }),
        }
    }

    pub fn validate_component_process_inputs(
        &self,
        executable: &std::path::Path,
    ) -> Result<(), String> {
        self.validate_component_product_inputs()?;
        if self
            .runtime_provisions()
            .is_none_or(|runtime| runtime.component_products().is_empty())
        {
            return Ok(());
        }
        if self.checked_native_executable_path() != Some(executable) {
            return Err(
                "component process root differs from its exact native publication receipt".into(),
            );
        }
        self.runtime_provisions()
            .ok_or("component process lost runtime provisions")?
            .validate_component_process_inputs(executable)
    }

    pub const fn runtime_provisions(&self) -> Option<&TerminalRuntimeProvisions> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => product.runtime_provisions.as_ref(),
            CompileProduct::RetainedDynamicElfArtifact(product) => {
                product.runtime_provisions.as_ref()
            }
            CompileProduct::NativeExecutable(product) => product.runtime_provisions.as_ref(),
            CompileProduct::DynamicElfExecutable(product) => product.runtime_provisions.as_ref(),
            CompileProduct::TerminalArtifact(product) => Some(&product.runtime_provisions),
            _ => None,
        }
    }

    pub fn application_name(&self) -> Option<&str> {
        self.product
            .application()
            .and_then(|application| application.name.as_deref())
    }

    pub fn application_intent(&self) -> Option<target::HostedApplicationIntent> {
        self.product
            .application()
            .and_then(|application| application.intent)
    }

    pub fn application_identifier(&self) -> Option<&target::ApplicationIdentifier> {
        self.product
            .application()
            .and_then(|application| application.identifier.as_ref())
    }

    pub fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.product {
            CompileProduct::PublishedPsi(artifact) => Some(artifact),
            CompileProduct::TerminalArtifact(product) => Some(product.artifact.artifact()),
            _ => None,
        }
    }

    /// The contained admission of a runtime product made from saved Psi the
    /// host does not trust. Such a product runs only on the contained route.
    pub fn contained_artifact(&self) -> Option<&psi_authorization::ContainedTerminalArtifact> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => {
                product.artifact.authorization().contained()
            }
            _ => None,
        }
    }

    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        match self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact),
            _ => None,
        }
    }

    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => Some(product.executable.output_path()),
            CompileProduct::DynamicElfExecutable(product) => Some(product.executable.output_path()),
            _ => None,
        }
    }

    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        let CompileProduct::NativeExecutable(product) = &self.product else {
            return None;
        };
        let package = product.package.as_ref()?;
        let retained_identity_matches = product.application.name.as_deref()
            == Some(package.application_name())
            && product.application.identifier.as_ref() == Some(package.application_identifier());
        (retained_identity_matches && package.binds_executable(&product.executable))
            .then(|| package.package_root())
    }

    pub fn package_publication(&self) -> Option<&NativePackagePublicationReceipt> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => product.package.as_ref(),
            _ => None,
        }
    }

    fn record_native_publication(
        mut self,
        receipt: ExecutablePublicationReceipt,
        package: Option<NativePackagePublicationReceipt>,
    ) -> Result<Self, String> {
        const REFUSAL: &str = "native publication requires exactly one retained native artifact";
        let CompileProduct::RetainedNativeArtifact(product) = self.product else {
            return Err(REFUSAL.into());
        };
        if self.wrote_output {
            return Err(REFUSAL.into());
        }
        self.product = CompileProduct::NativeExecutable(product.published(receipt, package));
        self.wrote_output = true;
        Ok(self)
    }

    fn record_dynamic_elf_publication(
        mut self,
        receipt: DynamicElfPublicationReceipt,
    ) -> Result<Self, String> {
        const REFUSAL: &str =
            "dynamic ELF publication requires exactly one retained dynamic ELF artifact";
        let CompileProduct::RetainedDynamicElfArtifact(product) = self.product else {
            return Err(REFUSAL.into());
        };
        if self.wrote_output {
            return Err(REFUSAL.into());
        }
        self.product = CompileProduct::DynamicElfExecutable(product.published(receipt, ()));
        self.wrote_output = true;
        Ok(self)
    }

    fn record_terminal_publication(mut self) -> Result<Self, String> {
        if self.output_kind() != CompileOutputKind::TerminalArtifact || self.wrote_output {
            return Err(
                "terminal publication requires exactly one retained terminal artifact".into(),
            );
        }
        self.wrote_output = true;
        Ok(self)
    }

    fn unpublished_psi(&self) -> Result<&std::sync::Arc<CanonicalTerminalArtifact>, String> {
        match &self.product {
            CompileProduct::PublishedPsi(artifact) if !self.wrote_output => Ok(artifact),
            _ => Err("saving requires an unpublished portable program".into()),
        }
    }

    /// Publish one file after every target produces the same portable bytes.
    pub fn save_shared_psi(products: &mut [&mut Self], path: &Path) -> Result<(), String> {
        Self::save_shared_psi_with_quota(products, path, None)
    }

    /// Apply the caller's output quota to the encoded artifact before touching the destination.
    pub fn save_shared_psi_with_quota(
        products: &mut [&mut Self],
        path: &Path,
        maximum_bytes: Option<u64>,
    ) -> Result<(), String> {
        use platform_custody::record_file::RecordFileRoot;
        let (first, rest) = products
            .split_first()
            .ok_or("saving requires a portable program")?;
        let artifact = first.unpublished_psi()?;
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
        {
            return Err("saving requires an unpublished program and a .psi output file".into());
        }
        let bytes = artifact.to_bytes();
        if let Some(limit) = maximum_bytes
            && bytes.len() as u64 > limit
        {
            return Err(format!(
                "Psi artifact requires {} bytes, exceeding the {limit}-byte output quota",
                bytes.len()
            ));
        }
        for other in rest {
            let other = other.unpublished_psi()?;
            if !std::sync::Arc::ptr_eq(artifact, other) && other.to_bytes() != bytes {
                return Err("the requested targets produce different Psi programs and cannot share one output file".into());
            }
        }
        let parent = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let file = Path::new(path.file_name().ok_or("missing Psi output file name")?);
        std::fs::create_dir_all(parent).map_err(|error| {
            format!("cannot open output directory {}: {error}", parent.display())
        })?;
        let refuse = |error| format!("cannot save {}: {error:?}", path.display());
        let directory = RecordFileRoot::open_directory(parent).map_err(refuse)?;
        directory.publish_output(file, &bytes).map_err(refuse)?;
        for product in products {
            product.wrote_output = true;
        }
        Ok(())
    }

    /// Install the primary artifact and retain the receipts for these exact bytes.
    pub fn publish(self, root: &Path, build_dir: &Path) -> Result<Self, String> {
        if self.wrote_output {
            return Err("publication requires an unpublished product".into());
        }
        match self.output_kind() {
            CompileOutputKind::RetainedNativeArtifact => self.publish_native(build_dir),
            CompileOutputKind::TerminalArtifact => {
                let artifact = self.artifact().ok_or_else(|| {
                    "terminal publication requires exactly one retained terminal artifact"
                        .to_owned()
                })?;
                crate::publish_psi(root, artifact, build_dir)?;
                self.record_terminal_publication()
            }
            _ => Err("compilation did not retain a publishable product".into()),
        }
    }
    fn publish_native(mut self, build_dir: &Path) -> Result<Self, String> {
        self.validate_component_product_inputs()?;
        let components = self
            .runtime_provisions()
            .map(|runtime| runtime.component_products())
            .unwrap_or_default();
        if let Some(artifact) = self.retained_dynamic_elf_artifact() {
            let (receipt, companions) =
                crate::publish_dynamic_elf(artifact, components, build_dir)?;
            if !companions.is_empty() {
                self.record_component_publications(companions)?;
            }
            return self.record_dynamic_elf_publication(receipt);
        }
        let artifact = self.retained_native_artifact().ok_or_else(|| {
            "native publication requires exactly one retained native artifact".to_owned()
        })?;
        let published = crate::publish_executable(
            crate::ExecutableProduct {
                artifact,
                components,
                intent: self.application_intent(),
                name: self.application_name(),
                identifier: self.application_identifier(),
            },
            build_dir,
        )?;
        if !published.components.is_empty() {
            self.record_component_publications(published.components)?;
        }
        self.record_native_publication(published.receipt, published.package)
    }
}

#[cfg(test)]
#[path = "package_path_tests.rs"]
mod package_path_tests;
