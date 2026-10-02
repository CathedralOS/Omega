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
        sha256: Some("c787628cb73a6111cfb96b070b41711b40c5d0ea9d0a9b8c218d771f7b0837b7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/propositions.rs",
        sha256: Some("7085441884af0ce7bd9ffe11f3030dae53523c56574d8c15b7c5c3f2a3bd7c4c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/value_ids.rs",
        sha256: Some("5a139626c591debe967f3f10fb1668feeb190add5fe5d5e1e762f227967cfcd2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/language-semantics/src/byte_predicates/mod.rs",
        sha256: Some("3f3e27ac4d38a5ca4972086399b31de2382bf77b9049db241ace721d77b83fab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/types/structural.rs",
        sha256: Some("bf7e8b02791385c06fdfa300ffafd403e5835d2ebd00a1191072aee3f0ddbac3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        sha256: Some("26fa854ab35f5b5f884fe3412cf86d47328b4ae5fb2d701e8ad5744e51b9682a"),
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
        sha256: Some("08491af34fea718154cb5770121babdbcc938d1293dcce424ef3f5cc2e8ae68f"),
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
        sha256: Some("d3e09e9e7f7fbe4b7ef6cb3b70716470e7919be6f5036eb8cfda19f5c4dbe339"),
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
        sha256: Some("62afc2c3601a06a5175556d98022ec80d66321e158ae2dd1b296bf9a04ade90d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/bound_mapping.rs",
        sha256: Some("efe5a7a6dbed69150b814fcff0e07c08fc968178015fbf6e9812373649e93edd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/remainder.rs",
        sha256: Some("4b18f94c8235c9e32a77b5f41f20e4af09572d9c82320bfc2d1061e49a1d04d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/truth_bounds.rs",
        sha256: Some("4dcfeb434fb8e982156f24f6f4003ab4ec397598242709c8e91232f00fd5c1f4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/integer_rules/integer_affine/witness_checking.rs",
        sha256: Some("b03d82b2c427d50366670b9337b2f0f58395e2b4e5907b5eb63e92680642e55e"),
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
        sha256: Some("91a0ba613c8b02c4baec7ec2dc5270946475fde32cba6a63c178f00f8dcc10bd"),
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
        sha256: Some("ddaf01a947110041b1cd935684ccbd06fb128e36be625615abafbc4f11e96dee"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/budget.rs",
        sha256: Some("ae676ddef2f0961f8e895ec384d0dccbd5e91d3b6784a952a28446de1f540a16"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/proof/admission/src/predicate_denotation/value_equalities.rs",
        sha256: Some("c3a8c80721390ed8ca199d080bc694cf0f98a69a69fcd6aee01de6edc879d04f"),
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
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/arrival_storage.rs",
        sha256: Some("d39da4f6a0fd046bd8ff845a77bdf535700a2ef103ea6d3c401b7b8d96ef8f4e"),
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
        sha256: Some("789e1750b650d04e9e80c76bce0bc6be0c0829c3949794939ededceac5e1743a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("261a5bd656cc983f141c1bc81f53aa172bcf6252d7edb9fdeb516792401b4d78"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/lib.rs",
        sha256: Some("9b63a2aa8797d73de569ca2f9f38daead07d5b5b5c8930365bddff1cc1b029e7"),
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
        sha256: Some("7e13d5340cd1fdd6670b250cd692e5cdaa3420f9292e839e4eca76a6018be151"),
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
        sha256: Some("89de672076251bcc65fddc00700a5ad7b16adad36f56e9e1b32b116cc7c55202"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/static_path.rs",
        sha256: Some("1ff4c82ef93b1644a277aa2331e223a730fac77a0d66949629da0b5c690baa81"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
        sha256: Some("a43116e5c61df74a0dfad25cf0a56e758989a273a0a7dd3176cf554da5b4571e"),
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
        sha256: Some("4bfb6ea2e373b898c45313637d84e35a9782f44c63b62e4a5097336e0fa3aef8"),
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
        sha256: Some("a4bbc7dbd42d65f38a917449a42abe804455abb34336c825f0c3dc54f0aa0f19"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar.rs",
        sha256: Some("b2f3b1e1ffb54587cd2be866badbf6a97d9d155213ae753dcd2be614db534ba3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural.rs",
        sha256: Some("13bdd37a7c91a3601708df59f27154c3c2fa090cf6293d1c45217ddd1c033f8b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation.rs",
        sha256: Some("a4a22cb4aa8f9de10ae9ac27ae201c5b3d78fbd6e0f8d79955ac84ea0ffe7bf1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup.rs",
        sha256: Some("644feee128c6834ad4edce214724e063f70e5bc5e1cd932aa2ccee566bc6723e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("73fef6c9b939972f8fb11ae1453c79c7eac38f0ffd874f0a231aa6c9645b3c5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views.rs",
        sha256: Some("2bc19d86ee2e4fc6408b820c610ffd1764114cc9d80bd0eacb333c6fc60127ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/block_views/mutable_records.rs",
        sha256: Some("daaf160f20d0772773b5aedd467e36fd7ec4372bc4107f6da3d551432f5ff44d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs",
        sha256: Some("b93b1204f75315245a4b1784c09c5afe157a457c4923e4f1d77b0ee293763d9a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/concatenation.rs",
        sha256: Some("8aef3782aa5f489e4819e56b137b3f81a8cdb9087e270925ec68822b1a9d6c55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/length.rs",
        sha256: Some("727bc3224e0a88264e2e0447b8f75849b7e5f7d816c68d68e30f54f5126fd829"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs",
        sha256: Some("5299d96877b115a718eb460e1ee7b3c883fde7be67d82b66de1cbc28bcd6b673"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/subslice.rs",
        sha256: Some("7a17c5c69807fa86b79b06868e420f9924e8a7784d39b0c3044a9632eadadc35"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/establishment.rs",
        sha256: Some("502a3e72c269d486a50605efb3becb774609abda7537b785c3aac7bbad536c7c"),
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
        sha256: Some("c7734a472338294d95b002d8fbd2ce8e928dc8c1d5f85ee84adb342f3b5dd6b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/contracts.rs",
        sha256: Some("1a1bbaf27ed7890868d5768ec8922ebca5b0b5b63e46bc9f6e18690c2a3bf867"),
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
        sha256: Some("59a2093ef913a54efb3ea404345d6071943308160448f9f373c984bbf399297d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/crash.rs",
        sha256: Some("d0b3eaca03aaba1c53c77414b7a0425e9bab9a1fd63af7f816845521606eef7d"),
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
        sha256: Some("d44b6e4002f054689d11bf0402023d478919ac17cbcaa90ac6a15e20d4963450"),
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
        sha256: Some("5bbea1ec38b916fb8684e625cfc140bf84696f736c0b634adee0247fa007a1ce"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation.rs",
        sha256: Some("91471ea9be83f0776cd633b27b47becece9d39ee190e288d5ef06895327c96db"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/boundary_machines.rs",
        sha256: Some("7fd9e4191774ad78bd03630b9a178cf1b9109924d415547c43bd60a64b6095de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/domains.rs",
        sha256: Some("4c0d5a1a9bc2de911b57d21546cf9c0103dca75fa9968c78087bcfd72b0a2547"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/machine_foundations.rs",
        sha256: Some("16da88542d46a73c2d8cd57211be841c221313598479b55123b16a1a3f216786"),
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
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/service_binding_custody.rs",
        sha256: Some("0d8a234a4c73ecafd178ace3904c73cecf8aac3b51d4775085de0e689fe277b1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation/structural_types.rs",
        sha256: Some("9b1c07de5a53398dfc1e8e3ce5aa18d32ce2e917dcd0c0b0b271c2e8f58c36d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs",
        sha256: Some("aef0967271d7dd0369e6a754edeb2101e350d03a7ea3c85f48d9adf62298ed7a"),
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
        sha256: Some("e2ae4f342bd6d4f82ac85675510d2e764f18f214420badf3d754a2548c6032c0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/shared_loans.rs",
        sha256: Some("56140047a9bddfa93cbc51f61783334d6b67cf1e246a114edd68983295a623f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier/terminators.rs",
        sha256: Some("0e570ec2e70c52fa92f7ac970eea909bd1f7ec4b6d96d0e8b00954db4a0c0f80"),
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
        sha256: Some("99b2ce5275a717b1488db869c323e234624aacfa669dd88024a4511b099a07c6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("22acd4dae2cc203aa30c06b0902d57cf5095946622e53c91ec348ab5f8ce9512"),
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
        sha256: Some("eb1e57534826194685f8a547266ef26dc308a3c9e99ed0b19828626d4a5116bc"),
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
        sha256: Some("a039d6ecedcef7c25307390730f20e08838769bb4f2e2696ca8c3ab0d562b7f0"),
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
        sha256: Some("b7a2cef563228df46d079bb8504dc4ed216807e5ab900cc70a086908f21f77da"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs",
        sha256: Some("00b433837410a9db64cfa7e3f22980da1040cacb82ba2ed9731fff13ac86e51e"),
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
        sha256: Some("a003fb543f552699bec02462106722a1141934bca28c3deb513194ba253ae744"),
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
        sha256: Some("26fd81daaa4748bbbb7bbb5c6fabd56d9e93755761dc02dc2a68e2a73aa54791"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
        sha256: Some("ab8e3a73a3693b39d151590a944800ad1e44be5ee92869018dc4d34caf437765"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_store.rs",
        sha256: Some("49c135c091ead638a20662d4d33f3880a11de0662bdbeff7aa26c264ca2f3083"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_view_field_store.rs",
        sha256: Some("70551cde3193ce1e516f8702392e12ae2fd53e946d4cf7d05ad6725a7dd02508"),
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
        sha256: Some("b55fa7bbeb72d3551884edd4fe539c27597cd666579c673e195d9c4344fd106f"),
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
        sha256: Some("11c5f71bad33fbefc314f50aeb8b90e998b07091f687e8208edfd247f0178d4d"),
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
        sha256: Some("28eefc468aa48a504d5d7e2386d559c57946fb89df764263ffcf2d5a428ddef5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_checks.rs",
        sha256: Some("5d3f25c553d34dc530f972aa2924bd89534318459fd9669efcda871c23dad54c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_arguments/argument_pairs.rs",
        sha256: Some("71df2105e806dc911e22ee8f4a5e8f6717ec9f8f4f819a80c40d024586d27709"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs",
        sha256: Some("b6724ad1fe0853b5bacf6d3e6bac90a29732b68762bece459d56f3caf5af326b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation.rs",
        sha256: Some("8b35b5b66670d125283a5c844717952f53c64cf06dcbb7c88c31cffa173a0a9d"),
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
        sha256: Some("41eb55e160cb0b2ef6731a2fd19b5d0c2177f1fb2bd22d67816e834a4a978723"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/runtime_indexes.rs",
        sha256: Some("13b7fd96591e100b5abb763c29ea65a6361607aba4bcd3eaa6d04f779f290b7e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
        sha256: Some("2be77ca8487bac48f34ff16432158efc954f1ebe99ddd132f85bc058e35e56a5"),
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
        sha256: Some("8e485de28ab6254d75cfe3b6e150212add0305b15b2b2afea7647774892d089e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs",
        sha256: Some("5013830c719f7f45a003653fb0703d1b5bbf9bf9de7d2c7e94963d10831e503a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/field_snapshots.rs",
        sha256: Some("5e3ae38d6c6dcd91881c175a6361b5ac85e30e605eab7b16cca3fe700a0ef71d"),
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
        sha256: Some("433f79cdb521a5c1037145604a17704fef55e88a865eae0826db43ba8e14874c"),
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
        sha256: Some("39186c11987fd81e9326d174ba3aedfe9443e0f6111626491a2937067fc821b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("29ee558d677cda35b24be01c2ee009e946d0876153c8711f537cfd519bb72a31"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/case_hops.rs",
        sha256: Some("d29d14c8fef6a2dce701869017ed11b41ea4117f5ff62764dd29353a521ece06"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("9638710b486cc09a89c461d5a75b74a4a80cd4e3f1a4424cfc3001bfdfc88116"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("5cbdf91d88cbaa8cb246d52f6319a406ad899e85cc4f6cfc16bc7a7abd5f08b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("be8eada17ad9160a6ee361579f414486d023d4be474e50861dc009a30a63b66a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        sha256: Some("c16a16d27e00be755863dc6b8fcc146191a893a920f55d2c8a992fe8c144568d"),
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
        sha256: Some("35d8fe8ac70b6b55e149195e33c0bdc5b668c655be12f55369e556f13ca60106"),
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
        sha256: Some("b515d7676d3cd28736a60c7f51823e5b3be496505763e842dcfb891cbd016f4e"),
        inventory_machinery: false,
    },
];
