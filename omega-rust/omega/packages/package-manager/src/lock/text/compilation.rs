//! Bounded, exact accepted compilation inputs, separate from review snapshots.

use super::{
    MAXIMUM_POLICY_TEXT_BYTES,
    budget::Budget,
    framing::{Reader, Writer},
};
use crate::hex_digest;
use crate::lock::{AcceptedPackageCompilation, PackageCheckedContext, PackageLockError as Error};
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use filesystem_authority::observation_codec::ObservationCursor;
use package_compilation::{
    AcceptedSemanticBinding,
    review_record_encoding::{recover_semantic_binding_role, semantic_binding_role_tag},
};
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn write(
    value: &AcceptedPackageCompilation,
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
) -> Result<String, Error> {
    let mut writer = Writer::new(MAXIMUM_POLICY_TEXT_BYTES);
    writer.row("compiler", hexadecimal(value.compiler_identity()))?;
    writer.row("package", hexadecimal(package.digest()))?;
    writer.row("purpose", context.purpose().name())?;
    writer.row("target", context.target().identity().as_str())?;
    writer.row(
        "execution",
        context
            .build_execution_profile()
            .map(|target| target.identity().as_str())
            .unwrap_or("none"),
    )?;
    writer.row("bindings", value.semantic_bindings().len())?;
    for binding in value.semantic_bindings() {
        writer.row("role", semantic_binding_role_tag(binding.role()))?;
        writer.row("owner", hexadecimal(binding.package().digest()))?;
        writer.section("declaration", binding.declaration_path())?;
        writer.row(
            "schema",
            hexadecimal(*binding.normalized_schema_digest().as_bytes()),
        )?;
        writer.row(
            "plan",
            binding
                .selected_provider_plan_digest()
                .map(|value| hexadecimal(*value.as_bytes()))
                .unwrap_or_else(|| "none".into()),
        )?;
        writer.row(
            "permissions",
            binding.terminal_authority_permissions().len(),
        )?;
        for permission in binding.terminal_authority_permissions() {
            writer.row(
                "schema",
                hexadecimal(*permission.service_schema().as_bytes()),
            )?;
            writer.section(
                "requirement",
                binding
                    .terminal_authority_permission_identities()
                    .text(permission.requirement_identity())
                    .unwrap_or_default(),
            )?;
            writer.row("classes", permission.permitted().classes().len())?;
            for class in permission.permitted().classes() {
                writer.row("class", class.canonical_tag())?;
            }
        }
    }
    writer.append("end_compilation\n")?;
    writer.finish()
}

