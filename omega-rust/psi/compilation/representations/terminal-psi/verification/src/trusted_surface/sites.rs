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
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations/tests.rs",
        parent: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
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
        sha256: Some("f17628aa82c98019b4a9955a7f76a8cce61248fff3ef036d386465e60d0e3d9e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_ordering.rs",
        sha256: Some("fa3b1728211315fdbc37ae4ab5e5793ba03e2e3eb244552871a74b5afeb1247a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_lifetimes.rs",
        sha256: Some("62135a7e31b286eab1ab059e054a2de2874c9d29a5857555f85af298b5added4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_results.rs",
        sha256: Some("bb0a5afddbde5a34f05e9446f7f4bbf028db6847cf9de1d335e01363f418c5fb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans.rs",
        sha256: Some("b6fc6da561c1245d0f8802ea94f521b2d0427b42d6ae8b46443667ceaeeffa3d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/aliases.rs",
        sha256: Some("cf30d8df2ee29b5557545d67aa76530617b4290156c7fcc2a06b409941f97722"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/call_accesses.rs",
        sha256: Some("0c5c9204e906437d372ad60779c9bbe10bbb6e9d7971ced72726c11e3567d24a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/accesses.rs",
        sha256: Some("1fd0ef148d5c10fef4a763ee20f1261cfb7eb0a1976fd32ef78e0e242b8b3923"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/flow.rs",
        sha256: Some("ca379b801eaf3d2996a89add2f73d8c5a5313ccfb71dc58d59b93bc141b5bf4b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/descriptor_loans/scalar_ordering.rs",
        sha256: Some("3489e28febd70c402a8a057b1d8a355185f078212ce6b99740ae7a3c89e62996"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/access_regions.rs",
        sha256: Some("f06d9db774fb066ae2fffaace15861fa1df38d0c734bfa3d84ad1341b09122f1"),
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
        sha256: Some("37ea3e01b0326c4bbc56e0fb6c922f33a6d48495216d7c28ca068c2839dcc101"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs",
        sha256: Some("0af0cb0f621f482572725cce1bc8cbfeb28863536245e49a9f50a787fe1e25de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs",
        sha256: Some("b3f6a17af4c51594089f8e69bcc5f4966424988eade91c524b0ccb5f3aed6285"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        sha256: Some("f646c0a7c39ef5cbf1c037bfbb2d43f596b77e17336452cd66d5073e34eb0e4a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/bounded_byte_owner.rs",
        sha256: Some("fcfa45b82b6bc9dd25897863e799c8b37198b87fe91b404c40076d3043b794bb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/store.rs",
        sha256: Some("41ed5e3bef911964c00b1b2b27a52ba63a1a359a3b0a2e7ba05600ba418a2e55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/structural_places.rs",
        sha256: Some("70755eae021773d15b051e13332253ff46ccdda55c0db48383a8e2ddd66ec9b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/content.rs",
        sha256: Some("1ff23d325829ae6246308a404c051308813352caa6e902b2f4089ecffe52928b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs",
        sha256: Some("a36e62df711dddf9142ffe69aaf9cba84180d1edbdae77fcbff5134af456700b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/admission.rs",
        sha256: Some("c3978fcdcd9cdf232f31d2f092f52361c98b967a44bb287d8091d1e6f1d99362"),
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
        sha256: Some("00093c8eff9a404719b978bbfd77587552d492ae2d2834993f75349c71c6b68f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("898176b83f921ba6a2a7faeadf5bdd7232acb9866338ea98b555a644b92a7fe7"),
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
        sha256: Some("661acfa1de43ed33a88afb6a7ac7103dac14af94a641f14d4915f8f16f552fd7"),
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
        sha256: Some("7d9d27008dab51cdacb8ba1d6323996380225057c8ab2f2a17de23393b589770"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs",
        sha256: Some("ceba4d99eedd93996fd37b9b99065b5d360c7f8abeb3304cf593dda20d41e571"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("9dc5596b41535456344167ab64109dfccedf7760552d5b2640cc13f15914c869"),
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
        sha256: Some("904bdf506b4f65ed93034ed2dcd0cec28f94ccf66ca34b5dc006c0bcf6386833"),
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
        sha256: Some("2907397396f97feec3847127a7a05a26aacb5f72089abe32bf991aa4b6bd2031"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof.rs",
        sha256: Some("727cf1a39f6539ad54e18e56cd18545560145ad3f82481b86036a4fd884549a6"),
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
        sha256: Some("92f608963a6180cf09238dd48c83b0ce1c3b402d1a9ba69cfd40f28af1a2d77b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/collection_backing.rs",
        sha256: Some("be99bf47d1b20400640a7a7dabf710fad6c09be936459b71e0b7fc3f5d2f1661"),
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
        sha256: Some("b0aa36196e4854daa6543b1e851dd54095e02f20ca00968c701fbe62f33f4b4b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("6939acbeaaf1810da138bdb605386dc31d1ce929a30e5d5702de0fc22515af41"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/lib.rs",
        sha256: Some("660c4c517a056e37b24fa9940991d84bac1752628d4ac4c8f474042b65f044d8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/service_binding.rs",
        sha256: Some("775ba2c7d5f69e8a06f46ed8fce9a18e9aa56f7a65f3c7cc957f2cffd04d4505"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/placed_view_referent.rs",
        sha256: Some("a9ab51bcda82b55289782c8b13ebe2567ed5294ae88b3ee9d2dd429686dd1393"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs",
        sha256: Some("2d8bda3b456747635289dbf8fc5fcdfad0082442f6aed7b24effb6bb075a00a6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar.rs",
        sha256: Some("835a578dbc77976687a8384b96a8f285f4516efa7d7b0afe45f4c03d603b79b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("f6efffc68ef21b0fc67a1aca7f3e8359e23ab1e7765b2bf00acc91b8a2672b14"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/elision.rs",
        sha256: Some("23da37635f46de93c394e57534aebbb5b7b118695b4cb931217fc5b04c0feaed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs",
        sha256: Some("d509d94bca2812a60c58f8a4b70d6da7f01e325596a2fcd929fd79e65bbe99da"),
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
        sha256: Some("5fbb008e228fbbcccfcbf1c66d87f50450fbc1cf5ac631d4a8f72d45eaaa1883"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        sha256: Some("7b1e52f09df84d430bc7c8c4c1561f7113c9c187604b7a476d3485e6707f837f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/static_path.rs",
        sha256: Some("5c1a0765e9077890ecca95a5bc01fa26eae4a433d558dd300851932a0f45c7bf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage_regions.rs",
        sha256: Some("170d8b91b0558087324930b21ccc765cb3cf20501110a12ef48fc7b461f8e783"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        sha256: Some("d2da834be4c0453bb3f36f3acb03e5c55ad0858b7cdf7d74982cf08f2653ff60"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("034f50051513a8fb48cb751b5b6874d8ddda30d9ff771c854ad3ae399cd33b9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs",
        sha256: Some("898b407d8b8a09492089f220eda3e6c3fb624b84399ddfc04f1a5d3d778dd2d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_window.rs",
        sha256: Some("2dd16585366ddf6e69868ad4f65869b477353cded4abe6a2787f7017a431d11b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/zeroed_structural.rs",
        sha256: Some("48d23d78773be104df8ac234296f52e3193fde655012ca96b7496ce0f9886a33"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles.rs",
        sha256: Some("9cd7a695bef98ec4b33d5e70e01238ba6d02c928f0b13f0153f4a1027e2cbc56"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles/reconstruction.rs",
        sha256: Some("95ac1fcb07416039030baf1e8f48a672bfcf3e000e7c756bd9b6e087f598e11f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_cycles/validation.rs",
        sha256: Some("6800abc1c3c113132df53a2060cd1e0356f6f27606096bd84353f1e8bc9bdb8a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_graph.rs",
        sha256: Some("49c60170b5977126d23dbaf96934d41701ecadef97a0018f6d68f66641c59b02"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/lib.rs",
        sha256: Some("cbdb0199a43fce7d88b3bb25cfafccf8334e493eeae1064902141a9bf197707e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization.rs",
        sha256: Some("85d0db5d972437cf52ebe10795568fd9426108873255336e25bbf4f3cd032320"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/control_flow_cleanup.rs",
        sha256: Some("543fffc93a54f50d2715769a1bc9ff8e3a38173ad5c249f5425095d6c928b0d2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/copy_propagation.rs",
        sha256: Some("dc82d6b4e85a697af18d079600318e36d0116e3e60fccb887353b4d6258d1285"),
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
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/quotient_correspondence.rs",
        sha256: Some("8efde6290b40a73898fd883a03a8b54532439b956628b62ce5f323c52cdc0acf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/terminal_trace_v1.rs",
        sha256: Some("4458b921591599ef5df43a73f72a0ee62fb5c1117604f170d5e893139a29c8a5"),
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
        sha256: Some("3364af90698b8b1b76fec93593995a20b91888c71806e766d660569aae423b5e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence.rs",
        sha256: Some("918196e5cc06d782798f30a628b84cf5c6b39f1c97b51aa5cf6daf18f8efd9d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view.rs",
        sha256: Some("e8e3d7fcd6d5b9345d8e7d3c5bb9a7e224f21b36efe449f446d9447df761062f"),
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
        sha256: Some("5fa94dbb6ba49d90c09fab304b3c39f287fac051b3acc0b9ebbb3c44f2d8ab1b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup.rs",
        sha256: Some("78cf2f9d55c277c52af68599271fbdde2c4b487433468d7275d6ec6cc8a9861b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("73fef6c9b939972f8fb11ae1453c79c7eac38f0ffd874f0a231aa6c9645b3c5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views.rs",
        sha256: Some("6b07ffdf4a6bcdb5b05692da9a160b82b181a98fc9e9223c9243c1b16cc00273"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views/mutable_records.rs",
        sha256: Some("97592aff54f42cf36f5b1e41dc06409c1eb2a8d258996246fb61f1a668136335"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs",
        sha256: Some("c09f3a88aa0ee8981447e3ea9b9d041e35521beb2ba619494b73807b58c20edf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/concatenation.rs",
        sha256: Some("8aef3782aa5f489e4819e56b137b3f81a8cdb9087e270925ec68822b1a9d6c55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/length.rs",
        sha256: Some("6a7b008183a8f8d4ce32f3ca985bdecaab4bfd42aa30b3192788965b4097e039"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/predicate.rs",
        sha256: Some("64053ed15e5237fb533997a0d9f335cd84985bf329d46f6f0f88b7375906635c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs",
        sha256: Some("5299d96877b115a718eb460e1ee7b3c883fde7be67d82b66de1cbc28bcd6b673"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/subslice.rs",
        sha256: Some("c28161fe5a70ebcbf97783a0defc84d5193c85cea4fd00c123d8d887aca91ba6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/establishment.rs",
        sha256: Some("aed072d760ac74664329d418688b6ffad46d1e214cbc4e809dc0de91881e1a1c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/length.rs",
        sha256: Some("2986fe09dd2bc755cfb447a8e1afb58164d16db9ffc07d52da79154d5cea8dbb"),
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
        sha256: Some("cf7dc79ec65894a49f8e3546cf96da09a5252aa174e9b193492e4908cf748959"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/mutable_establishment.rs",
        sha256: Some("129cbb7ef99ae4d1b189008e1d959b7ce34a5ab08b2bdc2b37c48ba9c33ffd77"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/write.rs",
        sha256: Some("699302865970a293c422d0b75adfe3b11241852e8be518d06832c1de35ab551c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/call_graph.rs",
        sha256: Some("eedcb4e24799672963155fce929c140829d92603e972bcc9f00bcd130b6d0194"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/conformance_applications.rs",
        sha256: Some("7e3615f1fac758826ae18604a14f7480205849dab7d79d6971f1dccd6d2d9450"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/content.rs",
        sha256: Some("be631ed22276fb4a0935d1ada2489800a7bc5dd315041ac2cdd72dad9c4e7493"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/contracts.rs",
        sha256: Some("929ec1658b480b421c86ffa762938f93452088affa6b5b1132143f06b155b261"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow.rs",
        sha256: Some("0c03a27ac81dadff8fd4716c3551a7160bd8b0d213e62d02b83b17876dd5731f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/block_checks.rs",
        sha256: Some("834fbec70a30341a90458b0ec83a6776116a9a551d134052ba7eae1a522f4749"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/block_graph.rs",
        sha256: Some("ecefe5b9c720e99b8d7859c708f08a0168176a3cfa56145ca07c47454225b98e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/definitions.rs",
        sha256: Some("0a6c8b43bd4f1a1978e550d392dc3d98d625120ce8dab91e64411b46fcfcdee5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("6b9a717309a11463f2428e91214f6ae883faffbd847378195cfae6b611f3a8e4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash.rs",
        sha256: Some("c698a66aacbb895fc2a69a6ab792ee0c33642211bd8fda5250408bfbadf2c614"),
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
        sha256: Some("9782fc7d38099441dc94a2fc8415ba665b9f57c2fe5ccd9addca6f702ba48ee3"),
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
        sha256: Some("930af6e9d7aa92808408a4ac12a880523d31a86fe730613e6f30608b5a4a5de3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/evidence/proof_output_calls.rs",
        sha256: Some("6d832fac6bff0e0c2e84bcae86cc1219d6cd0e2ae1f6eb43920c4a15a29e2c09"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_meaning.rs",
        sha256: Some("32b38cb250f5400852d001730927df5825b8036cc02b331b7879c43f412b1141"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar_representation_view.rs",
        sha256: Some("0383c3aebd9c2b6cca3a3e84399dbb9ea47a6d03d7393612eecc458de2293bb0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_result_policy.rs",
        sha256: Some("1b8eab50feb02978de9f59135d19acb0afc573507e359bf407824b214f34f9d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation.rs",
        sha256: Some("00bf2bf5cd15ed438f61601b54e3f049455f938a1f4437c9f8d1d38db6b8c02f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/boundary_machines.rs",
        sha256: Some("6d1f237004e68d70009c26b38fbbcbad86ad0ffb81356ca7e0ce257fe214899d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/domains.rs",
        sha256: Some("4c0d5a1a9bc2de911b57d21546cf9c0103dca75fa9968c78087bcfd72b0a2547"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/machine_foundations.rs",
        sha256: Some("8e8f50dab77e511b659d5a5f0ef6b5f0e69284fea8aff4074e6fc62fa64f8f73"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_candidates.rs",
        sha256: Some("d7d129d0cdfcdee6153a5ac6cfe127f227466715b39981cf3c937557cbc131b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_result.rs",
        sha256: Some("03491d791d53796a5780df0169e3df664b40d5ae693fc27506595947e54bdff0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/services.rs",
        sha256: Some("05400709dbd93216f90fc66f210873e5124b70b6aed5c0c5868f678852bb16f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/service_binding_custody.rs",
        sha256: Some("0d8a234a4c73ecafd178ace3904c73cecf8aac3b51d4775085de0e689fe277b1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/structural_types.rs",
        sha256: Some("506477c88841d1bcc43e8c1cb08b2c7e40fa0f87425a76ef3c0f7ce94af39ed5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs",
        sha256: Some("80a6ace8e8d004a928af78153ace2aeccd3f23d17582edfab437972a56433b6f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_entry.rs",
        sha256: Some("0d625f4c19b2f7b1f9ed66b31422008d39be29084206641cfdc44b0afea9584b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_parameters.rs",
        sha256: Some("78fd5141b51d0a970b1ef84bc807fd838e668588b3da9cb512e245c5ce3a550b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/operations.rs",
        sha256: Some("4e1af8d5057c9b04af9bfc348bb9b570fc7057137d49476e5a5c98fc86eeaa95"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/shared_loans.rs",
        sha256: Some("0cfdc4307828f54490ccad88aaa22b428bde0d380e916ab3e2f1545bbb1cb9d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/terminators.rs",
        sha256: Some("3f4859a13e6b5cd37fed5f92142807e6cbbe2b01f07d533d96c305c0a09d3e16"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/traversal.rs",
        sha256: Some("30f2b690c99efe51abeb58d8e733fa9ffbc33ab3b89b07fcbd01d73fa6c82db0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine.rs",
        sha256: Some("33eeae3d62c4b1ab9f37151a95671aa67f475545023d969b1157a5958167981c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/blocks.rs",
        sha256: Some("78124514fb2a960e9352aed0af1b74b78456daa39a07d34a432b633bbebb737f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/contract_clauses.rs",
        sha256: Some("a43f7f4fe24ee7fd151818f1df80909fb78d5571ac776bda612306a67614ae4c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/custody_operations.rs",
        sha256: Some("ad9b52fbac08319c44bdc9319abb894378a8fc98c22d7574c48e3c0679e50160"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("1463b1cd06436bb8cc545c7eca3734e2fbe8218d61bb56269d39ee2bdd70ea03"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/structural_places.rs",
        sha256: Some("4cb5e6f1c03e58a323cdbe90e3c00adaf8504459f14faca3265438d573ffd266"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations.rs",
        sha256: Some("22b86d8ddee2b6f4e98778a7cb92cc2a0d092b282439b10a22be0caea1d738d8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/arithmetic_operands.rs",
        sha256: Some("01be51fa3477c3ebebefe60d654a975dfd33efc44797e670a0ad62a2a2b6892d"),
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
        sha256: Some("6129f618557f095cedebbc27355e21c39c3e6ccc7602038e1678e251bc4f7fc8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/partial_affine.rs",
        sha256: Some("7df81dbee700a249a91351b395d5a8e07bd72ce38da8c0a45fcbeab93363f92c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/primitive_storage.rs",
        sha256: Some("b7edee6152a33a5b625f8f65d0f32d714b0451197bf28a3fe51a84f0554370c4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/proof_recursion.rs",
        sha256: Some("904e3ab8ee2f6ae6e0a7c216eb21bbca54cd2e13bd9274f7f1fa25410f7c31ed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/propositions.rs",
        sha256: Some("5fa23fdcf4be1625d21f7caa53304d80600ceae00df2fc442e62909fc6a36017"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/qualification_establishments.rs",
        sha256: Some("62f7be66518e5632db34ce8d0e600fe2b0a6ef17c91217305aeaf7f91059c3c3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/quotient_correspondence.rs",
        sha256: Some("d38746cbfc4ffdc4f9bb158f4025b97f32265ead709988a28bf94f523fa3e5c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/reach_applications.rs",
        sha256: Some("e0acd8284ce8ee3658357c8769b95f0ebcc6e0f1409bdb7f1078b4f125a0c633"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/record.rs",
        sha256: Some("fd351767c7c7ff0670cb62b700df148fe58225ff9c46182a7039ae25c9c8991f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs",
        sha256: Some("5bd5dea0bf107c7f961431e0d1690dbf7baf4d299b7575c59b745379298a9f62"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/root_service_reach.rs",
        sha256: Some("6d02331dcb042968cec2b4788b16fc9e8984b3c6cd85cd45146a55c48e782a35"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/array.rs",
        sha256: Some("e566a4ff5d0a2c63ea3439d8e11f81eb74df959b52ce52aa79dec616274efe74"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/block_invariants.rs",
        sha256: Some("c82a1da5555605ccf1d889f10d4e2e6fa1970360697a1dd908647d655ab68a38"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/case.rs",
        sha256: Some("69b4447593d6a1d4bf4891ba651770851b757507b079ed93827c820c5ce600d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/qualifications.rs",
        sha256: Some("044eb80a052f0ae11c67c2f2b89c6410113f960dc3241bfc106c533f189cc1a5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields.rs",
        sha256: Some("6e6ecc46de9cfb6d0e9fae8f1c04758718c0e6bc53cc1fb34e1de6518c773fbf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
        sha256: Some("6f48e234b8ca8f190cc42f9c09975dda84090dfb46e6cf9c8f063898eed2a322"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_store.rs",
        sha256: Some("49c135c091ead638a20662d4d33f3880a11de0662bdbeff7aa26c264ca2f3083"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_view_field_store.rs",
        sha256: Some("ac66f13dadbf28ee97f82e69cbb6dfec662a3b6dd135f13831d9c61ed51fa452"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case.rs",
        sha256: Some("db3c2a8a92a944395e965aadcf0fb74769a6ac846f0b0ce9e471b8e34b00c872"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case_membership.rs",
        sha256: Some("0a863bf426392de3b34f34c4a706bd1d66032618b17f69eb5d8d4a0401de0fcc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/leaf_copy.rs",
        sha256: Some("1baf5a190b4b73d7d4f95daa38f853069fabe123a54430709f30094afda1e504"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations.rs",
        sha256: Some("d780ad3ddfe184dd6c5ddaeabb53e120ecbad03d3c29e98f76d67e25db3b7980"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/boundary_requirements.rs",
        sha256: Some("271842c2de2f78024f7b143b8d4527aeab937c2d60bc78f3810d75cd720b877b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers.rs",
        sha256: Some("9b3dcf33b3a17a87511e50415af3fb26688a2a4e95cb7c946fd0ae527dc5084e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers/argument_claims.rs",
        sha256: Some("814d00a452f45088b4f265d66838d1e5063d0cc49abe223584a84e8bab9fac3e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers/transfer_roster.rs",
        sha256: Some("347ca6627f90acedb65cab1ead6620f9f512a0f3427c85a625d3903cf0e16518"),
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
        sha256: Some("7b566b8971899165278ca5a7ee0ee53828ecb94715435a4300bda3c80efa9b3c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_checks.rs",
        sha256: Some("f5b80a9ddf0cb43dcf600561e7121eafe3a17c6406d65f3bfb6e4784c24a0dde"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_pairs.rs",
        sha256: Some("71df2105e806dc911e22ee8f4a5e8f6717ec9f8f4f819a80c40d024586d27709"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs",
        sha256: Some("4690088322b53a69f1b2e5c69a6c487ac2ca0bf6c7948bd91ae375467df02993"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation.rs",
        sha256: Some("36461068d5e7faea25e7af528169bbe31f35c9e1b1e801be18229288077b1e54"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/boundary_calls.rs",
        sha256: Some("5e7e71c40533c7b8590dc060016fa314bb13d7199c49ff658c7e1e0a270f74f5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/local_establishments.rs",
        sha256: Some("ca81495abb24cc9841041bbca02430bcdde353a8ccca172834a31d0ec884c9b6"),
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
        sha256: Some("2535bf0d7c4557bacb50d097c6a34270ad8ef0d59f28c0e9961104f3b401f5c0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/runtime_indexes.rs",
        sha256: Some("63ae4559c2e2b710af55b5ed2f3c7cf70321eb89e42921e512f3ddf714bb30e7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/field_stores.rs",
        sha256: Some("687fc1a392f8fc9f7cac35e57ee74de41901746ba179ee085bc19e70c330ab80"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
        sha256: Some("232037121d91397b8b0e0a72d3cf7deed393fb04acf9e7040606a369f9ed8736"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/suspension_call_plan.rs",
        sha256: Some("f1391d944eed60f6301f3bdcd9f8f25a8b9201d2f02b8388e337b7c61b6f05e7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/trapping_integer.rs",
        sha256: Some("311676b5c75e9967d374e896947637e4343788c78fcba29132f90eda3d3fb99d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification.rs",
        sha256: Some("e88f0755583d8361fd9b311d368ce3c8e859391bf5b347c4f83c749fea9cb0e3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs",
        sha256: Some("302a5364cec8814fae85a107f85e715277139e110606b4ccb782147dc51cb1ce"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
        sha256: Some("8ea7dc26f90cc23daf0afd5f8ef12455c4cead1f669b92506a9148313fda2ccb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/field_snapshots.rs",
        sha256: Some("0575d02db0dee482ece1bdd8065df85548dda35b97cc22df3b680020e34ba112"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/float_meaning_projection.rs",
        sha256: Some("11979334f6365cef694ad3ed1f129d8a45c11e182b67196d5c780469dda48de6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/proof_bundle.rs",
        sha256: Some("3256b1bcd5d955e31099db6d4873ec4755719706d05dbbe174d62b1525524ae0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction.rs",
        sha256: Some("cbf9b38c42b5551f4c239887a2e1d6dd6ee30960861e9a990fe5cf9e95266cf1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("1c31a1b96b62e08c08d2aa61be98a199b8338150abd1d7a44850a2bf44feae6a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_obligations.rs",
        sha256: Some("b7303b124b5e893a89706583466d4ed88bc8c0ab5a3143c8d2dc0fb99ecfe64e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("e82b7dbd0728d4f0585392af5c34e4846fafbed1a03d9107c4c2cf04e4f434c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_context.rs",
        sha256: Some("8ed002136d50ba3ec2cd8283fffeb5529540ee11b39f9a8bd94b15fcf4c7fb83"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("29ee558d677cda35b24be01c2ee009e946d0876153c8711f537cfd519bb72a31"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/case_hops.rs",
        sha256: Some("60e0e67945a16a5ddf52367bb84d7c6bb73fd968aa80d8c5d020cc7d0f487859"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("ddf9d16ff60bddb6c6deeac0f06ab5cec1d328b4cd9dff9c09ea21322c36f049"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("5cbdf91d88cbaa8cb246d52f6319a406ad899e85cc4f6cfc16bc7a7abd5f08b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("33495781eb11053b453357d8f6451f43397e5a1465512cfdcd1e79782119f615"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        sha256: Some("95a78ed63625e21763e2ff8c166146f7bbecaf9acb1dc2618c5056d5ca36f13b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("f0acc1e488af5e0f02cb72b5199003bce5bfa55674f328dc02104946a20dc97c"),
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
        sha256: Some("45d1ca39b33bb223b1d2578cb2db540bc81df988f37e2b167b900b5548702db9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/structural_case.rs",
        sha256: Some("51eed63cfc1fde832d9619c5050bb4578c6db0a15100f85f02357e2e5364fa5f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts.rs",
        sha256: Some("87b968ec065e130d6d0ea2d1c41faf017930dc36e07b219ad5c5f4bdc29118f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/conditions.rs",
        sha256: Some("2680d7cf4e41ade56b634ac74ccf2135e82ea51ef4f4574e3d517cf776527222"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/discrete.rs",
        sha256: Some("ae495c6aac20ddae2ab1644dd0a538ee9308f330a52345e15a3aa5459aa20bba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/transport.rs",
        sha256: Some("fd6dfef083338996172d2c1b6ea82e2f74019e5b6ad32eca793ce9312faed26b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("b8393d2b0ce58030bff70455d3ed61dcf795993cfc1d9fe1c175da341e5c474d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("d945a4d1358c285f4bb4ec67909d1b7d23c8f2ec682a82b7eeef927e35f762c5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("8c1a87efdd9b12b806f78fcb07c22ee0ea45f3ea033fd0a5e5dfa87519e0ed52"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/substitution.rs",
        sha256: Some("7dbf93b1827f483ebb0adf8affb043e08fc2aa2ae7060cee953494351aae5a4e"),
        inventory_machinery: false,
    },
];
