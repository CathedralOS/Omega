#[path = "support/source.rs"]
mod sources;

use omega_lowering::{AliasTarget, LoweredPackage, LoweringError, PredicateOperator, PredicateStep, ScalarType, TypeShape};
use omega_typer::Multiplicity;

// The shared helper reports each fixture's outcome, so the readiness runner
// can reconcile every refusal below against tools/spec-corpus/l5_refusals.tsv.
fn lower(files: &[(&str, &str)]) -> Result<LoweredPackage, LoweringError> {
    sources::lower(files)
}

#[test]
fn an_affine_container_retains_the_linear_carrier_only_where_the_payload_exists() {
    let package = lower(&[(
        "main.omg",
        "data Lease [linear] {}
        data Holder { lease:Lease; }
        data Choice { case Empty; case Held(lease:Lease); }",
    )])
    .unwrap();
    let data = |name| {
        package
            .data
            .iter()
            .find(|data| data.declaration.path == name)
            .unwrap()
    };
    let lease = data("Lease");
    assert_eq!(lease.multiplicity, Multiplicity::Linear);
    let holder = data("Holder");
    assert_eq!(holder.multiplicity, Multiplicity::Affine);
    assert_eq!(holder.fields[0].carrier, lease.carrier);
    let choice = data("Choice");
    assert_eq!(choice.multiplicity, Multiplicity::Affine);
    let [empty, held] = choice.cases.as_slice() else {
        panic!("expected Empty and Held");
    };
    assert!(empty.fields.is_empty());
    assert_eq!(held.fields[0].carrier, lease.carrier);
}

#[test]
fn record_and_sum_preserve_declaration_order_and_nested_carry() {
    let lease = (
        "lease.omg",
        "public data Lease [linear, carry(
        suspension: forbidden, cpu: same, thread: any, address: stable,
    )] { key:u64; enabled:bool; }",
    );
    let message = (
        "message.omg",
        "data Message [linear] {
        case Empty;
        case Payload(entries:[Lease; 2], urgent:bool);
    }",
    );
    let package = lower(&[message, lease]).unwrap();
    assert_eq!(package, lower(&[lease, message]).unwrap());
    let [lease, message] = package.data.as_slice() else {
        panic!("expected the record and sum");
    };
    assert_eq!(lease.declaration.path, "Lease");
    assert_eq!(message.declaration.path, "Message");
    assert!(lease.public);
    assert!(!message.public);
    assert_eq!(lease.multiplicity, Multiplicity::Linear);
    assert_eq!(message.multiplicity, Multiplicity::Linear);
    let carry = lease.carry.as_ref().unwrap();
    assert!(!carry.allows_suspension);
    assert!(carry.requires_same_cpu);
    assert!(!carry.requires_same_thread);
    assert!(carry.requires_stable_address);
    assert!(message.carry.is_none());
    assert_eq!(package.types.get(lease.carrier), Some(&TypeShape::Nominal(lease.id)));
    assert_eq!(package.types.get(message.carrier), Some(&TypeShape::Nominal(message.id)));
    let [key, enabled] = lease.fields.as_slice() else {
        panic!("expected record fields");
    };
    assert_eq!(key.declaration.path, "Lease::key");
    assert_eq!(enabled.declaration.path, "Lease::enabled");
    assert_eq!(
        package.types.get(key.carrier),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 }))
    );
    assert_eq!(package.types.get(enabled.carrier), Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean)));
    assert!(lease.cases.is_empty());
    assert!(message.fields.is_empty());
    let [empty, payload] = message.cases.as_slice() else {
        panic!("expected both sum cases");
    };
    assert_eq!(empty.declaration.path, "Message::Empty");
    assert!(empty.fields.is_empty());
    assert_eq!(payload.declaration.path, "Message::Payload");
    let [entries, urgent] = payload.fields.as_slice() else {
        panic!("expected both payload fields");
    };
    assert_eq!(entries.declaration.path, "Message::Payload::entries");
    assert_eq!(urgent.declaration.path, "Message::Payload::urgent");
    assert_eq!(
        package.types.get(entries.carrier),
        Some(&TypeShape::FixedArray {
            element: lease.carrier,
            length: 2
        })
    );
    assert_eq!(urgent.carrier, enabled.carrier);
}

