#[cfg(test)]
mod layout_demand_tests {
    #[test]
    fn an_entryless_product_does_not_demand_every_library_layout() {
        let scopes = source_compiler::product_source::layout_target_scopes(
            &source_compiler::source_state::TypedTrees::default(),
            &[],
            &[],
            &[],
            &[],
            &package_compilation::source_imports::source_targets(None),
        )
        .unwrap();
        assert!(
            scopes
                .iter()
                .all(|scope| scope.entry_declarations.is_empty()),
            "an entryless product needs explicit empty layout roots, not all declarations: {scopes:?}"
        );
    }

    /// A claimed native binding's requirement is read in full, so the types
    /// in its signature are laid out, and only on the target the binding
    /// names: a macOS binding's `[CLayout<DarwinTimespec>; 2]` has macOS
    /// geometry and no other.
    #[test]
    fn a_claimed_native_binding_is_a_layout_root_on_its_own_target() {
        use semantic_vocabulary::source_selections::NativeBindingProducer;
        use semantic_vocabulary::source_selections::ProductTarget;
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([3; 32]).unwrap();
        let subject = symbols::SymbolHandle::from_parts(1, 0);
        let requirement = symbols::SymbolHandle::from_parts(2, 0);
        let selection = source_compiler::evaluation::native_bindings::NativeBindingSelection {
            application: None,
            subject,
            requirement,
            requirement_identity: "DarwinTimespec::read".into(),
            service_declaration: [1; 32],
            target: ProductTarget::MacosArm64,
            locator: semantic_vocabulary::NativeBindingLocator::Syscall { number: 1 },
            producer: NativeBindingProducer {
                package,
                declaration_path: "build".into(),
                callable_identity: "build::bind_times".into(),
            },
            source_occurrence_digest: [5; 32],
            supplier_package: package,
            source_span: Default::default(),
        };
        let scopes = source_compiler::product_source::layout_target_scopes(
            &source_compiler::source_state::TypedTrees::default(),
            &[],
            &[],
            &[],
            &[&selection],
            &package_compilation::source_imports::source_targets(None),
        )
        .unwrap();
        for scope in &scopes {
            let roots = scope.entry_declarations.contains(&subject)
                && scope.entry_declarations.contains(&requirement);
            assert_eq!(roots, scope.target == "macos_arm64", "{scopes:?}");
        }
        assert!(scopes.iter().any(|scope| scope.target == "macos_arm64"));
    }
}
