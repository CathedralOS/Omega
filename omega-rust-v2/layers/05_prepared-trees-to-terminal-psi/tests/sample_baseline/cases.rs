use super::pipeline::Refusal;
use omega_psi::{
    BlockTerminator, CallTarget, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, PlaceProjection, ReturnEdge, ScalarLiteral,
};

pub struct Case {
    pub id: &'static str,
    pub directory: &'static str,
    pub files: &'static [&'static str],
    /// Current-language files used instead of the pinned history. The
    /// historical build still selects the configuration.
    pub migrated: Option<&'static [(&'static str, &'static [u8])]>,
    pub configuration: &'static str,
    /// Dependency packages that the root's pinned `build.omg` registers under an alias.
    pub dependencies: &'static [Admitted],
    pub expectation: Expectation,
}

/// One direct dependency edge that the root's Build recipe admits. The harness never runs
/// that recipe. It confirms the registration in both `build.omg` files and supplies the
/// resulting package and alias edge to the production resolver profile.
pub struct Admitted {
    pub alias: &'static str,
    /// The package identity the dependency's own `build.omg` declares.
    pub package: &'static str,
    /// The dependency's source directory, relative to the pinned samples root.
    pub directory: &'static str,
    pub files: &'static [&'static str],
    /// The text in the dependency's `build.omg` that declares `package`.
    pub configuration: &'static str,
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
}

/// Where the configuration actually stopped, whatever the case expected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Actual {
    /// Psi was produced. The witness is absent when the case names no output check.
    Product { witness: Option<bool> },
    Refused(Refusal),
}

impl Actual {
    pub fn stage(&self) -> &'static str {
        match self {
            Self::Product { .. } => "L5",
            Self::Refused(refusal) => refusal.stage,
        }
    }

    /// The observation recorded after the case id in `observations.tsv`.
    pub fn row(&self) -> String {
        match self {
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
    /// Lowers files that all belong to the root package, which has no admitted dependency.
    pub fn observe(
        self,
        files: &[(&str, Vec<u8>)],
    ) -> Observation {
        let sources: Vec<_> = files
            .iter()
            .map(|(path, bytes)| (super::pipeline::ROOT, *path, bytes.as_slice()))
            .collect();
        self.observe_admitted(&sources, &[])
    }

    /// Lowers `(package, file, bytes)` sources with the dependency edges Build admitted for them.
    pub fn observe_admitted(
        self,
        sources: &[(&str, &str, &[u8])],
        admitted: &[omega_resolver::Dependency],
    ) -> Observation {
        let actual = match self {
            Self::MustRefuse(_) => match super::pipeline::lower_admitted(sources, admitted) {
                Ok(_) => Actual::Product { witness: None },
                Err(refusal) => Actual::Refused(refusal),
            },
            Self::MustLower(check) | Self::TrackCompatibility(check) | Self::ExpectCheckerRefusal(check) => {
                match super::pipeline::lower_admitted(sources, admitted) {
                    Ok(product) => Actual::Product {
                        witness: Some(check.matches(&product)),
                    },
                    Err(refusal) => Actual::Refused(refusal),
                }
            }
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
/// Ratified decision `no-data-code-attachment` retires `machine Main::main`.
pub const CROSS_WIDTH_OWNER: &str = "L2\tmain.omg:20:9\ta data carrier cannot own a machine";
pub const RATIONAL_LANDING_OWNER: &str = "L2\tmain.omg:25:9\ta data carrier cannot own a machine";
pub const IMPORT_A: (&str, &[u8]) = ("m/a.omg", include_bytes!("../fixtures/direct_file_imports/m/a.omg"));
pub const IMPORT_B: (&str, &[u8]) = ("m/b.omg", include_bytes!("../fixtures/direct_file_imports/m/b.omg"));
/// The historical sample imports `m\a` by its filename. A filename names no namespace component.
pub const FILE_IMPORT: &str = "L2\tm/b.omg:1:5\tunavailable import m\\a";

#[derive(Clone, Copy)]
pub enum Check {
    EmptyMain,
    ModuleConstants,
    NestedField,
    Comparisons,
    IdentityCalls,
    Premise(bool),
    /// The root's `entry` returns the result of calling `other_v` in the `mod_kernel` package.
    DependencyCall,
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
            Self::DependencyCall => returns_dependency_call(product),
        }
    }
}

fn returns_dependency_call(package: &LoweredPackage) -> bool {
    let owned = |owner: &str, name: &str| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.package == owner && m.declaration.path == name)
    };
    let (Some(entry), Some(callee)) = (owned(super::pipeline::ROOT, "entry"), owned(DEPENDENCY, "other_v")) else {
        return false;
    };
    let [block] = &entry.body.blocks[..] else { return false };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = block.terminator
    else {
        return false;
    };
    returns_literal(callee, &ScalarLiteral::Integer(7.into()))
        && entry.body.operations.iter().any(|op| {
            op.result == OperationResult::Scalar(returned)
                && matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), arguments, .. } if *id == callee.id && arguments.is_empty())
        })
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
pub const NONMAIN: &str = "probes/build_nonmain_module";
const DEPENDENCY: &str = "mod_kernel";
const DEPENDENCY_PACKAGE: &str = r#"Build::package(builder, "mod_kernel")"#;
/// `probes/build_nonmain_module/build.omg` registers `dep` under the alias `kit`.
pub const KIT: &[Admitted] = &[Admitted {
    alias: "kit",
    package: DEPENDENCY,
    directory: "probes/build_nonmain_module/dep",
    files: &["main.omg", "other.omg"],
    configuration: DEPENDENCY_PACKAGE,
}];