#[test]
fn stable_numbers_refer_to_authored_members_in_their_own_scope() {
    let source = "data Packet {
        #9 status:Status;
        data Status { #9 case Ready; retired #0; #1 case Done(retired #8, #9 count:u64, #0 flag:bool); }
        retired #3; #0 retired:u8; retired #2;
    }
    data Empty { retired #0; }
    data Plain { retired:u8; }";
    let package = lower(&[("schema.omg", source)]).unwrap();
    let data = |path| {
        package
            .data
            .iter()
            .find(|d| d.declaration.path == path)
            .unwrap()
    };
    let packet = data("Packet");
    let canonical = |scope: &omega_lowering::SchemaNumbering| {
        scope
            .members
            .iter()
            .map(|m| (m.number, m.declaration_index))
            .collect::<Vec<_>>()
    };
    let numbering = packet.numbering.as_ref().unwrap();
    assert_eq!(canonical(numbering), [(0, 1), (9, 0)]);
    assert_eq!(numbering.retired, [2, 3]);
    assert_eq!(packet.fields[0].declaration.path, "Packet::status");
    assert_eq!(packet.fields[1].declaration.path, "Packet::retired");
    let status = data("Packet::Status");
    let numbers = status.numbering.as_ref().unwrap();
    assert_eq!(canonical(numbers), [(1, 1), (9, 0)]);
    assert_eq!(numbers.retired, [0]);
    let [ready, done] = status.cases.as_slice() else {
        panic!("expected both cases in authored order")
    };
    assert_eq!(ready.declaration.path, "Packet::Status::Ready");
    assert!(ready.numbering.is_none());
    assert_eq!(done.declaration.path, "Packet::Status::Done");
    let payload = done.numbering.as_ref().unwrap();
    assert_eq!(canonical(payload), [(0, 1), (9, 0)]);
    assert_eq!(payload.retired, [8]);
    assert_eq!(done.fields[1].declaration.path, "Packet::Status::Done::flag");
    assert!(data("Plain").numbering.is_none());
    let empty = data("Empty").numbering.as_ref().unwrap();
    assert!(empty.members.is_empty());
    assert_eq!(empty.retired, [0]);
}

#[test]
fn data_templates_retain_the_type_parameter_and_nominal_results_keep_structural_identity() {
    let echo = lower(&[("item.omg", "data Item [copy] { key:u64; } machine echo(x:Item)->Item{x}")]).unwrap();
    assert!(matches!(echo.machines[0].body.parameters[..], [omega_lowering::Parameter::Structural(_)]));
    let package = lower(&[("item.omg", "data Item<T> { value:T; }")]).unwrap();
    let template = &package.data_templates[0];
    assert_eq!(
        template.fields[0].ty.0,
        [omega_lowering::TypeExpressionStep::GenericParameter {
            owner: template.declaration.clone(),
            index: 0
        }]
    );
}

#[test]
fn default_domains_retain_truth_and_exact_fields_after_preparation_is_dropped() {
    let source = "public data Never requires false {}
                  data Other requires true {}
                  data Flags { allowed:bool; denied:bool; }
                  data Gate requires self.flags.allowed && !self.flags.denied { flags:Flags; }";
    let package = lower(&[("domains.omg", source)]).unwrap();
    let [flags, gate, never, other] = package.data.as_slice() else {
        panic!("expected four data schemas")
    };
    assert!(flags.requires.is_empty());
    assert_eq!(never.requires[0].predicate, [PredicateStep::Boolean(false)]);
    assert_eq!(other.requires[0].predicate, [PredicateStep::Boolean(true)]);
    let [clause] = gate.requires.as_slice() else {
        panic!("expected one default-domain predicate")
    };
    assert_eq!(
        clause.predicate,
        [
            PredicateStep::SelfValue,
            PredicateStep::Field(gate.fields[0].declaration.clone()),
            PredicateStep::Field(flags.fields[0].declaration.clone()),
            PredicateStep::SelfValue,
            PredicateStep::Field(gate.fields[0].declaration.clone()),
            PredicateStep::Field(flags.fields[1].declaration.clone()),
            PredicateStep::Apply(PredicateOperator::Not),
            PredicateStep::Apply(PredicateOperator::And),
        ]
    );
    assert_eq!(clause.site.file, "domains.omg");
}

#[test]
fn data_requirement_applies_the_selected_predicate_at_its_occurrence() {
    let source = "machine predicate()->bool reaches none crashes none terminates {true}
                  data Guarded requires true && predicate() {}";
    let package = lower(&[("guard.omg", source)]).unwrap();
    let schema = &package.data[0];
    let predicate = &package.machines[0].declaration;
    assert_eq!((schema.declaration.path.as_str(), predicate.path.as_str()), ("Guarded", "predicate"));
    spec_corpus::observe_handoff!(
        psi "psi-output/data-requirement-call",
        assert!(matches!(schema.requires[0].predicate.as_slice(), [PredicateStep::Boolean(true), PredicateStep::MachineApplication {callee, site, ..}, PredicateStep::Apply(PredicateOperator::And)] if callee == predicate && site.span.slice(source.as_bytes()) == Some(b"predicate()"))),
    );
}

#[test]
fn inline_data_inherits_its_owners_resolved_visibility() {
    for (modifier, public) in [("public ", true), ("", false)] {
        let source = format!("{modifier}data Envelope {{ body: data Payload {{ case Empty; }}; }}");
        let package = lower(&[("envelope.omg", &source)]).unwrap();
        let [envelope, payload] = package.data.as_slice() else {
            panic!("expected the enclosing and inline data declarations");
        };
        assert_eq!(envelope.declaration.path, "Envelope");
        assert_eq!(payload.declaration.path, "Envelope::Payload");
        assert_eq!(envelope.public, public);
        assert_eq!(payload.public, public);
        assert_eq!(envelope.fields[0].carrier, payload.carrier);
    }
}

#[test]
fn transparent_aliases_share_the_underlying_schema_and_field_carrier() {
    let source = "data Original { bit:bool; }
                  data First = Original;
                  data Container {
                      original:Original;
                      through_alias:Second;
                      data Second = First;
                  }";
    let file = "alias.omg";
    let package = lower(&[(file, source)]).unwrap();
    spec_corpus::observe_handoff!("product/l5", {
        let [container, original] = package.data.as_slice() else {
            panic!("expected only the two nominal declarations");
        };
        assert_eq!(container.declaration.path, "Container");
        assert_eq!(original.declaration.path, "Original");
        assert_ne!(container.id, original.id);
        let [direct, through_alias] = container.fields.as_slice() else {
            panic!("expected distinct field declarations");
        };
        assert_eq!(direct.declaration.path, "Container::original");
        assert_eq!(through_alias.declaration.path, "Container::through_alias");
        let [second, first] = package.aliases.as_slice() else {
            panic!("expected both alias declarations in sorted order");
        };
        assert_eq!(second.declaration.path, "Container::Second");
        assert_eq!(first.declaration.path, "First");
        assert!(!second.public);
        assert!(!first.public);
        assert_eq!(second.target, AliasTarget::Declaration(first.declaration.clone()));
        assert_eq!(first.target, AliasTarget::Declaration(original.declaration.clone()));
        let authored_site = |declaration: &str| {
            let start = source.find(declaration).unwrap();
            omega_lowering::Site {
                package: "app".into(),
                file: file.into(),
                span: omega_lowering::SourceSpan::new(start, start + declaration.len()),
            }
        };
        assert_eq!(first.site, authored_site("data First = Original;"));
        assert_eq!(second.site, authored_site("data Second = First;"));
        assert_eq!(direct.carrier, original.carrier);
        assert_eq!(through_alias.carrier, original.carrier);
        assert_eq!(package.types.get(original.carrier), Some(&TypeShape::Nominal(original.id)));
    });
}

#[test]
fn alias_multiplicity_asserts_the_expansion_without_creating_an_owner() {
    let source = "data Alias [copy] = u64;";
    let plain = lower(&[("alias.omg", "data Alias = u64;")]).unwrap();
    let files = [("alias.omg", source)];
    let package = {
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let typed = prepared.typed();
        let (id, target) = typed.type_alias_targets.iter().find(|(id, _)| id.path == "Alias").unwrap();
        assert_eq!(target, &omega_typer::ValueType::Integer { signed: false, width: Some(64) });
        assert_eq!(typed.type_alias_properties[id].multiplicity, Multiplicity::Copy);
        assert!(typed.data_properties.iter().all(|(nominal, _)| nominal != id));
        sources::lower_prepared(&prepared, &files).unwrap()
    };
    let ([alias], [unmarked]) = (package.aliases.as_slice(), plain.aliases.as_slice()) else {
        panic!("expected one alias in each package");
    };
    assert_eq!(alias.declaration.path, "Alias");
    assert_eq!(alias.site.span.slice(source.as_bytes()), Some(source.as_bytes()));
    assert_eq!(unmarked.target, AliasTarget::Builtin("u64".into()));
    spec_corpus::observe_handoff!(psi "psi-output/alias-multiplicity-assertion",
        assert_eq!(
            (alias.asserted_multiplicity, unmarked.asserted_multiplicity, &alias.target, package.data.len()),
            (Some(Multiplicity::Copy), None, &unmarked.target, 0)
        ),
    );

    let source = "data Lease [linear] {} data Handle [linear] = Lease; data Slot { handle:Handle; }";
    let package = lower(&[("alias.omg", source)]).unwrap();
    let ([handle], [lease, slot]) = (package.aliases.as_slice(), package.data.as_slice()) else {
        panic!("the alias should introduce no third schema");
    };
    assert_eq!(handle.asserted_multiplicity, Some(Multiplicity::Linear));
    assert_eq!(handle.target, AliasTarget::Declaration(lease.declaration.clone()));
    assert_eq!(lease.multiplicity, Multiplicity::Linear);
    assert_eq!(slot.fields[0].carrier, lease.carrier);
}

#[test]
fn alias_multiplicity_cannot_override_the_expansion() {
    for source in ["data Alias [linear] = u64;", "data Alias [copy] = @mut u64;", "data Alias<T> [copy] = T;"] {
        let resolved = sources::resolve(&[("alias.omg", source)]);
        let error = omega_preparation::prepare_source_package(&resolved).unwrap_err();
        assert_eq!(error.message, "a transparent alias cannot change its target's multiplicity");
        assert_eq!(error.file, "alias.omg");
        assert_eq!(error.span.slice(source.as_bytes()), Some(source.as_bytes()));
    }
}

#[test]
fn alias_assertion_survives_structural_and_generic_expansion() {
    use omega_lowering::{DeclarationTerm, TemplateBinder, TypeExpression, TypeExpressionStep};
    let source = "data Words [copy] = [u64;2]; data Same<T [copy]> [copy] = T;";
    let package = lower(&[("alias.omg", source)]).unwrap();
    let [_, words] = package.aliases.as_slice() else { panic!("expected Same and Words") };
    let AliasTarget::Structural(ty) = words.target else { panic!("expected array type row") };
    let Some(TypeShape::FixedArray { element, length: 2 }) = package.types.get(ty) else { panic!("expected two elements") };
    assert_eq!(package.types.get(*element), Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 })));
    assert_eq!(words.asserted_multiplicity, Some(Multiplicity::Copy));
    let (same, expansion) = alias_expansion(&package, "Same");
    assert_eq!(same.asserted_multiplicity, Some(Multiplicity::Copy));
    assert_eq!(expansion.parameters, [TemplateBinder::Type { multiplicity: Some(Multiplicity::Copy) }]);
    assert_eq!(expansion.expansion, DeclarationTerm::Type(TypeExpression(vec![TypeExpressionStep::GenericParameter {
        owner: same.declaration.clone(), index: 0,
    }])));
}

