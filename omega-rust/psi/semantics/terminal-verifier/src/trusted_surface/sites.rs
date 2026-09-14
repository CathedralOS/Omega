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
//! test-gated so production code cannot hide behind the label.

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
    /// Parent module file that must declare `#[cfg(test)] mod <module>;`.
    pub parent: &'static str,
    /// The declared module name (file stem).
    pub module: &'static str,
}

pub static TEST_ONLY_SOURCES: &[TestOnlySource] = &[
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/kernel/carrier_bounds.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        module: "carrier_bounds",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/disjunction_elimination.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        module: "disjunction_elimination",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_substitution_tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof.rs",
        module: "order_substitution_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation/value_equality_tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/predicate_denotation.rs",
        module: "value_equality_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity/tests.rs",
        parent: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity.rs",
        module: "tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/subslice_tests.rs",
        parent: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        module: "subslice_tests",
    },
    TestOnlySource {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/case_membership_tests.rs",
        parent: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        module: "case_membership_tests",
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
];

pub static IMPLEMENTATION_SITES: &[ImplementationSite] = &[
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/content.rs",
        sha256: Some("3d6cf79eb08bdb36cf8e176afdb07f4091b2797a3b03148f0506c985b646f1ce"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs",
        sha256: Some("20c782eb868b50e8814b25c2f426c9de80d4d835498c84f2b8de82a7eb025734"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/admission.rs",
        sha256: Some("959e4d584bd1b918ee925076705bc72d5407fbb8df4126eb7952d2b246dd4883"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/nodes.rs",
        sha256: Some("c37259271072c53fc71c218763db83d498572c15b76fb1812f1fcc5564eda74f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/artifacts/proof_bundle/witnesses.rs",
        sha256: Some("e84d2df5d97be728a4e94d2a4cb008448ac2196ebf64ab41a1a4f2e47d393dc9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/operations.rs",
        sha256: Some("4709b6af738b8bd36c7603e68282df005452ca805c039415c85142b535840cf2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("63f58da3db974b534cb9b4ca1aad799052d8c15889baf2a1bbef03a0f3223bf1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/closed_integer.rs",
        sha256: Some("c19f178676771feafd841838972e0f45dd708fa7ecbd902fdb0ec3e6408dcb1b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/evidence.rs",
        sha256: Some("eaf956334cbf67463703a460516f70c636111d629de4ea7221fef67fd6f992b1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_affine.rs",
        sha256: Some("4d1daf570f3e819164530f5d10f26567f0ac2744d4892a6031b7325e61f316af"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_cast.rs",
        sha256: Some("da16b64dc48f83ac905597d2ecb4f66964e42db204837ba099a4d469c28b438d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_forbidden_root.rs",
        sha256: Some("c2a54fde00c12eb8dc0b824243d7837237ae994210103e2f2601a019094c925b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_shift.rs",
        sha256: Some("03159712862bbd147506a6b9d866209744d170e0ac12c044fd7104773002ae55"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/kernel.rs",
        sha256: Some("b155ab535c2a473bc7210d02667c0b45fe4a5e0283ccdcc70b8c6115a21b04de"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/lib.rs",
        sha256: Some("6523fbb5afe1055157f2ccf45569543216b1648848b7bbed21ec262602021b04"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        sha256: Some("9ef274dfeb26aec4c343199bad558c66f85cf527e9ef5b7b372a7531415f29f9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/certificate.rs",
        sha256: Some("db0780c95f66c46104005c71f57502efd128377427e690123760a191caeb78ac"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/conversion.rs",
        sha256: Some("49df2739b9f3ec533ad26c072763778c3f0066b916b8ed36d20d46c50d3b2f58"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/substitution.rs",
        sha256: Some("a223759358d4fd9fd6be1fac329acdf5a00d3de64cf54f6204f789d029a15e12"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/term.rs",
        sha256: Some("2710d572d20e07bec9c34942c5242720432d522af4189c67657a0727ca4a5b40"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/typing.rs",
        sha256: Some("93bcec7e62535047a4605b23c2c574dfb04726ec774749f41149e2a5ee1ccfae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/normalization.rs",
        sha256: Some("c6125df81983cb4255c9b8c664d4e88f8eebc7b2f44c9798b53e4d449034e606"),
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
        sha256: Some("153eacd5f902e149971334edf65c198deb2a97b653d0cf5aa86d3699080e64d9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/order_discreteness.rs",
        sha256: Some("38d94fad48bf0b4b236cd4cfefa3d6b4da762a4c2eb0cdd013b2eedccaa0da29"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/strict_order_transitivity.rs",
        sha256: Some("2f884701fe0f68561cdeb957f39f9c3baba9dfea53ebd3782ad197c02fdfbb5e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/subtract_order.rs",
        sha256: Some("a15c78a3cee6916fffe7119f95207204cca6290cf9c498ccef70919d76daa816"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/proof/traversal.rs",
        sha256: Some("7c47e2947ebab853800fcaeef3c3dc860904492db6918b4818ce2495453f40e5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/recursion.rs",
        sha256: Some("878420e1c259971d766cece22a47f45a2d54fd8fad531c3b59deb1858f27705d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition.rs",
        sha256: Some("8b334936c54008186fd590eaaddb0bbda8aba3da0ec1d6ee275afc546d789087"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/call_composition/fixed_byte_view.rs",
        sha256: Some("20ee08135d1907834c1418fdaf0c05313b91ebb4aac474f886b5f3bced463d90"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/lib.rs",
        sha256: Some("f912cf8be4a1904517491b68f0361b6f103a70d7babc5008655f40543837dc07"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar.rs",
        sha256: Some("96929d227c9337d8baf2939597e632c15c20a525aa3c414f7dacea1761054be4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("080eb93280f3a4947d47c8ee2b4eceaeaedd5bef35cd098dda91226b7dbb1ea5"),
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
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        sha256: Some("9b1ecc85543f2139792c5d309b21a460eb69e840050c97cd8c6a4214438a0bd1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("f32bc057bb61dfdf6576d14169519afffce80c218548fc4ad4b158639cb47ae6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles.rs",
        sha256: Some("fc565dbb66b0a946775ad7b7274cb09c02b0bb68e46cad6b6ecd4976ae7b2ce9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/reconstruction.rs",
        sha256: Some("468e9fac02da45389d93603682c0ca894fac8b7bd2f379f3d57ce848762f6dcf"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/validation.rs",
        sha256: Some("6e70bfe120f27cfe831397fd4cd2b36b4b61ffdfc64260d4b5918a27e4108c54"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_graph.rs",
        sha256: Some("fbb4769867bf0a5ea53337fc0c59a863261911b8477da6b7036aa6a877703170"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/lib.rs",
        sha256: Some("f04edcf73e615ce9e41497a1f489705ee3858df7f50dbb13c7d53d1ddfc96e3f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization.rs",
        sha256: Some("5399c8034da29c7f87ffa3a3c792bb8149a9522a864def900baa5ab68ec8ccd8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/proof_recursion.rs",
        sha256: Some("d2a8b7d21c2a768be1ea7d16187891d2e0b41de1c833c0662538c73c4125544a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/quotient_correspondence.rs",
        sha256: Some("7f8c8a45ea591dd6e98a3ebb8bfc6e3475ccca11f6e304ed582a9fa052f16911"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/terminal_trace_v1.rs",
        sha256: Some("d7a45bd492a01c3eb843a56d8902c0c99d26873a958a6b0d85bb9acedafc7c8e"),
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
        sha256: Some("7764eab12b789733b62341a8db78c5d3fc688e2a252a61e14ae99acb917f4d26"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup.rs",
        sha256: Some("5d9c2641ba0a1de79b82b9bb2672f5dea6c96b6f0f45a7bdf8dba2f63076f332"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("331ae7347e767b3cc1f7795b415148f440dd7eab5f51545383633b3d49a23ec5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/block_views.rs",
        sha256: Some("c4c85b6df07893c834fb6000ca01585286e8a00ba320e35ae182cd1a7016ac8c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_length.rs",
        sha256: Some("3d56ecc97da9909038382fdd4837e9410da2b9604718fad1bcc24dee47935e64"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_read.rs",
        sha256: Some("4de8a813cbb27e8e47fec779126e38a303d5b8cb03ee0631cd24e14d991dd7a6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_subslice.rs",
        sha256: Some("de42bdc20a7f198b0836fde1c80f42686565a68a66c4a657488339db2fafc21b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_write.rs",
        sha256: Some("19556944e8c2ca2d6e9390c29360360d9b6dcf8fe9951fcf6ad877655b3bfb00"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/call_graph.rs",
        sha256: Some("dcfc6de95910a84d783d9a40d86dd0c13fe1d2d8249ee9d61ec7fd7fa5aac3eb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/conformance_applications.rs",
        sha256: Some("7755cecf5a6c7d847ba77752b57a0666affdf7fa27bb2f9652fae586d078ba64"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/content.rs",
        sha256: Some("7218e6f0c90bfb670f18615f40583bcd169044153b9b8bec74895e46674cc842"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/contracts.rs",
        sha256: Some("7d9ccec4775a19f5b6c3f0bbfceec40ede5f18dcc5015d90a289dbcd3572a361"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow.rs",
        sha256: Some("366f2a9410f5aa68466216fda8179843b7b34cdb823676ffa1b33422b7cc77d3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("dbd46cee43b8cfa392b34bd9436a1d96048888f152249a58cf53615a0fba06ca"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash.rs",
        sha256: Some("1ae645c005d9fe057ff9f911699d35cdd4b0c4c48818857cfd3882629b134b67"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/entry_requirements.rs",
        sha256: Some("c3ecf9ff3049c87cb1fa13e1b6ccb9b4e1642fb60f77e350d2df9425926e923c"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/outcome.rs",
        sha256: Some("26a19292010dcd8b42b3740855aa77a97c6d61f80c2808836f93f77db0468f37"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/crash/site_truth.rs",
        sha256: Some("71c890e9cfff5bda5cff300b9a7c754e343f5e2a2805f9fdfec210e29cdfcf1e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch.rs",
        sha256: Some("2d954fca3823d6c184ff0831e48dd567984497215519cc149e7c41ebbfcd5f3d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/error.rs",
        sha256: Some("610179a7122a2e6127f345fa7b02432b9da747c99f616c737c8b5d7301d44d38"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/evidence.rs",
        sha256: Some("693d4b19c6a8a798b6036e297a0756879846b825354f9ca07f71a2c55348331e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/float_meaning.rs",
        sha256: Some("ac68835fb3ab3a61df60665a909b6568ae6f5694fdaba6328217e893e1319b05"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation.rs",
        sha256: Some("c5a8f449cbb5cfe6b303e51c578198ed11800ce5e572d46e2b03a41d4f6a44f8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/foundation/provider_result.rs",
        sha256: Some("74c9d798b59589bb9f5808ed3ca9fcbeb68fd3f2abeb99babe5252b530287525"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier.rs",
        sha256: Some("1dad1d3750b6fa4c22bfb5dc747090efb4616b06cc53a1b6e8f0845c498215c7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_parameters.rs",
        sha256: Some("6b5fe12b7ff779fc6daac61082a032175b0b13c2acde0f5cb4676ee9a0008da7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/traversal.rs",
        sha256: Some("762aa8cfffbe6f87045a5b10a51e585dc5d810fa1e6bd6e3955bb8543d14bd1a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine.rs",
        sha256: Some("bf4fab4742704d0c5d1a8ac4291dd98778cee98598aa3c13378fbf33f3f6af42"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations.rs",
        sha256: Some("c791c5d0b9559c2f80da8a4487e1d67dfd784f7e9941a262a162319592396d62"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/partial_affine.rs",
        sha256: Some("88fbe370822099760ecbd8bf54615e616f6e79d797faa242ec9cdf0ebcac5329"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/primitive_storage.rs",
        sha256: Some("519913f8db3fad980b1263e4821dbb656ca90412e05709afe23bb9d3171e478f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/proof_recursion.rs",
        sha256: Some("82dde599f1569893ca9235e4ebe6d456611ded5ae0db4f872fe44bb040873b8d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/propositions.rs",
        sha256: Some("46e2db62500dc79249d8bdaabe9bde1429d4cd1a68cf555dfed93d93ade5d453"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/quotient_correspondence.rs",
        sha256: Some("d38746cbfc4ffdc4f9bb158f4025b97f32265ead709988a28bf94f523fa3e5c1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/ranked_scc.rs",
        sha256: Some("de4a724fdbd52046cf3336ca1d9a0a3346eb93cec6382b24643f7635413f05a3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/reach_applications.rs",
        sha256: Some("a1fb85fdf71bcee14746504370bf7bfabf21fc89a3463b1c4b266d452c3c9ed5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/record.rs",
        sha256: Some("c332722f775372dd272d946f81e55057124f9a3430fd9b80ddcfb84f1e05add0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/references.rs",
        sha256: Some("60a790ff1219ff675e2820eeb385c8a9b51726ff1e5d53223ba0d730c10dd2ec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/root_service_reach.rs",
        sha256: Some("7a2cd6089236886a606b72436a8ab85e119f85039226c791456643e73cb36995"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_array.rs",
        sha256: Some("dce2a6d7006e7ac950d341b1383c1bc03f6fce84c4b1b5f2b963b2b7984f60b5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_block_invariants.rs",
        sha256: Some("a5aaea2f946e19137d95741dab91e1e7cf784bc40fbbafd102525223b684c0a8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs",
        sha256: Some("4208a9ff4af0cf746636e302593f2cada634489f9b82ef1dab1d4f1f0410a919"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_qualifications.rs",
        sha256: Some("faaf6af5682eca0044981fc21e883e629750a1268d235c05602a5d8e9a3a68b0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields.rs",
        sha256: Some("84d7450aef54b04ccb39e952efac2a92675cd5f5f460a9136ca67fd5886158c4"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields/freshness.rs",
        sha256: Some("b263cbe0954a928604d40749572fb5210b88202d7a1c23da4293e98a9f02488e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_store.rs",
        sha256: Some("c375d5b06d9cb4f1daf8ec8fb6e11582dfac2882938ab411bb1a62bc6a511b51"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_case_membership.rs",
        sha256: Some("6a2c4b117f0ebebb3538aec310c1eeab531629358fa9c35dea6f7dd56ebf1068"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations.rs",
        sha256: Some("0b01069042659ad24825c1e9280d23fbae6c0521d10c6599294235e68f856bd0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_qualification_rosters.rs",
        sha256: Some("05f885bfe18ec25658ec4df4f31b617d2c93dc651309aa67c097f60c897a22ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_result_contracts.rs",
        sha256: Some("2981605bcdd84cea8fd37ccb711c3334e0963a04399a3c7cc1fe3eccac63ecb3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_scalar_fields.rs",
        sha256: Some("b8d045bd8b8e779bc532373617f9a1185dc18040350c6ab7567f621228a390d6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/suspension_call_plan.rs",
        sha256: Some("3de4b6461829b14cf2967f2fd178aff23f50e79ab4e7754fd09ea5cf0a415563"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification.rs",
        sha256: Some("e1deb38854a020e941a55114175489409edc05d8c5fc022e77bc16292d334416"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs",
        sha256: Some("df2b20205d133eff30c096c12b0b799b2dbf66151aab9bca185cbd5c8a9071e1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/evidence_provenance.rs",
        sha256: Some("828677a2b525a848e6f5be261c520d4cf0291947b8f763b4586c41beb1335a9c"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/float_meaning_projection.rs",
        sha256: Some("0fc39c9d2f55242f7f14459df2c9e9994658ea00133972d44279c1267f294513"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/proof_bundle.rs",
        sha256: Some("b22452c73b177327804ad09dbacb5e9af1fa24015455791970359dd873627851"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction.rs",
        sha256: Some("1cff3de3037135136d6bec8e08541378d88670d81d44de52978ba8753a918371"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_field_origins.rs",
        sha256: Some("9606a927766c81d3a199a411aec2ea2d07212fc6d63a061413130976491cf5ba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_paths.rs",
        sha256: Some("601b9a28031f9cb0b8bc4e25eaf49434ba2639c79c984766cb451062f08709f6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_context.rs",
        sha256: Some("fd78724db412ea3f506501ab962fa08735ccaec5ec73fd5209c5b1971cb8d495"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_flow.rs",
        sha256: Some("ca9d49136203367ea342215c665c6e36928aef891b45f10dd99b1fa61323ffea"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs",
        sha256: Some("6122336123d14110799649eefb2c9a096ac64c64cdc34e004a26833636e928be"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity.rs",
        sha256: Some("e89eb8b661fae7efeb42b28827208921cd19d55f6bf624acd06454056ea33aa6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/byte_extent.rs",
        sha256: Some("cd1d5298ae4639bcdac3db83d10b87e2968ece4b546a91acf53867489ec58fca"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/record.rs",
        sha256: Some("ad7d28b003dff91f5402653cf5dc4af5352f0c345736e7fdcebe94df7bfee0dc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/scalar_case.rs",
        sha256: Some("f1803c12ea67cc8ddbc16a0a938d8079da0fd3976644d042e86cc8374dee41e5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts.rs",
        sha256: Some("e7afbbf974395bcaa94cbd4a888bfa3ccd30f240ce801db919d39ae56bf1aa2d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs",
        sha256: Some("0d03d56127237d86888cc7e660f4c029957a06423cad36cae7b875184524718b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/discrete.rs",
        sha256: Some("ae495c6aac20ddae2ab1644dd0a538ee9308f330a52345e15a3aa5459aa20bba"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/primitive_snapshots.rs",
        sha256: Some("fb19ba08b5a532ed91d0e417bfc3ac1b8fb2c153715b0fec17b1250b8f1f5b2a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("81984697c9e193fe1ab9b50cb184a5f7a1ebd8bc2694d85543a36dbe8e05aefb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/terminator_facts.rs",
        sha256: Some("2756ac859a34ff923dbde307ad88b6882990f038405f0f18d2d467a420a3cddc"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/substitution.rs",
        sha256: Some("a0cb5682494b53de6ad47aed569f38504a3136ef079132bd96e8f542742ef46e"),
        inventory_machinery: false,
    },
];
