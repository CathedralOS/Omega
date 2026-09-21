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
    "omega-rust/psi/semantics/proof-admission/src",
    "omega-rust/psi/semantics/terminal-semantics/src",
    "omega-rust/psi/semantics/terminal-verifier/src",
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
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests/bound_mapping.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests.rs",
        module: "bound_mapping",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests/witness_checking.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests.rs",
        module: "witness_checking",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests/wrapping_bounds.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/tests.rs",
        module: "wrapping_bounds",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/kernel/carrier_bounds.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        module: "carrier_bounds",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/addition/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/addition.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/binary_numerals/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/binary_numerals.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/casts/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/casts.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/equality_transport/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/equality_transport.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/subtraction/bound_tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        module: "bound_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/subtraction/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/identity_and_w_types.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "identity_and_w_types",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/levels_and_declarations.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "levels_and_declarations",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "schemes_and_quotients",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients/indexed_schemes.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "indexed_schemes",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients/quotient_derived.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_derived",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients/quotient_receiver_policies.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_receiver_policies",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients/quotient_schemes.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/schemes_and_quotients.rs",
        module: "quotient_schemes",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/strict_layer.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "strict_layer",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/type_checking.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "type_checking",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/value_equality_tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs",
        module: "value_equality_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/disjunction_elimination.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        module: "disjunction_elimination",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_substitution_tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        module: "order_substitution_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/case_membership_tests.rs",
        parent: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        module: "case_membership_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/subslice_tests.rs",
        parent: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        module: "subslice_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-semantics/src/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-semantics/src/lib.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_graph/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/control_graph.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/integer_order/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/integer_order.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/order_chain/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/order_chain.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/transport/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/transport.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        module: "tests",
    },
];

