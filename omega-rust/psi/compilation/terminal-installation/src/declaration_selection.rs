//! Package and toolchain custody rules for selecting a declaration.
//!
//! These checks grant no execution effects. Callers retain the source-issued
//! occurrence and apply the authority for that occurrence's actual purpose.

use semantic_vocabulary::compiler_evaluation::{DeclarationCustody, DeclarationSelectionAuthority};

pub fn require_declaration_selection(
    requester: DeclarationCustody,
    owner: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
) -> Result<(), String> {
    match (requester, owner) {
        (DeclarationCustody::Missing, _) | (_, DeclarationCustody::Missing) => {
            Err("build-time invocation lacks compiler-owned source/package provenance".into())
        }
        (DeclarationCustody::UnownedUser, _) | (_, DeclarationCustody::UnownedUser) => {
            Err("build-time invocation has user source without reconciled package custody".into())
        }
        // Exact checked selections bind the contract closure before lowering.
        // Its retained role never grants a package caller a default std edge.
        (DeclarationCustody::ToolchainContract, DeclarationCustody::Package(_))
        | (DeclarationCustody::Package(_), DeclarationCustody::ToolchainContract) => {
            Err("build-time invocation crosses closed-contract/package custody".into())
        }
        (DeclarationCustody::ToolchainContract, DeclarationCustody::ToolchainContract) => Ok(()),
        (DeclarationCustody::Toolchain, _) | (_, DeclarationCustody::Toolchain) => Ok(()),
        (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner))
            if requester == owner =>
        {
            Ok(())
        }
        (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner))
            if authority.is_some_and(|authority| {
                authority.allows_declaration_selection(requester, owner)
            }) =>
        {
            Ok(())
        }
        (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner)) => {
            let authority = authority
                .ok_or("build-time invocation has no direct dependency selection authority")?;
            Err(format!(
                "build-time invocation selects package {} from package {} without direct dependency authority",
                authority.package_label(owner),
                authority.package_label(requester)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_contract_custody_never_substitutes_for_a_package_edge() {
        struct AllowPackages;
        impl DeclarationSelectionAuthority for AllowPackages {
            fn allows_declaration_selection(
                &self,
                _: semantic_vocabulary::PackageKeyIdentity,
                _: semantic_vocabulary::PackageKeyIdentity,
            ) -> bool {
                true
            }
            fn package_label(&self, _: semantic_vocabulary::PackageKeyIdentity) -> String {
                "package".into()
            }
        }
        let package = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        );
        let contract = DeclarationCustody::ToolchainContract;
        let core = DeclarationCustody::Toolchain;
        for (requester, owner) in [
            (package, contract),
            (contract, package),
            (DeclarationCustody::Missing, contract),
            (
                DeclarationCustody::Toolchain,
                DeclarationCustody::UnownedUser,
            ),
            (
                DeclarationCustody::UnownedUser,
                DeclarationCustody::Toolchain,
            ),
        ] {
            assert!(require_declaration_selection(requester, owner, Some(&AllowPackages)).is_err());
        }
        for (requester, owner) in [
            (contract, contract),
            (contract, core),
            (core, contract),
            (package, package),
        ] {
            assert!(require_declaration_selection(requester, owner, Some(&AllowPackages)).is_ok());
        }
        let other = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap(),
        );
        assert!(require_declaration_selection(package, package, None).is_ok());
        assert!(require_declaration_selection(package, other, None).is_err());
        assert!(require_declaration_selection(package, other, Some(&AllowPackages)).is_ok());
        assert!(require_declaration_selection(DeclarationCustody::Missing, core, None).is_err());
    }
}