/// The original case: `u64` demands nothing, so an alias that forbids suspension
/// and pins the CPU would change `u64` itself. Typing refuses it before lowering.
#[test]
fn alias_carry_cannot_add_a_demand_to_its_expansion() {
    let source = "// alias\r\ndata Alias [copy,carry(suspension:forbidden,cpu:same,thread:any,address:movable,)] = u64;";
    let resolved = sources::resolve(&[("alias.omg", source)]);
    let error = omega_preparation::prepare_source_package(&resolved).unwrap_err();
    assert_eq!(error.message, "a transparent alias cannot change its target's carry");
    assert_eq!(error.file, "alias.omg");
    let attribute = "carry(suspension:forbidden,cpu:same,thread:any,address:movable,)";
    assert_eq!(error.span.slice(source.as_bytes()), Some(attribute.as_bytes()));
    let control = lower(&[("alias.omg", "// alias\r\ndata Alias [copy] = u64;")]).unwrap();
    let [alias] = control.aliases.as_slice() else { panic!("one alias") };
    assert_eq!((alias.asserted_multiplicity, &alias.target), (Some(Multiplicity::Copy), &AliasTarget::Builtin("u64".into())));
}

/// A list equal to `u64`'s derived carry adds no demand. Whether an alias may
/// author it at all is an open spec question, so lowering still stops there.
#[test]
fn alias_with_carry_stops_at_its_attributes() {
    let source = "// alias\r\ndata Alias [copy,carry(suspension:allowed,cpu:any,thread:any,address:movable,)] = u64;";
    let files = [("alias.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let (id, target) = typed.type_alias_targets.iter().find(|(id, _)| id.path == "Alias").unwrap();
    assert_eq!(target, &omega_typed::ValueType::Integer { signed: false, width: Some(64) });
    let properties = &typed.type_alias_properties[id];
    assert_eq!(properties.multiplicity, Multiplicity::Copy);
    assert_eq!(properties.carry, Some(omega_typed::Carry {
        allows_suspension: true, requires_same_cpu: false, requires_same_thread: false, requires_stable_address: false,
    }));
    assert!(typed.data_properties.iter().all(|(nominal, _)| nominal != id));
    sources::refuse_prepared(
        &prepared,
        &files,
        "alias.omg",
        sources::unique_span(source, "[copy,carry(suspension:allowed,cpu:any,thread:any,address:movable,)]"),
        "transparent alias carry policy requires a checked relation to its expansion",
    );
}

#[test]
fn parked_alias_uses_the_selected_nominal_carriers_carry() {
    use omega_lowering::{OwnedCustody, Parameter};
    for (selected, allowed) in [("Mobile", true), ("Fixed", false)] {
        let source = format!("data Mobile [carry(suspension:allowed,cpu:any,thread:any,address:movable,)] {{key:u64;}}
            data Fixed [carry(suspension:forbidden,cpu:same,thread:any,address:stable,)] {{key:u64;}}
            data Alias = {selected};
            data Held {{item:Alias;}}
            machine wait() suspends {{}}
            machine hold(x:Held)->Held suspends {{suspend wait();x}}");
        let package = lower(&[("alias.omg", &source)]).unwrap();
        let [alias] = package.aliases.as_slice() else { panic!("one alias") };
        assert_eq!(alias.declaration.path, "Alias");
        let target = package.data.iter().find(|d| d.declaration.path == selected).unwrap();
        assert_eq!(package.data.len(), 3);
        let held = package.data.iter().find(|d| d.declaration.path == "Held").unwrap();
        let [item] = held.fields.as_slice() else { panic!("one live field") };
        assert_eq!(item.declaration.path, "Held::item");
        let body = &package.machines.iter().find(|m| m.declaration.path == "hold").unwrap().body;
        let [crossing] = body.suspension_sites.as_slice() else { panic!("one suspension") };
        let [owner] = crossing.retained_owners.as_slice() else { panic!("one retained owner") };
        assert_eq!(body.parameters, [Parameter::Structural(owner.place)]);
        assert_eq!(owner.custody, OwnedCustody::Caller);
        assert!(owner.moved.is_empty());
        spec_corpus::observe_handoff!(psi "psi-output/parked-alias-target-carry",
            assert_eq!(
                (&alias.target, package.types.get(owner.ty), package.types.get(item.carrier), target.carry.as_ref()),
                (&AliasTarget::Declaration(target.declaration.clone()), Some(&TypeShape::Nominal(held.id)), Some(&TypeShape::Nominal(target.id)),
                    Some(&omega_typed::Carry { allows_suspension: allowed, requires_same_cpu: !allowed,
                        requires_same_thread: false, requires_stable_address: !allowed }))
            ),
        );
    }
}

fn alias_expansion<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> (&'a omega_lowering::LoweredAlias, &'a omega_lowering::AliasExpansion) {
    let alias = package
        .aliases
        .iter()
        .find(|alias| alias.declaration.path == path)
        .unwrap_or_else(|| panic!("missing alias {path}"));
    let AliasTarget::Expansion(expansion) = &alias.target else {
        panic!("{path} should hand off its expansion: {:?}", alias.target)
    };
    (alias, expansion)
}

#[test]
fn reference_and_template_application_aliases_hand_off_their_exact_expansion() {
    use omega_lowering::{DeclarationTerm, TemplateAccess, TypeExpression, TypeExpressionStep as Term};
    let source = "data Reference = @mut u64; data Boxed = Box<u64>; data Box<T> { value:T; }";
    let package = lower(&[("alias.omg", source)]).unwrap();
    let boxed = package
        .data_templates
        .iter()
        .find(|t| t.declaration.path == "Box")
        .unwrap();
    let word = Term::Scalar(ScalarType::Integer { signed: false, width: 64 });
    let (_, reference) = alias_expansion(&package, "Reference");
    assert!(reference.parameters.is_empty() && reference.head.is_none());
    assert_eq!(
        reference.expansion,
        DeclarationTerm::Type(TypeExpression(vec![word.clone(), Term::Reference(TemplateAccess::Mut)]))
    );
    let (alias, applied) = alias_expansion(&package, "Boxed");
    assert_eq!(alias.site.span.slice(source.as_bytes()), Some(b"data Boxed = Box<u64>;".as_slice()));
    assert_eq!(applied.head.as_ref(), Some(&boxed.declaration));
    assert_eq!(
        applied.expansion,
        DeclarationTerm::Type(TypeExpression(vec![
            word,
            Term::Named {
                declaration: boxed.declaration.clone(),
                argument_count: 1
            }
        ]))
    );
    assert!(package.data.iter().all(|data| data.declaration.path != "Boxed"));
}

#[test]
fn generic_and_concrete_aliases_retain_exact_binders_and_normalized_expansion() {
    use omega_lowering::{DeclarationTerm, TemplateBinder, TypeExpression, TypeExpressionStep as Term};
    let definitions = ("types.omg", "public data Buffer<Limit:u32> { value:u32; } data Cell<T> { item:T; }");
    let aliases = (
        "aliases.omg",
        "data Same<T> = T;
         data Wrap<T> = Cell<T>;
         public data Window<const Limit:u32> = Buffer<Limit>;
         data Bytes = Buffer<4>;
         data Chained = Window<9>;",
    );
    let package = lower(&[aliases, definitions]).unwrap();
    assert_eq!(package, lower(&[definitions, aliases]).unwrap());
    let [buffer] = package.data.as_slice() else {
        panic!("aliases must not add nominal schemas")
    };
    assert_eq!(buffer.declaration.path, "Buffer");
    let buffer = &buffer.declaration;
    let cell = &package
        .data_templates
        .iter()
        .find(|t| t.declaration.path == "Cell")
        .unwrap()
        .declaration;
    let parameter = |owner: &omega_lowering::LoweredAlias| Term::GenericParameter {
        owner: owner.declaration.clone(),
        index: 0,
    };
    let named = |declaration: &omega_lowering::DeclarationId| Term::Named {
        declaration: declaration.clone(),
        argument_count: 1,
    };

    let (same, expansion) = alias_expansion(&package, "Same");
    assert_eq!(expansion.parameters, [TemplateBinder::Type { multiplicity: None }]);
    assert!(expansion.head.is_none() && expansion.enclosing_parameters.is_empty());
    assert_eq!(expansion.expansion, DeclarationTerm::Type(TypeExpression(vec![parameter(same)])));

    let (wrap, expansion) = alias_expansion(&package, "Wrap");
    assert_eq!(expansion.head.as_ref(), Some(cell));
    assert_eq!(expansion.expansion, DeclarationTerm::Type(TypeExpression(vec![parameter(wrap), named(cell)])));

    let (window, expansion) = alias_expansion(&package, "Window");
    assert!(window.public && !same.public);
    let u32_carrier = ScalarType::Integer { signed: false, width: 32 };
    assert_eq!(
        expansion.parameters,
        [TemplateBinder::Const(TypeExpression(vec![Term::Scalar(
            u32_carrier
        )]))]
    );
    assert_eq!(expansion.head.as_ref(), Some(buffer));
    spec_corpus::observe_handoff!(
        psi "psi-output/generic-alias",
        assert_eq!(
            expansion.expansion,
            DeclarationTerm::Type(TypeExpression(vec![parameter(window), named(buffer)]))
        ),
    );

    // Closed applications keep the selected carrier and canonical value; a chain
    // through another alias records that head but stores the complete expansion.
    for (path, head, extent) in [("Bytes", buffer, "4"), ("Chained", &window.declaration, "9")] {
        let (_, expansion) = alias_expansion(&package, path);
        assert!(expansion.parameters.is_empty(), "{path}");
        assert_eq!(expansion.head.as_ref(), Some(head), "{path}");
        let DeclarationTerm::Applied { declaration, arguments } = &expansion.expansion else {
            panic!("{path} should apply Buffer to a static value")
        };
        assert_eq!(declaration, buffer);
        let [DeclarationTerm::StaticInteger { carrier, value }] = arguments.as_slice() else {
            panic!("{path} should retain one static argument")
        };
        assert_eq!((*carrier, value.to_string().as_str()), (u32_carrier, extent));
    }
}

#[test]
fn aliases_preserve_direct_and_inherited_visibility_and_builtin_identity() {
    let base = ("base.omg", "public data Scalar = u64;");
    let envelope = ("envelope.omg", "public data Envelope { value:Word; data Word = Scalar; }");
    let package = lower(&[envelope, base]).unwrap();
    assert_eq!(package, lower(&[base, envelope]).unwrap());
    let [word, scalar] = package.aliases.as_slice() else {
        panic!("expected both exported aliases");
    };
    assert_eq!(word.declaration.path, "Envelope::Word");
    assert_eq!(scalar.declaration.path, "Scalar");
    assert!(word.public);
    assert!(scalar.public);
    assert_eq!(word.site.file, "envelope.omg");
    assert_eq!(scalar.site.file, "base.omg");
    spec_corpus::observe_handoff!(
        psi "psi-output/alias-target",
        assert_eq!(word.target, AliasTarget::Declaration(scalar.declaration.clone())),
    );
    assert_eq!(scalar.target, AliasTarget::Builtin("u64".into()));
    let [envelope] = package.data.as_slice() else {
        panic!("expected only the envelope's nominal schema");
    };
    assert_eq!(
        package.types.get(envelope.fields[0].carrier),
        Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 }))
    );
}

