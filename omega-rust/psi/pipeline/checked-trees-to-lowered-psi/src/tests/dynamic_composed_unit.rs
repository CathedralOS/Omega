//! Fixtures shared by the dynamic composed unit lowering tests.

use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
mod direct_dynamic_units;
mod finite_family;
mod mixed_results;
mod mutating_realizations_and_effects;
mod plan_isolation;
mod rebound_dynamic_custody;

use crate::tests::{LoweringError, checked_source, lower_machine};
use terminal_psi::OperationKind;

const DIRECT_DYNAMIC_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] {
        value: bool;
    }

    machine Item::measure(&self) -> bool {
        transition { _ -> false }
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&self) {
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: bool = erased.measure();
    }
"#;

const STORED_DYNAMIC_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] {
        value: bool;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool {
            transition { _ -> self.value }
        }
    }

    data Holder<'item> {
        handler: &'item dyn Measure;
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run<'item>(&self) {
        let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
        let holder: Holder<'item> = Holder { handler: erased };
        let result: bool = holder.handler.measure();
    }
"#;

const STORED_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Holder<'item> {
        handler: &'item dyn Measure;
    }

    data Main {
        console: Console;
        item: Item;
    }

    machine Main::run<'item>(&mut self) reaches Console {
        let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
        let holder: Holder<'item> = Holder { handler: erased };
        let result: i32 = holder.handler.measure();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const DIRECT_DYNAMIC_INTEGER_STORE_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 17;
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Measure {
        machine measure(&mut self) -> i32;
    }

    data Item [copy] {
        value: i32;
        enabled: bool;
        attempts: u16;
    }

    Primary: Item satisfies Measure {
        machine measure(&mut self) -> i32 {
            self.value = 23;
            self.enabled = true;
            self.attempts = 257;
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Measure = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const PROJECTED_MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Measure {
        machine measure(&mut self) -> i32;
    }

    data Payload [copy] {
        value: u16;
    }

    data Envelope [copy] {
        payload: Payload;
    }

    data Item [copy] {
        envelope: Envelope;
        code: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&mut self) -> i32 {
            self.envelope.payload.value = 513;
            transition { _ -> self.code }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Measure = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        item: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = erased.measure();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const FORWARDED_REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const CHANGED_CONFORMANCE_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure { machine measure(&self) -> i32; }
    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 { transition { _ -> self.value } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> i32 { transition { _ -> self.value } }
    }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        let result: i32 = erased.measure();
    }
"#;

const FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const FORWARDED_DIRECT_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] { selected: Item; }

    machine Main::run(&mut self) {
        self.selected.value = 23;
        let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const JOINED_DYNAMIC_BOOLEAN_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; }

    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

const JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

    data Item [copy] { marker: bool; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }

    data Main [copy] { first: Item; second: Item; }

    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = forward(selected);
        }

        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = forward(selected);
        }
    }

    machine forward(erased: &dyn Measure) -> bool {
        let result: bool = relay(erased);
        transition { _ -> result }
    }

    machine relay(erased: &dyn Measure) -> bool {
        let result: bool = finish(erased);
        transition { _ -> result }
    }

    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

const JOINED_DYNAMIC_UNIT_FORWARD_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }

    data Item { value: i32; }

    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }

    data Main { first: Item; second: Item; }

    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Touch = &self.first as &dyn Item::Primary;
            forward(selected);
        }

        state take_second(&self) {
            let selected: &dyn Touch = &self.second as &dyn Item::Secondary;
            forward(selected);
        }
    }

    machine forward(erased: &dyn Touch) { relay(erased); }
    machine relay(erased: &dyn Touch) { finish(erased); }
    machine finish(erased: &dyn Touch) { erased.touch(); }
"#;

const MULTI_HOP_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] { selected: Item; }

    machine Main::run(&self) {
        let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = finish(erased);
        transition { _ -> result }
    }

    machine finish(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const MULTI_HOP_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Console;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }

    machine forward(erased: &dyn Measure) -> i32 {
        let result: i32 = finish(erased);
        transition { _ -> result }
    }

    machine finish(erased: &dyn Measure) -> i32 {
        let result: i32 = erased.measure();
        transition { _ -> result }
    }
"#;

const DIRECT_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main { item: Item; }

    machine Main::run(&self) {
        let erased: &dyn Touch = &self.item as &dyn Item::Primary;
        erased.touch();
    }
"#;

const FORWARDED_DIRECT_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {}
    }

    data Main { selected: Item; }

    machine Main::run(&mut self) {
        let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) {
        erased.touch();
    }
"#;

const MULTI_HOP_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }
    Primary: Item satisfies Touch { machine touch(&self) {} }
    data Main { selected: Item; }

    machine Main::run(&self) {
        let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) {
        finish(erased);
    }

    machine finish(erased: &dyn Touch) {
        erased.touch();
    }
"#;

const REBOUND_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        erased.touch();
    }
"#;

const CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }

    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        erased.touch();
    }
"#;

const FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }

    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }

    data Main { decoy: Item; selected: Item; }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Secondary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) { erased.touch(); }
"#;

const FORWARDED_REBOUND_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine touch(&self);
    }

    data Item { value: i32; }

    Primary: Item satisfies Touch {
        machine touch(&self) {
        }
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        forward(erased);
    }

    machine forward(erased: &dyn Touch) {
        erased.touch();
    }
