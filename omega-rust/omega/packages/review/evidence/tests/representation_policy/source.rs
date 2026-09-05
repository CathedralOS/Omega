use crate::support::*;

const TYPES: &str = r#"use omega::language::core::representation;
pub boundary data Token;
pub data Carrier { value: u64; }
pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;
pub boundary data CopyToken [copy];
pub data CopyCarrier [copy] { value: u64; }
pub CopyTokenRepresentation: CopyCarrier satisfies OpaqueRepresentation<CopyToken>;
"#;

const CALLING: &str = r#"use calling;
data TransferPolicy { }
TransferPolicyCallingPolicy: TransferPolicy satisfies CallingPolicy;
machine TransferPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::MicrosoftX64;
    output.call.parameter_count = 1;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = signature.shapes[1].byte_size;
    output.call.parameters[0].shape.alignment = signature.shapes[1].alignment;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rcx, value_byte_offset: 0,
        byte_size: signature.shapes[1].byte_size,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}
boundary trait TransferEntry: Calling<TransferPolicy> {
    machine transfer(value: Token);
}
"#;

pub(super) struct Fixture {
    pub checked: CheckedCompilation,
    _package: TempPackage,
    _dependency: Option<TempPackage>,
}

pub(super) fn foreign_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([42; 32]).unwrap()
}

impl Fixture {
    pub fn new(selected: bool, used: bool, foreign: bool) -> Self {
        Self::with_sources(selected, used, foreign, TYPES, "")
    }

    pub fn moved_selection() -> Self {
        Self::with_sources(
            true,
            false,
            false,
            TYPES,
            "// Selection source custody moved.\n\n",
        )
    }

    pub fn generic_availability() -> Self {
        let types = TYPES.replace(
            "pub TokenRepresentation:",
            "pub TokenRepresentation<'scope, Element, const Marker: u64>:",
        );
        Self::with_sources(false, false, false, &types, "")
    }

    fn with_sources(selected: bool, used: bool, foreign: bool, types: &str, prefix: &str) -> Self {
        assert!(!used || selected, "a runtime opaque requires selection");
        let types = if selected {
            types.to_owned()
        } else {
            types.replace(
                "pub boundary data CopyToken [copy];",
                "pub boundary data CopyToken;",
            )
        };
        let package = TempPackage::new();
        let dependency = foreign.then(TempPackage::new);
        let mut sources = vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            package.0.clone(),
        )];
        let mut dependencies = Vec::new();
        let declarations = if let Some(dependency) = &dependency {
            dependency.write("types.omg", &types);
            sources.push(PackageSourceBinding::new(
                foreign_identity(),
                "representation-producer",
                dependency.0.clone(),
            ));
            dependencies.push(PackageDependencyBinding::new(
                package_identity(),
                "producer",
                foreign_identity(),
            ));
            "use producer::types;\n"
        } else {
            &types
        };
        if used {
            let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(5)
                .unwrap();
            package.write(
                "calling.omg",
                &fs::read_to_string(repository.join("source/library/std/calling.omg")).unwrap(),
            );
        }
        package.write(
            "main.omg",
            &format!("{declarations}{}", if used { CALLING } else { "" }),
        );
        package.write(
            "build.omg",
            &format!(
                r#"{prefix}{}
machine build(builder: &mut Build) {{
    builder.package("review-fixture");
    {}
}}
"#,
                if foreign { "use producer::types;" } else { "" },
                if selected {
                    "builder.select_representation<Token, TokenRepresentation>();\n\
                     builder.select_representation<CopyToken, CopyTokenRepresentation>();"
                } else {
                    ""
                },
            ),
        );
        let inputs =
            PackageCompilationInputs::new_package(package_identity(), sources, dependencies)
                .unwrap();
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            inputs,
        )
        .expect("representation policy source should check without native emission");
        Self {
            checked,
            _package: package,
            _dependency: dependency,
        }
    }

    pub fn changed_carrier(&self) -> CheckedCompilation {
        let mut changed = self.checked.clone();
        let selection = self
            .checked
            .opaque_representation_selections()
            .iter()
            .find(|selection| self.checked.symbols.name(selection.opaque()) == "Token")
            .unwrap();
        let other_carrier = self
            .checked
            .opaque_representation_selections()
            .iter()
            .find(|selection| self.checked.symbols.name(selection.opaque()) == "CopyToken")
            .unwrap()
            .carrier();
        let conformances = changed.typed.roots.conformances;
        changed
            .typed
            .tables
            .conformances
            .span_mut(conformances)
            .unwrap()
            .iter_mut()
            .find(|conformance| conformance.symbol == selection.application().declaration)
            .unwrap()
            .carrier_symbol = other_carrier;
        changed
    }

    pub fn changed_use_type(&self) -> CheckedCompilation {
        let mut changed = self.checked.clone();
        let boundary = changed
            .traits()
            .iter()
            .find(|boundary| boundary.name.as_str() == "TransferEntry")
            .unwrap();
        let parameters = changed.trait_machine_signatures(boundary)[0].parameters;
        let copy = changed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "CopyToken")
            .unwrap()
            .clone();
        let reference = changed.typed.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: copy.symbol,
                name: copy.name,
            },
        );
        changed.typed.state_parameters.span_mut(parameters).unwrap()[0].type_reference = reference;
        changed
    }
}
