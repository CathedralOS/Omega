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
        path: "omega-rust/psi/semantics/proof-admission/src/certificate_search/integer_order/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/certificate_search/integer_order.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/certificate_search/order_chain/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/certificate_search/order_chain.rs",
        module: "tests",
    },
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
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/forbidden_roots/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/forbidden_roots.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/multiplication/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/multiplication.rs",
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
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests/spine_chains.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        module: "spine_chains",
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
        sha256: Some("75ecc9dc5a2f553888617d36540bde58910028bbfbfa21287e420f0be4d7102f"),
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
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/crash.rs",
        sha256: Some("9c35ab7abd35ba261d9ff45f53cf14626e1b95ed2b28421305fb5c58d94c902e"),
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
        sha256: Some("5054c2cde726ea3a749d6438b312c2d6c451351da538077be936ddbca5917ccb"),
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
        path: "omega-rust/psi/semantics/proof-admission/src/certificate_search.rs",
        sha256: Some("9d5642235dd9c82d5bae753df16f159a1d1192b96f80005956445747d10b8ce4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/certificate_search/integer_order.rs",
        sha256: Some("51b50058cade8d80523241f61553fcc9c53eb73cdb6042b8496792d14c2df583"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/certificate_search/order_chain.rs",
        sha256: Some("bd658790de8dfb903cd0deead19988928c46f4c86c98f138be1583d97145d8b3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/classicality.rs",
        sha256: Some("9a77ad9db24f33e789f7f61063c7eb51fb8e96017bef8822736bd00d34b733f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules.rs",
        sha256: Some("6d8079c98bcbca0d308070dc28c03727085854fb4e0e6764a6aa9232091f6645"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/closed_integer.rs",
        sha256: Some("ff8aadab21e62ea1837c18d3691735f3c865cb8b4563b61adb3d363d8fadba5d"),
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
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/open_terms.rs",
        sha256: Some("7b10e43e0c93548adea949f77e07a792233da91a829de42e7c1eefa77c4c854c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        sha256: Some("121fd7ce05df9b372ee1dd2afd92177eee57722d3e78d48c7b47d2bcbe670196"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/lib.rs",
        sha256: Some("4c0009ac38b7faafb48271624040d1eefba152b59abe7204381909774fd7a8f9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        sha256: Some("ceba4d99eedd93996fd37b9b99065b5d360c7f8abeb3304cf593dda20d41e571"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("47f9ebe93645132cafd4ba20bb8d5ee501559095e4bf9cdc55164aeef3666165"),
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
        sha256: Some("3f1b9560bd97d51dc83d7fa02336b66dcea6168248040a1c964eb33a66653c87"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/casts.rs",
        sha256: Some("3369b23d9c23278f0e4ae15e7f677deeeff106d6c96d28f02db7e9d73ade4031"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/equality_transport.rs",
        sha256: Some("bdbec257dd9a2677537acecf3be2d0adaa3812c8c816a85fc81e3dc9f10f7712"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/forbidden_roots.rs",
        sha256: Some("dacf0587970cdc70480c99ee128a2526ccefd1d6756df90530c265a89ab978df"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/integer_operations.rs",
        sha256: Some("b65519fcf58ccd9bf4f3b9d332f6ae6664bbc780a8779b3aa5833bdfbb1943d7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation/multiplication.rs",
        sha256: Some("0470674b87e7def74c607017933abede6379a8cb6442926f94e5e7bf7c2db35c"),
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
        sha256: Some("dfa4f7cdb006aa7a3bd1ecd8362e87edd1baec9716788ef79970952e5b87bff5"),
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
        sha256: Some("7c1572f9305268dfbd8d3c29eacde03cadfa379e93c6e47a37224671dafe5be7"),
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
        sha256: Some("24a8101d0f08bfcf04c3848fe90188eaa4d7fe850332cb855a80c9a0b6c3a84b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/equality_rules.rs",
        sha256: Some("3b68873f5389268867075165cd59365713077a37c620e0543c1eb14f2484d7b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/integer_bound_rules.rs",
        sha256: Some("3d3dc1d4a56b87f42f798a0abc1435adec098e866affb8ceb621e611afb733c7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/integer_math_normalization.rs",
        sha256: Some("8815fa51e8802c60e0804e20f31ed88d3b272a3f49b4ece358c5e7cd1d535ee0"),
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
        sha256: Some("34900cf3863a1ac5ff9c48fcdda85af05e5871ff07ce329e819134cfd052ab94"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("fce52ef7b83500118a4304ac6a30ace83105f3b32ec4447be0453d275074e3e5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/lib.rs",
        sha256: Some("d416c1893fa1958a32a008b7640f551e0eb5acf685c8e7a9d5fe3d2d5e1ab2c4"),
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
        sha256: Some("cd70b9869f06581cb53763ff8d5d2605b50e372176c330140b5aff6c4244fc33"),
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
        sha256: Some("c9a58431df26f82cd10981959d75e193605c0239458a3b704b56ef17b222e5ed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/static_path.rs",
        sha256: Some("34a480db3c4c2743351c8b812e6c28ad074f6ac57096dd1079f7eb05f0207191"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        sha256: Some("5a29c8f1b0d6f7f6e50e3d2a6d1aa50d4db4e18527e656476aff8316b18c345e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("034f50051513a8fb48cb751b5b6874d8ddda30d9ff771c854ad3ae399cd33b9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/element_extent.rs",
        sha256: Some("0260134d2520f9c10b9dfe68615bc9cec1024a6f9cbfc87252d48a8ac6e2b9e9"),
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
        sha256: Some("49c60170b5977126d23dbaf96934d41701ecadef97a0018f6d68f66641c59b02"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/lib.rs",
        sha256: Some("975b293ed0251b98bd68707fc2b8c0eb416dc810132c3e573b14c1ce55d46d3d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization.rs",
        sha256: Some("e62c409f0f043c2aa81f90e8b35bc70df3ed2cdf77e4ba6d8569213bed3acef1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/control_flow_cleanup.rs",
        sha256: Some("543fffc93a54f50d2715769a1bc9ff8e3a38173ad5c249f5425095d6c928b0d2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/copy_propagation.rs",
        sha256: Some("a688617c2bdfcc79802f0ad9a177fd49a6866a8156e7dbfaf93421f0f827acae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/dead_scalar_elimination.rs",
        sha256: Some("3f8ca9145e4f956f64ea98ffd8b8da83d8b294fa4fde1baa80aa4c68b7f369d5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/global_value_numbering.rs",
        sha256: Some("e8b23437f9c02ea19c76b75a3bcc2884d055f888e296047ef726c620528f1791"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/proof_check_elision.rs",
        sha256: Some("4c571facdbb94b47ddab485aacde528d014c64486d06da66f50c6c4e61939320"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/sparse_conditional_constant_propagation.rs",
        sha256: Some("11b7c462eece45646e933a390804c8d35b9a95ec64ccee0626e47d565e1e8554"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/proof_recursion.rs",
        sha256: Some("d2a8b7d21c2a768be1ea7d16187891d2e0b41de1c833c0662538c73c4125544a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/quotient_correspondence.rs",
        sha256: Some("8efde6290b40a73898fd883a03a8b54532439b956628b62ce5f323c52cdc0acf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/terminal_trace_v1.rs",
        sha256: Some("643b34c9032cafe417ef2115f54b1140e12aaa24291808d2e99a902547dcb404"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/procedural.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs",
        sha256: None,
        inventory_machinery: true,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence.rs",
        sha256: Some("b303d7b8c7778f1b5da1544c96a68f842b16d536f5b50d5694df3e6b4c9c02c2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/element_view.rs",
        sha256: Some("fa0a6c389dfc4d708421633d287b0544626baa3bd74dba2faaa6d70caa869002"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar.rs",
        sha256: Some("b2f3b1e1ffb54587cd2be866badbf6a97d9d155213ae753dcd2be614db534ba3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural.rs",
        sha256: Some("8397df1c363c274ff08254ac6ebed33ba8e50e57da7f1f1678f3ab8dce9de29d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation.rs",
        sha256: Some("6cb3b8134e0c2f919bdff17a657de6a5a4ae2f8e36922752bf35ef418bc6280e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup.rs",
        sha256: Some("7fa11d57cac5ab3b4e246aa71a475e17e6c48e01ae8040e3c98267f15a6ac79f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("73fef6c9b939972f8fb11ae1453c79c7eac38f0ffd874f0a231aa6c9645b3c5d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/block_views.rs",
        sha256: Some("61f83a18ea4c8db87dc56d3bf99bc1cd59a209336796ca488afa7aa4160ff648"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/borrowed_windows.rs",
        sha256: Some("bf0effebc1baaca421e6fd2e704373c5a525831708d87ee68dd77c975f4b234a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence/length.rs",
        sha256: Some("7d348ccc8cd91463a3b3978bf91e4dedcbe47a5cff2dade5c0a14016585b4a46"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence/read.rs",
        sha256: Some("090b5e667d020cd8eb9a1f3cf8eaf524fba272dccd3da5d884c06747b837a8ad"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence/subslice.rs",
        sha256: Some("2a1e73a9d9fdb08cb69c94d020979dcdaa7b2cd2c3d677ce59d96b8f1706d192"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/element_view/establishment.rs",
        sha256: Some("4eefd2e955dae458dd773b8684936f4f03136b551e1059751cd427e0640a6711"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/element_view/length.rs",
        sha256: Some("5880d883103360455f37fdb2ed7569e29a7bb525933385a968e6c6c50321d29e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/element_view/read.rs",
        sha256: Some("bfa09f53e3e069101ee47ee50701dd5bd8bb952eefc9ba4c724c933de7eb0b80"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/element_view/subslice.rs",
        sha256: Some("b9a9d3fb88add2c73c845d3f3ae0f7888191d886b3954c9b92bec632df803d45"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence/write.rs",
        sha256: Some("ac4fb1f2b80570b9d16e8e55bd4ba1431135165df17c5565301090016c8a0ddc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/call_graph.rs",
        sha256: Some("eedcb4e24799672963155fce929c140829d92603e972bcc9f00bcd130b6d0194"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/conformance_applications.rs",
        sha256: Some("7e3615f1fac758826ae18604a14f7480205849dab7d79d6971f1dccd6d2d9450"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/content.rs",
        sha256: Some("010fb22874355d7e69b9608af50295d9b3e65cca0ebc2c1c561a2762403f1139"),
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
        sha256: Some("a5607738b3de70b37d086eb12d6f48143b1965aa12e24caeeea6e297588f3829"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/block_graph.rs",
        sha256: Some("de7ceca3b2c423051e92ee251c4b82fb58846111ad435873fe83e5ef932c749c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/definitions.rs",
        sha256: Some("0a6c8b43bd4f1a1978e550d392dc3d98d625120ce8dab91e64411b46fcfcdee5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("0cdc6ffa6056998c110c3098f09d6629f709e415f2b74f439c61692aad6105b3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash.rs",
        sha256: Some("44744370edf941efaf1b0318e68fe3fbd84d20988050f7b29eb86b32c985c485"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/operation_contracts.rs",
        sha256: Some("16dc7edfed6d8b28bb3e77ce087c31c9e450d9af890e59469046d4cc02343c24"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/outcome.rs",
        sha256: Some("e1873c66155a04aef909837db2a15407c033080ffa270a1951545411a65a9626"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch.rs",
        sha256: Some("b46221b69e52505cd853d7e651cb715db4e5ddde03a3562399c199ba219fca62"),
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
        sha256: Some("bae7156bd1bb89af28e03d48ba0f5bba1b4d1b64ce8ed175dd0557de26b2f480"),
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
        sha256: Some("0effa947ac4e31f31fd699ac14bea2fb8fe27038cfa8c3b17173f9b62fa033e4"),
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
        sha256: Some("e642eff7548136c1176ecbe7b205fe41737102e66072072c04dbdbd04aa4a05d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation.rs",
        sha256: Some("9c1aa277c0d1c51c19f6af43f840c5fbeb50fcae089f6a06154731064ad25cef"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/boundary_machines.rs",
        sha256: Some("49ff8f277abc5e6866f59ee4e9c08e356f344df048dd4e2e25ab4543b9519116"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/domains.rs",
        sha256: Some("d50e0c3b458d2ea997db6d4d111b5830da5f1af42c019cb423817848f219f7b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/machine_foundations.rs",
        sha256: Some("f2c0fbe2b713b5db06e47b1d4e40ab846f6624eac01be9850b72720bd56b3493"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/provider_candidates.rs",
        sha256: Some("1251f4e0277a224e6eff4d3d529cad57f63d847f85a12853cbe867b2152813a3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/provider_result.rs",
        sha256: Some("0035f09a584b474d413d8d3ff09fb85b90e3c777e6a0870dddfefed4a828ce31"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/services.rs",
        sha256: Some("05400709dbd93216f90fc66f210873e5124b70b6aed5c0c5868f678852bb16f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/structural_types.rs",
        sha256: Some("92c503440a7258620f1dded871a278d60eaffbc57bdfe8d93058758ee5e8edec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier.rs",
        sha256: Some("16e011fd051f0498268789cc2c64259507870646996cfd9fbc036d01743bd4f0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_entry.rs",
        sha256: Some("0d625f4c19b2f7b1f9ed66b31422008d39be29084206641cfdc44b0afea9584b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_parameters.rs",
        sha256: Some("e771df817ebb71c40277d6ecdbef8b12a3868eff810330b8253ffa1930482359"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/operations.rs",
        sha256: Some("dc579802e10a2edb450051086c2fc4fa4ce0828228c00d147134e03cdec68f00"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/terminators.rs",
        sha256: Some("1c188c7630c18208507bd2b9d6f07b215c7373c9fc20faf6cd1abeb0a3a7b24f"),
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
        sha256: Some("f17ac1eae00ae6f0d288e3c28540522370b5752ced8f8f5d2ea6e5b87734890d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("c71b09fbf37abe6263e167d3350b3a3536c49357b2d16a1219e4e51dda721b09"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/structural_places.rs",
        sha256: Some("4cb5e6f1c03e58a323cdbe90e3c00adaf8504459f14faca3265438d573ffd266"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations.rs",
        sha256: Some("f0e55de4f8207bfda31d1b5c0384aa64976277cde781c0c354e951087cc5cb8e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/arithmetic_operands.rs",
        sha256: Some("bd5a5c1c1d92e91619efd02da2cfe8fbadc578de88e01ac05a1ee6493811dc33"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/call_operands.rs",
        sha256: Some("54b956e1f40ee496b88c841d8ccebe43637b0db70ea2cc70a97a72ae5a79c396"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/scalar_operands.rs",
        sha256: Some("34619ab8594e7a3501d16f6f8a198006a8b61cde689fbdb9d0becb8bdbb199c9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations/storage_operands.rs",
        sha256: Some("c031232f8ef7b5256f9f9a077888512bb9b89a02265040791c874e4f4cdfd09d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/partial_affine.rs",
        sha256: Some("995c9a5addb82f595a6aaa6445be79dd263844ce30b9f666a87fc0b993ae7b50"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/primitive_storage.rs",
        sha256: Some("7f2a86dd325f40827a629efa724e4a3a9be6736dcaa214fc2f5497525615b9e7"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/qualification_establishments.rs",
        sha256: Some("62f7be66518e5632db34ce8d0e600fe2b0a6ef17c91217305aeaf7f91059c3c3"),
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
        sha256: Some("1b826c56e5d02ac18f7ec41fe85084efd535142e089c05f9cf4a912977d0c835"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/references.rs",
        sha256: Some("f3c8eef035418db9938505d269c0a685523328355295bea604f8192dfad74e17"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/root_service_reach.rs",
        sha256: Some("6d02331dcb042968cec2b4788b16fc9e8984b3c6cd85cd45146a55c48e782a35"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar/array.rs",
        sha256: Some("8ff9a69a87488ab7cfb5b258f2a6b3e970185360de714c393dd5817ce4935bcd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar/block_invariants.rs",
        sha256: Some("bc7a4c34a57f24787b04b96b3857074279539ad3e1c1212f84f453dd7bfc8cba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar/case.rs",
        sha256: Some("437b5882dc9bcaaa694ffd45d0e6ba3badb51c8fc4d032aa01eec4e90e1d7a29"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar/qualifications.rs",
        sha256: Some("044eb80a052f0ae11c67c2f2b89c6410113f960dc3241bfc106c533f189cc1a5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/byte_sequence_fields.rs",
        sha256: Some("a373ac6caa4f154017fcf4c453e3b1b55380957ac76c4b978c415733a122e07c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/byte_sequence_fields/freshness.rs",
        sha256: Some("288bae7117eec2f0789dd0a4cd8d71d68e053e7f837848f915bed0116778248c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/byte_sequence_store.rs",
        sha256: Some("a5a711c671e5f84fc46c5dcc57de2f9a5d1c6b0c4bedef6a78b6503a068fc97f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/case.rs",
        sha256: Some("7c842bd12d74a955c636dc1903e6f7ca16a09b2fc73e1aaa6b7caee022095832"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/case_membership.rs",
        sha256: Some("556205b9199b07cf999c01485dd0745d44b0c0ecc09baa1a1b5058ee110ca4cf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/leaf_copy.rs",
        sha256: Some("d5aea82ba5852dc7d0fda2523cb2a3ec99d146c536b570bd79ba94bd7c8d130f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations.rs",
        sha256: Some("3752b9e4162a4d4c22ee2a5bda136ec7fed41cc6c346db9cff9e98cbd12bc890"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/boundary_requirements.rs",
        sha256: Some("271842c2de2f78024f7b143b8d4527aeab937c2d60bc78f3810d75cd720b877b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/claim_transfers.rs",
        sha256: Some("a873b8e08e5d23d0e681a188acee0a15229cccec462a0da4fd9315429a6ffd58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/claim_transfers/argument_claims.rs",
        sha256: Some("34bb5a209cd990213b4c0b1d53c3f9d23fc819cb21f90c7213c21c791497aef3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/claim_transfers/transfer_roster.rs",
        sha256: Some("347ca6627f90acedb65cab1ead6620f9f512a0f3427c85a625d3903cf0e16518"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/contract_places.rs",
        sha256: Some("6ed377b5e376d491194319d1043a93a7a44faf85d78eb0192c2657d4c1276693"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/crash_continuations.rs",
        sha256: Some("e2e5757f1afdbf7a1e73547a5e5d81b8011757713d677c05d4c3ccfe30631ebf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/payloadless_calls.rs",
        sha256: Some("a90ba7c809c19ccb04ed5152726b97c36a898e0a8e926e613769a6f5598cb2e8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/primitive_calls.rs",
        sha256: Some("85f1fca5fbd1af4953ae061860b5ced88124e0bcf167cd7fbbdfac86fc18a220"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/structural_arguments.rs",
        sha256: Some("70ff79cbe4ed70c52e3ec334c04273d0d169ece48f590d4153b8b7bded9a36af"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/structural_arguments/argument_checks.rs",
        sha256: Some("0da402f916463b216f4b9ae6355bda6924d2d91db8f820eed8a6122bca96ffab"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/structural_arguments/argument_pairs.rs",
        sha256: Some("8db11f543741abc4bf5cd318c087e427322d5645d5e81ac960f4322763ce8048"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/structural_paths.rs",
        sha256: Some("c92c8bd90686460366a06ea2e5becc9e6af1bb23e96adddafaa905e47fbd2f03"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/unit_operation.rs",
        sha256: Some("14e771371a81a8263cd4aed30d2279bbbef5076bc50c21a89290830858d3f2fd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/unit_operation/boundary_calls.rs",
        sha256: Some("03b97a2f67541fe3a1497385a4b2fb245d07c2ddd59766e39b613e9d2ec3b65b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/unit_operation/local_establishments.rs",
        sha256: Some("6675f5736ee5bd6b58ab527d965ffc7f71699047a4b0c9976c5d2ea2ca8de580"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/unit_operation/structural_calls.rs",
        sha256: Some("6eb983d29a0e259a35a2c7fb245f6bcd6292b96e715920798f1b9c613da39e55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/operations/unit_operation/unit_calls.rs",
        sha256: Some("e078c0dd02a28b1a57856e321f4c31037a45a017db7af0f344e93b15564df9fd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/qualification_rosters.rs",
        sha256: Some("e478350a585aae369594ef12151a92415967b6182ab6ee4ffb8f9ab89388748e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/result_contracts.rs",
        sha256: Some("bd512bd6d65765e23e4df29c215a6243624f5baf64473be1cfd21b508929bf48"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural/scalar_fields.rs",
        sha256: Some("622f9cbb3b4704447bd3580462c027ae9b7a9092e42a6ee1f3ef8088007a3fe5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/suspension_call_plan.rs",
        sha256: Some("f1391d944eed60f6301f3bdcd9f8f25a8b9201d2f02b8388e337b7c61b6f05e7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification.rs",
        sha256: Some("fb8c1c2c24f290cd489a7751d2798f25f34bf048c4de8c5ffbf704c786e76c13"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs",
        sha256: Some("065b1e875a07be23c5d37b7a358a642f5e63715caf9832dc313c69d5e9ebfdf7"),
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
        sha256: Some("3256b1bcd5d955e31099db6d4873ec4755719706d05dbbe174d62b1525524ae0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction.rs",
        sha256: Some("0f532b941bb0f9e5b82ca0da2bf99f75fb8955960a4686db27dfdfad379d203d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("3da1beaa0e2660b440b5adfbfa9c22f13c07e07224bd37dd543e9cc532c62a6b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_obligations.rs",
        sha256: Some("a46a117bf4194244abffd7b550f4693c46eb9bc713ffd4e716a310358d3e05b3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("f6daefe634f27a23fc3d5e3a5cb2805b7d0bda8d59f8b701c49b53fd3449b0a9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_context.rs",
        sha256: Some("1a4998826e7f6ba5c9dc4c7ad4835ac0065bf056a1de00f159a361ff838a10a3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("ca9d49136203367ea342215c665c6e36928aef891b45f10dd99b1fa61323ffea"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("4490ec5dea66d423b53fbe4730326e7dc03f25e08755d6faab850c3e150736fa"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/element_extent.rs",
        sha256: Some("56bb08a48f6ddc3bfe2b34223581e3b9e146f2c7297ec466f3d4444c999ffe11"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("dcdd8d4b18d69cd7cd13bc6147cf1cb8981dc854743e87c6f404dd9746507aed"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/scalar_case.rs",
        sha256: Some("45d1ca39b33bb223b1d2578cb2db540bc81df988f37e2b167b900b5548702db9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/structural_case.rs",
        sha256: Some("dc1b27f8cc7c190c5fc8510fd903a8c2ddc66189c016c61585c7d2449ffb6771"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts.rs",
        sha256: Some("38c26cf05d6054836ec848bae50dde9c6029addc81d11751304be1de86bff765"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs",
        sha256: Some("6513d000c74a4402892d9be6da876728f739e04a8147ff28b1de9bb0b0da5ff8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/discrete.rs",
        sha256: Some("ae495c6aac20ddae2ab1644dd0a538ee9308f330a52345e15a3aa5459aa20bba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/transport.rs",
        sha256: Some("9dcef94f8db0e64301beaf37bd173d48cd80c9a7b3f39fb73e2a60f6e2bbfc94"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("5ddc85f1d537dcc413d8f08bd68d96ae15343b07ca3e0d69936b2a26095b933d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("7242e8f47b7595065f4a5a2ce0435ab85eed9a27f9c8be5ddff960241ea4153c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("cba5276405261cb787e144bbe99fc5873bedb6a1b443b54bced205f4a02dbd4c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/substitution.rs",
        sha256: Some("250b9962f16c45d7a290e9c36cf3b83d4c98f9bdeb8e3aeeb36b9226f4ebfdf2"),
        inventory_machinery: false,
    },
];
