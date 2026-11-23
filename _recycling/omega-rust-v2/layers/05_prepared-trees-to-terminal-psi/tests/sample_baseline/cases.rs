use super::pipeline::Refusal;
use omega_psi::{
    BlockTerminator, CallArgument, ContractClause, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceProjection,
    PredicateStep, ReturnEdge, ScalarLiteral, ScalarType,
};
use psi_handoff_consumer::{Operand, Term};

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
    /// The alias edge the harness supplies with the package's sources.
    pub edge: Edge,
}

/// Which `kit` edge reaches L2. Only `Registered` is what the pinned root recipe admits.
/// The other two are negative controls that keep every source byte and change only the edge.
#[derive(Clone, Copy)]
pub enum Edge {
    Registered,
    /// The package's sources are present, but no edge names them.
    Omitted,
    /// The same alias names this other package of the graph instead.
    Retargeted(&'static str),
}

impl Admitted {
    /// The edge supplied to L2, if any.
    pub fn admitted(&self) -> Option<omega_resolver::Dependency<'static>> {
        let package = match self.edge {
            Edge::Registered => self.package,
            Edge::Omitted => return None,
            Edge::Retargeted(package) => package,
        };
        Some(omega_resolver::Dependency {
            requester: super::pipeline::ROOT,
            alias: self.alias,
            package,
        })
    }
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
        self.inspect_lowering(super::pipeline::lower_admitted(sources, admitted))
    }

    pub(super) fn inspect_lowering(self, lowered: Result<LoweredPackage, Refusal>) -> Observation {
        let actual = match lowered {
            Err(refusal) => Actual::Refused(refusal),
            Ok(product) => {
                for machine in &product.machines {
                    if let Err(reason) = super::control_flow::check_operation_ownership(&machine.body) {
                        return Observation {
                            actual: Actual::Product { witness: None },
                            mismatch: Some(format!("machine {}: {reason}", machine.declaration.path)),
                        };
                    }
                }
                let witness = match self {
                    Self::MustRefuse(_) => None,
                    Self::MustLower(check) | Self::ExpectCheckerRefusal(check) => Some(check.matches(&product)),
                };
                Actual::Product { witness }
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
/// The historical receivers use the retired `@mut Main` spelling.
const CROSS_WIDTH_RECEIVER: &str = "L1\tmain.omg:20:31\texpected `,` or `)` (bytes 497..501)";
const RATIONAL_LANDING_RECEIVER: &str = "L1\tmain.omg:25:31\texpected `,` or `)` (bytes 742..746)";
/// The pinned premise samples use V1's `crash Trap;`, which the V2 parser refuses on purpose.
pub const ABSTRACT_PREMISE_CRASH: &str = "L1\tmain.omg:37:27\texpected `;` (bytes 772..776)";
/// The `false` premise is one byte longer, so the same token starts one byte later.
pub const FALSE_ABSTRACT_PREMISE_CRASH: &str = "L1\tmain.omg:37:27\texpected `;` (bytes 773..777)";
pub const ABSTRACT_PREMISE_V2: (&str, &[u8]) = ("main.omg", include_bytes!("../fixtures/abstract_signature_call/local/main.omg"));
pub const FALSE_ABSTRACT_PREMISE_V2: (&str, &[u8]) = ("main.omg", include_bytes!("../fixtures/abstract_signature_call/invalid_local/main.omg"));
pub const IMPORT_A: (&str, &[u8]) = ("m/a.omg", include_bytes!("../fixtures/direct_file_imports/m/a.omg"));
pub const IMPORT_B: (&str, &[u8]) = ("m/b.omg", include_bytes!("../fixtures/direct_file_imports/m/b.omg"));
/// Parsing the historical receiver precedes resolving the sample’s filename import.
const DIRECT_IMPORT_RECEIVER: &str = "L1\tmain.omg:13:31\texpected `,` or `)` (bytes 285..289)";
/// The historical sample imports the dependency file `other.omg` by its name under the admitted alias.
pub const DEPENDENCY_FILE_IMPORT: &str = "L2\tmain.omg:3:5\tunavailable import kit\\other";
/// The lawful twin's import when `kit` does not select `mod_kernel`.
pub const UNAVAILABLE_DECLARATION_IMPORT: &str = "L2\tmain.omg:3:5\tunavailable import kit\\other_v";
/// The lawful twin replaces only the root file. The dependency files still come from the pinned history.
pub const DEPENDENCY_IMPORT: (&str, &[u8]) = ("main.omg", include_bytes!("../fixtures/dependency_import/main.omg"));

#[derive(Clone, Copy)]
pub enum Check {
    EmptyMain,
    ModuleConstants,
    NestedField,
    Comparisons,
    IdentityCalls,
    /// `premise` returns this literal, and `apply<witness>`'s call through `Observe` owes the
    /// alias premise beside `witness`'s own premise.
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
            Self::Comparisons => psi_handoff_consumer::integer_comparisons(product, super::pipeline::ROOT, "main") == Ok(cross_width_comparisons()),
            Self::IdentityCalls => {
                let (seven, two) = (ScalarLiteral::Integer(7.into()), ScalarLiteral::Integer(2.into()));
                psi_handoff_consumer::call_arguments(product, super::pipeline::ROOT, "main", "identity") == Ok(identity_landings(&seven, &two))
            }
            Self::Premise(value) => {
                named("premise").is_some_and(|m| returns_literal(m, &ScalarLiteral::Boolean(value))) && callback_premise(product).is_ok()
            }
            Self::DependencyCall => returns_dependency_call(product),
        }
    }
}

fn integer(
    signed: bool,
    width: u64,
) -> Option<ScalarType> {
    Some(ScalarType::Integer { signed, width })
}

/// `main.small != main.big && main.mid < main.far && main.tiny == main.small`: each comparison
/// reads two fields of `main`'s formal, each at its own declared carrier, with no conversion between.
pub fn cross_width_comparisons() -> Vec<(IntegerPrimitive, Vec<Operand<'static>>)> {
    let field = |name, carrier| Operand {
        term: Term::FieldRead { formal: 0, fields: vec![name] },
        carrier,
    };
    vec![
        (IntegerPrimitive::NotEqual, vec![field("Main::small", integer(true, 8)), field("Main::big", integer(true, 32))]),
        (IntegerPrimitive::LessThan, vec![field("Main::mid", integer(true, 16)), field("Main::far", integer(true, 64))]),
        (IntegerPrimitive::Equal, vec![field("Main::tiny", integer(false, 8)), field("Main::small", integer(true, 8))]),
    ]
}

/// The value each `identity` call receives, in source order. The anonymous `7 / 2 * 2` and the
/// arithmetic past `u64::MAX` fold exactly to 7. The typed `7i32 / 2 * 2` stays an `i32` division
/// and multiplication, so the carrier decides its result.
pub fn identity_landings<'a>(
    seven: &'a ScalarLiteral,
    two: &'a ScalarLiteral,
) -> Vec<Vec<Operand<'a>>> {
    let i32 = integer(true, 32);
    let constant = |value| Operand {
        term: Term::Constant(value),
        carrier: i32,
    };
    let typed = Operand {
        term: Term::Integer {
            primitive: IntegerPrimitive::Multiply,
            operands: vec![
                Operand {
                    term: Term::Integer {
                        primitive: IntegerPrimitive::Divide,
                        operands: vec![constant(seven), constant(two)],
                    },
                    carrier: i32,
                },
                constant(two),
            ],
        },
        carrier: i32,
    };
    vec![vec![constant(seven)], vec![typed], vec![constant(seven)]]
}

