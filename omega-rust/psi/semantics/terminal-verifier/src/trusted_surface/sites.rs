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
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_graph/tests.rs",
        parent: "omega-rust/psi/semantics/terminal-verifier/src/control_graph.rs",
        module: "tests",
    },
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
        sha256: Some("447793c351edf5661842bc685e0d2e0b8e5517f34c9f82caf65f0df233ab3c2d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs",
        sha256: Some("63f58da3db974b534cb9b4ca1aad799052d8c15889baf2a1bbef03a0f3223bf1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/closed_integer.rs",
        sha256: Some("c19f178676771feafd841838972e0f45dd708fa7ecbd902fdb0ec3e6408dcb1b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission/evidence.rs",
        sha256: Some("eaf956334cbf67463703a460516f70c636111d629de4ea7221fef67fd6f992b1"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_affine.rs",
        sha256: Some("6aba8ea0ca8bae63d968a9c95356ae4ee6c0fadc17c508bc247373acce6264a6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_cast.rs",
        sha256: Some("da16b64dc48f83ac905597d2ecb4f66964e42db204837ba099a4d469c28b438d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_forbidden_root.rs",
        sha256: Some("c2a54fde00c12eb8dc0b824243d7837237ae994210103e2f2601a019094c925b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/integer_rules/integer_shift.rs",
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
        sha256: Some("ef6be6c7fb0fe95c7fd3491b0fd932b50c3632cd63f88f7350c0272354a61be3"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core.rs",
        sha256: Some("7d01582857f7e844d7d6e3d44a0a3fe6001c5e204d447484dc547ae7950411b9"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/certificate.rs",
        sha256: Some("2ee7a6a919d261bf61bf72acf122b1a612bcbc0873dcc61fc4be10b0a2609fc7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/conversion.rs",
        sha256: Some("34c3888864dd12a9828f8cc37e393f622e24d8904776a3ff30750b3de1a51c4d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/signature.rs",
        sha256: Some("7c1f35ff6224e5a0f38234dcc01c4f98776b1890804026c0e21b19cb948b1f4d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/substitution.rs",
        sha256: Some("0908b1468a1aea94ab0eac759839111e60462be38472b3657a6278718160e798"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/term.rs",
        sha256: Some("b6ac58d6801e39c168e564230f6de478b1b6f40001658528a96bde87272f80a6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/mathematical_core/typing.rs",
        sha256: Some("a4f0604d038bea97d90f01d4e072b810b5f3e297224845bf994c95748d1bc356"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/proof-admission/src/admission/normalization.rs",
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
        path: "omega-rust/psi/semantics/proof-admission/src/admission/recursion.rs",
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
        sha256: Some("e6955a563081f8a5b1f95c327ea5304b4a993f3ad5949bdd863514fa55009bfb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar.rs",
        sha256: Some("a548b1a99b9a760b4b51e04a6664f7317f10343964e9bad283caa802d477da92"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs",
        sha256: Some("080eb93280f3a4947d47c8ee2b4eceaeaedd5bef35cd098dda91226b7dbb1ea5"),
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
        path: "omega-rust/psi/semantics/terminal-semantics/src/primitive_place.rs",
        sha256: Some("d0b28b15417aa9ce9fbcd7c54c94950d94e68803cdedcf3e6d20a2e611fefd13"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/scalar_array.rs",
        sha256: Some("91fb0cd8ba60464e4073cd1092bbc139c1b1c9554e325816b266079bc5852507"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
        sha256: Some("c33538da186a92993ebca76de81edcf6a218efa05255846e4cf9c575b5cbd64b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs",
        sha256: Some("f32bc057bb61dfdf6576d14169519afffce80c218548fc4ad4b158639cb47ae6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles.rs",
        sha256: Some("bcfaef3747d6ca478eb0e4daa80a0c778e92f55aacfd6efcf2241a495fae5658"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/control_cycles/reconstruction.rs",
        sha256: Some("eb9b0d5478cd2e8ddcd91a500407e3ac355aaeb6513a7878a952cb7e7fe3eaf0"),
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
        sha256: Some("fdd65e00268baae053c895201b81033aed286cabfeadaa1c48dfb1989a04b583"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization.rs",
        sha256: Some("1e6fce136a52810953ddd706792c66e849a546bb54a1062fab74a2e3b06eb6ae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/dead_scalar_elimination.rs",
        sha256: Some("216fb8b00b572c07d94a653801e6b95981543c4eacb0e06667ed83b0b653fb8a"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/copy_propagation.rs",
        sha256: Some("ccff0be2c35f1d9c4132e4137f43ad7240da744cbe34acd24e7205b45836c3ae"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/global_value_numbering.rs",
        sha256: Some("260a2b2045f0c8dae11b314c2d36fb29fbee484c52ae0d94885fe35198e18c1d"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/sparse_conditional_constant_propagation.rs",
        sha256: Some("d5961335f363246038b81ddee881393866f790163a07c4ce7ad1eb68deb06040"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/control_flow_cleanup.rs",
        sha256: Some("d2f9c1447058a11378fd47899770e9338d0b003cadf4cd05443f900daa25f58f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/optimization/proof_check_elision.rs",
        sha256: Some("7dbc979c459344dc84b71953fea6e52e89463ea2887c77c41e5b13e97f1926b5"),
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
        sha256: Some("47eae49d51ada23161ed993348b2b726b471cc91ccba51ce76c26132998f3f30"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup.rs",
        sha256: Some("5d9c2641ba0a1de79b82b9bb2672f5dea6c96b6f0f45a7bdf8dba2f63076f332"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/affine_cleanup/continuation.rs",
        sha256: Some("e0ba2cc10bf17e0a3bddd802d80f48cab83ae95caffff32561f5ce7d0a84d46e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/block_views.rs",
        sha256: Some("8b2c08bf131bb899e10d87e6c42b3b49f793c60fcf710175cba068e5020a731f"),
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
        sha256: Some("e6bfa0c4b8b9742a56b13e55e0d537019ff6d682b293789a688a53931c4f752f"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/control_flow/unranked_cycles.rs",
        sha256: Some("9b4ce7e1027074d3c146aa815adb2d908bdfd4e9b1ad64e7e4e87cdff178bdc8"),
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
        sha256: Some("08227497aa59617c16fb47fbe0b1af77c811f61704904ad9cbc003db468f6ccb"),
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
        sha256: Some("fb15ba91d2f76e8a1a78ef352a97460b96766e8851f14caecffd81fd4e92598e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier.rs",
        sha256: Some("032f61c058dd2889d4d37ecf85e66c90e7fa6883cbfdc09d0aacc2a0f421164b"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/block_parameters.rs",
        sha256: Some("5884c4c6829d68e2a87e5fc1d5b406469fdbde33f3d5cbb01f36ead9a95c1bec"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier/traversal.rs",
        sha256: Some("30f2b690c99efe51abeb58d8e733fa9ffbc33ab3b89b07fcbd01d73fa6c82db0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/machine.rs",
        sha256: Some("51b8c0140e18e638b051bb5c3254c8323d2541256dacc19e274b07baab7b6ecb"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/operations.rs",
        sha256: Some("ab39959877a1668ee69abce8d8fc602c94b2ccdb0067b355096f4cc23b060cb7"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/partial_affine.rs",
        sha256: Some("5f95408606f4039b2380476187038fc6144b7b983fbf96f392c884af163b63b2"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/primitive_storage.rs",
        sha256: Some("fe2cd920ae39d6c2edac84b3593fc00a1768c174d808d405a5e5e2dbb0e5e539"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/reach_applications.rs",
        sha256: Some("a1fb85fdf71bcee14746504370bf7bfabf21fc89a3463b1c4b266d452c3c9ed5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/record.rs",
        sha256: Some("a2666275e5ab3de02f1e18b42c77e7023c0bb8c588fd5cfa53f8ab962f2f0906"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/references.rs",
        sha256: Some("5d224a1630473fb6e543c5e387f41ddd6c52600779c390948395543803e101e0"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/root_service_reach.rs",
        sha256: Some("7a2cd6089236886a606b72436a8ab85e119f85039226c791456643e73cb36995"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_array.rs",
        sha256: Some("b8de5db3be3ecf33e214b8f5b5a6a2368326e5cc80653e4371819bc566d70fa6"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_block_invariants.rs",
        sha256: Some("9704b230d723d5d975f58af3215e2bb65ea982166b71d498daca51b79f009240"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs",
        sha256: Some("4208a9ff4af0cf746636e302593f2cada634489f9b82ef1dab1d4f1f0410a919"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_qualifications.rs",
        sha256: Some("db611c46a22378c859f5db2762f6ca12fcf0879f625da8cba04fdb8f4117d018"),
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
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_paths.rs",
        sha256: Some("a8d8ccb6e7de318f0ffb101cc764b929645da69ac80605a5418252e12079be23"),
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
        sha256: Some("69dc561b546080404286340e2483f089a5b56f341667f9d943f5b2fa49b71ce5"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/validation/suspension_call_plan.rs",
        sha256: Some("3de4b6461829b14cf2967f2fd178aff23f50e79ab4e7754fd09ea5cf0a415563"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification.rs",
        sha256: Some("eedda6197cd106a272ddcad83d8106375904b1c8af99130c5ec4f9bea2ce89d8"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs",
        sha256: Some("0aefd73deba6498866f511dd36e7ee3593c0b5d6a44a25c5225d1f67c10f000e"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/field_snapshots.rs",
        sha256: Some("53db00fcbc09f03d15338e77f220792b345a8b1739a9ffc435d3ab238b435407"),
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
        sha256: Some("6ee204170401f9a2fdcabfc99f109dba40bc9997e74328eaf39d7127d125d87f"),
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
        sha256: Some("671a40bd6b3963f8763095c5916487eebbb7f4d8c863c05354f9db0d9839062c"),
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
        sha256: Some("765f7a9050ec6b08dc412c08f6572c72d36f0852b43e0b8b1ef491878135bdde"),
        inventory_machinery: false,
    },
    ImplementationSite {
        path: "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs",
        sha256: Some("553f7e965421fef04d1c8a0de1c8063ded2a16a332161ce168b05aca1c865bfb"),
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
