//! Implementation sites bound by ledger entries.
//!
//! A site is one source file whose recorded SHA-256 pins its contents. Any
//! edit to a bound file fails the coverage test until the citing entries have
//! been revalidated and the digest updated. Files under
//! `terminal-verifier/src/trusted_surface` are the inventory machinery itself:
//! they cannot carry their own digest and bind by path only.
//!
//! `TEST_ONLY_SOURCES` lists `#[cfg(test)]`-gated modules: they carry no
//! production authority, but the coverage check verifies each remains
//! test-gated so production code cannot hide behind the label. Gating is
//! transitive: a `mod` declared inside a gated test-only file is itself
//! test-only.

use super::ImplementationSite;

/// Crate roots whose entire non-test source tree is trusted surface.
pub static TRUSTED_SOURCE_ROOTS: &[&str] = &[
    "omega-rust/psi/compilation/proof/admission/src",
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src",
    "omega-rust/psi/compilation/representations/terminal-psi/verification/src",
];

/// A `#[cfg(test)]`-gated module file excluded from the trusted surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestOnlySource {
    /// Repository-relative file path.
    pub path: &'static str,
    /// Parent module file that must declare `mod <module>;` either directly
    /// under `#[cfg(test)]` or inside a parent file that is itself gated.
    pub parent: &'static str,
    /// The declared module name (file stem).
    pub module: &'static str,
}

pub static TEST_ONLY_SOURCES: &[TestOnlySource] = &[
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references/array_tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs",
        module: "array_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/shared_mutation/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/shared_mutation.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping/relative_translation/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping/relative_translation.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/wrapping_equality/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/wrapping_equality.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/order_antisymmetry_tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation.rs",
        module: "order_antisymmetry_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/order_translation/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/order_translation.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/call_accesses/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/call_accesses.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_ordering/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_ordering.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_lifetimes/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_lifetimes.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage_regions/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage_regions.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/remainder/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/remainder.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_cast/expression_roots.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_cast.rs",
        module: "expression_roots",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/implications/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/certificate_search/implications.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/integer_order/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/certificate_search/integer_order.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/order_chain/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/certificate_search/order_chain.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests/bound_mapping.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests.rs",
        module: "bound_mapping",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests/witness_checking.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests.rs",
        module: "witness_checking",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests/wrapping_bounds.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests.rs",
        module: "wrapping_bounds",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests/wrapping_subtraction.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/tests.rs",
        module: "wrapping_subtraction",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/kernel/carrier_bounds.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/kernel.rs",
        module: "carrier_bounds",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/addition/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/addition.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/binary_numerals/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/binary_numerals.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/casts/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/casts.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/equality_transport/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/equality_transport.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/forbidden_roots/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/forbidden_roots.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/multiplication/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/multiplication.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction/bound_tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        module: "bound_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction/order_tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        module: "order_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/identity_and_w_types.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "identity_and_w_types",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/levels_and_declarations.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "levels_and_declarations",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "schemes_and_quotients",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients/indexed_schemes.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "indexed_schemes",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients/quotient_derived.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_derived",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients/quotient_receiver_policies.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_receiver_policies",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients/quotient_schemes.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_schemes",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/spine_chains.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "spine_chains",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/strict_layer.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "strict_layer",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests/type_checking.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/tests.rs",
        module: "type_checking",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/value_equality_tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation.rs",
        module: "value_equality_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/disjunction_elimination.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/proof.rs",
        module: "disjunction_elimination",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/order_discreteness/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/proof/order_discreteness.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/order_substitution_tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/proof.rs",
        module: "order_substitution_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/strict_order_transitivity/tests.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/proof/strict_order_transitivity.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/case_membership_tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        module: "case_membership_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/subslice_tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        module: "subslice_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/lib.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_graph/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_graph.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/conditions/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/conditions.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/transport/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/transport.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants.rs",
        module: "tests",
    },
];