/// One root machine at `path`, or why there is not exactly one.
fn unique<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> Result<&'a LoweredMachine, String> {
    let mut found = package
        .machines
        .iter()
        .filter(|m| m.declaration.package == super::pipeline::ROOT && m.declaration.path == path);
    match (found.next(), found.next()) {
        (Some(machine), None) => Ok(machine),
        _ => Err(format!("expected one machine {path}")),
    }
}

/// The abstract-premise sample's callback, read through the Psi-only consumer. `main` calls the
/// `apply` instance on `true`. That instance's one listed invocation is binder 0, executes `witness`
/// on `apply`'s own formal and owes the alias's one premise. `witness` keeps its own premise, at
/// another site, for the refinement. Each clause applies `premise` to its owner's first formal.
/// Returns the first disagreement.
pub fn callback_premise(package: &LoweredPackage) -> Result<(), String> {
    let (apply, witness, premise) = (unique(package, "apply")?, unique(package, "witness")?, unique(package, "premise")?);
    let applies_premise = |clause: &ContractClause| {
        matches!(&clause.predicate[..], [PredicateStep::Formal(0), PredicateStep::MachineApplication { callee, .. }] if *callee == premise.declaration)
    };
    let rows = psi_handoff_consumer::callback_premises(package, apply).ok_or("malformed callback listing")?;
    let [row] = rows.as_slice() else {
        return Err(format!("expected one callback invocation, found {}", rows.len()));
    };
    let [Parameter::Scalar(kept)] = apply.body.parameters[..] else {
        return Err("apply has no single scalar formal".into());
    };
    if (row.binder, row.selected.id) != (0, witness.id) {
        return Err(format!("binder {} selects {}, not witness", row.binder, row.selected.declaration.path));
    }
    if row.arguments != [CallArgument::Scalar(kept)] {
        return Err("the call does not pass apply's own formal".into());
    }
    let [owed] = row.owed[..] else {
        return Err(format!("the call owes {} alias clauses, not one", row.owed.len()));
    };
    let [provided] = row.provided else {
        return Err(format!("witness provides {} clauses, not one", row.provided.len()));
    };
    let [own] = &apply.contract.requires[..] else {
        return Err("the apply instance does not keep its one premise".into());
    };
    if ![owed, provided, own].into_iter().all(applies_premise) {
        return Err("a clause does not apply premise to its first formal".into());
    }
    if owed.site == provided.site || owed.site == own.site {
        return Err("the owed clause is not the alias's own clause".into());
    }
    let truth = ScalarLiteral::Boolean(true);
    let expected = vec![vec![Operand {
        term: Term::Constant(&truth),
        carrier: Some(ScalarType::Boolean),
    }]];
    if psi_handoff_consumer::call_arguments(package, super::pipeline::ROOT, "main", "apply")? != expected {
        return Err("main does not call the apply instance once on true".into());
    }
    Ok(())
}

