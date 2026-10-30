//! Executable/public carrier correspondence consumes the retained application
//! frames, not the caller's runtime actual types. A captured map's telescope
//! and a member invocation's telescope are independent simultaneous scopes;
//! compose outer scopes first so permutations never substitute their siblings.

use symbols::SymbolHandle;
use typed_trees::static_applications::StaticMachineApplicationHandle;
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, applied_declared_carrier_identity,
};
use typed_trees::types::TypeReferenceHandle;

struct Frame<'program> {
    declaration: SymbolHandle,
    arguments: Vec<(SymbolHandle, IndexIdentityArgument<'program>)>,
}

pub(super) struct CarrierCorrespondence<'program> {
    frames: Vec<Frame<'program>>,
}

impl<'program> CarrierCorrespondence<'program> {
    pub(super) fn new(
        program: &'program dyn crate::CheckingInput,
        caller: SymbolHandle,
        selection: StaticMachineApplicationHandle,
        application: &'program typed_trees::typed_trees::ClosedConformanceApplication,
        lineage: &[(SymbolHandle, SymbolHandle)],
    ) -> Result<Self, &'static str> {
        // The selected map exists even when an open, unused binder prevents
        // executable capture materialization. Its admitted authored tuple is
        // the capture scope; a realized body copy does not introduce it again.
        let supplying = if selection.is_valid() {
            selection
        } else {
            let receipts = program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == caller)
                .collect::<Vec<_>>();
            let [receipt] = receipts.as_slice() else {
                return Err("selected carrier has no unique supplying invocation");
            };
            receipt.binding_application
        };
        if !supplying.is_valid() {
            return Err("selected carrier lost its supplying application");
        }
        let supplier = program
            .expression_table
            .static_machine_application(supplying)
            .lexical_owner;
        if !supplier.is_valid() {
            return Err("selected carrier lost its supplying declaration");
        }
        let mut frames = vec![Frame::from_conformance(program, application)?];
        for pair in lineage.windows(2).rev() {
            let (instance, template) = (pair[0].0, pair[1].0);
            let invocations = program
                .machine_specializations
                .iter()
                .filter(|row| row.instance == instance && row.template == template)
                .collect::<Vec<_>>();
            let captures = program
                .captured_machine_applications
                .iter()
                .filter(|(_, row)| row.instance == instance && row.template == template)
                .map(|(_, row)| row)
                .collect::<Vec<_>>();
            let application = match (invocations.as_slice(), captures.as_slice()) {
                ([invocation], []) => {
                    let handle = invocation.binding_application;
                    if !handle.is_valid() {
                        return Err("selected carrier invocation lost its binding application");
                    }
                    let binding = program.expression_table.static_machine_application(handle);
                    if binding.registration_operation != template
                        || binding.binding_declaration != template
                    {
                        return Err("selected carrier invocation names another template");
                    }
                    // An invocation may not replace the selected map's
                    // operands. If its ledger repeats one, it must carry the
                    // same admitted static argument under the exact suppliers.
                    let map = program
                        .conformance_by_symbol(application.declaration)
                        .ok_or("selected carrier lost its map declaration")?;
                    for binding_value in program
                        .expression_table
                        .static_machine_application_bindings(handle)
                    {
                        if let Some((parameter, argument)) = program
                            .conformance_type_parameters(map)
                            .iter()
                            .zip(&application.arguments)
                            .find(|(parameter, _)| parameter.symbol == binding_value.parameter)
                            && !program.static_binding_matches_argument(
                                &parameter.kind,
                                &binding_value.value,
                                argument,
                                supplier,
                                binding.lexical_owner,
                            )
                        {
                            return Err("selected invocation changes its map's static operand");
                        }
                    }
                    handle
                }
                ([], [capture]) => {
                    super::super::capture_binding_is_exact(program, capture)?;
                    continue;
                }
                _ => return Err("selected carrier frame has no unique application"),
            };
            frames.push(Frame::from_application(program, application)?);
        }
        if lineage.is_empty() {
            return Err("selected carrier has no executable lineage");
        }
        Ok(Self { frames })
    }

    pub(super) fn matches(
        &self,
        program: &dyn crate::CheckingInput,
        executable: TypeReferenceHandle,
        public: TypeReferenceHandle,
    ) -> bool {
        let scopes = self
            .frames
            .iter()
            .map(|frame| AppliedTypeIdentityScope {
                declaration: frame.declaration,
                arguments: &frame.arguments,
                lifetimes: &[],
            })
            .collect::<Vec<_>>();
        let left = applied_declared_carrier_identity(program, executable, &scopes);
        let right = applied_declared_carrier_identity(program, public, &[]);
        left.zip(right).is_some_and(|(left, right)| left == right)
    }
}

impl<'program> Frame<'program> {
    fn from_conformance(
        program: &'program dyn crate::CheckingInput,
        application: &'program typed_trees::typed_trees::ClosedConformanceApplication,
    ) -> Result<Self, &'static str> {
        let declaration = program
            .conformance_by_symbol(application.declaration)
            .ok_or("selected carrier lost its map declaration")?;
        let parameters = program.conformance_type_parameters(declaration);
        if parameters.len() != application.arguments.len()
            || declaration.lifetime_parameters.len() != application.lifetime_arguments.len()
        {
            return Err("selected carrier lost its complete map telescope");
        }
        if !application.callable_application.is_valid() {
            return Err("selected carrier lost its retained map application");
        }
        let bindings = program
            .expression_table
            .static_machine_application_bindings(application.callable_application);
        if bindings.len() != parameters.len()
            || !parameters
                .iter()
                .zip(bindings)
                .all(|(parameter, binding)| parameter.symbol == binding.parameter)
        {
            return Err("selected carrier changed its supplying binder order");
        }
        let arguments = parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    IndexIdentityArgument::StaticArgument(argument),
                )
            })
            .collect();
        Ok(Self {
            declaration: declaration.symbol,
            arguments,
        })
    }

    fn from_application(
        program: &'program dyn crate::CheckingInput,
        handle: StaticMachineApplicationHandle,
    ) -> Result<Self, &'static str> {
        let application = program.expression_table.static_machine_application(handle);
        if !application.lexical_owner.is_valid() {
            return Err("selected carrier frame lost its lexical supplier");
        }
        let arguments = crate::machine_calls::static_bindings::index_identity_arguments(
            program
                .expression_table
                .static_machine_application_bindings(handle),
        );
        Ok(Self {
            declaration: application.binding_declaration,
            arguments,
        })
    }
}
