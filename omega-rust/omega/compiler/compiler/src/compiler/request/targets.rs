use diagnostics::Diagnostic;
use target::TargetProfile;

/// A nonempty canonical set of exact profiles supplied by one compiler caller.
///
/// This is D54's request boundary, not a support matrix or a request to expand
/// the toolchain catalog. Construction retains only profiles the caller named,
/// removes duplicates, and orders them by the trusted profile catalog. It does
/// not yet perform compilation fan-out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitTargetSet {
    profiles: Box<[TargetProfile]>,
}

impl ExplicitTargetSet {
    pub fn from_caller_names<I, S>(names: I) -> Result<Self, Vec<Diagnostic>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names.into_iter().collect::<Vec<_>>();
        if names.is_empty() {
            return Err(vec![Diagnostic::error(
                "explicit target set must contain at least one exact target profile",
            )]);
        }

        let mut requested = Vec::new();
        let mut diagnostics = Vec::new();
        for name in names {
            let name = name.as_ref();
            if matches!(name, "all" | "*") {
                diagnostics.push(Diagnostic::error(format!(
                    "target-set input `{name}` is not an exact target profile; name each requested profile explicitly",
                )));
                continue;
            }
            match TargetProfile::from_omega_target_name(Some(name)) {
                Ok(profile) if !requested.contains(&profile) => requested.push(profile),
                Ok(_) => {}
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let profiles = TargetProfile::ALL
            .into_iter()
            .filter(|profile| requested.contains(profile))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self { profiles })
    }

    pub const fn profiles(&self) -> &[TargetProfile] {
        &self.profiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_catalog_expansion_spellings() {
        let empty = ExplicitTargetSet::from_caller_names::<[&str; 0], &str>([])
            .expect_err("empty target set must reject");
        assert_eq!(empty.len(), 1);
        assert!(empty[0].message.contains("at least one exact target"));

        for spelling in ["all", "*"] {
            let diagnostics = ExplicitTargetSet::from_caller_names([spelling])
                .expect_err("catalog expansion must reject");
            assert_eq!(diagnostics.len(), 1);
            assert!(
                diagnostics[0]
                    .message
                    .contains("name each requested profile")
            );
        }
    }

    #[test]
    fn rejects_unknown_profiles_without_losing_other_invalid_inputs() {
        let diagnostics = ExplicitTargetSet::from_caller_names(["unknown", "*", "also_unknown"])
            .expect_err("every invalid supplied profile must reject");
        assert_eq!(diagnostics.len(), 3);
        assert!(
            diagnostics[0]
                .message
                .contains("unknown target profile `unknown`")
        );
        assert!(diagnostics[1].message.contains("target-set input `*`"));
        assert!(
            diagnostics[2]
                .message
                .contains("unknown target profile `also_unknown`")
        );
    }

    #[test]
    fn aliases_normalize_deduplicate_and_sort_in_catalog_order() {
        let targets = ExplicitTargetSet::from_caller_names([
            "windows_x64",
            "linux_x86_64",
            "linux_x64",
            "macos_arm64",
        ])
        .expect("exact names and transitional aliases normalize");
        assert_eq!(
            targets.profiles(),
            &[
                TargetProfile::LinuxX64,
                TargetProfile::MacosArm64,
                TargetProfile::WindowsX64,
            ]
        );
    }

    #[test]
    fn input_order_does_not_change_canonical_order() {
        let forward =
            ExplicitTargetSet::from_caller_names(["linux_arm64", "uefi_x86_64", "local_unchecked"])
                .expect("forward request");
        let reverse =
            ExplicitTargetSet::from_caller_names(["local_unchecked", "uefi_x64", "linux_arm64"])
                .expect("reverse request");
        assert_eq!(forward, reverse);
    }

    #[test]
    fn one_target_preserves_exact_identity() {
        let targets = ExplicitTargetSet::from_caller_names(["cross_platform_cli"])
            .expect("one exact target is a valid set");
        assert_eq!(targets.profiles(), &[TargetProfile::CrossPlatformCli]);
    }

    #[test]
    fn normalization_never_adds_an_unrequested_catalog_profile() {
        let targets = ExplicitTargetSet::from_caller_names(["linux_x86_64", "windows_x86_64"])
            .expect("two exact targets");
        assert_eq!(targets.profiles().len(), 2);
        assert!(targets.profiles().contains(&TargetProfile::LinuxX64));
        assert!(targets.profiles().contains(&TargetProfile::WindowsX64));
        assert!(
            TargetProfile::ALL
                .into_iter()
                .filter(|profile| !matches!(
                    profile,
                    TargetProfile::LinuxX64 | TargetProfile::WindowsX64
                ))
                .all(|profile| !targets.profiles().contains(&profile))
        );
    }
}