"#;

const FAMILY_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Shape {
        machine Self::code<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Item [copy] {
        value: i32;
    }

    machine Item::code<Width: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&self) {
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code<16>();
    }
"#;

const REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Shape {
        machine Self::code<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Item {
        value: i32;
    }

    machine Item::code<Width: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main {
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) {
        let mut erased: &dyn Shape = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = erased.code<32>();
    }
"#;

const FORWARDED_FAMILY_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Shape {
        machine Self::code<Width: u32>(&self) -> i32 where Width == 16 || Width == 32;
    }

    data Item [copy] {
        value: i32;
    }

    machine Item::code<Width: u32>(&self) -> i32 satisfies Shape::code {
        transition { _ -> self.value }
    }

    Primary: Item satisfies Shape {
        Shape::code = Item::code;
    }

    data Main [copy] {
        selected: Item;
    }

    machine Main::run(&self) {
        let erased: &dyn Shape = &self.selected as &dyn Item::Primary;
        let result: i32 = forward(erased);
    }

    machine forward(erased: &dyn Shape) -> i32 {
        let result: i32 = erased.code<16>();
        transition { _ -> result }
    }
"#;

const JOINED_FAMILY_DYNAMIC_BOOLEAN_SOURCE: &str = r#"
    trait Shape {
        machine Self::ready<Width: u32>(&self) -> bool where Width == 16 || Width == 32;
    }

    data Item [copy] {
        marker: bool;
    }

    machine Item::ready<Width: u32>(&self) -> bool satisfies Shape::ready {
        transition { _ -> self.marker }
    }

    Primary: Item satisfies Shape {
        Shape::ready = Item::ready;
    }

    Secondary: Item satisfies Shape {
        Shape::ready = Item::ready;
    }

    data Main [copy] {
        first: Item;
        second: Item;
    }

    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }

        state take_first(&self) {
            let selected: &dyn Shape = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }

        state take_second(&self) {
            let selected: &dyn Shape = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }

    machine finish(erased: &dyn Shape) -> bool {
        let result: bool = erased.ready<32>();
        transition { _ -> result }
    }
"#;

const FAMILY_DYNAMIC_UNIT_SOURCE: &str = r#"
    trait Touch {
        machine Self::touch<Width: u32>(&self) where Width == 16 || Width == 32;
    }

    data Item {
        value: i32;
    }

    machine Item::touch<Width: u32>(&self) satisfies Touch::touch {
    }

    Primary: Item satisfies Touch {
        Touch::touch = Item::touch;
    }

    data Main {
        item: Item;
    }

    machine Main::run(&self) {
        let erased: &dyn Touch = &self.item as &dyn Item::Primary;
        erased.touch<16>();
    }
"#;

fn direct_dynamic_checked() -> checked_trees::CheckedTrees {
    checked_source(DIRECT_DYNAMIC_SOURCE)
}

fn direct_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedDynamicScalarCallPlan {
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls
            .is_empty(),
        "direct dynamic call must not enter the rebound catalog"
    );
    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one direct dynamic plan expected, got {plans:#?}")
    };
    plan
}

fn assert_dynamic_unit_artifact_executes(artifact: &terminal_codec::CanonicalTerminalArtifact) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("dynamic Unit module decodes for execution");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("dynamic Unit entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("dynamic Unit entry requires one structural self parameter")
    };
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("dynamic Unit artifact starts");
    let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("dynamic Unit executes"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
}

fn assert_stored_dynamic_scalar_artifact_executes(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("stored dynamic scalar module decodes for execution");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("stored dynamic scalar entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("stored dynamic scalar entry requires one structural self parameter")
    };
    let [descriptor] = module.dynamic_dispatch.stored_descriptors.as_slice() else {
        panic!("stored dynamic scalar execution requires one descriptor")
    };
    let selection = module
        .dynamic_dispatch
        .selections
        .iter()
        .find(|selection| {
            selection.owner == descriptor.owner && selection.ordinal == descriptor.selection_ordinal
        })
        .expect("stored descriptor retains its exact selection");
    let [dispatch] = module.dynamic_dispatch.stored_dispatches.as_slice() else {
        panic!("stored dynamic scalar execution requires one dispatch")
    };
    let realization = module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("stored dynamic scalar realization machine");
    let field = realization
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::BooleanStructuralField { field, .. } => Some(field),
            _ => None,
        })
        .expect("stored dynamic scalar realization reads one Boolean field");
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let boolean_field = terminal_interpreter::TerminalStructuralBooleanFieldValue {
        argument_index: 0,
        path: selection.source.path.clone(),
        field,
        value: true,
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            boolean_fields: &[boolean_field],
            ..Default::default()
        },
    )
    .expect("stored dynamic scalar artifact starts");
    let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("stored dynamic scalar executes"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
}

fn direct_plan_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedDynamicScalarCallPlan {
    let plans = &mut checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_mut_slice() else {
        panic!("one direct dynamic plan expected")
    };
    plan
}

fn unsupported_message(checked: &checked_trees::CheckedTrees) -> &'static str {
    match lower_machine(checked, "Main::run") {
        Err(LoweringError::Unsupported(message)) => message,
        result => panic!("tampered direct dynamic custody must reject, got {result:?}"),
    }
}
