//! Exact source-to-compiler agreement for the first public placement vocabulary.
//!
//! This pins nominal shape only. It deliberately does not exercise or imply
//! placement operations, authority issuance, custody agreement checking, or a
//! runtime representation for the `Extent` domains.

use omega_compiler::compile_to_checked;
use psi_language_core::DataSupplyMode;
use psi_source::SourceOrigin;
use psi_typed_trees::data::{DataMember, TypeParameterKind};
use std::fs;
use std::path::{Path, PathBuf};

struct TemporaryProgram(PathBuf);

impl TemporaryProgram {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-placement-core-surface-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create placement core-surface fixture");
        fs::write(
            directory.join("main.omg"),
            r#"use omega::language::core::extent;
use omega::language::core::layout;

data Main {
}

machine Main::main(&mut self) {
}
"#,
        )
        .expect("write placement core-surface fixture");
        Self(directory)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TemporaryProgram {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn parameter_names(parameters: &[psi_typed_trees::data::TypeParameter]) -> Vec<&str> {
    parameters
        .iter()
        .map(|parameter| {
            assert_eq!(parameter.kind, TypeParameterKind::Type);
            parameter.name.as_str()
        })
        .collect()
}

#[test]
fn imported_core_placement_vocabulary_has_the_exact_settled_shape() {
    let fixture = TemporaryProgram::new();
    let checked = compile_to_checked(&fixture.main(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "the normative core placement vocabulary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let placed = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Placed")
        .expect("core Placed declaration");
    assert!(placed.is_public);
    assert_eq!(placed.supply_mode, DataSupplyMode::BoundaryOpaque);
    assert_eq!(placed.generic_instance, None);
    assert_eq!(
        parameter_names(checked.typed.data_type_parameters(placed)),
        ["P", "T"]
    );
    assert!(checked.typed.data_members(placed).is_empty());
    assert_toolchain_source(&checked.typed, placed.symbol, "core/layout.omg");

    let outcome = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "PlacementOutcome")
        .expect("core PlacementOutcome declaration");
    assert!(outcome.is_public);
    assert_eq!(outcome.supply_mode, DataSupplyMode::CheckedShape);
    assert_eq!(
        parameter_names(checked.typed.data_type_parameters(outcome)),
        ["View", "Returned", "Reason"]
    );
    let outcome_parameters = checked.typed.data_type_parameters(outcome);
    let members = checked.typed.data_members(outcome);
    let [DataMember::Variant(ready), DataMember::Variant(rejected)] = members else {
        panic!("PlacementOutcome must remain the canonical two-case flat sum");
    };
    assert_eq!(ready.name.as_str(), "Ready");
    let [view] = checked.typed.data_payload_fields(ready) else {
        panic!("PlacementOutcome::Ready must carry exactly one field");
    };
    assert_eq!(view.name.as_str(), "view");
    assert_eq!(
        checked.typed.type_reference_symbol(view.type_reference),
        outcome_parameters[0].symbol
    );
    assert_eq!(rejected.name.as_str(), "Rejected");
    let [returned, reason] = checked.typed.data_payload_fields(rejected) else {
        panic!("PlacementOutcome::Rejected must carry returned then reason");
    };
    assert_eq!(returned.name.as_str(), "returned");
    assert_eq!(reason.name.as_str(), "reason");
    assert_eq!(
        checked.typed.type_reference_symbol(returned.type_reference),
        outcome_parameters[1].symbol
    );
    assert_eq!(
        checked.typed.type_reference_symbol(reason.type_reference),
        outcome_parameters[2].symbol
    );
    assert_toolchain_source(&checked.typed, outcome.symbol, "core/layout.omg");

    let returned = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "PlacementReturn")
        .expect("core PlacementReturn declaration");
    assert!(returned.is_public);
    let returned_parameters = checked.typed.data_type_parameters(returned);
    assert_eq!(parameter_names(returned_parameters), ["Source", "Custody"]);
    let members = checked.typed.data_members(returned);
    let [DataMember::Field(source), DataMember::Field(custody)] = members else {
        panic!("PlacementReturn must remain a two-field record");
    };
    assert_eq!(source.name.as_str(), "source");
    assert_eq!(custody.name.as_str(), "custody");
    assert_eq!(
        checked.typed.type_reference_symbol(source.type_reference),
        returned_parameters[0].symbol
    );
    assert_eq!(
        checked.typed.type_reference_symbol(custody.type_reference),
        returned_parameters[1].symbol
    );

    let custody_trait = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PlacementCustody")
        .expect("core PlacementCustody trait");
    assert!(custody_trait.is_public);
    assert!(!custody_trait.is_boundary);
    assert_eq!(
        parameter_names(checked.typed.trait_type_parameters(custody_trait)),
        ["P", "T"]
    );
    assert!(checked.typed.trait_requirements(custody_trait).is_empty());
    assert!(
        checked
            .typed
            .trait_machine_signatures(custody_trait)
            .is_empty()
    );
    assert_toolchain_source(&checked.typed, custody_trait.symbol, "core/layout.omg");

    let extent = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Extent")
        .expect("core Extent declaration");
    for (name, expected_parameters) in [("Vacant", &[][..]), ("Resident", &["P", "T"][..])] {
        let domain = checked
            .typed
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str().rsplit("::").next() == Some(name))
            .unwrap_or_else(|| panic!("core Extent::{name} domain"));
        assert!(domain.is_public);
        assert_eq!(
            checked.typed.type_reference_symbol(domain.target_type),
            extent.symbol,
            "Extent::{name} must qualify the runtime Extent carrier"
        );
        assert_eq!(
            parameter_names(checked.typed.domain_type_parameters(domain)),
            expected_parameters
        );
        assert_eq!(domain.index_arguments.len(), expected_parameters.len());
        for (argument, parameter) in domain
            .index_arguments
            .iter()
            .zip(checked.typed.domain_type_parameters(domain))
        {
            assert_eq!(
                checked.typed.type_reference_symbol(*argument),
                parameter.symbol,
                "Extent::{name} must retain each invariant type index in declaration order"
            );
        }
        assert!(checked.typed.proof_facts(domain).is_empty());
        assert!(domain.establishment_routes.is_empty());
        assert_toolchain_source(&checked.typed, domain.symbol, "core/extent.omg");
    }
}

fn assert_toolchain_source(
    checked: &psi_typed_trees::TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    suffix: &str,
) {
    let source = checked
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| checked.symbols.source_file(span))
        .expect("core declaration must retain authored source custody");
    assert_eq!(source.origin, SourceOrigin::Toolchain);
    assert!(
        source.path.ends_with(Path::new(suffix)),
        "expected source suffix {suffix}, got {}",
        source.path.display()
    );
}
