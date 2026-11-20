use super::pipeline::Refusal;
use omega_psi::{
    BlockTerminator, CallTarget, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, PlaceProjection, ReturnEdge, ScalarLiteral,
};

pub struct Case {
    pub id: &'static str,
    pub directory: &'static str,
    pub files: &'static [&'static str],
    pub main_override: Option<&'static [u8]>,
    pub configuration: &'static str,
    pub expectation: Expectation,
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
pub enum Expectation {
    MustLower(Check),
    TrackCompatibility(Check),
    ExpectCheckerRefusal(Check),
    MustRefuse(&'static str),
    MissingSetup(&'static str),
}

/// Where the configuration actually stopped, whatever the case expected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    MissingSetup(&'static str),
    /// Psi was produced. The witness is absent when the case names no output check.
    Product { witness: Option<bool> },
    Refused(Refusal),
}

impl Actual {
    pub fn stage(&self) -> &'static str {
        match self {
            Self::MissingSetup(_) => "setup",
            Self::Product { .. } => "L5",
            Self::Refused(refusal) => refusal.stage,
        }
    }

    /// The observation recorded after the case id in `observations.tsv`.
    pub fn row(&self) -> String {
        match self {
            Self::MissingSetup(reason) => format!("setup\t{reason}"),
            Self::Product { witness: Some(true) } => "psi\twitness matched".into(),
            Self::Product { witness: Some(false) } => "psi\twitness absent".into(),
            Self::Product { witness: None } => "psi\tnot inspected".into(),
            Self::Refused(refusal) => refusal.to_string(),
        }
    }
}

pub struct Observation {
    pub actual: Actual,
    /// Why the actual result breaks a required expectation.
    pub mismatch: Option<String>,
}

impl Expectation {
    pub fn observe(
        self,
        files: &[(&str, Vec<u8>)],
    ) -> Observation {
        let actual = match self {
            Self::MissingSetup(reason) => Actual::MissingSetup(reason),
            Self::MustRefuse(_) => match super::pipeline::lower(files) {
                Ok(_) => Actual::Product { witness: None },
                Err(refusal) => Actual::Refused(refusal),
            },
            Self::MustLower(check) | Self::TrackCompatibility(check) | Self::ExpectCheckerRefusal(check) => match super::pipeline::lower(files) {
                Ok(product) => Actual::Product {
                    witness: Some(check.matches(&product)),
                },
                Err(refusal) => Actual::Refused(refusal),
            },
        };
        let mismatch = match (self, &actual) {
            (Self::MustRefuse(expected), Actual::Refused(refusal)) if refusal.to_string() != expected => Some(format!("expected {expected}, got {refusal}")),
            (Self::MustRefuse(expected), Actual::Product { .. }) => Some(format!("produced Psi; expected {expected}")),
            (_, Actual::Product { witness: Some(false) }) => Some("produced Psi without the required behavior".into()),
            (Self::MustLower(_), Actual::Refused(refusal)) => Some(format!("expected Psi, got {refusal}")),
            _ => None,
        };
        Observation { actual, mismatch }
    }
}

pub const MISSING_IMPORT: &str = "L2\tmain.omg:4:30\tunresolved name A";

#[derive(Clone, Copy)]
pub enum Check {
    EmptyMain,
    ModuleConstants,
    NestedField,
    Comparisons,
    IdentityCalls,
    Premise(bool),
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
            Self::EmptyMain => named("main").is_some_and(|m| {
                m.body.parameters.is_empty()
                    && m.body.operations.is_empty()
                    && matches!(&m.body.blocks[..], [block] if matches!(block.terminator, BlockTerminator::Return { result: ReturnEdge::Unit, .. }))
            }),
            Self::ModuleConstants => [("v", 1), ("other_v", 7)]
                .iter()
                .all(|(name, expected)| named(name).is_some_and(|m| returns_literal(m, &ScalarLiteral::Integer((*expected).into())))),
            Self::NestedField => named("result").is_some_and(|machine| returns_nested_field(product, machine)),
            Self::Comparisons => [
                IntegerPrimitive::NotEqual,
                IntegerPrimitive::LessThan,
                IntegerPrimitive::Equal,
            ]
            .iter()
            .all(|primitive| {
                product.machines.iter().any(|m| {
                    m.body
                        .operations
                        .iter()
                        .any(|op| matches!(&op.kind, OperationKind::Integer { operation, .. } if operation.primitive == *primitive))
                })
            }),
            Self::IdentityCalls => named("identity").is_some_and(|callee| {
                product.machines.iter().any(|m| {
                    m.body
                        .operations
                        .iter()
                        .filter(|op| matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == callee.id))
                        .count()
                        == 3
                })
            }),
            Self::Premise(value) => {
                named("premise").is_some_and(|m| returns_literal(m, &ScalarLiteral::Boolean(value)))
                    && named("witness").is_some_and(|m| !m.contract.requires.is_empty())
            }
        }
    }
}

