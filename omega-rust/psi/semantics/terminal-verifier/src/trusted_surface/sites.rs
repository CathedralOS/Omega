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
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs",
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
        sha256: Some("713cf0c4ec70825470e89d69e30ee919c908aa6d337651121fb6af46abbce865"),
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
        sha256: Some("90d2719d6f828b10c0ddafbd5d85231575add9c83d52261dfc66dbd4d9d5c0b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("f2422ab3e127185d43d7022136080c0edb10441f6b5c82b0af829e65303be84c"),
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
        sha256: Some("63a9c0fb4aa3775932cb62280fcd192417b94b4e9a871252b3803332ba078f73"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/truth_bounds.rs",
        sha256: Some("af5a8073e8df73ebf30b74f41813def41f20c2463f5cb04a1914f2733d8ea4ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine/witness_checking.rs",
        sha256: Some("e88edd836ac01ccce449ecf88ebbf1c6ee03bf17074474ed00bd4bea11cebf9f"),
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
        sha256: Some("da17575482db2750ac6ea68d86a04eae0887e7e0bf14e79bd75f5944294b7dbd"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        sha256: Some("ceba4d99eedd93996fd37b9b99065b5d360c7f8abeb3304cf593dda20d41e571"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/bounded_denotation.rs",
        sha256: Some("cd419175acaa3fc28fc66d2e989c72140104f22c2fe466b9e788626561a1a111"),
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
        sha256: Some("edd56e37d18f9d290f194a435cb59542d11a507a4b6ef4a1abe5a5b3378c9861"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/value_equalities.rs",
        sha256: Some("2613c36df908d0abe1c7882c6b7fdfc7bf3fa96df19a24b0ff582dd6922533a7"),
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
        sha256: Some("fc1d4dc4a445e5e9ddd0812e8cdbc185118c37dc8ad08180f5300b340c9ec5f4"),
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
        sha256: Some("6bd093efffde86670241a69982293518200211fcfa106b937d652de3b3e2cd03"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("bc2f019bd0576ef23eb79a03a32dce6ba92b900bf80b5d34cc89e97c28782990"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/lib.rs",
        sha256: Some("73f110b783784e23fff9cc6649bdeab73d86045cabdcaffd984ae75842e578b1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/primitive_place.rs",
        sha256: Some("7ac4fbf9f6ee4ab6fd996a20ca3815e87bb635f2ccd957cd553adf14f663d582"),
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
        sha256: Some("7a1f12e4b8d367402fd2f58c0ad69da9d4cfc73bf66e8e1fc783fb42ee58b8e7"),
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
        sha256: Some("05b707bbf70a981c45c308851b4eac705f655a5d8c4a8ecdcea7529e3d8d4434"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/reconstruction.rs",
        sha256: Some("b0c5ed7d173f809849986fd31672b2a628e97e1ca22be6a5fc8be936f6edb494"),
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
        sha256: Some("cc786efaea89ea59f9eaa3836c4b9726b2b293cfca5a843704cc86c3f349bde9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization.rs",
        sha256: Some("7c18f23862c53fda108e4df2885838c50bd2533a639f12c148e7b5e3299737e2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/control_flow_cleanup.rs",
        sha256: Some("ba6a5955217686038a68a81247204d59c4e5e8150fad3d631d666fad6756810b"),
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
        sha256: Some("d5961335f363246038b81ddee881393866f790163a07c4ce7ad1eb68deb06040"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/proof_recursion.rs",
        sha256: Some("d2a8b7d21c2a768be1ea7d16187891d2e0b41de1c833c0662538c73c4125544a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/quotient_correspondence.rs",
        sha256: Some("b3831a85825a232a9fb0c9c975761062fed754e81444dc742a4c064fefa329be"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/terminal_trace_v1.rs",
        sha256: Some("bf6c62b2d5175d338c5aff13a5a97df55fbeb4e32f732c9d6175780306a2fd2e"),
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
        sha256: Some("82fd7a54ec957f5a1743a6fcf88f1962951cef660f57f61730a1db5fc7f4b9e2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup.rs",
        sha256: Some("ade803df6001b9d2b1f2de682433cfe25fbd3cb9fb19db24e3d5eb84553cccea"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("6ed69068a29009c7dd27fba37c4e642d63e0dc4adf39220c613171052cd30d50"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/block_views.rs",
        sha256: Some("4d6b149d0fcbde2a714613e0f1c618da1517cef499a354b4feee50614a476480"),
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
        sha256: Some("f474f3aaac0f610c5bb55a75fb7e01b9e0cfc4a3928a3a16b2d702fa03a192a1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/contracts.rs",
        sha256: Some("6b1ed8473522384eee204414c0e925c3a967ead0930d4fd5f5bea8f9afae8f59"),
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
        sha256: Some("f621483df3c133723fa1e91d5bc052125df93b4d2bec2897367d54d302836101"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash.rs",
        sha256: Some("86ca7a98bb26312e60218a9382fcc3c0cf53f4d9eb42ec22e2e952d77c309d24"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements.rs",
        sha256: Some("584e663466459c99b38585bd16b07e6946c99a2e53b2fdeb4f0bfcb0bead6435"),
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
        sha256: Some("90108bef438bc61f9d469753b3765f7735c06c7f2367aa6011dcd744c0c4e194"),
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
        sha256: Some("27694de40c637b24adc58dd58fc7db74b644dc20d1fe0891c2e08198c8048ab8"),
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
        sha256: Some("43a34861d563773b9fc6ab5ee88be65a08e1ef6b3bf830b6723957da9f527767"),
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
        sha256: Some("ba4871fdb21231a9df2e3238ba51f77d703ee3a6ce634b2fd4590265a5dca16f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation.rs",
        sha256: Some("d2cfed1ac4f8462b59e4e38ca3822d56702f8b364a84a0a64bd71a59ce95ccae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/boundary_machines.rs",
        sha256: Some("00f33db9ce2de8b06f4a45e1b8d5f5fb2df77da46ec8fc4192374ef9a649941a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/domains.rs",
        sha256: Some("b98ff5da9e87a862297dffd5098c5d77a04c41ba2f6c6a18b9c9dfbc5f24ad03"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/machine_foundations.rs",
        sha256: Some("28e1cb58b82bab13d0e22f57426620f0904298b36ab996851499e2a25f924425"),
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
        sha256: Some("901ce6bb3f9a9d2cdaa1ba6f19be2431b881a23cdecd600c27c0a331b3cbd34e"),
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
        sha256: Some("f89d7f0d91addf070f1187e2660770c7173e8ba08c2b61a933cd68cc0be495da"),
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
        sha256: Some("08463a2bd9f3c1e555cda04616e611ae62ed9510f201f61c039d4d8e4c06e982"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine/scalar_result_operations.rs",
        sha256: Some("ddb3a1bb55f43eebc0f6a04e222c887fc67c108a0174f773a0692933a6357059"),
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
        sha256: Some("33860aae98931eea783c980fddbe0b7993e21f0536877bbaf840cf1e93fbd763"),
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
        sha256: Some("901e58b2555fc41101e07cc50dabef7131d7ac50aeab92850c1411dc0341119f"),
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
        sha256: Some("c3b33a583a3207e072c27da09a144546dc85057dd179f9a6ceb425096b6a104e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs",
        sha256: Some("f1e172ab7462ab208a47472d2f08306b27de84fb4464c0159349c95e1d43128a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_qualifications.rs",
        sha256: Some("eb068d71a68bdf15633ca8d54f5e6c6f2bd6609063eb8f97d7567e9c1aa283a9"),
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
        sha256: Some("1bc966901eaa07f1b1164cc2e11d92ed0228c7781a18526c4539225024ea0349"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/boundary_requirements.rs",
        sha256: Some("0b17ca77f69a1c9c7f8bd186b4cf12a47ef52dc37d1813cfab097d5370b667e9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/claim_transfers.rs",
        sha256: Some("a873b8e08e5d23d0e681a188acee0a15229cccec462a0da4fd9315429a6ffd58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/claim_transfers/argument_claims.rs",
        sha256: Some("fe405226ca470408d835a01003b0257bc194819ed7bb341502241af56bfed0ce"),
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
        sha256: Some("b533950faa4e04abb2402c349f7cc057d2722921ebf7d5d4876ec2eef6262e78"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_arguments.rs",
        sha256: Some("de8725e77cd57a9f8c66ca5111307356536ed992f0beb8f0e798ce97e0ca809d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_arguments/argument_checks.rs",
        sha256: Some("44ebfeb5b5b5997a3fe621d9eb9b0b4f2ea5bdfdfe5fcd41ec716a9b7704c266"),
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
        sha256: Some("1b73c13cbd85a30e20a4d65610916f7cd4dbb37d1fde907de81c56951592c3c0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/boundary_calls.rs",
        sha256: Some("69657e8ebf4b09b9ceaba6ba03fe339b3e88ac6face6a2c29de07640e7141edf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/local_establishments.rs",
        sha256: Some("92215dbf024ec226bab353a713512294d028faa8fbc8544776dd5fe2cc7da53a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/structural_calls.rs",
        sha256: Some("c9135b1a61840ff963b0ed44a2970ad2ea1aa1c0bb927d9767d8c4a99bc8abb0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/unit_operation/unit_calls.rs",
        sha256: Some("e7074ca1703c3aa9ba584baa6b4f7c7f5ce5d804fd7468d0089f0af5b9df1f04"),
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
        sha256: Some("9542a2984ffdaf0d0cbe76a1aa9d342a0fd9d33d6c46de1564dd1c9ad12e675f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/proof_bundle.rs",
        sha256: Some("b22452c73b177327804ad09dbacb5e9af1fa24015455791970359dd873627851"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction.rs",
        sha256: Some("71368683c576bde3eb12c64453d3676df33c801c58362d5a369cfec52a0e0d82"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("9606a927766c81d3a199a411aec2ea2d07212fc6d63a061413130976491cf5ba"),
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
        sha256: Some("7dc63ba06eeb03fe539fad1cc909142c6d57f826e473180bc73a312116a0ae04"),
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
        sha256: Some("e7afbbf974395bcaa94cbd4a888bfa3ccd30f240ce801db919d39ae56bf1aa2d"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("cd8ef6a2ebc84d30e737faacfc710d9edd5ea0caef4956ed1aeb7f6f20316d26"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("01b2a9e5b7f280102609650a147e018bfe7c8b379a993c79c88241196f3fd365"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("a7f5710978f3fad95f7ba1997a0842b611825b97659b8fe956d99b9f276abbe8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/substitution.rs",
        sha256: Some("a0cb5682494b53de6ad47aed569f38504a3136ef079132bd96e8f542742ef46e"),
        inventory_machinery: false,
    },
];
