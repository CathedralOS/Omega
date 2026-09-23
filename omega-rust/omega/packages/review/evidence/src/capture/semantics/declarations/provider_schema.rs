//! Exact declaring-schema joins for selected provider requirements.

use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

/// The selected schema owns provider/calling policy. An inherited requirement
/// retains its own declaring schema; only exact reachable symbols establish
/// that association. Repeated paths to the same declaration do not add owners.
pub(crate) fn provider_requirement_schema(
    compilation: &PackageReviewInput<'_>,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
) -> Result<ProviderSchemaDeclaration, Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(root) = schema else {
        return Ok(schema);
    };
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut owners = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let definitions = compilation
            .traits()
            .iter()
            .filter(|candidate| candidate.symbol == symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "selected schema has no unique exact declaring trait",
            ));
        };
        let matching = compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|candidate| candidate.symbol == requirement)
            .count();
        if matching > 1 {
            return Err(rejected("selected schema repeats its exact requirement"));
        }
        if matching == 1 {
            owners.push(symbol);
        }
        pending.extend(
            compilation
                .trait_requirements(definition)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    let [owner] = owners.as_slice() else {
        return Err(rejected(
            "selected schema does not inherit one exact requirement declaration",
        ));
    };
    Ok(ProviderSchemaDeclaration::BoundaryTrait(*owner))
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected provider requirement rejects {reason}"
    ))]
}

#[cfg(test)]
mod tests {
    use super::provider_requirement_schema;
    use crate::capture::PackageReviewInput;
    use compiler::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
    use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
    use provider_planning::ProviderSchemaDeclaration;
    use semantic_vocabulary::PackageKeyIdentity;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use symbols::SymbolHandle;
    use typed_trees::trait_definition::TraitDefinition;

    struct Source(PathBuf);

    impl Drop for Source {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn checked_source() -> (Source, CheckedCompilation) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let source = Source(std::env::temp_dir().join(format!(
            "omega-trait-scope-collision-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )));
        std::fs::create_dir(&source.0).unwrap();
        std::fs::write(
            source.0.join("main.omg"),
            "pub trait Host { machine work() -> u64; }\n",
        )
        .unwrap();
        std::fs::write(
            source.0.join("build.omg"),
            "machine build(builder: &mut Build) { builder.package(\"review_fixture\"); }\n",
        )
        .unwrap();
        let package = PackageKeyIdentity::from_digest([43; 32]).unwrap();
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "review_fixture",
                source.0.clone(),
            )],
            Vec::new(),
        )
        .unwrap();
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&source.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("single-requirement trait fixture checks");
        (source, checked)
    }

    fn host_fixture(custody: &CheckedCompilation) -> (SymbolHandle, SymbolHandle) {
        let trait_definition = custody
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Host")
            .expect("fixture declares Host");
        let requirement = custody
            .trait_machine_signatures(trait_definition)
            .first()
            .expect("Host declares work")
            .symbol;
        (trait_definition.symbol, requirement)
    }

    #[test]
    fn provider_requirement_rejoins_its_unique_declaring_trait() {
        let (_source, custody) = checked_source();
        let program = custody.clone().into_program();
        let (trait_symbol, requirement) = host_fixture(&custody);
        let input = PackageReviewInput::supplied(&program, &custody);

        let resolved = provider_requirement_schema(
            &input,
            ProviderSchemaDeclaration::BoundaryTrait(trait_symbol),
            requirement,
        )
        .expect("a unique declaring trait rejoins the schema");
        assert_eq!(
            resolved,
            ProviderSchemaDeclaration::BoundaryTrait(trait_symbol)
        );
    }

    #[test]
    fn provider_requirement_rejects_a_scope_colliding_declaring_trait() {
        // A second definition bound to the trait's symbol is the reviewed
        // scope collision: the schema join must reject rather than pick one.
        let (_source, custody) = checked_source();
        let mut program = custody.clone().into_program();
        let (trait_symbol, requirement) = host_fixture(&custody);
        program.typed.push_trait_definition(TraitDefinition {
            symbol: trait_symbol,
            ..TraitDefinition::default()
        });
        let input = PackageReviewInput::supplied(&program, &custody);

        let error = provider_requirement_schema(
            &input,
            ProviderSchemaDeclaration::BoundaryTrait(trait_symbol),
            requirement,
        )
        .expect_err("a duplicated declaring trait must reject");
        assert!(error[0].message.contains("no unique exact declaring trait"));
    }

    #[test]
    fn provider_requirement_rejects_a_requirement_outside_the_trait_scope() {
        // A requirement symbol the trait graph does not declare leaves zero
        // owners — no fallback to the schema's root symbol is allowed.
        let (_source, custody) = checked_source();
        let program = custody.clone().into_program();
        let (trait_symbol, _) = host_fixture(&custody);
        let input = PackageReviewInput::supplied(&program, &custody);

        let error = provider_requirement_schema(
            &input,
            ProviderSchemaDeclaration::BoundaryTrait(trait_symbol),
            SymbolHandle::from_arena_index(0x0FFF_FFFF),
        )
        .expect_err("an undeclared requirement must reject");
        assert!(
            error[0]
                .message
                .contains("does not inherit one exact requirement declaration")
        );
    }
}