pub(super) fn read(
    text: &str,
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    budget: &mut Budget,
) -> Result<AcceptedPackageCompilation, Error> {
    // Every retained string is a disjoint slice of this input. Charge the
    // entire input before retaining strings, then charge every vector below.
    budget.owned(text.len())?;
    let mut reader = Reader::new(text);
    let compiler = digest(reader.field("compiler")?).map_err(|_| Error::CompilerIdentity)?;
    if digest(reader.field("package")?)? != package.digest()
        || reader.field("purpose")? != context.purpose().name()
        || reader.field("target")? != context.target().identity().as_str()
        || reader.field("execution")?
            != context
                .build_execution_profile()
                .map(|target| target.identity().as_str())
                .unwrap_or("none")
    {
        return Err(Error::OccurrenceCoverage);
    }
    let count = reader.count("bindings", budget.maximum_elements())?;
    if count > text.len() / 32 {
        return Err(Error::InvalidFraming);
    }
    budget.elements(count)?;
    budget.entries::<AcceptedSemanticBinding>(count)?;
    let mut bindings = Vec::new();
    crate::reserve!(bindings, count, Error);
    for _ in 0..count {
        let role = [reader.count("role", u8::MAX as usize)? as u8];
        let role = recover_semantic_binding_role(&mut ObservationCursor::new(&role))
            .ok_or(Error::InvalidFraming)?;
        let owner = PackageKeyIdentity::from_digest(digest(reader.field("owner")?)?)
            .ok_or(Error::InvalidFraming)?;
        let path = reader.section("declaration", MAXIMUM_POLICY_TEXT_BYTES)?;
        let schema = ServiceSchemaDigest::from_digest(digest(reader.field("schema")?)?);
        let plan = reader.field("plan")?;
        let binding = if plan == "none" {
            AcceptedSemanticBinding::new_service(role, owner, path, schema)
        } else {
            AcceptedSemanticBinding::new(
                role,
                owner,
                path,
                schema,
                ProviderPlanDigest::from_digest(digest(plan)?),
            )
        }
        .map_err(|_| Error::InvalidFraming)?;
        let count = reader.count("permissions", budget.maximum_elements())?;
        if count > text.len() / 32 {
            return Err(Error::InvalidFraming);
        }
        budget.elements(count)?;
        budget.entries::<ServiceTerminalAuthorityPermission>(count)?;
        let mut permission_identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut permissions: Vec<ServiceTerminalAuthorityPermission> = Vec::new();
        crate::reserve!(permissions, count, Error);
        for _ in 0..count {
            let permission_schema =
                ServiceSchemaDigest::from_digest(digest(reader.field("schema")?)?);
            let requirement = reader.section("requirement", MAXIMUM_POLICY_TEXT_BYTES)?;
            if permissions.last().is_some_and(|last| {
                permission_identities.text(last.requirement_identity()) >= Some(requirement)
            }) {
                return Err(Error::InvalidFraming);
            }
            let count = reader.count("classes", TerminalAuthorityClass::ALL.len())?;
            budget.elements(count)?;
            budget.entries::<TerminalAuthorityClass>(count)?;
            let mut classes = Vec::new();
            crate::reserve!(classes, count, Error);
            for _ in 0..count {
                let tag = reader.count("class", u8::MAX as usize)? as u8;
                classes.push(
                    TerminalAuthorityClass::ALL
                        .into_iter()
                        .find(|class| class.canonical_tag() == tag)
                        .ok_or(Error::InvalidFraming)?,
                );
            }
            let disposition = TerminalAuthorityDisposition::try_from_canonical_classes(classes)
                .map_err(|_| Error::InvalidFraming)?;
            permissions.push(ServiceTerminalAuthorityPermission::new(
                permission_schema,
                permission_identities.intern(requirement),
                disposition,
            ));
        }
        bindings.push(
            binding
                .with_terminal_authority_permissions(permissions, &permission_identities.build())
                .map_err(|_| Error::InvalidFraming)?,
        );
    }
    reader.expect("end_compilation")?;
    reader.finish()?;
    AcceptedPackageCompilation::new(compiler, bindings)
}

fn hexadecimal(digest: [u8; 32]) -> String {
    crate::Hex(&digest).to_string()
}

