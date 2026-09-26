//! Fixtures shared by the generics tests: specialized machine lookup.

mod conformance_binders;
mod const_arguments;
mod const_values;
mod empty_ranges;
mod named_conformance;
mod named_witnesses;
mod nested_calls;
mod nominal_machine_parameters;
mod result_local_providers;
mod specialization_identities;
mod specializations;
mod symbolic_ranges;

fn specialized_machine<'program>(
    program: &'program checked_trees::CheckedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    let template = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored generic template");
    assert!(!program.machine_type_parameters(template).is_empty());
    let instances = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == template.symbol)
        .collect::<Vec<_>>();
    let [specialization] = instances.as_slice() else {
        panic!("expected one selected instance of {name}")
    };
    assert_ne!(specialization.instance, template.symbol);
    let instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.instance)
        .expect("selected private instance");
    assert!(!instance.is_public);
    instance
}
