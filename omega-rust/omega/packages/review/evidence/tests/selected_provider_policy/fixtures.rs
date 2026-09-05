use crate::support::*;
use target::TargetProfile;

pub(super) const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
}
"#;

pub(super) struct Fixture {
    pub checked: CheckedCompilation,
    pub target: TargetProfile,
    _package: TempPackage,
    _dependency: Option<TempPackage>,
}

pub(super) fn foreign_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([42; 32]).unwrap()
}

impl Fixture {
    pub fn local(source: &str, build: &str, target: TargetProfile) -> Self {
        Self::new(source, build, target, None)
    }

    pub fn foreign(source: &str, producer: &str, target: TargetProfile) -> Self {
        Self::new(source, BUILD, target, Some(producer))
    }

    pub fn without_typed_via(&self, machine: &str) -> CheckedCompilation {
        let mut changed = self.checked.clone();
        let conformances = changed
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.name.as_str() == machine)
            .unwrap()
            .satisfies;
        changed
            .typed
            .machine_trait_conformances
            .span_mut_or_empty(conformances)[0]
            .via_expression = typed_trees::expression::ExpressionHandle::invalid();
        changed
    }

    pub fn changed_build_target(&self, prefix: &str, replacement: &str) -> CheckedCompilation {
        use typed_trees::{expression::ExpressionNode, name::Identifier, statement::StatementNode};
        let mut changed = self.checked.clone();
        let build = changed
            .machines()
            .iter()
            .find(|machine| Some(machine.symbol) == changed.selected_build_machine_symbol())
            .unwrap();
        let statements = changed
            .machine_states(build)
            .iter()
            .map(|state| state.statement_nodes)
            .collect::<Vec<_>>();
        let mut replaced = 0;
        let mut expressions = Vec::new();
        for statements in statements {
            for statement in changed.typed.statement_table.statements_mut(statements) {
                match statement {
                    StatementNode::Call(call) if call.target.as_str().starts_with(prefix) => {
                        call.target = Identifier::generated(replacement);
                        replaced += 1;
                    }
                    StatementNode::Expression(expression) => expressions.push(*expression),
                    _ => {}
                }
            }
        }
        for expression in expressions {
            if let ExpressionNode::Call(call) =
                changed.typed.expression_table.expression_mut(expression)
                && call.target.as_str().starts_with(prefix)
            {
                call.target = Identifier::generated(replacement);
                replaced += 1;
            }
        }
        assert_eq!(
            replaced, 1,
            "mutate one authored authoritative build marker"
        );
        changed
    }

    fn new(source: &str, build: &str, target: TargetProfile, producer: Option<&str>) -> Self {
        let package = TempPackage::new();
        let dependency = producer.map(|source| {
            let dependency = TempPackage::new();
            dependency.write("bindings.omg", source);
            dependency
        });
        let mut sources = vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            package.0.clone(),
        )];
        let mut dependencies = Vec::new();
        if let Some(dependency) = &dependency {
            sources.push(PackageSourceBinding::new(
                foreign_identity(),
                "binding-producer",
                dependency.0.clone(),
            ));
            dependencies.push(PackageDependencyBinding::new(
                package_identity(),
                "producer",
                foreign_identity(),
            ));
        }
        if source.contains("use calling;") {
            let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(5)
                .unwrap();
            package.write(
                "calling.omg",
                &fs::read_to_string(repository.join("source/library/std/calling.omg"))
                    .unwrap()
                    .replace("\ndata ", "\npub data ")
                    .replace("\ntrait ", "\npub trait ")
                    .replace("\ndomain ", "\npub domain "),
            );
        }
        package.write("main.omg", source);
        package.write("build.omg", build);
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, dependencies)
                .unwrap();
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target.target_name()),
            inputs,
        )
        .unwrap_or_else(|diagnostics| {
            panic!("provider policy fixture should check: {diagnostics:#?}")
        });
        Self {
            checked,
            target,
            _package: package,
            _dependency: dependency,
        }
    }
}

pub(super) fn import_producer(indirect: bool, export: &str) -> String {
    let value = format!(
        r#"Binding::DllImport {{
        import: DllImport::PeByName {{ library: "kernel32.dll", export: "{export}" }},
    }}"#
    );
    let body = if indirect {
        format!("let selected: Binding<12, 11, 0> = {value};\n    selected")
    } else {
        value
    };
    format!(
        r#"use omega::language::core::external_binding;
pub windows_x86_64 machine import_binding() -> Binding<12, 11, 0> {{
    {body}
}}
"#
    )
}

pub(super) const IMPORT_LEAF: &str = r#"pub boundary trait Host { machine ping(); }
pub machine ping_leaf() satisfies Host::ping via import_binding();
"#;

pub(super) const SYSCALL: &str = r#"use omega::language::core::external_binding;
pub boundary trait Process { machine exit(code: i32); }
pub linux_x86_64 machine exit_binding() -> Binding<0, 0, 0> {
    Binding::Syscall { number: 60 }
}
pub machine exit_leaf(code: i32) satisfies Process::exit via exit_binding();
"#;

pub(super) const FAMILY: &str = r#"pub data CheckedMath {}
pub boundary operator CheckedMath::convert(value: u64) -> u64;
pub boundary operator CheckedMath::convert(value: i32) -> i32;
pub data ConvertProvider {}
pub machine ConvertProvider::convert_u64(input: u64) -> u64
satisfies CheckedMath::convert { input }
pub machine ConvertProvider::convert_i32(input: i32) -> i32
satisfies CheckedMath::convert { input }
"#;

pub(super) const FAMILY_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<CheckedMath::convert, ConvertProvider>();
}
"#;

pub(super) const GENERIC: &str = r#"pub data GenericMath {}
pub boundary operator GenericMath::identity<Element>(value: Element) -> Element;
pub data GenericProvider {}
pub machine GenericProvider::identity<Value>(value: Value) -> Value
satisfies GenericMath::identity { value }
pub machine exercise_i32(value: i32) -> i32 { GenericMath::identity(value) }
pub machine exercise_u64(value: u64) -> u64 { GenericMath::identity(value) }
"#;

pub(super) const INHERITED: &str = r#"use calling;
pub data HostPolicy {}
pub HostPolicyCalling: HostPolicy satisfies CallingPolicy;
pub machine HostPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::MicrosoftX64;
    output.call.parameter_count = 1;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = 8;
    output.call.parameters[0].shape.alignment = 8;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rcx, value_byte_offset: 0, byte_size: 8,
    };
    output.call.has_result = true;
    output.call.result.shape.class = AbiValueClass::Integer;
    output.call.result.shape.byte_size = 8;
    output.call.result.shape.alignment = 8;
    output.call.result.location_count = 1;
    output.call.result.locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rax, value_byte_offset: 0, byte_size: 8,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}
pub boundary trait BaseHost { machine ping(value: u64) -> u64; }
pub boundary trait SelectedHost: BaseHost + Calling<HostPolicy> {}
pub data HostProvider {}
HostProviderSelected: HostProvider satisfies SelectedHost;
pub machine HostProvider::ping(value: u64) -> u64
    satisfies BaseHost::ping { value }
"#;
