//! Canonical type identity grammar: package-owner visits over rendered identities.

use crate::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};

/// Counts the package owners a canonical type identity names.
struct PackageOwnerCounter {
    owners: usize,
}

impl TypeIdentityPackageOwnerVisitor for PackageOwnerCounter {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn leave(&mut self) {}
    fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn package_owner(&mut self, _: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        self.owners += 1;
        Ok(())
    }
    fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
}

/// The renderer follows a `dyn` trait's name with its generic arguments as
/// type identities; the grammar must read them (a package review of a program
/// passing `dyn Hooks<T>` could not encode its lock otherwise).
#[test]
fn dynamic_trait_identity_reads_generic_arguments() {
    let identity = concat!(
        r"dynamic-trait(name(nominal\(package-owner\(32:",
        "7f51c408406b55784a8d8130b3ddf511c938d99257c59be2ea74046ebc7bd4e2",
        r"\)\,path\(src::structures::structs::symbolic_struct_resolver::SymbolicStructResolverHooks\)\)),",
        "named(name(type-parameter:0)))",
    );
    let mut counter = PackageOwnerCounter { owners: 0 };
    visit_type_identity_package_owners(identity, &mut counter)
        .expect("a dyn trait with one generic argument is a complete identity");
    assert_eq!(counter.owners, 1);
    let bare = r"dynamic-trait(name(compiler-type\(atom\(u8\)\)))";
    visit_type_identity_package_owners(bare, &mut counter)
        .expect("a dyn trait without arguments is a complete identity");
    let malformed = r"dynamic-trait(name(compiler-type\(atom\(u8\)\)),conformance(x))";
    assert!(visit_type_identity_package_owners(malformed, &mut counter).is_err());
}

/// A const generic parameter renders its argument through the index grammar
/// (`integer(50)`), not as a type identity. A package review of a signature
/// returning `ForeignBinding<50, 18, 0>` could not encode its lock otherwise.
#[test]
fn generic_identity_reads_const_index_arguments() {
    let identity = concat!(
        r"generic(name(nominal\(toolchain-source-owner\(32:",
        "9ced29a4dddb319ba059ac16618d7e595ebb8d643d3a3b9f51c56245361ce613",
        r"\)\,path\(ForeignBinding\)\)),integer(50),integer(18),integer(0))",
    );
    let mut counter = PackageOwnerCounter { owners: 0 };
    visit_type_identity_package_owners(identity, &mut counter)
        .expect("const index arguments complete a generic identity");
    let mixed = concat!(
        r"generic(name(compiler-type\(atom\(u8\)\)),",
        r"named(name(type-parameter:0)),integer(3))",
    );
    visit_type_identity_package_owners(mixed, &mut counter)
        .expect("type and const arguments may share one application");
    let malformed = r"generic(name(compiler-type\(atom\(u8\)\)),conformance(x))";
    assert!(visit_type_identity_package_owners(malformed, &mut counter).is_err());
}