fn returns_nested_field(
    package: &LoweredPackage,
    machine: &LoweredMachine,
) -> bool {
    let data = |name| {
        package
            .data
            .iter()
            .find(|record| record.declaration.path == name)
    };
    let (Some(outer), Some(inner)) = (data(r"m\B"), data(r"m\A")) else {
        return false;
    };
    let ([outer_field], [inner_field]) = (&outer.fields[..], &inner.fields[..]) else {
        return false;
    };
    let [block] = &machine.body.blocks[..] else { return false };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = block.terminator
    else {
        return false;
    };
    machine.body.operations.iter().any(|op| {
        let OperationKind::ProjectedScalarRead { source } = &op.kind else {
            return false;
        };
        op.result == OperationResult::Scalar(returned)
            && matches!(&source.path[..], [PlaceProjection::Field(first), PlaceProjection::Field(second)]
                if first == &outer_field.declaration && second == &inner_field.declaration)
            && machine
                .body
                .places
                .get(source.root.index())
                .is_some_and(|place| place.ty == outer.carrier)
            && machine
                .body
                .operations
                .iter()
                .any(|op| op.result == OperationResult::Structural(source.root) && matches!(op.kind, OperationKind::EstablishRecord { .. }))
    })
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
        expectation: Expectation::MustLower(Check::EmptyMain),
    },
    Case {
        id: "module-library",
        directory: "probes/build_nonmain_module/dep",
        files: &["main.omg", "other.omg"],
        main_override: None,
        configuration: r#"Build::package(builder, "mod_kernel")"#,
        expectation: Expectation::MustLower(Check::ModuleConstants),
    },
    Case {
        id: "cross-width",
        directory: "probes/cross_width_compare",
        files: MAIN,
        main_override: None,
        configuration: HOST_START,
        expectation: Expectation::TrackCompatibility(Check::Comparisons),
    },
    Case {
        id: "rational-landing",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        main_override: None,
        configuration: HOST_START,
        expectation: Expectation::TrackCompatibility(Check::IdentityCalls),
    },
    Case {
        id: "direct-imports",
        directory: DIRECT,
        files: IMPORTS,
        main_override: None,
        configuration: FREESTANDING,
        expectation: Expectation::TrackCompatibility(Check::NestedField),
    },
    Case {
        id: "direct-imports-v2",
        directory: DIRECT,
        files: IMPORTS,
        main_override: Some(include_bytes!("fixtures/direct_file_imports/main.omg")),
        configuration: FREESTANDING,
        expectation: Expectation::MustLower(Check::NestedField),
    },
    Case {
        id: "missing-import-v2",
        directory: DIRECT,
        files: IMPORTS,
        main_override: Some(include_bytes!("fixtures/direct_file_imports/missing_import.omg")),
        configuration: FREESTANDING,
        expectation: Expectation::MustRefuse(MISSING_IMPORT),
    },
    Case {
        id: "abstract-premise",
        directory: "facts/abstract_signature_call/local",
        files: MAIN,
        main_override: None,
        configuration: HOST_MAIN,
        expectation: Expectation::TrackCompatibility(Check::Premise(true)),
    },
    Case {
        id: "false-abstract-premise",
        directory: "facts/abstract_signature_call/invalid_local",
        files: MAIN,
        main_override: None,
        configuration: HOST_MAIN,
        expectation: Expectation::ExpectCheckerRefusal(Check::Premise(false)),
    },
    Case {
        id: "import-from-dependency",
        directory: "probes/build_nonmain_module",
        files: MAIN,
        main_override: None,
        configuration: r#"with_alias(@mut kit_dependency, "kit")"#,
        expectation: Expectation::MissingSetup("requires Build dependency alias kit -> dep (mod_kernel), selecting other.omg"),
    },
];