#[test]
fn unused_array_aliases_keep_their_own_extent_and_element_type() {
    let defs = (
        "aliases.omg",
        "public data Bytes = [u8; 4]; data Signed = [i8; 4]; data Wide = [u8; 8]; data Forward = Bytes;",
    );
    let package = lower(&[defs]).unwrap();
    assert!(package.machines.is_empty() && package.data.is_empty());
    let aliases = &package.aliases;
    assert_eq!(aliases.len(), 4);
    for (position, name, signed, length) in [
        (0, "Bytes", false, 4),
        (2, "Signed", true, 4),
        (3, "Wide", false, 8),
    ] {
        let alias = &aliases[position];
        assert_eq!(alias.declaration.path, name);
        assert_eq!(alias.public, name == "Bytes");
        let AliasTarget::Structural(ty) = alias.target else {
            panic!("expected a structural alias")
        };
        let Some(TypeShape::FixedArray { element, length: extent }) = package.types.get(ty) else {
            panic!("expected an array")
        };
        assert_eq!(*extent, length);
        assert_eq!(
            package.types.get(*element),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width: 8 }))
        );
    }
    assert_eq!(aliases[1].target, AliasTarget::Declaration(aliases[0].declaration.clone()));
}

#[test]
fn unused_array_alias_receives_the_prepared_extent_of_its_initializer() {
    let package = lower(&[
        ("width.omg", "machine width()->u64 reaches none crashes none terminates {4}"),
        ("alias.omg", "data Bytes = [u8; width()];"),
    ])
    .unwrap();
    let [alias] = &package.aliases[..] else { panic!("expected only Bytes") };
    let AliasTarget::Structural(ty) = alias.target else {
        panic!("expected a closed array")
    };
    assert!(matches!(package.types.get(ty), Some(TypeShape::FixedArray { length: 4, .. })));
}

#[test]
fn sum_invariants_retain_the_case_guard_and_both_payload_fields() {
    let source = "data Interval requires (self in Interval::Empty) || ((self in Interval::Range) && self.lo <= self.hi) {
        case Empty; case Range(lo:u64, hi:u64); }";
    let package = lower(&[("interval.omg", source)]).unwrap();
    let interval = &package.data[0];
    let [empty, range] = interval.cases.as_slice() else {
        panic!("expected the declared cases")
    };
    let [lo, hi] = range.fields.as_slice() else {
        panic!("expected the range payload fields")
    };
    use PredicateStep::{Apply, CasePayload, IsCase, SelfValue};
    let payload = |field| CasePayload {
        case: range.declaration.clone(),
        field,
    };
    assert_eq!(
        interval.requires[0].predicate,
        [
            SelfValue,
            IsCase(empty.declaration.clone()),
            SelfValue,
            IsCase(range.declaration.clone()),
            SelfValue,
            payload(lo.declaration.clone()),
            SelfValue,
            payload(hi.declaration.clone()),
            Apply(PredicateOperator::LessOrEqual),
            Apply(PredicateOperator::And),
            Apply(PredicateOperator::Or),
        ]
    );
}
