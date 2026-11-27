/*
The Identity table of each generic check form with const record parameters
(`terminal_check_tables::mathematics::IdentityTable`).

Stage 06 proves an Identity goal of a generic machine in the kernel. Beyond
the machine's own inputs, what such a goal names is the same in every
obligation of the form: the abstract carriers, the trait operations of its
const record parameters and the laws those parameters' traits state. This
reader collects them once per form, so stage 06 declares and checks them once
and each obligation adds only what it names of its own inputs.

The operations are the ones the clause reader applied, which each record
parameter's binding keeps with the carriers they were read at
(`StaticRecordBinding::applied`). The laws are the `ensures` equalities of
the machines that stand for the records' law requirements in the form's
module. A law is written over its own formals, structural formals first and
then scalar ones, which is the order in which a citation supplies them. A law
that requires something, or whose equality names anything of a formal but
the whole value, stays out of the table, and stage 06 refuses a citation of
it.
*/

use semantic_vocabulary::{
    ClauseCarrier, ClauseIdentity, ClauseOperand, MachineId, Proposition, ScalarTerm,
};
use terminal_check_tables::compilation::GenericForms;
use terminal_check_tables::mathematics::{IdentityLaw, IdentityOperation, IdentityTable};
use terminal_check_tables::program::Elaboration;
use terminal_psi::MachineContract;

/// The Identity table of every check form in `forms` that has const record
/// parameters.
pub(crate) fn of_forms(forms: &[GenericForms]) -> Vec<IdentityTable> {
    let mut tables = Vec::new();
    for (declaration, generic) in forms.iter().enumerate() {
        let Ok(elaborations) = &generic.forms else {
            continue;
        };
        for (form, elaboration) in elaborations.iter().enumerate() {
            let (Ok(declaration), Ok(form)) = (u32::try_from(declaration), u32::try_from(form))
            else {
                continue;
            };
            tables.extend(of_form(elaboration, (declaration, form)));
        }
    }
    tables
}

fn of_form(elaboration: &Elaboration, form: (u32, u32)) -> Option<IdentityTable> {
    let bindings: Vec<_> = elaboration
        .static_schemas
        .iter()
        .flat_map(|schema| &schema.bindings)
        .collect();
    if bindings.is_empty() {
        return None;
    }
    let mut table = IdentityTable {
        form,
        suppliers: Vec::new(),
        carriers: Vec::new(),
        operations: Vec::new(),
        laws: Vec::new(),
    };
    let mut instances = Vec::new();
    for binding in bindings {
        if !table.suppliers.contains(&binding.parameter) {
            table.suppliers.push(binding.parameter);
        }
        for applied in &binding.applied {
            operation(&mut table, applied.clone());
        }
        instances.extend(
            binding
                .members
                .iter()
                .flat_map(|member| member.instances.iter().map(|instance| instance.machine)),
        );
    }
    instances.sort_unstable();
    instances.dedup();
    for instance in instances {
        let Some(contract) = elaboration
            .module
            .invocation_machine_contract(instance, None)
        else {
            continue;
        };
        if !contract.requires.is_empty() {
            continue;
        }
        for (clause, ensured) in contract.ensures.iter().enumerate() {
            let (Some(identity), Ok(clause)) =
                (identity(&ensured.proposition), u32::try_from(clause))
            else {
                continue;
            };
            let Some(law) = law(instance, clause, contract, identity) else {
                continue;
            };
            add_identity(&mut table, identity);
            for &formal in &law.formals {
                carrier(&mut table, formal);
            }
            table.laws.push(law);
        }
    }
    Some(table)
}

fn identity(proposition: &Proposition) -> Option<&ClauseIdentity> {
    match proposition {
        Proposition::DeclarationIdentity(identity) if !identity.is_float_meaning() => {
            Some(identity)
        }
        _ => None,
    }
}

/// Adds the carriers and operations an Identity names.
fn add_identity(table: &mut IdentityTable, identity: &ClauseIdentity) {
    for value in &identity.values {
        carrier(table, value.carrier);
        match &value.operand {
            ClauseOperand::Application {
                declaration,
                arguments,
            } => {
                let domains = arguments
                    .iter()
                    .filter_map(|&argument| identity.values.get(argument as usize))
                    .map(|argument| argument.carrier)
                    .collect();
                operation(
                    table,
                    IdentityOperation {
                        declaration: *declaration,
                        domains,
                        result: value.carrier,
                    },
                );
            }
            ClauseOperand::Scalar(term) => {
                carrier(table, ClauseCarrier::Scalar(term.scalar_type()))
            }
            ClauseOperand::Input(_) | ClauseOperand::Formal(_) | ClauseOperand::FloatMeaning(_) => {
            }
        }
    }
}

fn carrier(table: &mut IdentityTable, carrier: ClauseCarrier) {
    if !table.carriers.contains(&carrier) {
        table.carriers.push(carrier);
    }
}

fn operation(table: &mut IdentityTable, operation: IdentityOperation) {
    for &domain in &operation.domains {
        carrier(table, domain);
    }
    carrier(table, operation.result);
    if !table.operations.contains(&operation) {
        table.operations.push(operation);
    }
}

/// The law `identity` states, over the formals of `contract`: each whole
/// structural formal and each scalar formal written as its position among
/// them. `None` when the equality names anything else of a formal, any
/// other input, or leaves a formal out, since no citation could supply it.
fn law(
    instance: MachineId,
    clause: u32,
    contract: &MachineContract,
    identity: &ClauseIdentity,
) -> Option<IdentityLaw> {
    let structural = contract.structural_parameters.len();
    let count = structural + contract.parameters.len();
    let mut formals: Vec<Option<ClauseCarrier>> = vec![None; count];
    let mut statement = identity.clone();
    for value in &mut statement.values {
        let position = match &value.operand {
            ClauseOperand::Input(input) => {
                let position = contract
                    .structural_parameters
                    .iter()
                    .position(|formal| formal.place == input.root())?;
                if !input.path().is_empty() {
                    return None;
                }
                position
            }
            ClauseOperand::Scalar(ScalarTerm::Value { id, .. }) => {
                structural
                    + contract
                        .parameters
                        .iter()
                        .position(|formal| formal.id == *id)?
            }
            ClauseOperand::Scalar(term) if mentions_value(term) => return None,
            ClauseOperand::Formal(_) => return None,
            _ => continue,
        };
        if formals[position]
            .replace(value.carrier)
            .is_some_and(|known| known != value.carrier)
        {
            return None;
        }
        value.operand = ClauseOperand::Formal(u32::try_from(position).ok()?);
    }
    Some(IdentityLaw {
        instance,
        clause,
        formals: formals.into_iter().collect::<Option<_>>()?,
        statement,
    })
}

/// Whether a scalar term reads a value: a law's citation supplies only a
/// whole formal, so a law that computes with one is not in the table.
fn mentions_value(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::Value { .. } => true,
        ScalarTerm::DeclarationApplication { arguments, .. } => {
            arguments.iter().any(mentions_value)
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_) => {
            false
        }
        _ => true,
    }
}