pub static IMPLEMENTATION_SITES: &[ImplementationSite] = &[
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/shared_mutation.rs",
        sha256: Some("82a731f83e16770241cda5b105b3931bb6b11f46fecdf54b4e38f0435a00d75a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/reference_views.rs",
        sha256: Some("72949075f98bbb5bde830cd545b872eb85edcc18fbf945c027eb786e49dacece"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs",
        sha256: Some("0af0cb0f621f482572725cce1bc8cbfeb28863536245e49a9f50a787fe1e25de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_predicate.rs",
        sha256: Some("be3d63ca1d173d5a51b98bb0f816f3d8afe8d22f957c7eedfaf07c4741f171e6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_ordering/contracts.rs",
        sha256: Some("b0efc8edf8ba477e0ce918faedb6b17d61956845dc44bd25840093ec7c6fa1d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/borrowed_storage.rs",
        sha256: Some("d359bda608c1d43da38546fee493c4cca2033ec8e11f3b61ae2d38391068c5f0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_ordering.rs",
        sha256: Some("8ad9f2f8a322c42b75e822b642b2b43b5929bbb2a0606d29bc56a25db37e2482"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_lifetimes.rs",
        sha256: Some("8008b817e463dc887ac3a013258049254a95304f5ec276205100ff9b0d0ac568"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_lifetimes/locations.rs",
        sha256: Some("521937701063031baa37832f14947c90c5ed68d728edc788fe10a31d281859cf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/boundary_calls.rs",
        sha256: Some("70da71fb45e6d43dcaefe99b3ccd8d947291fc995d3fc737dd42b4ab5fbe3e06"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/owned_shape.rs",
        sha256: Some("b61b272e1943c223c627672e5994f916b876552fbc5ba06add269e1510b1cd32"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/replay.rs",
        sha256: Some("4210efed99d3635bad72124902f637f633dc2dd6b04dcb24722d6dd1d57c9490"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/terminal_flow.rs",
        sha256: Some("e6472dcb9aec138c7e061ca60eedf07706b0e226fb783dafdcaf566c0ab3992e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results/compatibility.rs",
        sha256: Some("06dd5b32f62caff788501643032c5df379637ed1d90f495f54d80415113f9d62"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/compatibility.rs",
        sha256: Some("12826e101cd126f54dcdd296a26e7626e0ed56acedce1855a7836511f06867cb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results.rs",
        sha256: Some("d4ff7717067af40172d99c5a7b23b62cdc9e95995b48d2f40fa39fff74c0132c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans.rs",
        sha256: Some("b2a7902d09ed08504c43fe48d134ee5f456ac749dcdd52bba5d100566905fd67"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/aliases.rs",
        sha256: Some("02adb0dba63261f5b9a58ae78a0279bc0eacf2115f30c390a3575b6d3760c1f3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/call_accesses.rs",
        sha256: Some("536c5fd0b243a2ab63856ca4915ca03c0510d694b0ed766d55b8f3fbfa79f101"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/accesses.rs",
        sha256: Some("52fe8dd7e9dbc4685628bc1a8026a849a3ba90985b8aa1c1ba59fbd5c05b340a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/flow.rs",
        sha256: Some("755518f38fd96c630fc9d67f8a14f0494abd2bf9cd89a45333b9d94ead539bd2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/scalar_ordering.rs",
        sha256: Some("080e019d080ff8afc44b04b14789a75897f0aef1845b6dca3d6ad8e50b62ad63"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/access_regions.rs",
        sha256: Some("7fb612b5f843abdcc9652ae2e4d953d1b94ca0d30326c74ac7c36d89d927201e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/numerics/src/float_semantics.rs",
        sha256: Some("699ea5a9337710de2a97e013365185059411166a1e4f3be23ec277874a26b6a8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/numerics/src/bignum/exact_float.rs",
        sha256: Some("71d49803b8af967d5b0b8d1d7c8df960b6329c267e691b2ae2b580b65db86e7f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/scalar_terms.rs",
        sha256: Some("493521753c444411b8db09f96992742588037088dc50d8b66b73e829c6a6d234"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/propositions.rs",
        sha256: Some("4e2ebfc2e7807adfae26cd01d5a738bb7f6407658b767a378572fabe2b4e69d3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/value_ids.rs",
        sha256: Some("2ea66758cdb4f9026ba27e55a6cc3104d79aa02ba270e2d19633dc4caafd8bd8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/language-semantics/src/byte_predicates/mod.rs",
        sha256: Some("6f39e7941f5e5694b15173f05aa04220127698fc9ddbea2664e5de6a8835b5ca"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs",
        sha256: Some("6d2ed7bb9cd835395b537f868b5a8a11a24894dfc3d7d8c90ac87b1450d32c11"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        sha256: Some("941d5c1c37dc38a86d878c6b79b7465d0b8fa356e6678dc9ebcbb387f97652b7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/bounded_byte_owner.rs",
        sha256: Some("b0f2c0d8d1c599f9b74e0cbf186186510d3691fa7d7edf3c142a2e960d36d35f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/store.rs",
        sha256: Some("41ed5e3bef911964c00b1b2b27a52ba63a1a359a3b0a2e7ba05600ba418a2e55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/structural_places.rs",
        sha256: Some("2962d58d58ef90b27c6eac2de081ef7b6f134d1b34163a6f9ae36f32dc2fc687"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/content.rs",
        sha256: Some("8d8ec5a13b932f22942b2ba4171c7032681f2e79187a695df8119892cfdeb24f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs",
        sha256: Some("a36e62df711dddf9142ffe69aaf9cba84180d1edbdae77fcbff5134af456700b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/admission.rs",
        sha256: Some("7ad95ac4518c8a012912448d0392d067b4de3b99215362fa31b40b0cd6586809"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/crash.rs",
        sha256: Some("7d41030fd5d2a7214096f9647109abffc4ed4f9204d8ba8a07313f02fa92ba75"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/nodes.rs",
        sha256: Some("ee89426aff0de22e258e9b697f2924e72d50714581e93cfb92a37c4d13d64402"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/witnesses.rs",
        sha256: Some("e84d2df5d97be728a4e94d2a4cb008448ac2196ebf64ab41a1a4f2e47d393dc9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/float_result_policy.rs",
        sha256: Some("58a8c5068af98ada134458f0e306e22f4b5e0f79bf0273160b9ec358e71f6145"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/operations.rs",
        sha256: Some("6abd81884059b82e295bc1e0ce56a841c870fd0c05b2509117fb520a62a7574d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("e22fd69c0f1171e2775698ce775c02e28c1d56600771403648134dd1be424b13"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/admission.rs",
        sha256: Some("12ced6eecd22f86f8c238ffb1f14cf96299a86021abc37cc9b28b7ea68413bb0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/admission/denotation.rs",
        sha256: Some("d81711d9858c6b8ceb8882ae3a63a9f602a7723615203f63c64c0f5f466b0b2f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/admission/evidence.rs",
        sha256: Some("c37914c305f49aa3c2265011341307d5f91bff91138dccbf1d8c25a52c100884"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/admission/normalization.rs",
        sha256: Some("cfb73772f16ad4583520c7797bd9c76bd4cf89f432524935e3cf4518f91ceeac"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/admission/recursion.rs",
        sha256: Some("de08da430299a812bbf426462ee880971d77809a503858b09772c854fd087c93"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search.rs",
        sha256: Some("9f5da2bc71be3172ff3f44e5c0eff37c05bcb5ca92bada9b65ba2ab562fede08"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/integer_order.rs",
        sha256: Some("51b50058cade8d80523241f61553fcc9c53eb73cdb6042b8496792d14c2df583"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/order_chain.rs",
        sha256: Some("162a0bbc1188a11ef9bb76f24330313273056025907881db20f6fd1d4d0784cf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/implications.rs",
        sha256: Some("3de7bedae5593bc596f2804b0cde758fa1c217b6ff81c39d4abb0a28dc02008b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/certificate_search/original_citations.rs",
        sha256: Some("50fe7b0868c3fe6fe4dce80c65480c891c3c19b4cbdbf8f455c46c874f41c6e7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/classicality.rs",
        sha256: Some("bdb947ddcf1cb971ffd8016f6512c20ca95ea7986b4c7a8cd5024f11d5efb540"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules.rs",
        sha256: Some("35bd97ac098f7f81d927f050fb7f57b30bbfea0f588c9c117f8deb07c4145963"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/closed_integer.rs",
        sha256: Some("ff8aadab21e62ea1837c18d3691735f3c865cb8b4563b61adb3d363d8fadba5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine.rs",
        sha256: Some("62afc2c3601a06a5175556d98022ec80d66321e158ae2dd1b296bf9a04ade90d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping.rs",
        sha256: Some("9742157417cb3f17e9e0d7d80a51030390032020bae9949a769474a360e205d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping/relative_translation.rs",
        sha256: Some("fffb1aa80b0bc1c4cf1ee3db3db8d2d005b254183b9b3df31b12a62c98941b92"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/remainder.rs",
        sha256: Some("4b18f94c8235c9e32a77b5f41f20e4af09572d9c82320bfc2d1061e49a1d04d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/truth_bounds.rs",
        sha256: Some("046a3a3513572b95359c37e16274b72473dae2ceb46a3f8fc360c5497575a40b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/witness_checking.rs",
        sha256: Some("b5e00dd4a8250188e2f9bc154c9c293dc7b1612682e79e48cd20f238803f8110"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_cast.rs",
        sha256: Some("044301057d5708b2e18f1a1f4337dbff3cb2e958df1a2b03332e40f5c89722c7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_forbidden_root.rs",
        sha256: Some("da61566583f17564c7d5165fbf71989188633ab7d1633ecbb14dd0071795d035"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_shift.rs",
        sha256: Some("294332d58db90760583e627b54c1c0c90f9eb33b9885ed563bd8d285dd20a60f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/open_terms.rs",
        sha256: Some("e26145fd2dccd1c5172103ae422da576704598f192c8dbc2d815062bc7654736"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/wrapping_equality.rs",
        sha256: Some("a0c4361fc2459e3dae078e7593b96cced32180f631c52ae5aa4bfdef7a8eaf67"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/kernel.rs",
        sha256: Some("b4febd793396ddb49cb2d3bbccf9ebd4c357915b582cbdb8f26768008300a125"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/lib.rs",
        sha256: Some("36c52b8b1c5db4285fdfd5ae1028cab6cb68e6e696a8a7ec8f399575527b31d4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs",
        sha256: Some("773af35589f52db245c3106fe467822dc8d8cbac6052bee6bf31e7f77f4c37c5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("b816a7dca6a3e829a812cb1967ea47724664f1e335a22c537d7309776f63ee96"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/addition.rs",
        sha256: Some("260f6ffe3f9fb7da2ec6a6fd5aed11f6dee88634c4896f1ffbe37d1bb4492680"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/binary_numerals.rs",
        sha256: Some("2086cd2677ebaddae90d2ce4d06b1a350e505af5cd7829bef0d23a8e85a1aa37"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/booleans.rs",
        sha256: Some("46dd515b0be7114a6ac563809c9c2ca8101c122a86fa25043c7d900a94b56a34"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/casts.rs",
        sha256: Some("b9bd71ca951481c19648ac075e2543c7bb5192b017743a4fd19bb5425a7c953c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/equality_transport.rs",
        sha256: Some("14fa8de593fba6b42de942cbfca3be4da699c198cd19668d046c5a29fe7fc4c4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/forbidden_roots.rs",
        sha256: Some("638acff1f243928a16b3a4b7bbdd0660477b167ce8104314ae918906850f0966"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/integer_operations.rs",
        sha256: Some("b65519fcf58ccd9bf4f3b9d332f6ae6664bbc780a8779b3aa5833bdfbb1943d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/multiplication.rs",
        sha256: Some("0470674b87e7def74c607017933abede6379a8cb6442926f94e5e7bf7c2db35c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/order_translation.rs",
        sha256: Some("4e476f0ef6c7b8d039391f0da5b7e8e4bbf2a5cfb7e6f02750a46fc469897b08"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        sha256: Some("b6de97ccf6105570a648fdb669b8654cf52f0a849c28e7fccb9b0712575aadc6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/certificate.rs",
        sha256: Some("a8324854250d63cf51f8b9534c4af80c01df650594a587b06ea41d1f0d30d4f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/conversion.rs",
        sha256: Some("4f1da58b4cb32429bb37c5502e77dac6fdfe2ac85112e32995bebd0aebdf9f60"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/indexed.rs",
        sha256: Some("6cc89bb22f8cb76cd9a4055bb3ef3d56947f08fe80e3d379b8a78b18e774cf05"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/quotient.rs",
        sha256: Some("827d583d57882acdb0bdb75b3f67a545538f770e25d288007a7fae222efea064"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/scheme_dsl.rs",
        sha256: Some("dfa4f7cdb006aa7a3bd1ecd8362e87edd1baec9716788ef79970952e5b87bff5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/signature.rs",
        sha256: Some("4b3a1f4d394f0bc86c40d26c4c92a3301d83463f917efd888678cc14b6d6dc46"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/substitution.rs",
        sha256: Some("e6d1a91185300eaec8cf89f03e4bbf07815bf5789c5b1b51b47f9a463da6f1b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/term.rs",
        sha256: Some("de10ba6064e0381cf236e0460b07a6a012ffbcabb0d768e5fa171030a9448eba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/theorems.rs",
        sha256: Some("739eb92ea4e0983f8a45a3e06ce8c60eaf5bff23bc48c6d4f94eee695e2dbb11"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/typing.rs",
        sha256: Some("7c1572f9305268dfbd8d3c29eacde03cadfa379e93c6e47a37224671dafe5be7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation.rs",
        sha256: Some("5841b053796a1ff6a5e46a3cf6f9d1366d0271f4d59f0be290498c17b36e9067"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/budget.rs",
        sha256: Some("138c3c1b8a09bb0d7b55cdb00cc0ff927284db2a0ac671e048338cdbae48d8cb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/value_equalities.rs",
        sha256: Some("1df46f02791ae7c95c15c5344d76729ffd579bc526f444b18b58920ea7d2d615"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof.rs",
        sha256: Some("0d498bba4c05ab7373ea89db92ca8425cbf2a32a546589d79682f6931862072f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/add_order.rs",
        sha256: Some("c42a8d709dc770e816eba32f2124b7f731cefc6f9223b822b68603a10ae572cc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/equality_rules.rs",
        sha256: Some("3b68873f5389268867075165cd59365713077a37c620e0543c1eb14f2484d7b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/integer_bound_rules.rs",
        sha256: Some("bf9bc5562e93139b285e50da238a243893e9de5928da43c181137ab490a3cfaf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/integer_math_normalization.rs",
        sha256: Some("722385a8c984413138e11053ea1f37cdf96cb2aacf19ee8929bcb143d079b799"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/integer_order_rules.rs",
        sha256: Some("a6d6542ef184640ac375f2f4526f79f9b9d5ac488fd8cdcc0d8b61affe2980ab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/order_discreteness.rs",
        sha256: Some("c2a95e2c435b9fc0db8a75d6ca1baf8e4af5962d592a73691796b2c4f144b429"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/propositional_rules.rs",
        sha256: Some("14de313a1ed0b0feddf322f36b2f5146399244048745ddb199c3fdb866f319ec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/strict_order_transitivity.rs",
        sha256: Some("69e745be27e311a6a5c4408f2460482e7370a0f324d5d147c386447948601f5f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/subtract_antitone.rs",
        sha256: Some("936f6e6225de33debd36745c303f2fca2e97488072ade18bd77dfe4a59944f13"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/subtract_order.rs",
        sha256: Some("09c561a250c78e4f10467a8978955a9aae1e285ddb3ffeed6b96d45d4a783df0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/traversal.rs",
        sha256: Some("7661230013fe83bf9759c4c52aef709c778c00d03aa4524ffbaeb50b90edef2d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/arrival_storage.rs",
        sha256: Some("99208dcdcae1f3a775d52b9898e3e2a91f3dbebbb2f49d3b507769f5bf99186e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_field_requirements.rs",
        sha256: Some("edca984f5b0db9ec56166c192c47208fa6e7efc4dd2ab364017993e9f5f40eee"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/scalar_refinement.rs",
        sha256: Some("edf0b7b4027896a8fc791b10ce96568c0a5c84a1e899209f9b1178b1d1464f1c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/value_ids/remap.rs",
        sha256: Some("a0184e5c1862328aee919654f0eb115a1749082ae7ae88ec074f0ce5475b02bf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/element_representation.rs",
        sha256: Some("02fffae93c76e985e870d69e13044c70204b75baa11ca8543a585f5c6acf23b9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/representation_regions.rs",
        sha256: Some("bdb8b19fdd60fa4b6cb18c28eef2e4a2a1bf7fa5bf3f01597501a6c238010ece"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/collection_backing.rs",
        sha256: Some("ed49b76376543b921c52aaa71a963b3a9c5449105cb7de80326cbbd864f0f80b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_carriers.rs",
        sha256: Some("e1875df867084361476a530ed32930fa4713aee5648b8468439016fe923e0acf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_concatenation.rs",
        sha256: Some("2d4572ec9e0b27a2d88a8591c07a913b438030c4b9a2784f182d7c030513245e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition.rs",
        sha256: Some("39344115a5ff14ed57e2b3f4988690166f6e6ea0cf0d00484e3888f930d375cb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("ea05f04b2b3dcde6612970b7e08a43718c8128aabb08f13c9dc6055d71340fdd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/lib.rs",
        sha256: Some("1d1d14dd4dfdb10d288c46a09e2421c07112aa6c888f99ebeee16e2bc6f567dc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/service_binding.rs",
        sha256: Some("c046c46b179b978a705a061aabd2c95f2d3178aadab989c6f1261924c82c927a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/placed_view_referent.rs",
        sha256: Some("a9ab51bcda82b55289782c8b13ebe2567ed5294ae88b3ee9d2dd429686dd1393"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/partial_affine.rs",
        sha256: Some("a5bc41aa52435ed7b64f5ecd430c356ed521f50634b35882732324ff768b8f15"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs",
        sha256: Some("c5e0eb801a997e3fc3ce8c431550d8543db920bdebbbc739bd1593aae6b89172"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar.rs",
        sha256: Some("db864109d7a4ad31d2a349939ccc8ae9055a5fdd7d45c636d589c8f843d08374"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("b5cbd7260fb6028970c24b164f4965d0ef52f8058283d7f46cf74e2f5eb5444e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/elision.rs",
        sha256: Some("23da37635f46de93c394e57534aebbb5b7b118695b4cb931217fc5b04c0feaed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs",
        sha256: Some("fed2f7f0abb1774ca8de57d62e99df7dbbc7d1999fdf0b19e22a72319fb4fa2f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_array.rs",
        sha256: Some("91fb0cd8ba60464e4073cd1092bbc139c1b1c9554e325816b266079bc5852507"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_schema.rs",
        sha256: Some("cb15757990e790f9f1f01cb0dcccc2b915136b506e68e2214daea8e25e09de19"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_semantics.rs",
        sha256: Some("659ac6a05f65ee637b0451cde7b0776f6c58d1d4964907016fe802f94f5e1183"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_representation_view.rs",
        sha256: Some("efce2b67fb4516164c5b0251edad7be593a87a240cff12dfcc395ef903dc9549"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        sha256: Some("625e7aa04e60e5e4bef1e367f98fd1935d1393a1dba35af6e4b03c6dc1dd9aad"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/static_path.rs",
        sha256: Some("826b8e0990114ddac9f89d8b8122bc172053a0dae927ac6e21af2e0be4705a18"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage_regions.rs",
        sha256: Some("d706a80832e400a9dcc220313a056c824f14c3b3660265477a9609e869b13d73"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        sha256: Some("60021278a668679ec8f9505d11367a23a0241c22563359acac068576ddc58eb4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("2fb50c7cc46fd208f232a92617aa594edff94e8091975bcf5fd31045e6788b6a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_equal.rs",
        sha256: Some("ab0f5db64ba4e833eb2ea866d30aae79c6b55379503b0922694f52cb10e0eb93"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs",
        sha256: Some("2073f8fc203353bb30d36987d14c64c5b5e353d6f24f35b0decde65df9228168"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_window.rs",
        sha256: Some("cc53642aa5daafe14e594f329fa0396db15cadde6984b35ae7d2a31a7b6a6de5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/zeroed_structural.rs",
        sha256: Some("ff6957626af109384aa2a54df8af085862a5907ae6fff068e754fa8f042c8f9d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles.rs",
        sha256: Some("7d3cddf3f11bd81f41d37cb8f0e00192a365ced5673b1d08666ced62a5d2d2d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles/reconstruction.rs",
        sha256: Some("51598fd52e83f8aaf7723414d7ca12be7062822050146d81f5cdfd9129e2c7e7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles/validation.rs",
        sha256: Some("6800abc1c3c113132df53a2060cd1e0356f6f27606096bd84353f1e8bc9bdb8a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/control_graph.rs",
        sha256: Some("29002a0efec83fec3ba142ee4eeffb75c3d59b07ee9acdd14a906ed44858d2c4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/lib.rs",
        sha256: Some("ac24996010594edda4a027d71d145cd5fe772fdcc47a8c4cd4f46168b43fa305"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization.rs",
        sha256: Some("7729460b689c062383d44b368b68ecc217112bed3d00a4acd5bef153155e4745"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/control_flow_cleanup.rs",
        sha256: Some("1d2533357c6878f762f8683ae02cf59aead52f7f44293635bc2edd046d3304f2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/copy_propagation.rs",
        sha256: Some("770af9d457c93da422a2fd20addc3a392e6f6ffc3f150f269c032af3d48884f3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/dead_scalar_elimination.rs",
        sha256: Some("995c9a94f2f0f34025830ab10def9ee75e9931cd3e983b6499b54d2eaff5f6c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/proof_context_transport.rs",
        sha256: Some("7aad790f56931c99b8d7eae21692626b7784702e7cbc9ef3aa036fb58c1a07b7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/global_value_numbering.rs",
        sha256: Some("e8b23437f9c02ea19c76b75a3bcc2884d055f888e296047ef726c620528f1791"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/proof_check_elision.rs",
        sha256: Some("4c571facdbb94b47ddab485aacde528d014c64486d06da66f50c6c4e61939320"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/sparse_conditional_constant_propagation.rs",
        sha256: Some("11b7c462eece45646e933a390804c8d35b9a95ec64ccee0626e47d565e1e8554"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/proof_recursion.rs",
        sha256: Some("d2a8b7d21c2a768be1ea7d16187891d2e0b41de1c833c0662538c73c4125544a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/terminal_trace_v1.rs",
        sha256: Some("8169d6b6968fd21c8217c9f060c21162798056e621529862b501f29f7dd6bf77"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/checker.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/operations.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/reconstruction.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/procedural.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/trusted_surface/sites.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/atomic_access.rs",
        sha256: Some("f5353e19f3f22136dc901e1f7b223580789522741d806dcdac54574f4c8aafd6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence.rs",
        sha256: Some("296d08a60171cd914ac6dc00887f7c14fbbd0185c2a7c074700b440be2ed8e45"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view.rs",
        sha256: Some("2c1193a9807feade1694423cdcb4c88ded6e9e608a8092e7d967806908955282"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar.rs",
        sha256: Some("b2f3b1e1ffb54587cd2be866badbf6a97d9d155213ae753dcd2be614db534ba3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural.rs",
        sha256: Some("072db0d1644fd8ac4d4b5834f4a76f31c19e6f8cb58899a4906dceda99442099"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation.rs",
        sha256: Some("ad52787d3350c7455c65c8df7fd2a3d1117f53995e36f8840ebb9bf5cc905472"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup.rs",
        sha256: Some("98a6125b2fd62db57e2d7b342bc7c74ee4a78daa0a36fe73b3e63120c56da6b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("aabf004002165bb45fb175d17203a160a6a573a3dc5904dcfe53fc02c8640e8c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views.rs",
        sha256: Some("eb078a875aaaa62df827d0c11b50525e8d5399879e98225c26e66db9c25b317e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views/mutable_records.rs",
        sha256: Some("94f1263752579a3125939ad43f8a06a1fc54ff73cb96f5a78edc1642f8b048d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs",
        sha256: Some("6661f5944e4b9bbd3d741b33e2dae17adc9813b89033c0d7c0702a50b5648fe5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/concatenation.rs",
        sha256: Some("8aef3782aa5f489e4819e56b137b3f81a8cdb9087e270925ec68822b1a9d6c55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/length.rs",
        sha256: Some("2741fb94304e121a6ce6e5a65ad3846808fd9e74463c69346c80e988328a54aa"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/predicate.rs",
        sha256: Some("64053ed15e5237fb533997a0d9f335cd84985bf329d46f6f0f88b7375906635c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/equal.rs",
        sha256: Some("e7bd46409e3dd1740e6922eae53150b3c4fee8623ebaa9d33934c25c390284a4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs",
        sha256: Some("070493e4c66a1778d557b142d1d5f2539f4070a616c6709bda68af280bc6c6cd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/subslice.rs",
        sha256: Some("c28161fe5a70ebcbf97783a0defc84d5193c85cea4fd00c123d8d887aca91ba6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/establishment.rs",
        sha256: Some("e385d88fb91625015a7245d3deba460129c037b2ac567b6c96abdf96a12d0722"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/length.rs",
        sha256: Some("cf253affa2ee159bad768d13d3d5237cb4252e9c99f586b1b5cf2ee372c02d8a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/read.rs",
        sha256: Some("97aae7013ea134dd6b8f8276ddd296ec11a14808a3a92e86aefceb3cec5a7de0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/subslice.rs",
        sha256: Some("83eb8ad90ec3fba3974a9776d00c2e63b91fddc987f3162f9ce49af514411465"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/write.rs",
        sha256: Some("41ae9832e96444f2e17015725c4996922589de3c03d46b083c1f17d17602cb8b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/mutable_establishment.rs",
        sha256: Some("10db12836c711dde3c6a11e8e433e5e5e5ac803c260982e31ef7e57a98dcc5e5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/write.rs",
        sha256: Some("d3a550b8af72af1299a9700ddd037854672f6a3e2d6e77aba08a379b51f13967"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/call_graph.rs",
        sha256: Some("6dffab6102fab4ad73fa09d9c20b45660f18dd33119824e8fe074d408b36eacb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/conformance_applications.rs",
        sha256: Some("7e3615f1fac758826ae18604a14f7480205849dab7d79d6971f1dccd6d2d9450"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/content.rs",
        sha256: Some("aae4e5878f61017d0e68ae9dcc9195e77a8cfc2ecdc97e8b9f2bc9aeed1b62cf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/contracts.rs",
        sha256: Some("5bd06b90a1647b4305fa79bd50b9209445d746d8241708d00cdfa0a5a689384b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow.rs",
        sha256: Some("a3514f098e546befa1d1b60b7bcaf1133d40bfe5969a8d00de8fad46755e8cce"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/block_checks.rs",
        sha256: Some("516975703a35e57f960b323528af1b5f2c2fc94fd9d4b6e3f1f03b7e91b34400"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/block_graph.rs",
        sha256: Some("ce68fb5c227f61f7868549bb5715ade7240b7f4e104c3bfdb08b6ef64c00feef"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/definitions.rs",
        sha256: Some("0a6c8b43bd4f1a1978e550d392dc3d98d625120ce8dab91e64411b46fcfcdee5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("fc0a71b09ad73075705f2b161b65efb90f179496f0f7c30bdfa40a3abdd74b62"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash.rs",
        sha256: Some("794769f87d430fd1375f7ad3b1e5818ec3f969c2a78d608362d7491ca0b13a65"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash/operation_contracts.rs",
        sha256: Some("16dc7edfed6d8b28bb3e77ce087c31c9e450d9af890e59469046d4cc02343c24"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash/outcome.rs",
        sha256: Some("e1873c66155a04aef909837db2a15407c033080ffa270a1951545411a65a9626"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch.rs",
        sha256: Some("b46221b69e52505cd853d7e651cb715db4e5ddde03a3562399c199ba219fca62"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch/consumption.rs",
        sha256: Some("aeba74c8fdb8633021c3112cf59bbef3c8f73ab52439694ad219c34fbccbab0a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch/direct_dispatches.rs",
        sha256: Some("cc992c6423d7feeb30e46fe053e89a2fc4e7b3c7c2f213e8c086602c7001bfa8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch/indirect_dispatches.rs",
        sha256: Some("bae7156bd1bb89af28e03d48ba0f5bba1b4d1b64ce8ed175dd0557de26b2f480"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch/rebound_descriptors.rs",
        sha256: Some("30eac1c2934756d030c52d764327b0736dbf3a7842fbecffe5d2a3993809566e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch/selections.rs",
        sha256: Some("b9bd4f59b9a8ada583ad0b493bd13a10ff8bbbd94734d682484f4ad879bd84c2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/error.rs",
        sha256: Some("249b73af195ddd00f28b975bcc4dff9b4c296fb7f8f73f2d2a05338c639ff9b7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/evidence.rs",
        sha256: Some("69dcc074549edfef9e1244867f7a4e89bcf13ab399fa6e336d58025f16bd807f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/evidence/contract_lanes.rs",
        sha256: Some("a86aefac9711e2747fc9ce7acb8426fac3862729312106496931e82b2f94ff23"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/evidence/guarded_call_outputs.rs",
        sha256: Some("443260bc9e0d602e8c7f9550c6ab73d540f67b757850623889b7444a6e98f082"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/evidence/proof_output_calls.rs",
        sha256: Some("6d832fac6bff0e0c2e84bcae86cc1219d6cd0e2ae1f6eb43920c4a15a29e2c09"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_meaning.rs",
        sha256: Some("5e38ce576a2a72bdb0c2d9f40956100b0af56696c1f94079fe64f687e4bfde3f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar_representation_view.rs",
        sha256: Some("ff967b1f984bc1e406f00377bd5e178d9d3575ad89f8061a68bc872e5afcf921"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_result_policy.rs",
        sha256: Some("f1bc89545e8c3d5695b1136a16562dc5c5c940550f114ef10f103210727e1a44"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation.rs",
        sha256: Some("23d37a0ff32bfc1a275794fb11dbd12e2a524d2087f85b8587b552876d44d5cd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/boundary_machines.rs",
        sha256: Some("d11a8995f316089f5a69a662447d9b3e0424f849cee84a1372ca749e02d39dbe"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/domains.rs",
        sha256: Some("4c0d5a1a9bc2de911b57d21546cf9c0103dca75fa9968c78087bcfd72b0a2547"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/machine_foundations.rs",
        sha256: Some("897a98b84b15eb05ce3c2332ec29ff77ba8d628919cfb959846da260fde7b08a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_candidates.rs",
        sha256: Some("d7d129d0cdfcdee6153a5ac6cfe127f227466715b39981cf3c937557cbc131b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_result.rs",
        sha256: Some("036cca9592bbfb88ab434c146aaa109cc619249b12086f6a1eaf77b14754ea83"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/services.rs",
        sha256: Some("05400709dbd93216f90fc66f210873e5124b70b6aed5c0c5868f678852bb16f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/service_binding_custody.rs",
        sha256: Some("629c20557cac25526ce394ca078cce31e3929d12b5a39b676dd01a029cdeffc7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/structural_types.rs",
        sha256: Some("2dc796c578a8bb96d822643308d63b56b79827cec720d7fd14cd6002e10d2eee"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs",
        sha256: Some("3ca1d5d266eba621daeacac3f199375a624f227f35246bd126631e7299c61d7a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/cleanup.rs",
        sha256: Some("f304d68a12bf1b800afc31240448e9d32f5e00f26da4952cc3ba2d43fe068e04"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_entry.rs",
        sha256: Some("0d625f4c19b2f7b1f9ed66b31422008d39be29084206641cfdc44b0afea9584b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_parameters.rs",
        sha256: Some("1190fb2af7b6d0559950005af424e28e4b08e024dd48cef27b6ac0c985881d53"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/operations.rs",
        sha256: Some("b52fe6a802bc8d5d730199ad369d61e8613e7240ce85988ba6681a6ba171b5ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/shared_loans.rs",
        sha256: Some("071278221d17de292a8933f642d941dadfc24531988deee6661bca89f738da98"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/terminators.rs",
        sha256: Some("63951347ee8e9dd2891e00fe30f7f15d5bfd9f388e0ec4f434e5e31fccb63308"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/traversal.rs",
        sha256: Some("6b78a2061b6be4f79a64683c1e111b97621351fa0dc35ed51109e5f8f626e5dd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine.rs",
        sha256: Some("7b2457cab3d8b7fa21a38fbd03ed222843132d9e9e4d161e577fa6071fc79f59"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/blocks.rs",
        sha256: Some("78124514fb2a960e9352aed0af1b74b78456daa39a07d34a432b633bbebb737f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/contract_clauses.rs",
        sha256: Some("a76b7cca44c3a227955ee73ab2c113c6c45c1db47a0b6cdbc8562f0c1788eae1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/custody_operations.rs",
        sha256: Some("3c707ed3719c034ae57cbb6b8c6714ed56ff39154f0f71334591c7a981704a78"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("dcd6c83baabb93f9781431cb1fa24eb2341e7406207a29cb0b2765b074aaedab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/structural_places.rs",
        sha256: Some("4cb5e6f1c03e58a323cdbe90e3c00adaf8504459f14faca3265438d573ffd266"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations.rs",
        sha256: Some("ed29cfb9903b5c2eed5d07fbf2cd6f79d7274c3d5190c02b7fc396b580c96445"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/arithmetic_operands.rs",
        sha256: Some("fd1bf3cd5b284a325f7583a1cf7542d3720b9e8ff8d29df7ca1d8b8c97ea16ce"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/call_operands.rs",
        sha256: Some("8d623a8125ba416bc73930af9c052d28d9ce4005792fe2be14b52ad978892add"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/scalar_operands.rs",
        sha256: Some("9f785dfe81c155b22bf8cdde7481b0b0bf769f81416ad75c1b5e0f566da3dfa2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/storage_operands.rs",
        sha256: Some("3f4b5812f6a595ea7ceceba5ddc816a21a8ffd644e4c78ecc80e61e68b050f3b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/partial_affine.rs",
        sha256: Some("2f70e987b1065c2d1b290eac3fa0cc305c1f6d69a92f95e6e01f63107b99c81b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/primitive_storage.rs",
        sha256: Some("ca12771dbec9686a4aed3696a80a8529c05114bc88a40ef601d21d77d4064ba5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/proof_recursion.rs",
        sha256: Some("904e3ab8ee2f6ae6e0a7c216eb21bbca54cd2e13bd9274f7f1fa25410f7c31ed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/propositions.rs",
        sha256: Some("a84b07c5222a669a390c03d11929a93393bb174a4a49633e1429e665890d0d81"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/qualification_establishments.rs",
        sha256: Some("62f7be66518e5632db34ce8d0e600fe2b0a6ef17c91217305aeaf7f91059c3c3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/reach_applications.rs",
        sha256: Some("e0acd8284ce8ee3658357c8769b95f0ebcc6e0f1409bdb7f1078b4f125a0c633"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural_array.rs",
        sha256: Some("50a841eaccf1db90a4334185766b8566ad194a663e7645f2c96c620e7acfc3c7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/record.rs",
        sha256: Some("ea2098fda1da99c269973fd9f1b3c05030e4c401b7882ec2d0567e112a6c5a99"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs",
        sha256: Some("c543d32c536bfa75de6c95cc9b688c02e6007e0d65e8543f11d112dcc78db169"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references/access_paths.rs",
        sha256: Some("c8a9b1f2559c0a64623b34caa2917451997218f17bc96f5ca6d88e2a79edef3e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/root_service_reach.rs",
        sha256: Some("a97e563038d9c62871e1639ad1b48d819a32b0fb61622dbe0da5a463795dc735"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/array.rs",
        sha256: Some("7ac4d76d930d21bcdae961ab76fe89e031cf61b2b92e9dd21de9f56552ffa777"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/block_invariants.rs",
        sha256: Some("1cefe5d978cea8bf40e59a7183e5a515d6d41be6cb9ea7e424c355a3aeb97c49"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/case.rs",
        sha256: Some("7a758099fa0109e5766cb144584689619281f8fb24df4a7c11c3def9c7d4644c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/qualifications.rs",
        sha256: Some("044eb80a052f0ae11c67c2f2b89c6410113f960dc3241bfc106c533f189cc1a5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields.rs",
        sha256: Some("5a72e2473ad94b3e9e94e592ab2bb30a70e168af50693e11cef56543c1548eb3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
        sha256: Some("cdb1f2014f6a0d1b0f3bd6da3fa39f54d7f5465c6b5254a53769b435c2e9d470"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_store.rs",
        sha256: Some("49c135c091ead638a20662d4d33f3880a11de0662bdbeff7aa26c264ca2f3083"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_view_field_store.rs",
        sha256: Some("b8dd486938c79ba1a3526c1e01a7db7513af046e8daf4313c632ca0dffaae33c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case.rs",
        sha256: Some("4ae416886a7dfbdb93f011847c74c8c6a7f0a41adbca3e189ef443e3b4b0bbfc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case_membership.rs",
        sha256: Some("0a863bf426392de3b34f34c4a706bd1d66032618b17f69eb5d8d4a0401de0fcc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/leaf_copy.rs",
        sha256: Some("0e0f4cc841fe45c4b6a96b3e75eded04e1fc7307d7a74e9e78b10f3e2b74be10"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations.rs",
        sha256: Some("67241094da58f928d7e6bd6dab4fdeadfca14f8c517109f4ceec01d0bc348bc3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/boundary_requirements.rs",
        sha256: Some("e9ea2f03c1000d2344f586601a36bdd9897c537249202614e1d2d8bb4d584791"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers.rs",
        sha256: Some("0f663a2bf648414b9b0304ea334449a0b20104edb9c6f26e57e5722fcb43909a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers/argument_claims.rs",
        sha256: Some("ad1fcfee9526931abe55dcda8124c1beae30f87920df6cb7b72322d3809cef32"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers/transfer_roster.rs",
        sha256: Some("583decb17af4bb5abfdaa8a7b57a8bc4f93e13fd25220750a4411e901bb72358"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/contract_places.rs",
        sha256: Some("6ed377b5e376d491194319d1043a93a7a44faf85d78eb0192c2657d4c1276693"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/crash_continuations.rs",
        sha256: Some("e2e5757f1afdbf7a1e73547a5e5d81b8011757713d677c05d4c3ccfe30631ebf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/payloadless_calls.rs",
        sha256: Some("a90ba7c809c19ccb04ed5152726b97c36a898e0a8e926e613769a6f5598cb2e8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/primitive_calls.rs",
        sha256: Some("328e2ce5d5d297b52d152443a471b25732b6073a56a174e5e5da97afc895dc4a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments.rs",
        sha256: Some("8c7155837d3eca316548459afd280106ec560dec5764391dee22cad8ab76b045"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_checks.rs",
        sha256: Some("1a0bab5f176c3d83f100113b9d19484547e8d51047e2217b76a0f7cb0797df95"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_pairs.rs",
        sha256: Some("fc581bb0fda177e6739aa00c6ccbbdc4641deaaadd507887866dc34cba03f5f2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs",
        sha256: Some("10a1bb1c3169248082eaa6e6de980e00234cd7afeae6f5fbc54ce31337d62121"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation.rs",
        sha256: Some("21ee72537efffe3b1b13f4c0fbff355df9cbdb42b55ac090e1696160299a86c3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/boundary_calls.rs",
        sha256: Some("037daca460c5f6f9e7c08c5e4d79fbe026641330a4b5251b403be7b40a2790eb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/local_establishments.rs",
        sha256: Some("006acf4852eee562cdb9b2f5222fdea0648dee3bf529a4c426a05401b002b563"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/structural_calls.rs",
        sha256: Some("b591de6db1e6df1c74b57a588cec471598b151921db47f4304643fa81994228a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/unit_calls.rs",
        sha256: Some("0fa67b380d7c7ba3a3b4eeb694fd6e7da6439aa44440b2c1250042e4cffb9b47"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/qualification_rosters.rs",
        sha256: Some("e478350a585aae369594ef12151a92415967b6182ab6ee4ffb8f9ab89388748e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/result_contracts.rs",
        sha256: Some("afe2b8fb8b1904b46406c6ef9d841d6d4cc006b717b97d809999ce5689014265"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/runtime_indexes.rs",
        sha256: Some("f672152ea09e90d46844bec36fbfce552340028854e1a13805008af7dfd75b50"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/field_stores.rs",
        sha256: Some("0508960e0efcd538a5b8b4f5640d2065b89d650d0098cb78bcff7ed710b6e103"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
        sha256: Some("5ed4a47d0b1c16f4fa5079937574ebe93774d1ff82044db517a72c40f7f9beee"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/suspension_call_plan.rs",
        sha256: Some("6628a7acdbaa3cc8b9a7e3db5ee70a6cd2630b8731fbb39555e0c32d1f87d4ff"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/trapping_integer.rs",
        sha256: Some("311676b5c75e9967d374e896947637e4343788c78fcba29132f90eda3d3fb99d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification.rs",
        sha256: Some("737fb730978dc755f8e645726032c5f98a4f0c072485fb40161ebce54a1a99c5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs",
        sha256: Some("cb63137cb4b2d10bc68a8e329fad0aa03121cb8e10e5a779a628123f6b74a859"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
        sha256: Some("59d23fa62f81f716c69f634422aec0e4e1d473424366761fc3242895476aa49f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/field_snapshots.rs",
        sha256: Some("6546990a30530101f0a40bf251eef0b69d07cd8260a71d384d52ae40e4d5e8b7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/float_meaning_projection.rs",
        sha256: Some("177a5cd07231b1831247dd82a48956ebae2db6be46918d52197431ca6b1bfa87"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/proof_bundle.rs",
        sha256: Some("3256b1bcd5d955e31099db6d4873ec4755719706d05dbbe174d62b1525524ae0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction.rs",
        sha256: Some("656729e1f9b1f9f43e7e42af84fac6ab60b9909b5a660b285ee73f6115bc255a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("77c1917d7b2acc3052cf7a5b4d84b3d144a480d898ab6a9951191aecb13851f7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_obligations.rs",
        sha256: Some("d17d13976e3c1121a10ba5fdaaa7f52ff79a6cf179eba647fd9c317f5297e4fb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("94e43a2db51c67536970715fafccf682591877eed2fc33cc7bde5413463bca60"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_context.rs",
        sha256: Some("7bb1c7b7be40aac7db288d5e00cf7a4a2dcd78fd448d44e36471ed16e08c818e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("10102a617fd4cb40c1db9cee0bda3c19eadd54583847bc6a8ea9ff9a2f5b20ae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/case_hops.rs",
        sha256: Some("26fe4502adfe6d1ad948bd8ce87da50a28afdf522c8287268ee329a8e4c74655"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("fb57980206dde33ab4ba5665388814d52f00bd327e0cab11c6ad424f7237f288"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("fddec551cd7f88c70b0e860d0cf3c07bdbe57617055da39b09d65ad8fa1cce58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("64b68054babd5538151137b91d73e789b4e44572125e31ae9669fb3bc5f51624"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        sha256: Some("e93ca29ec037eeb6ec3b25fe89b114a1b8c80d8e089eb241929b4fca98059d1a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/observation_substitution.rs",
        sha256: Some("dc2ab3215434382a3a9995c5b222b03d29b707fecaa6fa6c08ba4ed68d193863"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/owned_contents.rs",
        sha256: Some("2686b458a3a2cc2152e2c9b101c5dc7aad09259fcc25acb047b18f8970ebeade"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("de400b2951be9a2b0c370b746a81c313057d4ac6e3569a225c8cf35367d1f984"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/primitive_elements.rs",
        sha256: Some("46fbd22d89f7517fa1240370105a1dad6fcf89054245c4de734b02b72163b833"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/float_fields.rs",
        sha256: Some("aadc59b3280a8f603c1ec0f638596295e6b4d0c719f3455d3e731b7ef3c28417"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/runtime_index.rs",
        sha256: Some("3bcd0949119580fb6ac6261b843629c844b8a981a2e70525ec06e81862e815e9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/scalar_case.rs",
        sha256: Some("0e9bcf8fda8841fd565c32296de184762398029f4cfded85f8697e577b1974ec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/structural_case.rs",
        sha256: Some("462f73e542d434a8be803763cdbf15d51fa30bb602b8408a93001d746041451f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts.rs",
        sha256: Some("87b968ec065e130d6d0ea2d1c41faf017930dc36e07b219ad5c5f4bdc29118f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/conditions.rs",
        sha256: Some("9054644bd92d03c56984594acc48f16f5334959010a1ca8bc3c980a44c30dc98"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/discrete.rs",
        sha256: Some("ae495c6aac20ddae2ab1644dd0a538ee9308f330a52345e15a3aa5459aa20bba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/transport.rs",
        sha256: Some("0c780e7577ec24fe6d656adc678df3c7baf152dc25abb86dbf29ea3c085effa0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("49f56b3af4c2dda086e9a8ae2f3f4809d316d3d8674afa97bc77feb50a232f09"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/primitive_snapshots/observations.rs",
        sha256: Some("cac649ed40f31d078f512f08a0ed8dc06387aaccdf1b50c8e41dc4a80b717614"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("000910771a3b2d03195db3036297f116a040ee275da086e94f8d7cc30480ae18"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("20896bc58db377aae397d53b9d52adb6e1a8168bee08d1be5f5653515569ea79"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/substitution.rs",
        sha256: Some("a507c53f60646ee3d5d06ba47039665680d310457438631e8ab096e1fd1cc75e"),
        inventory_machinery: false,
    },
];
