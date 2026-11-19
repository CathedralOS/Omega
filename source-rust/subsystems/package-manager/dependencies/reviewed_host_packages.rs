use crate::lock::{PackageLock, PackageLockError, PackageLockRecoveryLimits};

/// A host can pin an existing application package lock for saved Psi programs.
/// Only the host selects this path and digest. Artifact and current-directory
/// contents play no part in selecting an installation.
pub struct ReviewedHostPackages(PackageLock);

impl ReviewedHostPackages {
    pub fn open(path: &std::path::Path, expected_sha256: &str) -> Result<Self, String> {
        use std::io::Read;
        if !path.is_absolute() {
            return Err("the host package lock path must be absolute".into());
        }
        let expected = crate::hex_digest(&expected_sha256.to_ascii_lowercase())
            .ok_or("the host package lock pin must be a 64-digit SHA-256 digest")?;
        let limits = PackageLockRecoveryLimits::default();
        let file = std::fs::File::open(path)
            .map_err(|e| format!("cannot open host package lock {}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        file.take(limits.maximum_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read host package lock {}: {e}", path.display()))?;
        if bytes.len() > limits.maximum_bytes {
            return Err(PackageLockError::ByteLimitExceeded.to_string());
        }
        if crate::digest_encoder::content_identity(&bytes) != expected {
            return Err("host package lock differs from its approved SHA-256 digest".into());
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|e| format!("host package lock text: {e}"))?;
        PackageLock::recover_text(text, limits)
            .map(Self)
            .map_err(|e| e.to_string())
    }

    pub fn providers(
        &self,
        profile: target::TargetProfile,
    ) -> Result<host_provider_inputs::InstalledPackageProviders, String> {
        use declaration_scope::{DeclarationSelectionAuthority, ProductDeclarationScope};
        let target = self.0.target(profile).ok_or_else(|| {
            format!(
                "host package lock has no reviewed {} installation",
                profile.target_name()
            )
        })?;
        if target.source().root_role() != crate::declarations::BuildDeclarationKind::Application {
            return Err("host package permissions must come from an application root".into());
        }
        let root = target.source().root().selected().key().identity();
        let mut packages = Vec::new();
        let mut bindings = Vec::new();
        let mut root_grants = None;
        for occurrence in target
            .occurrences()
            .iter()
            .filter(|row| row.context().purpose().is_product())
        {
            let package = occurrence.acceptance().package();
            let compilation = occurrence
                .compilation()
                .ok_or_else(|| PackageLockError::MissingCompilationInputs.to_string())?;
            packages.push(package);
            bindings.extend(compilation.native_bindings().iter().cloned());
            if package == root {
                root_grants = Some(compilation.semantic_bindings());
            }
        }
        let dependencies = target
            .source()
            .dependency_requests()
            .iter()
            .filter(|edge| edge.purpose().is_product())
            .map(|edge| {
                (
                    edge.requester().identity(),
                    edge.selected().key().identity(),
                )
            });
        let declarations =
            ProductDeclarationScope::from_direct_dependencies(&packages, dependencies);
        let root_grants =
            root_grants.ok_or_else(|| PackageLockError::OccurrenceCoverage.to_string())?;
        let mut roles = std::collections::BTreeSet::new();
        for binding in root_grants {
            if !declarations.allows_declaration_selection(binding.package(), binding.package()) {
                return Err(
                    "host semantic role names a package outside its Product dependencies".into(),
                );
            }
            if !roles.insert(binding.role()) {
                return Err("host application supplies a semantic role more than once".into());
            }
        }
        Ok(host_provider_inputs::InstalledPackageProviders::new(
            profile,
            bindings,
            declarations,
            root_grants.into(),
        ))
    }
}
