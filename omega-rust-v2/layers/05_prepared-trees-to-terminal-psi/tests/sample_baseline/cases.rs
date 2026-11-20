use omega_psi::{
    BlockTerminator, CallTarget, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, PlaceProjection, ReturnEdge, ScalarLiteral,
};

pub struct Case {
    pub id: &'static str,
    pub directory: &'static str,
    pub files: &'static [&'static str],
    pub main_override: Option<&'static [u8]>,
    pub configuration: &'static str,
    pub check: Check,
}

impl Case {
    pub fn confirm_configuration(
        &self,
        recipe: &str,
    ) {
        assert!(
            recipe.contains(self.configuration),
            "{}: selected configuration absent from pinned build.omg",
            self.id
        );
    }
}

#[derive(Clone, Copy)]
pub enum Check {
    EmptyMain,
    ModuleConstants,
    NestedField,
    Comparisons,
    IdentityCalls,
    Premise(bool),
    NegativeImport,
    MissingSetup(&'static str),
}

fn returns_literal(
    machine: &LoweredMachine,
    literal: &ScalarLiteral,
) -> bool {
    machine.body.blocks.iter().any(|block| {
        let BlockTerminator::Return {
            result: ReturnEdge::Scalar(returned),
            ..
        } = block.terminator
        else {
            return false;
        };
        machine
            .body
            .operations
            .iter()
            .any(|op| op.result == OperationResult::Scalar(returned) && matches!(&op.kind, OperationKind::Constant(value) if value == literal))
    })
}

impl Check {
    pub fn matches(
        self,
        product: &LoweredPackage,
    ) -> bool {
        let named = |name: &str| product.machines.iter().find(|m| m.declaration.path == name);
        match self {
            Self::EmptyMain => named("main").is_some_and(|m| m.body.parameters.is_empty() && m.body.operations.is_empty()
                && matches!(&m.body.blocks[..], [block] if matches!(block.terminator, BlockTerminator::Return { result: ReturnEdge::Unit, .. }))),
            Self::ModuleConstants => [("v", 1), ("other_v", 7)].iter().all(|(name, expected)| named(name)
                .is_some_and(|m| returns_literal(m, &ScalarLiteral::Integer((*expected).into())))),
            Self::NestedField => named("result").is_some_and(|m| m.body.operations.iter().any(|op| {
                matches!(&op.kind, OperationKind::ProjectedScalarRead { source } if matches!(&source.path[..],
                    [PlaceProjection::Field(outer), PlaceProjection::Field(inner)] if outer != inner && outer.path.ends_with("value") && inner.path.ends_with("value")))
            })),
            Self::Comparisons => [IntegerPrimitive::NotEqual, IntegerPrimitive::LessThan, IntegerPrimitive::Equal].iter().all(|primitive|
                product.machines.iter().any(|m| m.body.operations.iter().any(|op| matches!(&op.kind, OperationKind::Integer { operation, .. } if operation.primitive == *primitive)))),
            Self::IdentityCalls => named("identity").is_some_and(|callee| product.machines.iter().any(|m| m.body.operations.iter().filter(|op|
                matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == callee.id)).count() == 3)),
            Self::Premise(value) => named("premise").is_some_and(|m| returns_literal(m, &ScalarLiteral::Boolean(value)))
                && named("witness").is_some_and(|m| !m.contract.requires.is_empty()),
            Self::NegativeImport | Self::MissingSetup(_) => false,
        }
    }
}

const MAIN: &[&str] = &["main.omg"];
const IMPORTS: &[&str] = &["main.omg", "m/a.omg", "m/b.omg"];
const HOST_MAIN: &str = r"linux_x86_64\ProgramEntry, main)";
const HOST_START: &str = r"linux_x86_64\ProgramEntry, entry_start)";
const FREESTANDING: &str = "builder.freestanding = true;";
const DIRECT: &str = "probes/direct_file_imports";

pub const CASES: &[Case] = &[
    Case {
        id: "standalone",
        directory: "basics/standalone",
        files: MAIN,
        main_override: None,
        configuration: HOST_MAIN,
        check: Check::EmptyMain,
    },
    Case {
        id: "module-library",
        directory: "probes/build_nonmain_module/dep",
        files: &["main.omg", "other.omg"],
        main_override: None,
        configuration: r#"Build::package(builder, "mod_kernel")"#,
        check: Check::ModuleConstants,
    },
    Case {
        id: "cross-width",
        directory: "probes/cross_width_compare",
        files: MAIN,
        main_override: None,
        configuration: HOST_START,
        check: Check::Comparisons,
    },
    Case {
        id: "rational-landing",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        main_override: None,
        configuration: HOST_START,
        check: Check::IdentityCalls,
    },
    Case {
        id: "direct-imports",
        directory: DIRECT,
        files: IMPORTS,
        main_override: None,
        configuration: FREESTANDING,
        check: Check::NestedField,
    },
    Case {
        id: "direct-imports-v2",
        directory: DIRECT,
        files: IMPORTS,
        main_override: Some(include_bytes!("fixtures/direct_file_imports/main.omg")),
        configuration: FREESTANDING,
        check: Check::NestedField,
    },
    Case {
        id: "missing-import-v2",
        directory: DIRECT,
        files: IMPORTS,
        main_override: Some(include_bytes!("fixtures/direct_file_imports/missing_import.omg")),
        configuration: FREESTANDING,
        check: Check::NegativeImport,
    },
    Case {
        id: "abstract-premise",
        directory: "facts/abstract_signature_call/local",
        files: MAIN,
        main_override: None,
        configuration: HOST_MAIN,
        check: Check::Premise(true),
    },
    Case {
        id: "false-abstract-premise",
        directory: "facts/abstract_signature_call/invalid_local",
        files: MAIN,
        main_override: None,
        configuration: HOST_MAIN,
        check: Check::Premise(false),
    },
    Case {
        id: "import-from-dependency",
        directory: "probes/build_nonmain_module",
        files: MAIN,
        main_override: None,
        configuration: r#"with_alias(@mut kit_dependency, "kit")"#,
        check: Check::MissingSetup("requires Build dependency alias kit -> dep (mod_kernel), selecting other.omg"),
    },
];
