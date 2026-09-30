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
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_cast/expression_roots.rs",
        parent: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_cast.rs",
        module: "expression_roots",
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
        sha256: Some("8b7cf10c841029b0cffe0114757c09c6144316cf1d4a069c4404277fb5eaa83c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/propositions.rs",
        sha256: Some("432f0e4b005034cd775f69b2c92bf3f2b9ae94f9712a97ab28120b16c84976d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/value_ids.rs",
        sha256: Some("d4c367a4efc4a01a2c5669e90d9d78ed2ab09075b8113b4a0739c05306c2c736"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/language-semantics/src/byte_predicates/mod.rs",
        sha256: Some("3f3e27ac4d38a5ca4972086399b31de2382bf77b9049db241ace721d77b83fab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs",
        sha256: Some("a9729e47632c78bef2fd344e3f11b21c12f96f40acaa03acf718fcc2dfa2839e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        sha256: Some("88adc10fedeaa25184459c1a48e893fb6286093a510e34fa6642028463fae00a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/bounded_byte_owner.rs",
        sha256: Some("fcfa45b82b6bc9dd25897863e799c8b37198b87fe91b404c40076d3043b794bb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/store.rs",
        sha256: Some("953782e6c7c53331a237789ff7d3e16964256f57b1c0dc86ecfb5a2c120c4642"),
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
        sha256: Some("aca1127d639279bef7e60a180850c1100353eb21978f58e0637ddc1cc4527d74"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/artifacts/proof_bundle/witnesses.rs",
        sha256: Some("e84d2df5d97be728a4e94d2a4cb008448ac2196ebf64ab41a1a4f2e47d393dc9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/operations.rs",
        sha256: Some("e032248cbf785ccf1577f204ee2105801d3b033f536a38c46c1e0918eb354393"),
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
        sha256: Some("dfc3829e4a06433f81793d31e481e0484636e1916734bccaf2878521d6ec83cd"),
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
        path: "omega-rust/psi/compilation/proof/admission/src/classicality.rs",
        sha256: Some("c2876c5d0770ddcbd8ff00bc6baa5641048ba929bb1070d3b141149b46632bf7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules.rs",
        sha256: Some("6d8079c98bcbca0d308070dc28c03727085854fb4e0e6764a6aa9232091f6645"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/closed_integer.rs",
        sha256: Some("ff8aadab21e62ea1837c18d3691735f3c865cb8b4563b61adb3d363d8fadba5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine.rs",
        sha256: Some("573cf4a16b3e17ae75ca34a988306aef3ea620a14e9776e9b18de6d296300eb6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping.rs",
        sha256: Some("eb6bcd824c72f047aaed84508c65fb98cd019ea4d5ad7721463d7283c2c40b9b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/truth_bounds.rs",
        sha256: Some("14698fd254936e56884e5f4c1843b436f36c58d1b43bdbe321e914d05389f90d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/witness_checking.rs",
        sha256: Some("c23c9176ef968086866578197b37745c0d7cc71d68fd4fa3063d1a53cff87797"),
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
        sha256: Some("7b10e43e0c93548adea949f77e07a792233da91a829de42e7c1eefa77c4c854c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/kernel.rs",
        sha256: Some("121fd7ce05df9b372ee1dd2afd92177eee57722d3e78d48c7b47d2bcbe670196"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/lib.rs",
        sha256: Some("4d4af3bd653a6586862d2a7b470c102117e28b4bcca44fb812af31f9e760fa5b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs",
        sha256: Some("ceba4d99eedd93996fd37b9b99065b5d360c7f8abeb3304cf593dda20d41e571"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("4a452ade997bbdf0e4642ff249eadba3b3f57675db4946c0b3e168e1de31baa6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/mathematical_core/bounded_denotation/addition.rs",
        sha256: Some("5df4121cbeae848cc6481685e394af6a44685f72be8e85944f7e684b379cc665"),
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
        sha256: Some("7bc1b03a4554e8d7ac140f9aebecfc959537bc038432c0981d442414101e985e"),
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
        sha256: Some("a7eac754bccea0a2b04a7e28deef3691d3aa984a1562a202bdc2ad68f235943c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/budget.rs",
        sha256: Some("636176124961bf6c4c537c95401f222679c4fe7c16871fba9fae20f4f071691a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/value_equalities.rs",
        sha256: Some("18dba7ffd0eafa775c4dda44b2b27aa6d8b8878f8592ab3f23ee24e14b1e85f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof.rs",
        sha256: Some("d9d4067e40b787d3035869dcd62e8263f80003cc1d7ea99a47b57c3556e0a9ef"),
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
        sha256: Some("59cf13ae2ea63b17dd7937c9f9eefbd1ecf05ed756b9157f83abed2ddb61a140"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/proof/integer_order_rules.rs",
        sha256: Some("b34c5aa7b3aab8082910bc8030b23029637c704247e709238e1eec3093859860"),
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
        sha256: Some("0766c5aa87e17483d3a4aadbdcdbdb3169b36aa75447ae9e27035762b3ac2681"),
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
        sha256: Some("8279f75296401df409d481156580375ec8cc46c607783705cc367a5e0984f278"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("261a5bd656cc983f141c1bc81f53aa172bcf6252d7edb9fdeb516792401b4d78"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/lib.rs",
        sha256: Some("660c055d9aa0e9bee649c133b2febcbf4c8df99a6d12fb31758d181457b32091"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/placed_view_referent.rs",
        sha256: Some("a9ab51bcda82b55289782c8b13ebe2567ed5294ae88b3ee9d2dd429686dd1393"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs",
        sha256: Some("7e13d5340cd1fdd6670b250cd692e5cdaa3420f9292e839e4eca76a6018be151"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar.rs",
        sha256: Some("cd70b9869f06581cb53763ff8d5d2605b50e372176c330140b5aff6c4244fc33"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("55366708fd3b4fefff67d1ee55b692d3a2bf6f3a46e1f36227c09a80c2324971"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/elision.rs",
        sha256: Some("00e659c02dab94a991b5fb78816d9d671457aeb4f50f9f0b1127cf30064480f4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs",
        sha256: Some("39f9571765770489099e41e5417cb3f643bbfc38548718430642b80f40314d4d"),
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
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        sha256: Some("9a36ccb7e7a1aba47b5ffc2f266f01b095025907203dd44f0480b788a3998b42"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/static_path.rs",
        sha256: Some("1ff4c82ef93b1644a277aa2331e223a730fac77a0d66949629da0b5c690baa81"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        sha256: Some("0c474c22c98b3290e59282e7b60609eb8f5d33f2e470d2328bc8805571131b6e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("034f50051513a8fb48cb751b5b6874d8ddda30d9ff771c854ad3ae399cd33b9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs",
        sha256: Some("f5808933bb053be03c85858131f182808e9c8395e17e2fa32eb5c2c81e5fa765"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/zeroed_structural.rs",
        sha256: Some("8f5eb9cba92d0850c12d2101ac7f3375f16b65f10a59323a84923585edadfcbd"),
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
        sha256: Some("8a99ed9eadadbf7f0a3ed0d80cfa3b788ccdc6067a2ffe637854f18ad6bff38b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/control_graph.rs",
        sha256: Some("49c60170b5977126d23dbaf96934d41701ecadef97a0018f6d68f66641c59b02"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/lib.rs",
        sha256: Some("0d4642d6c98c628caef65fdf8ed777631949483bdc8711403525d765344d88f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization.rs",
        sha256: Some("e62c409f0f043c2aa81f90e8b35bc70df3ed2cdf77e4ba6d8569213bed3acef1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/control_flow_cleanup.rs",
        sha256: Some("543fffc93a54f50d2715769a1bc9ff8e3a38173ad5c249f5425095d6c928b0d2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/copy_propagation.rs",
        sha256: Some("a688617c2bdfcc79802f0ad9a177fd49a6866a8156e7dbfaf93421f0f827acae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/optimization/dead_scalar_elimination.rs",
        sha256: Some("3f8ca9145e4f956f64ea98ffd8b8da83d8b294fa4fde1baa80aa4c68b7f369d5"),
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
        sha256: Some("c85cb8d11bf74427246a054c1d0c158c5122ff90f61335204914f987cc7898ad"),
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
        sha256: Some("97504ee969246ceb56137111887399fc1bfd9366d2f4696a4da1bce116f43041"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence.rs",
        sha256: Some("9d88a810d88cd02d25beafcd39b7aa65b12eaf3759dede8e3f8e7da67c7d5974"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view.rs",
        sha256: Some("2ae442e3b515d265f38bb2172659156df73ee3e6a4f18b996af2f9e3454e3507"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar.rs",
        sha256: Some("b2f3b1e1ffb54587cd2be866badbf6a97d9d155213ae753dcd2be614db534ba3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural.rs",
        sha256: Some("312302ffd5e2f1b122e520169f66b76b0d1c2de351b22e374fb54d09883c1951"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation.rs",
        sha256: Some("ac396d556a3fe4f7f4670dcd9955199e3b25b341e6331dfdb652e01378532b4b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup.rs",
        sha256: Some("a002b2e1d6cbf2a2de21221e5336fadffdc3c6081db891e621448a01cf3cc3a2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("73fef6c9b939972f8fb11ae1453c79c7eac38f0ffd874f0a231aa6c9645b3c5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views.rs",
        sha256: Some("6f995a5fd643200c8f84333ce80fd17dbf454ad47d3d558a7de18171cc7f106d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views/mutable_records.rs",
        sha256: Some("600aec574044896256dab243eb4153a74e0aaa1c95f5b3b75d2bc97c0f39cf64"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs",
        sha256: Some("27d3a9fe72cf832b8df5cfa67a1e7c4c17162d914fa7d03538465954ad66757c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/concatenation.rs",
        sha256: Some("91c81c71ef296fab2be3ae7b4fad784c3e7df08e37fb7cba52576059eee9c092"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/length.rs",
        sha256: Some("7be7528a0abbee8f082a6849a7705c821a41e573e503ae5f6d8cd8b2caed55fd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs",
        sha256: Some("7cb538b5e4dc31fe89dc663cdb59ff268b90ce06387fc68d498bfdf1d81e7629"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/subslice.rs",
        sha256: Some("b9fc05b446d38b6383270551e258a1dddc4d4866fd97703f01ec85168782b363"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/establishment.rs",
        sha256: Some("305b196978ef7bb3a847c987ed0ea813b64ce01709425d299e4af79b4fc377d1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/length.rs",
        sha256: Some("f26911fb5a2477bd8579650e713cc7e11fcacd4323c0b222886fce9996f0d0a4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/read.rs",
        sha256: Some("97aae7013ea134dd6b8f8276ddd296ec11a14808a3a92e86aefceb3cec5a7de0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/subslice.rs",
        sha256: Some("563eedb9ed931169adc6fe1eff0bf0f33acfbc8b476baeaba6a6f3929524093b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/write.rs",
        sha256: Some("6dab7fb81c5d5e823adbb749422be0a5f65781f14c77848091a3f9c0ccdc0125"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/mutable_establishment.rs",
        sha256: Some("e448f0fb206b8cd1f8864768d2b409a07f22b05447228f47244de628372189eb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/write.rs",
        sha256: Some("d7691ec85afd476bd8c90740a814157a599695669746eaa0a7c6b0c230aa1a16"),
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
        sha256: Some("c7734a472338294d95b002d8fbd2ce8e928dc8c1d5f85ee84adb342f3b5dd6b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/contracts.rs",
        sha256: Some("5e7e67d1d4c851d44a75906218cafeaf2a6c0ab8fb1ae47670804103d6de51bd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow.rs",
        sha256: Some("0c03a27ac81dadff8fd4716c3551a7160bd8b0d213e62d02b83b17876dd5731f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/block_checks.rs",
        sha256: Some("c663e6a65cd6d3e5cff20452833b390d9ba4e3da6a51d6275ddc559716cf7131"),
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
        sha256: Some("e07a1e038ac83d277f4ef68cf25b3363999e0125bc3500b142fa05c1ed61ca38"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash.rs",
        sha256: Some("9a327f92b490ff047db9e499ff9d666fcf59987f3d672fca4c6638c6d8c80687"),
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
        sha256: Some("f469632842c8456ee3c4182032f8b6b8068ce034497d22b0ac33a48418202f8f"),
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
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation.rs",
        sha256: Some("d2963a91ac3465640910e2a94e3a6ba092ee75c3de6024f26965b2a29d0b452b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/boundary_machines.rs",
        sha256: Some("7fd9e4191774ad78bd03630b9a178cf1b9109924d415547c43bd60a64b6095de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/domains.rs",
        sha256: Some("bf3852cabe5036b96ff46d99fd52bac723a6c36432e15235941f8eb4985142de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/machine_foundations.rs",
        sha256: Some("b9631504b4d5f5f1019d1cbc6af1c12d84effb22db0b40deea334ec7111f6214"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_candidates.rs",
        sha256: Some("d7d129d0cdfcdee6153a5ac6cfe127f227466715b39981cf3c937557cbc131b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/provider_result.rs",
        sha256: Some("6245f9eace98f0fd3789f0e5280b787800c2e8c9e7747d5af1adf8ef3b84a7bf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/services.rs",
        sha256: Some("05400709dbd93216f90fc66f210873e5124b70b6aed5c0c5868f678852bb16f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/structural_types.rs",
        sha256: Some("8122b94ed4a3af9e728d4aea871b13524a086f1413ad461cd713cca243b78cbf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs",
        sha256: Some("7b8588ce91144badfa46c858e65e37138497fd52566b23bec3444ed011931c49"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_entry.rs",
        sha256: Some("0d625f4c19b2f7b1f9ed66b31422008d39be29084206641cfdc44b0afea9584b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/block_parameters.rs",
        sha256: Some("ad5fd526fcf65ddf1fe951c993d7ab025273c35c3100c0fd1bca2d137a89deb8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/operations.rs",
        sha256: Some("3b38cea5a2d1dfd5e8c5ccdfa8009aece60bf8bf2aa4e93576e95a46b172a07f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/terminators.rs",
        sha256: Some("d28c22673190afe6eb08e0c4fc1eba4084499800dcea46221369addf66219282"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/traversal.rs",
        sha256: Some("30f2b690c99efe51abeb58d8e733fa9ffbc33ab3b89b07fcbd01d73fa6c82db0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine.rs",
        sha256: Some("5367985eb666355c635353339ccd83beaa8dd45869b81544182234b696299f13"),
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
        sha256: Some("1cb0fe39c52354c98622201c8b4a14a2573ef3813d8afc81b909e052e9fe7e3f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("3135002d96ae3ef0b9a554cb9d2294769c217f88c0fd878ee4059a84722a3317"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/structural_places.rs",
        sha256: Some("4cb5e6f1c03e58a323cdbe90e3c00adaf8504459f14faca3265438d573ffd266"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations.rs",
        sha256: Some("351ad6dc768c98a8cec2573e8a3c1d7bdab68b2aa7f2916798f2e425ed36a9c7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations/arithmetic_operands.rs",
        sha256: Some("09f380450463228cdb46af6020db404650a9ecafe3ce160ff95c3862d193c866"),
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
        sha256: Some("7e6f08e9ccfcf01fba9e5daf47cb7a627ebeafc98c718793b8287c4d88057edc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/proof_recursion.rs",
        sha256: Some("904e3ab8ee2f6ae6e0a7c216eb21bbca54cd2e13bd9274f7f1fa25410f7c31ed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/propositions.rs",
        sha256: Some("468a33bd462148773cf12ab82a8271c5168d2264845a6471f3254b7520e6601f"),
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
        sha256: Some("8737b191ff874765db91d6b38f26cdb361cf0649f901217ba6cd1ed0127a3a05"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs",
        sha256: Some("6210165e4b5e4f77583070621e897912d234b284db946d9a8bb81b77b9c97e17"),
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
        sha256: Some("bc7a4c34a57f24787b04b96b3857074279539ad3e1c1212f84f453dd7bfc8cba"),
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
        sha256: Some("90b6531b727a2efbff6f0f2c38272992dbc9d11c2b78b257ed18900f0b08c01f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
        sha256: Some("ad797ff7e76945ed7e211090c25ae3368a7927716a425b035d99eb3f6f28254c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_store.rs",
        sha256: Some("90be8f2d7cf3d50c1180369c2fb18418563f6d26b732c3f43e76e54d333d6681"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case.rs",
        sha256: Some("1b5c70c62d1e571cd44989381b22ed6f95ceedeb3053fc3344548fb1600221ae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case_membership.rs",
        sha256: Some("0a863bf426392de3b34f34c4a706bd1d66032618b17f69eb5d8d4a0401de0fcc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/leaf_copy.rs",
        sha256: Some("c2eacdda333a7d557b37f2fa35635b37407935514a07036c1dd6c363a39ff10d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations.rs",
        sha256: Some("82ab295fdb35b9cadfd3e99f2c6e440286115f75498052b8177936f3d87f3d9a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/boundary_requirements.rs",
        sha256: Some("271842c2de2f78024f7b143b8d4527aeab937c2d60bc78f3810d75cd720b877b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers.rs",
        sha256: Some("a873b8e08e5d23d0e681a188acee0a15229cccec462a0da4fd9315429a6ffd58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/claim_transfers/argument_claims.rs",
        sha256: Some("34bb5a209cd990213b4c0b1d53c3f9d23fc819cb21f90c7213c21c791497aef3"),
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
        sha256: Some("20e119625242a824fe8f5f1fc1e2384ef095fc8f55a1602b42b3d98a5407cf26"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments.rs",
        sha256: Some("75309777c47c45101189c33a5bca1e17cf6e855cceeff8d5c087bf046b346a25"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_checks.rs",
        sha256: Some("d35c53eb5bc541dfb49d519da6b087f124b8cae6f37d15a07ed5b380d054ac05"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_pairs.rs",
        sha256: Some("71df2105e806dc911e22ee8f4a5e8f6717ec9f8f4f819a80c40d024586d27709"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs",
        sha256: Some("36a4746bad315a626785f4f81e6b5187c408d98d619e4b8fd1b17c0f4f62b9d3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation.rs",
        sha256: Some("80a0169592a72baed1de666de2af9f82d26e3dac6b596051e5bd8e7560f9378f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/boundary_calls.rs",
        sha256: Some("5e7e71c40533c7b8590dc060016fa314bb13d7199c49ff658c7e1e0a270f74f5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/local_establishments.rs",
        sha256: Some("2da90c1f568626cc3fae937f12934b94cd0fe537ddaada6b2ad771280f1aca5c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/structural_calls.rs",
        sha256: Some("6eb983d29a0e259a35a2c7fb245f6bcd6292b96e715920798f1b9c613da39e55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation/unit_calls.rs",
        sha256: Some("ec6977a35fa4e78ee7ca6a8e29635cd85f5fa74e6ba1f4aa6a1a19a0270ed034"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/qualification_rosters.rs",
        sha256: Some("e478350a585aae369594ef12151a92415967b6182ab6ee4ffb8f9ab89388748e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/result_contracts.rs",
        sha256: Some("c2d896fde70794802e47cf3a297b7cc49cdc7131fb72a451755024ebee932529"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/runtime_indexes.rs",
        sha256: Some("13b7fd96591e100b5abb763c29ea65a6361607aba4bcd3eaa6d04f779f290b7e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
        sha256: Some("0f0cd35ba55a0cb962893c613ae63dc2fbfae44be30a5d0789c8b5ef18ec52c0"),
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
        sha256: Some("8f348531834fefa6a147f324f1062971b666126ebcf521118e51b0e3c679cf23"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs",
        sha256: Some("989d2f7702fe97db1a26cae08a83d0df5c092428170d00728003fa51088d0b2d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/field_snapshots.rs",
        sha256: Some("5aa43940c1b478d6c68b37376e0e11b68bd9a8692104d46229c7ff3dbdc35e46"),
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
        sha256: Some("8baed7474c46d416778909171383e184e2ba9388809d4ee41e573262d66b77d5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("a039a6680a749c1bdd639ebf3c5bfb1f3f4b19a4aba82a34167fbf23820286a2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_obligations.rs",
        sha256: Some("9d731e639373489a1172c7dec11fe19ee307118cefac4ab0a025758850fa0405"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("e82b7dbd0728d4f0585392af5c34e4846fafbed1a03d9107c4c2cf04e4f434c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_context.rs",
        sha256: Some("dd84774706458ed7fe5cda34478108ac36bc9367511c33a2ec8717cff45aec79"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("d4430f842b024b0a1a4e87c15bd1cd74f52b59df1c3be3a5b1c82810f24a984a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/case_hops.rs",
        sha256: Some("912223e23a09f280f76022c7ad081334cfab670619ed074e801736fb990405b9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("fae022c8cddd0baff981ee348f5dbce2554136db9247dc8b711a3247d2c3d47b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("5cbdf91d88cbaa8cb246d52f6319a406ad899e85cc4f6cfc16bc7a7abd5f08b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("9bad8ff6f81a2ed7f2a6e9e5c093088d629735b25f6940dc730b56ba6f150074"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        sha256: Some("7cffe94f75f0af7e01a4816751b54d86ac7b1db98d3d40aff6e3ac680c56efbb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("7dda9be1c6d8992d2aedb3862b47f01250568d6fa228ead14156be97d24ada32"),
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
        sha256: Some("69acc6229a9281e8d5ea5dbecd164f04ca3bea1240323c50d95a7ddb84f6900e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("7242e8f47b7595065f4a5a2ce0435ab85eed9a27f9c8be5ddff960241ea4153c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("8c1a87efdd9b12b806f78fcb07c22ee0ea45f3ea033fd0a5e5dfa87519e0ed52"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/substitution.rs",
        sha256: Some("161f8e60ebe22e9765edd22f55f8bcf2bbb4e4dc2544b1a7ec0d71a346c5abd1"),
        inventory_machinery: false,
    },
];