fn digest(text: &str) -> Result<[u8; 32], Error> {
    hex_digest(text).ok_or(Error::InvalidFraming)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declarations::DependencyPurpose;
    use crate::lock::PackageLockRecoveryLimits;
    use package_compilation::AcceptedSemanticBindingRole;
    use target::TargetProfile;

    fn package() -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([7; 32]).unwrap()
    }

    fn context() -> PackageCheckedContext {
        PackageCheckedContext::new(
            DependencyPurpose::Product,
            TargetProfile::WindowsX64,
            Some(TargetProfile::WindowsX64),
        )
    }

    fn sample() -> AcceptedPackageCompilation {
        let schema = ServiceSchemaDigest::from_digest([3; 32]);
        let mut sample_identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let binding = AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            package(),
            "std::Console",
            schema,
            ProviderPlanDigest::from_digest([5; 32]),
        )
        .unwrap()
        .with_terminal_authority_permissions(
            vec![ServiceTerminalAuthorityPermission::new(
                schema,
                sample_identities.intern("exit_process(code: I32)"),
                TerminalAuthorityDisposition::from_classes(vec![
                    TerminalAuthorityClass::ProcessTermination,
                ]),
            )],
            &sample_identities.build(),
        )
        .unwrap();
        let service = AcceptedSemanticBinding::new_service(
            AcceptedSemanticBindingRole::TimeHostService,
            package(),
            "std::Clock",
            ServiceSchemaDigest::from_digest([4; 32]),
        )
        .unwrap();
        AcceptedPackageCompilation::new([9; 32], vec![binding, service]).unwrap()
    }

    fn recover(text: &str) -> Result<AcceptedPackageCompilation, Error> {
        read(
            text,
            package(),
            context(),
            &mut Budget::new(PackageLockRecoveryLimits::default()),
        )
    }

    /// A compilation record names exactly one occurrence: a different package,
    /// purpose, target or execution profile must refuse to decode it.
    fn refuses_occurrence(text: &str, key: PackageKeyIdentity, context: PackageCheckedContext) {
        let mut budget = Budget::new(PackageLockRecoveryLimits::default());
        assert_eq!(
            read(text, key, context, &mut budget),
            Err(Error::OccurrenceCoverage)
        );
    }

    #[test]
    fn exact_compilation_bindings_and_permissions_roundtrip() {
        let original = sample();
        let text = write(&original, package(), context()).unwrap();
        let recovered = recover(&text).unwrap();
        assert_eq!(original, recovered);
        assert_eq!(write(&recovered, package(), context()).unwrap(), text);
        assert_ne!(
            recover(&text.replace("std::Console", "std::Another")).unwrap(),
            original
        );
        assert_ne!(
            recover(&text.replace(&"05".repeat(32), &"06".repeat(32))).unwrap(),
            original
        );
    }

    #[test]
    fn compilation_identity_is_required_and_canonical() {
        let text = write(&sample(), package(), context()).unwrap();
        assert_eq!(
            AcceptedPackageCompilation::new([0; 32], Vec::new()),
            Err(Error::CompilerIdentity)
        );
        for changed in [
            text.replace(&"09".repeat(32), &"00".repeat(32)),
            text.replace(&"09".repeat(32), "missing"),
            text.replace(&"09".repeat(32), &"AB".repeat(32)),
            text.lines().skip(1).collect::<Vec<_>>().join("\n"),
        ] {
            assert!(recover(&changed).is_err());
        }
    }

    #[test]
    fn compilation_inputs_cannot_move_between_occurrences_or_contexts() {
        let text = write(&sample(), package(), context()).unwrap();
        let other_package = PackageKeyIdentity::from_digest([8; 32]).unwrap();
        refuses_occurrence(&text, other_package, context());
        assert_eq!(
            recover(&text.replace("purpose product", "purpose build")),
            Err(Error::OccurrenceCoverage)
        );
        let build = PackageCheckedContext::new(
            DependencyPurpose::Build,
            context().target(),
            context().build_execution_profile(),
        );
        refuses_occurrence(&text, package(), build);
        let other_target = TargetProfile::ALL
            .into_iter()
            .find(|target| *target != context().target())
            .unwrap();
        let other_target = PackageCheckedContext::new(
            DependencyPurpose::Product,
            other_target,
            context().build_execution_profile(),
        );
        refuses_occurrence(&text, package(), other_target);
        let other_execution =
            PackageCheckedContext::new(DependencyPurpose::Product, context().target(), None);
        refuses_occurrence(&text, package(), other_execution);
    }

    #[test]
    fn compilation_decode_counts_and_storage_are_aggregate_and_bounded() {
        let text = write(&sample(), package(), context()).unwrap();
        let mut limits = PackageLockRecoveryLimits::default();
        limits.maximum_policy_elements = 4; // two bindings, one permission, one class
        let mut budget = Budget::new(limits);
        assert!(read(&text, package(), context(), &mut budget).is_ok());
        assert!(read(&text, package(), context(), &mut budget).is_err());
        limits.maximum_owned_bytes = text.len() - 1;
        assert_eq!(
            read(&text, package(), context(), &mut Budget::new(limits)),
            Err(Error::AllocationLimitExceeded)
        );
        for changed in [
            text.replace("bindings 2", "bindings 999999999"),
            text.replace("permissions 1", "permissions 999999999"),
            text.replace("classes 1", "classes 99"),
            text.replace("role 0", "role 99"),
            text.replace("class 7", "class 99"),
            format!("{text}trailing\n"),
            text[..text.len() - 1].to_owned(),
        ] {
            assert!(recover(&changed).is_err());
        }
    }
}