pub static IMPLEMENTATION_SITES: &[ImplementationSite] = &[
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/content.rs",
        sha256: Some("c19e7b811e13165463f7c49f6b11bf095f6adf1ae9ff6fa58fc70780b0a59faa"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs",
        sha256: Some("a36e62df711dddf9142ffe69aaf9cba84180d1edbdae77fcbff5134af456700b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/admission.rs",
        sha256: Some("c3978fcdcd9cdf232f31d2f092f52361c98b967a44bb287d8091d1e6f1d99362"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/nodes.rs",
        sha256: Some("fa64c64d9792609447b1375def10eb806f9258e459289d8096b7c74f174e325e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/witnesses.rs",
        sha256: Some("e84d2df5d97be728a4e94d2a4cb008448ac2196ebf64ab41a1a4f2e47d393dc9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/operations.rs",
        sha256: Some("99ccb42301cdcc56fd9ebf5f7b45c84e4faa0ae1665e239e32f3cdf848303b38"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("898176b83f921ba6a2a7faeadf5bdd7232acb9866338ea98b555a644b92a7fe7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission.rs",
        sha256: Some("a1166a15e645d8bafa31b8091c753a3d4c48e9e9f4dcc8832f6e47f3125e5683"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission/evidence.rs",
        sha256: Some("661acfa1de43ed33a88afb6a7ac7103dac14af94a641f14d4915f8f16f552fd7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission/normalization.rs",
        sha256: Some("cfb73772f16ad4583520c7797bd9c76bd4cf89f432524935e3cf4518f91ceeac"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission/recursion.rs",
        sha256: Some("de08da430299a812bbf426462ee880971d77809a503858b09772c854fd087c93"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/classicality.rs",
        sha256: Some("9a77ad9db24f33e789f7f61063c7eb51fb8e96017bef8822736bd00d34b733f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules.rs",
        sha256: Some("e954c7d0db149b1929d898e706bb7fa150499bd04bf2b37b36ad9bbd80e913d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/closed_integer.rs",
        sha256: Some("1abac5e7b8cc82d589f1726b4109679c596668f2f4b06e92b707068d345e1ece"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine.rs",
        sha256: Some("573cf4a16b3e17ae75ca34a988306aef3ea620a14e9776e9b18de6d296300eb6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/bound_mapping.rs",
        sha256: Some("958320506f868a0d211fcd16d0f585a02549d9979d2361de12d1166ef65f4056"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/truth_bounds.rs",
        sha256: Some("e3d31496dffc65790ff7399950b28d90352a5878fe203953843c2c46b07bd8db"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/witness_checking.rs",
        sha256: Some("1b3655122e18f10e6cf773321c94bbdefc1d2084f5cef72978250d01ee95ff6f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_cast.rs",
        sha256: Some("51296b4fe47fd7d7832036460e6881fd3f65325ac98626090dfee689bac41a99"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_forbidden_root.rs",
        sha256: Some("da61566583f17564c7d5165fbf71989188633ab7d1633ecbb14dd0071795d035"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_shift.rs",
        sha256: Some("294332d58db90760583e627b54c1c0c90f9eb33b9885ed563bd8d285dd20a60f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        sha256: Some("b203ebd83396519ff1ef32decf514950c62d35f32f71eed50c9e62f44c9a0429"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/lib.rs",
        sha256: Some("bb67b1d0f2fc4ef00dba0ef37686a3f25663a4d84d4a2772fdec2acbde66c0b3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        sha256: Some("ceba4d99eedd93996fd37b9b99065b5d360c7f8abeb3304cf593dda20d41e571"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("2c853e2f995172a98557fa4454baafde821711cc1efda860405241d2194d28aa"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/addition.rs",
        sha256: Some("5df4121cbeae848cc6481685e394af6a44685f72be8e85944f7e684b379cc665"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/binary_numerals.rs",
        sha256: Some("2086cd2677ebaddae90d2ce4d06b1a350e505af5cd7829bef0d23a8e85a1aa37"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/booleans.rs",
        sha256: Some("6c359795f5c0aaf6434ea030e5ba46e6fc7dfc481e140224ae4a6cd87846ce57"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/casts.rs",
        sha256: Some("3369b23d9c23278f0e4ae15e7f677deeeff106d6c96d28f02db7e9d73ade4031"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/equality_transport.rs",
        sha256: Some("d747de08a88fe99c3f74e26183f80201a3f73e90862536287edfeb7392316f19"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/integer_operations.rs",
        sha256: Some("b65519fcf58ccd9bf4f3b9d332f6ae6664bbc780a8779b3aa5833bdfbb1943d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/subtraction.rs",
        sha256: Some("5316ad1175e5dcf24ef830a27bf3eee134db16a3a0f28b7b167263b9176d0f87"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/certificate.rs",
        sha256: Some("904bdf506b4f65ed93034ed2dcd0cec28f94ccf66ca34b5dc006c0bcf6386833"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/conversion.rs",
        sha256: Some("4f1da58b4cb32429bb37c5502e77dac6fdfe2ac85112e32995bebd0aebdf9f60"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/indexed.rs",
        sha256: Some("6cc89bb22f8cb76cd9a4055bb3ef3d56947f08fe80e3d379b8a78b18e774cf05"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/quotient.rs",
        sha256: Some("827d583d57882acdb0bdb75b3f67a545538f770e25d288007a7fae222efea064"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/scheme_dsl.rs",
        sha256: Some("26f1607b07c7543e13520ef8f1ac172ce181b2ea0784057d9c834a973d57850d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/signature.rs",
        sha256: Some("4b3a1f4d394f0bc86c40d26c4c92a3301d83463f917efd888678cc14b6d6dc46"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/substitution.rs",
        sha256: Some("e6d1a91185300eaec8cf89f03e4bbf07815bf5789c5b1b51b47f9a463da6f1b6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/term.rs",
        sha256: Some("62ab7f693e70a81abba42f814412b373bef6ba095bc77d54657131163cad7c2c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/theorems.rs",
        sha256: Some("739eb92ea4e0983f8a45a3e06ce8c60eaf5bff23bc48c6d4f94eee695e2dbb11"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/typing.rs",
        sha256: Some("5e5982bbaff7af65149c12ec269d22bb744be67f015540893e8e0a3841962c10"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs",
        sha256: Some("0d999197d5cdcb619beb535de10f4625677cb19a6a5d337cfc9a10cfc7cccb8c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/budget.rs",
        sha256: Some("8e773b035bcd2569642257747aeb49a61830dd9f43f805f8f311bbc5e1fb5e7c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/value_equalities.rs",
        sha256: Some("5f1de567c198d105c218b34e2a662c689c19c434bd253885aba41156091fece5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        sha256: Some("712b6c20189ab04c4e346ad64cba43f62e2306a3f7af7ec167bca82c30bb9b1a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/equality_rules.rs",
        sha256: Some("3b68873f5389268867075165cd59365713077a37c620e0543c1eb14f2484d7b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/integer_bound_rules.rs",
        sha256: Some("1334ae397bee040b4e863ed7d0b248d8bbb7dec2d9d722a05e56007bd80aed1a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/integer_math_normalization.rs",
        sha256: Some("208870501e4e6a917b53cd130a89a12166816a157b58423667e52e9a651b37e0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/integer_order_rules.rs",
        sha256: Some("09af469f0cce94efc61839c06f601299cdc4aa6790ee7d92b8a0fa47b2b6fef3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness.rs",
        sha256: Some("4ff3c63e8f4949d87deefba5eb2f8a6b548624cf0283de60f6fa3ff1a1146542"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/propositional_rules.rs",
        sha256: Some("14de313a1ed0b0feddf322f36b2f5146399244048745ddb199c3fdb866f319ec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity.rs",
        sha256: Some("69e745be27e311a6a5c4408f2460482e7370a0f324d5d147c386447948601f5f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/subtract_order.rs",
        sha256: Some("09c561a250c78e4f10467a8978955a9aae1e285ddb3ffeed6b96d45d4a783df0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/traversal.rs",
        sha256: Some("c6b143aa74a48f599cc75b03105139d06ebe17740229004cde2f4c79eb269463"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition.rs",
        sha256: Some("2894a907c1ed21a5ba41b505dd0a0a32622beb1eaf75a98d8b11a791ae9765a2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("47bf190aef9bbf913fccef32bd3c8563c78e71aea49b59ae78b0f3fac3311293"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/lib.rs",
        sha256: Some("eb74319a51b9602a29a7b94016496d5506068519a8f824e71ff67fa585694263"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/placed_view_referent.rs",
        sha256: Some("a9ab51bcda82b55289782c8b13ebe2567ed5294ae88b3ee9d2dd429686dd1393"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/primitive_place.rs",
        sha256: Some("04cafe54d5bf30b44c121bb0f652ae489de7db2fe54b3d71bd73c44488b9136b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar.rs",
        sha256: Some("8d542d0362467a0af9a388ebd347ced9705272c8ff0e2e2ed340a5217f2c4c22"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("55366708fd3b4fefff67d1ee55b692d3a2bf6f3a46e1f36227c09a80c2324971"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/elision.rs",
        sha256: Some("00e659c02dab94a991b5fb78816d9d671457aeb4f50f9f0b1127cf30064480f4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/record_field.rs",
        sha256: Some("94252fc469f0bf2498997f9948f50fcfb9015c1a234d288a8f901a7f8e4b5d2e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/scalar_array.rs",
        sha256: Some("91fb0cd8ba60464e4073cd1092bbc139c1b1c9554e325816b266079bc5852507"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_schema.rs",
        sha256: Some("cb15757990e790f9f1f01cb0dcccc2b915136b506e68e2214daea8e25e09de19"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_semantics.rs",
        sha256: Some("2e6d7af1dd5e234cc33e894734746b2114997615245d6e99d90cd3068e99a628"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
        sha256: Some("a4107a9441001f2c0e40a1676a301e5a910169b969b24e8fb687a117fd1e1a06"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/static_path.rs",
        sha256: Some("abad2a5d4ad2dd4211afa9a6e73eebc8321cc851d0e3358ded28fcd7688d7672"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        sha256: Some("6c914cbfa152fdbb53abf35dc3f41d4bc05dd34e75f78b633c93a93db7e4c7a1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("f32bc057bb61dfdf6576d14169519afffce80c218548fc4ad4b158639cb47ae6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles.rs",
        sha256: Some("9cd7a695bef98ec4b33d5e70e01238ba6d02c928f0b13f0153f4a1027e2cbc56"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/reconstruction.rs",
        sha256: Some("cff52ac8323039e588f62cd2f7cfb867004f51d678bc8dca7c9edb6f3a36936e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/validation.rs",
        sha256: Some("2e4d6e43ebd95787acf6c87975785c24f5015d677b73230523cfd73a50346d1b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_graph.rs",
        sha256: Some("81da57b1e63d87cdfd902c49948b0cdac1c314b0208ba42f03102ec7ef44408a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/lib.rs",
        sha256: Some("a8ea48310b60342e1ef3e10deefa0f759cb68e9bfdf1e3bef293211208188c90"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization.rs",
        sha256: Some("e62c409f0f043c2aa81f90e8b35bc70df3ed2cdf77e4ba6d8569213bed3acef1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/control_flow_cleanup.rs",
        sha256: Some("827d7e567d00353ff67537e17a8ebce9db0976b46d464291b4f6d859e5aef41d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/copy_propagation.rs",
        sha256: Some("ccff0be2c35f1d9c4132e4137f43ad7240da744cbe34acd24e7205b45836c3ae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/dead_scalar_elimination.rs",
        sha256: Some("216fb8b00b572c07d94a653801e6b95981543c4eacb0e06667ed83b0b653fb8a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/global_value_numbering.rs",
        sha256: Some("260a2b2045f0c8dae11b314c2d36fb29fbee484c52ae0d94885fe35198e18c1d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/proof_check_elision.rs",
        sha256: Some("7dbc979c459344dc84b71953fea6e52e89463ea2887c77c41e5b13e97f1926b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/sparse_conditional_constant_propagation.rs",
        sha256: Some("92db19ecbbafbac18607753c77fd93583501809fabec15ff8ffc84febef0dc79"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/proof_recursion.rs",
        sha256: Some("d2a8b7d21c2a768be1ea7d16187891d2e0b41de1c833c0662538c73c4125544a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/quotient_correspondence.rs",
        sha256: Some("0f3e5a399ce5e940d928d41eac93cb3d8e2af2559cec33d9d3daadea9f5f7428"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/terminal_trace_v1.rs",
        sha256: Some("df34346c9f056960cc7890f4bbc211201290b480eea156e889f7b448b0666947"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/checker.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/operations.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/reconstruction.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/shared.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation.rs",
        sha256: Some("5f13239f2df52a73eafb4502ebe1e7826060e43a442b197f161e7a1141a2af19"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup.rs",
        sha256: Some("89b42b784228b3bf03eb437e6d869da3bc903ffa153d9c91bd745eb06fb9cb52"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("6ed69068a29009c7dd27fba37c4e642d63e0dc4adf39220c613171052cd30d50"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/block_views.rs",
        sha256: Some("a86ec351b2f729222662320a478dcd8559efed535bdd6d2c4b795cfad1162755"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/borrowed_windows.rs",
        sha256: Some("3c2794b3b517c4db29ec0e7829ff2c0e934c5b29a5044a84ce93a9f0063d8a0b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_length.rs",
        sha256: Some("6d8320af1e390dbea69a3d16e77cbd26b1eb31533182e558579924aa583a886b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_read.rs",
        sha256: Some("7e0a120f84a26074cc4550d62938e542931fcabd1f10c15388d4e2ed4f78379a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_subslice.rs",
        sha256: Some("08e9986b1abd10e46b48d0d02c1873639a75eb9cbc84f3af3e943f28fdf1c394"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_write.rs",
        sha256: Some("937c429c237d66c28e46dcf7990ee123aaa53b70b129271cbc437b32841d864c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/call_graph.rs",
        sha256: Some("e9a273e68cdeb695eba23138d51e926b43717a77d0f437935d21eb673791f43b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/conformance_applications.rs",
        sha256: Some("7e3615f1fac758826ae18604a14f7480205849dab7d79d6971f1dccd6d2d9450"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/content.rs",
        sha256: Some("502cb731370f6d366b7bfafa16c891f3e211e7a24d14ca19f59e94e707627ceb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/contracts.rs",
        sha256: Some("7017d872a42063485602fd2288271e82e4640e8944d40619f10412242181f010"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow.rs",
        sha256: Some("65a6a36014322820fc2f1a5df11ce12ad0848c4a5a093257dc7c6c12dbf977f3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/block_checks.rs",
        sha256: Some("94871fc2d7f91a63070a17b41856f19c96c22a8354dbc1ce32388b0f01f4a4a8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/block_graph.rs",
        sha256: Some("b7acc64116f33c7140f81df76a1b7a8d26562095b522a571ecdc1ea9e7df0e31"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/definitions.rs",
        sha256: Some("60c7df08e7f45cbc705c56deb3341cbcd4efed2e123129cefe992ba73fc55e87"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("9daa8dacc9aa2a8ad926d8e3662bc1a308c88e50eba5b3b1d0e9be2f025d993f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash.rs",
        sha256: Some("e34b39c4e925fde4efc98fe101c60fdc22c98686e136236f9e284da4da772656"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements.rs",
        sha256: Some("c946a8a2f1a31ac93577f3b3f6b440a3545fb5f08595a25c60fa64577954a8a3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/integer_order.rs",
        sha256: Some("945602709bf1e068626ce3cab85577d539b6dc4073fcce24c156e3a547beccae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements/order_chain.rs",
        sha256: Some("668b64f66cf474eb76a6ee46afd3e2e3baf855f1bbc9d15d97a58061cdb22b1c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/operation_contracts.rs",
        sha256: Some("3fa46f27cb46613daa8097397719c145f4885cf063abf00c46a1ed0c642a9334"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/outcome.rs",
        sha256: Some("e1873c66155a04aef909837db2a15407c033080ffa270a1951545411a65a9626"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/site_truth.rs",
        sha256: Some("a5a41170feb5b53c48a70c05d658b353d02a08e06e7e054ecaabf52160c841ff"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch.rs",
        sha256: Some("7b45141a41353a58015c17895e43c8cf68bfb91446d5f3c1c7e2fb3c34dc0141"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch/consumption.rs",
        sha256: Some("aeba74c8fdb8633021c3112cf59bbef3c8f73ab52439694ad219c34fbccbab0a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch/direct_dispatches.rs",
        sha256: Some("cc992c6423d7feeb30e46fe053e89a2fc4e7b3c7c2f213e8c086602c7001bfa8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch/indirect_dispatches.rs",
        sha256: Some("9ccfaa1ab7b19bb71213e3fc845cbff33fd088e17bc8a7e4a8d8b065b9e54955"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch/rebound_descriptors.rs",
        sha256: Some("30eac1c2934756d030c52d764327b0736dbf3a7842fbecffe5d2a3993809566e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch/selections.rs",
        sha256: Some("b9bd4f59b9a8ada583ad0b493bd13a10ff8bbbd94734d682484f4ad879bd84c2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/error.rs",
        sha256: Some("fea550a20c68c25ae9439911148ce5dc7a3e65dd22416046a2188438636e77e1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/evidence.rs",
        sha256: Some("69dcc074549edfef9e1244867f7a4e89bcf13ab399fa6e336d58025f16bd807f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/evidence/contract_lanes.rs",
        sha256: Some("a86aefac9711e2747fc9ce7acb8426fac3862729312106496931e82b2f94ff23"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/evidence/guarded_call_outputs.rs",
        sha256: Some("930af6e9d7aa92808408a4ac12a880523d31a86fe730613e6f30608b5a4a5de3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/evidence/proof_output_calls.rs",
        sha256: Some("6d832fac6bff0e0c2e84bcae86cc1219d6cd0e2ae1f6eb43920c4a15a29e2c09"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/float_meaning.rs",
        sha256: Some("dd08d046bde16620c57e0982036ab0f27e542921856a4d073914cf290611f746"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation.rs",
        sha256: Some("d2cfed1ac4f8462b59e4e38ca3822d56702f8b364a84a0a64bd71a59ce95ccae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/boundary_machines.rs",
        sha256: Some("49ff8f277abc5e6866f59ee4e9c08e356f344df048dd4e2e25ab4543b9519116"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/domains.rs",
        sha256: Some("b98ff5da9e87a862297dffd5098c5d77a04c41ba2f6c6a18b9c9dfbc5f24ad03"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/machine_foundations.rs",
        sha256: Some("da6cbd5f0f4294ddecc7e04221b7bd5321930b3563b0b339fef215cbad8cc44a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/provider_candidates.rs",
        sha256: Some("1251f4e0277a224e6eff4d3d529cad57f63d847f85a12853cbe867b2152813a3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/provider_result.rs",
        sha256: Some("fb15ba91d2f76e8a1a78ef352a97460b96766e8851f14caecffd81fd4e92598e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/services.rs",
        sha256: Some("05400709dbd93216f90fc66f210873e5124b70b6aed5c0c5868f678852bb16f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/structural_types.rs",
        sha256: Some("c61bd3ce0194583f69d1fbce6b843212064587ae48e98cc9e01b61c524a616a7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier.rs",
        sha256: Some("a4bf33f676e067548ddf3b0ed7143583d185b215d880d0967e094323260b4784"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_entry.rs",
        sha256: Some("0d625f4c19b2f7b1f9ed66b31422008d39be29084206641cfdc44b0afea9584b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_parameters.rs",
        sha256: Some("d22fd9e7023d90c1ef8e2f072373fff883ff0b4bd3afd09b2b4740e07f37504f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/operations.rs",
        sha256: Some("cc2d5cb4f3536f390a4fccdac8fef4ccfef4bf02d36a30e09dcbf05d173cc712"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/terminators.rs",
        sha256: Some("4272b55641b2842c93ef6110b26c0f4cbee4c2ddf1769d8a796f8b4863d5180a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/traversal.rs",
        sha256: Some("30f2b690c99efe51abeb58d8e733fa9ffbc33ab3b89b07fcbd01d73fa6c82db0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine.rs",
        sha256: Some("23185c36fa23478da8ce7350e366ea323688630b9b74e8e10cf05dc0af53b9a6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/blocks.rs",
        sha256: Some("d5db3918f257baf8f07f309db4a14ed583248321bd502bc461e74fc928ca701c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/contract_clauses.rs",
        sha256: Some("a43f7f4fe24ee7fd151818f1df80909fb78d5571ac776bda612306a67614ae4c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/custody_operations.rs",
        sha256: Some("8f0fb4a93bde606bb44adc1f8db05cc2fa9f5143ab8972bac038d718313a809c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("46eab6ce22f596880a0892bf48ec6b26d33b7539b2b3cee98bc29c8a21fc09bc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/structural_places.rs",
        sha256: Some("4cb5e6f1c03e58a323cdbe90e3c00adaf8504459f14faca3265438d573ffd266"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations.rs",
        sha256: Some("4798a68f7200b1a681ad362b21b25a8b7152aa327e62c65381a56925714dd577"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/arithmetic_operands.rs",
        sha256: Some("7fa4d0d2cabcc6dca239cf1c1dc18417982dfffaa52dd68684233428ed4cf4d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/call_operands.rs",
        sha256: Some("d01e0ed62aa736a7becbe0e7b629fbeb68065104ef065e84db545340dea6ffef"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/scalar_operands.rs",
        sha256: Some("34619ab8594e7a3501d16f6f8a198006a8b61cde689fbdb9d0becb8bdbb199c9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/storage_operands.rs",
        sha256: Some("ab46f3ea920eb6eb557a484455394a6e0e628f644cf59c3f19924ddeba5cf432"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/partial_affine.rs",
        sha256: Some("5f95408606f4039b2380476187038fc6144b7b983fbf96f392c884af163b63b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/primitive_storage.rs",
        sha256: Some("74dd17c143f47ddb114fdad0729bcfcc3b18ffc32d18bce1c45c44570695865c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/proof_recursion.rs",
        sha256: Some("904e3ab8ee2f6ae6e0a7c216eb21bbca54cd2e13bd9274f7f1fa25410f7c31ed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/propositions.rs",
        sha256: Some("7d7e5dd1402590d5f5e06615b9f197ef7e616f2b1b05b35322fe0793729da9ff"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/quotient_correspondence.rs",
        sha256: Some("d38746cbfc4ffdc4f9bb158f4025b97f32265ead709988a28bf94f523fa3e5c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/reach_applications.rs",
        sha256: Some("e0acd8284ce8ee3658357c8769b95f0ebcc6e0f1409bdb7f1078b4f125a0c633"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/record.rs",
        sha256: Some("4b5da6f7f9e4f73fcc86910e5550dd391c30c1cdd6a184f8700f79dffdcff2db"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/references.rs",
        sha256: Some("5fbf50c83f228b96810e511b1ee33f405273a69596804330855f0022e198818c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/root_service_reach.rs",
        sha256: Some("005cb54f507ec1673e86d393e10272a9f8ccd3828c2652e09e00f8b0b84d1674"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_array.rs",
        sha256: Some("13995df6dc7d313fa2586d082f3acf8520c275592f8ae3893123cd44cda1ea64"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_block_invariants.rs",
        sha256: Some("c0be8fdc705ddb753e1a64e38a984cbc838bfcaa9831ad3b6f396fec86f92301"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs",
        sha256: Some("f1e172ab7462ab208a47472d2f08306b27de84fb4464c0159349c95e1d43128a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_qualifications.rs",
        sha256: Some("065634a0943eca3da323073b48b6c571a3c374b851ce0849c66fc94974da43ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields.rs",
        sha256: Some("053c2ad6635f84792b5f8eadb0f3b5d3b5f3b2453ed1dadc185fd3003096f570"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields/freshness.rs",
        sha256: Some("9369634a8fbf7f36ae5444f560aa133fe6f9c50d3f9939a8751d65cda7d327d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_store.rs",
        sha256: Some("72f6330ccd832b7ae26bbc61a9799e19db50287d9c32dc509a6c593db892dd81"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_case_membership.rs",
        sha256: Some("1065970ab4c95aca3fcc1bba9d417bca015ab644a474d4bae9b93a360915e0d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations.rs",
        sha256: Some("e163123726b492059a9cf0586b542e608b6ffdd898e0790ba9132f79b6b75a6d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/boundary_requirements.rs",
        sha256: Some("7a6653b8d8ccd66a3375d2510ee7a7b7212bf3098e8328678f9d7b10193fb736"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/claim_transfers.rs",
        sha256: Some("a873b8e08e5d23d0e681a188acee0a15229cccec462a0da4fd9315429a6ffd58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/claim_transfers/argument_claims.rs",
        sha256: Some("68ea1329212ff72afd68b2667ed447c15c307167053aac2daa828e52b23f4bb8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/claim_transfers/transfer_roster.rs",
        sha256: Some("91ec87d2d81afc5f73e79a80cf75b20dbdff496f2c42e66e6994b2ce404b7eb5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/contract_places.rs",
        sha256: Some("6ed377b5e376d491194319d1043a93a7a44faf85d78eb0192c2657d4c1276693"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/crash_continuations.rs",
        sha256: Some("74aa9e2019abe5ff5d27f61ef1349509cd97569c22791dbbd1e2a871f75066e9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/payloadless_calls.rs",
        sha256: Some("0788c694fed4bd68beb1244e24e352e50e58180dbe3fe2c7347fbdbf7c38d843"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/primitive_calls.rs",
        sha256: Some("75ef96b30e5a8010f2362ced0a81bc7c3241f57aec453d1e02533ba744f922e8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_arguments.rs",
        sha256: Some("0aff9244f6ab4b631b3b48e925cd5a5142bda338ffd2d35e7029653af82c17a8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_arguments/argument_checks.rs",
        sha256: Some("3fce9bee5a1357a73c44b889983eede5106f4dc8668c50a8a169d02b2f634f88"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_arguments/argument_pairs.rs",
        sha256: Some("0c4bc0f9302fd4b11703bef1ab90ee2a1c5ef8cde7b6fb36d0fa9c781ffcab94"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_paths.rs",
        sha256: Some("ed428aa6def211f1fd28926c3966eac49bebf6bfa09d0585bf8cadd4d6feab48"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation.rs",
        sha256: Some("1119392624dabf5fe089c276720253c80397950c937292e155352b7511cc3c61"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/boundary_calls.rs",
        sha256: Some("5246a7bdb8253a3e320da40b9ce2910711756e917726515816d8ac3801d1cc76"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/local_establishments.rs",
        sha256: Some("9b191d73bdb13295f087103208eec626b8d5a81380ad8218ace9ab45267c0268"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/structural_calls.rs",
        sha256: Some("43d51e063b5a0ea1a774b3c60a9ba1031959b51d4d281b198764e20b9bfc8b6e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/unit_calls.rs",
        sha256: Some("dace980b70d02ef498e35849bb36d90d919b8f40e08a37d9e5940e7af934d1e8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_qualification_rosters.rs",
        sha256: Some("1482dcbd37fff0f3dfa1639b83dd5e573e3ecf99b4aab246b616a8cac7c53bab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_result_contracts.rs",
        sha256: Some("418f083b950de4a24cdb6c0dd5b5f4ce5f42a6fb8a2fdfd6e0abad0c9f2050ad"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_scalar_fields.rs",
        sha256: Some("f3a5535c569b434585e1ae3a72e4418bfb5f49558ccd2bb9bd1bd3cd4f8341a8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/suspension_call_plan.rs",
        sha256: Some("9977c8f0211cab9896aa62e3cb39685366fb67b7178b7edb562618caed79f4f9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification.rs",
        sha256: Some("18158e83e0054c9f9c12a912b980e1918901ca5a174e0d24dbbef339267da477"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs",
        sha256: Some("04275621d76dd7d9aaa144f81d114c781aa9c5f95d60ca488159fd9d14f10ddb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/field_snapshots.rs",
        sha256: Some("aee80be2ddcf7bd2f17b6307d13d06c8c790080e9264c2290a7c8a9facb69ff6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/float_meaning_projection.rs",
        sha256: Some("11979334f6365cef694ad3ed1f129d8a45c11e182b67196d5c780469dda48de6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/proof_bundle.rs",
        sha256: Some("b22452c73b177327804ad09dbacb5e9af1fa24015455791970359dd873627851"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction.rs",
        sha256: Some("11c7da845b96a9702cc81675608b70687eed3cab5add2e79147fd4436cba7ac7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("3da1beaa0e2660b440b5adfbfa9c22f13c07e07224bd37dd543e9cc532c62a6b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("f6daefe634f27a23fc3d5e3a5cb2805b7d0bda8d59f8b701c49b53fd3449b0a9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_context.rs",
        sha256: Some("a3fad7e0a7d13510cd5a72e4cb03e2034c172ffd7bf629289de13a617da00f4d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("ca9d49136203367ea342215c665c6e36928aef891b45f10dd99b1fa61323ffea"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("ea34d300ff8354db8ddebaa1c06726b198b74fb7f9195aadca0fe13e236c7a11"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("5cbdf91d88cbaa8cb246d52f6319a406ad899e85cc4f6cfc16bc7a7abd5f08b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("4dfb0d4b9d3a45182ab01d424476c09bbda155856b8a622388bb88315ec0704e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("34fafc6eefafa49c23120bec72ed9c8d6f9e6574334143f224efdd4708571f64"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/scalar_case.rs",
        sha256: Some("27650287aab514589ddd1ea4689f6e05233ddf2f24915045588449021519fa81"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts.rs",
        sha256: Some("63c8699084d143d30c7638fff1a1a76f284138f6fc5326fcbd5c6f14e01e8256"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs",
        sha256: Some("43e69c913ca04fcfbef2b33e89ca7f03f60c9ac460ead112052b96369a803a49"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/discrete.rs",
        sha256: Some("ae495c6aac20ddae2ab1644dd0a538ee9308f330a52345e15a3aa5459aa20bba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/transport.rs",
        sha256: Some("4a88a18a24432b29036ae77af7fae6099f1e58b62103918375113fcfca02f1d1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("cd8ef6a2ebc84d30e737faacfc710d9edd5ea0caef4956ed1aeb7f6f20316d26"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("7242e8f47b7595065f4a5a2ce0435ab85eed9a27f9c8be5ddff960241ea4153c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("edb1bcec78353cd0bde2cf21f19837941a065e4cb0b9ed6d265d318affdf39c8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/substitution.rs",
        sha256: Some("250b9962f16c45d7a290e9c36cf3b83d4c98f9bdeb8e3aeeb36b9226f4ebfdf2"),
        inventory_machinery: false,
    },
];