pub const CASES: &[Case] = &[
    Case {
        id: "standalone",
        directory: "basics/standalone",
        files: MAIN,
        migrated: None,
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::EmptyMain),
    },
    Case {
        id: "module-library",
        directory: "probes/build_nonmain_module/dep",
        files: &["main.omg", "other.omg"],
        migrated: None,
        configuration: DEPENDENCY_PACKAGE,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::ModuleConstants),
    },
    Case {
        id: "cross-width",
        directory: "probes/cross_width_compare",
        files: MAIN,
        migrated: None,
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::MustRefuse(CROSS_WIDTH_OWNER),
    },
    Case {
        id: "rational-landing",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        migrated: None,
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::MustRefuse(RATIONAL_LANDING_OWNER),
    },
    // The same programs with `Main::main` declared as the free machine `main`.
    Case {
        id: "cross-width-v2",
        directory: "probes/cross_width_compare",
        files: MAIN,
        migrated: Some(&[("main.omg", include_bytes!("../fixtures/cross_width_compare/main.omg"))]),
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::TrackCompatibility(Check::Comparisons),
    },
    Case {
        id: "rational-landing-v2",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        migrated: Some(&[("main.omg", include_bytes!("../fixtures/rational_arg_landing/main.omg"))]),
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::TrackCompatibility(Check::IdentityCalls),
    },
    Case {
        id: "direct-imports",
        directory: DIRECT,
        files: IMPORTS,
        migrated: None,
        configuration: FREESTANDING,
        dependencies: &[],
        expectation: Expectation::MustRefuse(FILE_IMPORT),
    },
    Case {
        id: "direct-imports-v2",
        directory: DIRECT,
        files: IMPORTS,
        migrated: Some(&[
            ("main.omg", include_bytes!("../fixtures/direct_file_imports/main.omg")),
            IMPORT_A,
            IMPORT_B,
        ]),
        configuration: FREESTANDING,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::NestedField),
    },
    Case {
        id: "missing-import-v2",
        directory: DIRECT,
        files: IMPORTS,
        migrated: Some(&[
            ("main.omg", include_bytes!("../fixtures/direct_file_imports/missing_import.omg")),
            IMPORT_A,
            IMPORT_B,
        ]),
        configuration: FREESTANDING,
        dependencies: &[],
        expectation: Expectation::MustRefuse(MISSING_IMPORT),
    },
    Case {
        id: "abstract-premise",
        directory: "facts/abstract_signature_call/local",
        files: MAIN,
        migrated: None,
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::TrackCompatibility(Check::Premise(true)),
    },
    Case {
        id: "false-abstract-premise",
        directory: "facts/abstract_signature_call/invalid_local",
        files: MAIN,
        migrated: None,
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::ExpectCheckerRefusal(Check::Premise(false)),
    },
    Case {
        id: "import-from-dependency",
        directory: NONMAIN,
        files: MAIN,
        migrated: None,
        configuration: r#"with_alias(@mut kit_dependency, "kit")"#,
        dependencies: KIT,
        expectation: Expectation::TrackCompatibility(Check::DependencyCall),
    },
];
