//! Package and toolchain custody rules for selecting a declaration.
//!
//! These checks grant no execution effects. Callers retain the source-issued
//! occurrence and apply the authority for that occurrence's actual purpose.

use crate::DeclarationSelectionAuthority;
use semantic_vocabulary::compiler_evaluation::{DeclarationCustody, declaration_authority_request};

pub fn require_declaration_selection(
    requester: DeclarationCustody,
    owner: DeclarationCustody,
    authority: Option<&dyn DeclarationSelectionAuthority>,
) -> Result<(), String> {
    let Some(request) = declaration_authority_request(requester, owner)? else {
        return Ok(());
    };
    if request.requester == request.owner {
        return Ok(());
    }
    let authority =
        authority.ok_or("build-time invocation has no direct dependency selection authority")?;
    if authority.allows_declaration_selection(request.requester, request.owner) {
        return Ok(());
    }
    Err(format!(
        "build-time invocation selects package {} from package {} without direct dependency authority",
        authority.package_label(request.owner),
        authority.package_label(request.requester)
    ))
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
            assert!(
                declaration_authority_request(requester, owner).is_err(),
                "collecting proof references must reject {requester:?} -> {owner:?}"
            );
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