/// Read through the Psi-only consumer, so the sample check and the L5 test share one reading.
fn returns_dependency_call(package: &LoweredPackage) -> bool {
    psi_handoff_consumer::returned_call(package, super::pipeline::ROOT, "entry")
        == Ok(psi_handoff_consumer::ReturnedCall {
            package: DEPENDENCY,
            path: "other_v",
            arguments: 0,
            returns: Some(&ScalarLiteral::Integer(7.into())),
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
const KIT_ALIAS: &str = r#"with_alias(@mut kit_dependency, "kit")"#;
const DEPENDENCY: &str = "mod_kernel";
const DEPENDENCY_PACKAGE: &str = r#"Build::package(builder, "mod_kernel")"#;
/// `probes/build_nonmain_module/build.omg` registers `dep` under the alias `kit`.
pub const KIT: &[Admitted] = &[kit(Edge::Registered)];
/// The same sources without the edge.
pub const KIT_WITHOUT_EDGE: &[Admitted] = &[kit(Edge::Omitted)];
/// The same sources with `kit` naming the toolchain package, which every graph contains and
/// whose root namespace declares no `other_v`.
pub const KIT_RETARGETED: &[Admitted] = &[kit(Edge::Retargeted(omega_toolchain::TOOLCHAIN_PACKAGE))];

const fn kit(edge: Edge) -> Admitted {
    Admitted {
        alias: "kit",
        package: DEPENDENCY,
        directory: "probes/build_nonmain_module/dep",
        files: &["main.omg", "other.omg"],
        configuration: DEPENDENCY_PACKAGE,
        edge,
    }
}

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
        expectation: Expectation::MustRefuse(CROSS_WIDTH_RECEIVER),
    },
    Case {
        id: "rational-landing",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        migrated: None,
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::MustRefuse(RATIONAL_LANDING_RECEIVER),
    },
    // The same programs with `Main::main` declared as the free machine `main` and the
    // unimported `Wrapping` removed from the unread `hits` counter.
    Case {
        id: "cross-width-v2",
        directory: "probes/cross_width_compare",
        files: MAIN,
        migrated: Some(&[("main.omg", include_bytes!("../fixtures/cross_width_compare/main.omg"))]),
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::Comparisons),
    },
    Case {
        id: "rational-landing-v2",
        directory: "probes/rational_arg_landing",
        files: MAIN,
        migrated: Some(&[("main.omg", include_bytes!("../fixtures/rational_arg_landing/main.omg"))]),
        configuration: HOST_START,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::IdentityCalls),
    },
    Case {
        id: "direct-imports",
        directory: DIRECT,
        files: IMPORTS,
        migrated: None,
        configuration: FREESTANDING,
        dependencies: &[],
        expectation: Expectation::MustRefuse(DIRECT_IMPORT_RECEIVER),
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
        expectation: Expectation::MustRefuse(ABSTRACT_PREMISE_CRASH),
    },
    Case {
        id: "false-abstract-premise",
        directory: "facts/abstract_signature_call/invalid_local",
        files: MAIN,
        migrated: None,
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::MustRefuse(FALSE_ABSTRACT_PREMISE_CRASH),
    },
    // The same pair with the retired callback binder written as a `const` binder of a callable
    // alias, and a trapping operation in the failure arm. Only `premise`'s returned literal differs.
    Case {
        id: "abstract-premise-v2",
        directory: "facts/abstract_signature_call/local",
        files: MAIN,
        migrated: Some(&[ABSTRACT_PREMISE_V2]),
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::MustLower(Check::Premise(true)),
    },
    Case {
        id: "false-abstract-premise-v2",
        directory: "facts/abstract_signature_call/invalid_local",
        files: MAIN,
        migrated: Some(&[FALSE_ABSTRACT_PREMISE_V2]),
        configuration: HOST_MAIN,
        dependencies: &[],
        expectation: Expectation::ExpectCheckerRefusal(Check::Premise(false)),
    },
    Case {
        id: "import-from-dependency",
        directory: NONMAIN,
        files: MAIN,
        migrated: None,
        configuration: KIT_ALIAS,
        dependencies: KIT,
        expectation: Expectation::MustRefuse(DEPENDENCY_FILE_IMPORT),
    },
    Case {
        id: "import-from-dependency-v2",
        directory: NONMAIN,
        files: MAIN,
        migrated: Some(&[DEPENDENCY_IMPORT]),
        configuration: KIT_ALIAS,
        dependencies: KIT,
        expectation: Expectation::MustLower(Check::DependencyCall),
    },
    Case {
        id: "missing-alias-v2",
        directory: NONMAIN,
        files: MAIN,
        migrated: Some(&[DEPENDENCY_IMPORT]),
        configuration: KIT_ALIAS,
        dependencies: KIT_WITHOUT_EDGE,
        expectation: Expectation::MustRefuse(UNAVAILABLE_DECLARATION_IMPORT),
    },
    Case {
        id: "retargeted-alias-v2",
        directory: NONMAIN,
        files: MAIN,
        migrated: Some(&[DEPENDENCY_IMPORT]),
        configuration: KIT_ALIAS,
        dependencies: KIT_RETARGETED,
        expectation: Expectation::MustRefuse(UNAVAILABLE_DECLARATION_IMPORT),
    },
];
