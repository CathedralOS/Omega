use omega_lowering::{FoldedTargetObservation, FoldedValueOrigin, LoweredPackage, TypeShape, lower_package};
use omega_preparation::{LayoutExtent, LayoutGeometry, SelectedLayout, TargetIdentity, TargetSemantics, prepare_source_package_for_target};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{Access, LayoutQuery, ValueType, resolve_source_package};

fn replay_u64<'a>(
    closure: &'a omega_psi::FoldedTargetClosure,
    profile: &str,
    plan: u8,
) -> Result<(), &'a omega_psi::FoldedTargetObservation> {
    psi_handoff_consumer::replay_layout(closure, profile, plan, 8, omega_psi::FoldedSubject::Integer { signed: false, width: 64 })
}

const FOLDED: &str = "use omega\\language\\core;
    const EXTENT:u64=core\\size_of::<u64>();
    const ALIAS:u64=EXTENT;
    machine copy(arr:[u8; ALIAS])->[u8; ALIAS]{arr}";

fn lower(
    source: &str,
    target: &TargetSemantics,
) -> LoweredPackage {
    lower_files(&[("extent.omg", source)], target)
}

fn lower_files(
    files: &[(&str, &str)],
    target: &TargetSemantics,
) -> LoweredPackage {
    let parsed: Vec<_> = files
        .iter()
        .map(|(_, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&parsed)
        .map(|(&(logical_path, text), (tokens, tree))| StageSource {
            package: "app",
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let package = resolve_source_package(&sources).unwrap();
    let prepared = prepare_source_package_for_target(&package, &[], target).unwrap();
    lower_package(&prepared).unwrap()
}

fn capsule(
    profile: &str,
    plan: u8,
    size: u64,
) -> TargetSemantics {
    let identity = TargetIdentity {
        profile: profile.into(),
        semantics_version: 1,
        realization: vec![7].into(),
    };
    let layout = SelectedLayout {
        target: identity.clone(),
        subject: ValueType::Integer {
            signed: false,
            width: Some(64),
        },
        plan: vec![plan].into(),
        schema: vec![1].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(size),
            alignment: Some(8),
        },
    };
    TargetSemantics::new(identity, [layout], []).unwrap()
}

#[test]
fn folded_extent_keeps_the_exact_target_plan_and_origin() {
    let selected = capsule("test-target", 1, 8);
    // The consumer reads the closure after the producer's source is dropped.
    let lowered = {
        let source = FOLDED.to_owned();
        lower(&source, &selected)
    };
    let length = |package: &LoweredPackage| psi_handoff_consumer::array_parameter_length(package, "copy").unwrap();
    assert_eq!(length(&lowered), 8);
    let copy = lowered.machines.iter().find(|machine| machine.declaration.path == "copy").unwrap();
    assert_eq!(copy.result_type, Some(copy.parameter_types[0]), "both sides of copy use [u8; ALIAS]");
    let closure = lowered.target_closure.as_ref().unwrap();
    assert_eq!(closure.target(), selected.identity());
    let [
        observation @ FoldedTargetObservation::Layout {
            projection: LayoutQuery::Size,
            layout,
            ..
        },
    ] = closure.observations()
    else {
        panic!("expected the used layout projection");
    };
    assert_eq!(layout.plan.as_ref(), &[1]);
    let constant = |name: &str| {
        closure
            .inputs()
            .iter()
            .find(|input| matches!(&input.origin, FoldedValueOrigin::Constant(id) if id.path == name))
            .unwrap()
    };
    let origin = constant("EXTENT");
    assert_eq!(origin.site.file, "extent.omg");
    assert_eq!(origin.site.span.slice(FOLDED.as_bytes()), Some(b"core\\size_of::<u64>()".as_slice()),);
    assert_eq!(origin.observations, [0]);
    let alias = constant("ALIAS");
    assert_eq!(alias.constants[0].path, "EXTENT");
    assert_eq!(alias.site.file, "extent.omg");
    assert_eq!(alias.site.span.slice(FOLDED.as_bytes()), Some(b"EXTENT".as_slice()), "ALIAS keeps its own initializer site");
    assert_both_extents_read_alias(closure, FOLDED);

    assert_eq!(replay_u64(closure, "test-target", 1), Ok(()));
    let competing_plan = capsule("test-target", 2, 8);
    spec_corpus::observe_handoff!(
        psi "psi-output/replay-wrong-plan",
        assert_eq!(replay_u64(closure, "test-target", 2), Err(observation)),
    );
    assert_eq!(replay_u64(closure, "other-target", 1), Err(observation));
    let competing = lower(FOLDED, &competing_plan);
    assert_eq!(length(&competing), 8);
    assert_ne!(lowered.target_closure, competing.target_closure);

    let literal = lower("machine copy(arr:[u8; 8])->[u8; 8]{arr}", &selected);
    assert_eq!(length(&literal), 8);
    assert_eq!(literal.target_closure, None);
}

fn assert_both_extents_read_alias(closure: &omega_psi::FoldedTargetClosure, source: &str) {
    let alias = closure
        .inputs()
        .iter()
        .find_map(|input| match &input.origin {
            FoldedValueOrigin::Constant(id) if id.path == "ALIAS" => Some(id),
            _ => None,
        })
        .expect("declared ALIAS");
    let expected: Vec<_> = source.match_indices("ALIAS]").map(|(offset, _)| offset).collect();
    assert_eq!(expected.len(), 2, "fixture has ALIAS in both parameter and result extents");
    let mut observed = Vec::new();
    let indices = closure.inputs().iter().filter(|input| matches!(input.origin, FoldedValueOrigin::StaticIndex { .. }));
    for input in indices {
        assert_eq!((input.site.package.as_str(), input.site.file.as_str()), ("app", "extent.omg"));
        assert_eq!(input.site.span.slice(source.as_bytes()), Some(b"ALIAS".as_slice()));
        assert_eq!(input.constants.as_slice(), std::slice::from_ref(alias), "direct constant read at {:?}", input.site);
        observed.push(input.site.span.get_start_offset());
    }
    observed.sort_unstable();
    assert_eq!(observed, expected, "each extent must carry its own dependency");
}

/// The constants each folded constant consumed, by path. A constant with no
/// recorded inputs, or a package without a target closure, reads nothing.
fn constant_reads(
    package: &LoweredPackage,
    name: &str,
) -> Vec<String> {
    let Some(closure) = package.target_closure.as_ref() else {
        return Vec::new();
    };
    closure
        .inputs()
        .iter()
        .find(|input| matches!(&input.origin, FoldedValueOrigin::Constant(id) if id.path == name))
        .map(|input| {
            input
                .constants
                .iter()
                .map(|id| id.path.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_named_read_names_the_constant_it_consumed_even_when_both_values_agree() {
    // Size and alignment are both 8, so only the dependency tells the two reads apart.
    let target = capsule("test-target", 1, 8);
    for read in ["EXTENT", "ALIGNMENT"] {
        let source = format!(
            "use omega\\language\\core;
             const EXTENT:u64=core\\size_of::<u64>();
             const ALIGNMENT:u64=core\\align_of::<u64>();
             const ALIAS:u64={read};
             machine copy(arr:[u8; ALIAS])->[u8; ALIAS]{{arr}}"
        );
        let lowered = lower(&source, &target);
        assert_eq!(psi_handoff_consumer::array_parameter_length(&lowered, "copy"), Some(8));
        spec_corpus::observe_handoff!("named-constant-reads/consumer", assert_eq!(constant_reads(&lowered, "ALIAS"), [read]));
        let closure = lowered.target_closure.as_ref().unwrap();
        assert_both_extents_read_alias(closure, &source);
    }
}

#[test]
fn a_discarded_constant_read_remains_an_input_of_the_constant_that_performed_it() {
    // L4 evaluates `keep` while freezing KEPT. The discard leaves the value 8
    // unchanged, so only the folded dependency records which constant it read.
    let target = capsule("test-target", 1, 8);
    let program = |body: &str| {
        format!(
            "use omega\\language\\core;
             const EXTENT:u64=core\\size_of::<u64>();
             const ALIGNMENT:u64=core\\align_of::<u64>();
             machine keep(x:u64)->u64{{{body}}}
             const KEPT:u64=keep(8u64);
             machine copy(arr:[u8; KEPT])->[u8; KEPT]{{arr}}"
        )
    };
    for read in ["EXTENT", "ALIGNMENT"] {
        let lowered = lower(&program(&format!("_ = {read}; x")), &target);
        assert_eq!(psi_handoff_consumer::array_parameter_length(&lowered, "copy"), Some(8));
        spec_corpus::observe_handoff!("discarded-constant-reads/consumer", assert_eq!(constant_reads(&lowered, "KEPT"), [read]));
    }
    let without_discard = lower(&program("x"), &target);
    assert_eq!(psi_handoff_consumer::array_parameter_length(&without_discard, "copy"), Some(8));
    assert!(constant_reads(&without_discard, "KEPT").is_empty());
}

#[test]
fn one_open_package_resumes_under_each_selected_layout() {
    let tokens = omega_lexer::lex(FOLDED.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(FOLDED.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "extent.omg",
            text: FOLDED,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let package = resolve_source_package(&sources).unwrap();
    for (plan, size) in [(1, 8), (2, 16)] {
        let selected = capsule("test-target", plan, size);
        let progress = omega_preparation::prepare_source_package_with_open_target(&package, &[]).unwrap();
        let omega_preparation::PreparationProgress::OpenTarget(open) = progress else {
            panic!("the extent needs a selected target");
        };
        let resumed = lower_package(&open.close(&selected).unwrap()).unwrap();
        assert_eq!(resumed, lower(FOLDED, &selected));
        spec_corpus::observe_handoff!(
            "open-target-and-selected-layout/consumer",
            assert_eq!(psi_handoff_consumer::array_parameter_length(&resumed, "copy"), Some(size))
        );
        let closure = resumed.target_closure.as_ref().unwrap();
        assert_eq!(closure.target(), selected.identity());
        let subject = || omega_psi::FoldedSubject::Integer { signed: false, width: 64 };
        assert_eq!(psi_handoff_consumer::replay_layout(closure, "test-target", plan, size, subject()), Ok(()));
        assert!(psi_handoff_consumer::replay_layout(closure, "test-target", 3 - plan, size, subject()).is_err());
    }
}

#[test]
fn equal_size_and_alignment_keep_their_own_dependencies_after_file_reorder() {
    let target = capsule("test-target", 1, 8);
    let extent = ("extent.omg", FOLDED);
    let alignment = (
        "alignment.omg",
        "use omega\\language\\core;
         const ALIGNMENT:u64=core\\align_of::<u64>();
         machine aligned(arr:[u8; ALIGNMENT])->[u8; ALIGNMENT]{arr}",
    );
    let ordered = lower_files(&[extent, alignment], &target);
    let reversed = lower_files(&[alignment, extent], &target);
    assert_eq!(ordered, reversed);

    // Both queries yield 8. Swapping them keeps the array shapes and the
    // receiving target compatible, but changes which premise each constant used.
    let size_from_alignment = extent.1.replace("core\\size_of", "core\\align_of");
    let alignment_from_size = alignment.1.replace("core\\align_of", "core\\size_of");
    let swapped = lower_files(
        &[
            (extent.0, &size_from_alignment),
            (alignment.0, &alignment_from_size),
        ],
        &target,
    );
    assert_eq!(ordered.types, swapped.types);
    for (package, extent_query, alignment_query) in [
        (&ordered, LayoutQuery::Size, LayoutQuery::Alignment),
        (&swapped, LayoutQuery::Alignment, LayoutQuery::Size),
    ] {
        let closure = package.target_closure.as_ref().unwrap();
        assert_eq!(replay_u64(closure, "test-target", 1), Ok(()));
        assert_eq!(closure.observations().len(), 2);
        for (name, expected) in [("EXTENT", extent_query), ("ALIGNMENT", alignment_query)] {
            let input = closure
                .inputs()
                .iter()
                .find(|input| matches!(&input.origin, FoldedValueOrigin::Constant(id) if id.path == name))
                .expect("each folded constant must retain its target input");
            let [index] = input.observations.as_slice() else {
                panic!("{name} must reference exactly one projection")
            };
            let FoldedTargetObservation::Layout { projection, .. } = &closure.observations()[*index] else {
                panic!("{name} must reference a layout projection");
            };
            spec_corpus::observe_handoff!("target-observation-projection/consumer", assert_eq!(*projection, expected, "{name} must reference its own query"));
        }
    }
}

#[test]
fn folded_extent_preserves_an_unbounded_referent_identity() {
    let identity = capsule("test-target", 1, 8).identity().clone();
    let target = |signed| {
        let layout = SelectedLayout {
            target: identity.clone(),
            subject: ValueType::Reference(Access::Shared, Box::new(ValueType::Integer { signed, width: None })),
            plan: vec![1].into(),
            schema: vec![1].into(),
            geometry: LayoutGeometry {
                extent: LayoutExtent::Fixed(8),
                alignment: Some(8),
            },
        };
        TargetSemantics::new(identity.clone(), [layout], []).unwrap()
    };
    fn replay(
        closure: &omega_psi::FoldedTargetClosure,
        signed: bool,
    ) -> Result<(), &omega_psi::FoldedTargetObservation> {
        psi_handoff_consumer::replay_layout(
            closure,
            "test-target",
            1,
            8,
            omega_psi::FoldedSubject::Reference(
                omega_psi::FoldedReferenceAccess::Shared,
                Box::new(omega_psi::FoldedSubject::UnboundedInteger { signed }),
            ),
        )
    }
    let selected = target(true);
    let source = "use omega\\language\\core;
                  const EXTENT:u64=core\\size_of::<@i>();
                  machine copy(arr:[u8; EXTENT])->[u8; EXTENT]{arr}";
    let package = lower(source, &selected);
    let carrier = package.machines[0].parameter_types[0];
    assert!(matches!(package.types.get(carrier), Some(TypeShape::FixedArray { length: 8, .. })));
    let closure = package.target_closure.unwrap();
    assert_eq!(replay(&closure, true), Ok(()));
    let competing = target(false);
    spec_corpus::observe_handoff!(
        psi "psi-output/signed-referent-replay",
        assert_eq!(replay(&closure, false), Err(&closure.observations()[0])),
    );

    let competing_source = source.replace("@i", "@u");
    let competing_closure = lower(&competing_source, &competing).target_closure.unwrap();
    assert_eq!(replay(&competing_closure, false), Ok(()));
    assert_eq!(replay(&competing_closure, true), Err(&competing_closure.observations()[0]));
}
