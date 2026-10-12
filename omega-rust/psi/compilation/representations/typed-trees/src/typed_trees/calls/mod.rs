//! calls in the current typed trees program.

pub mod boundary;
pub mod dynamic_traits;
pub mod finite_family;
pub mod service;
pub mod signature;
pub mod static_applications;

/// Borrowed public callable coordinates, including the source declaration of
/// either kind of produced executable requirement. Application custody is
/// identity only; it does not admit operands or select a provider.
#[derive(Clone, Copy)]
pub struct PublicTraitRequirementSignature<'program> {
    pub owner: &'program crate::trait_definition::TraitDefinition,
    pub declaration: &'program crate::signature::StateSignature,
    pub signature: &'program crate::signature::StateSignature,
    /// Applied facts/formals keep source identities independently of the
    /// executable header used for provider routing and target realization.
    pub contract: &'program crate::signature::StateSignature,
    pub application: crate::static_applications::StaticMachineApplicationHandle,
    /// Demanded headers retain the selected interface. A finite tuple has no
    /// invocation presentation; zero leaves that identity with the source call.
    pub selected_interface: symbols::SymbolHandle,
}

impl super::TypedTrees {
    /// One public target rejoins its exact original declaration and producing
    /// application. Finite-family instances and demanded public headers have
    /// distinct producers but share this read-only callable projection.
    pub fn public_trait_requirement_signature(
        &self,
        target: symbols::SymbolHandle,
    ) -> Option<PublicTraitRequirementSignature<'_>> {
        if !target.is_valid() {
            return None;
        }
        let mut selected = self
            .public_trait_requirement_signatures()
            .filter(|(_, signature)| signature.symbol == target);
        let (owner, signature) = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        let mut headers = self
            .instantiated_requirement_signatures
            .iter()
            .filter(|(_, row)| self.trait_machine_signatures.get(row.signature).symbol == target);
        let header = headers.next().map(|(_, row)| row);
        let mut families = self
            .closed_requirement_applications()
            .filter(|row| row.signature.symbol == target);
        let family = families.next();
        if headers.next().is_some()
            || families.next().is_some()
            || (header.is_some() && family.is_some())
        {
            return None;
        }
        let (template, application, selected_interface) = if let Some(header) = header {
            if header.declaring_trait != owner.symbol {
                return None;
            }
            (
                header.requirement_template,
                header.binding_application,
                header.selected_interface,
            )
        } else if let Some(family) = family {
            if family.declaring_trait != owner.symbol || !std::ptr::eq(family.signature, signature)
            {
                return None;
            }
            (
                family.template,
                family.application,
                symbols::SymbolHandle::invalid(),
            )
        } else {
            return Some(PublicTraitRequirementSignature {
                owner,
                declaration: signature,
                signature,
                contract: signature,
                application: Default::default(),
                selected_interface: owner.symbol,
            });
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return None;
        }
        let original = self
            .expression_table
            .static_machine_application(application);
        let declaration = self
            .trait_machine_signatures(owner)
            .iter()
            .find(|declaration| declaration.symbol == template)?;
        let presentation = if original.selected_interface.is_valid() {
            original.selected_interface
        } else {
            owner.symbol
        };
        if original.registration_operation != template
            || original.binding_declaration != template
            || declaration.name != signature.name
            || (selected_interface.is_valid() && selected_interface != presentation)
            || self.trait_definition_by_symbol(presentation).is_none()
        {
            return None;
        }
        Some(PublicTraitRequirementSignature {
            owner,
            declaration,
            signature,
            contract: signature,
            application,
            selected_interface,
        })
    }

    /// Exact selected public signature under its original declaring trait.
    /// An owned header additionally answers to the call's retained original
    /// application; a fresh target symbol alone cannot substitute another
    /// tuple. This projection grants no provider or argument authority.
    pub fn selected_trait_requirement_signature(
        &self,
        target: symbols::SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<(
        &crate::trait_definition::TraitDefinition,
        &crate::signature::StateSignature,
    )> {
        let selected = self.selected_public_trait_requirement_signature(target, application)?;
        Some((selected.owner, selected.signature))
    }

    pub fn selected_public_trait_requirement_signature(
        &self,
        target: symbols::SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<PublicTraitRequirementSignature<'_>> {
        let mut selected = self.public_trait_requirement_signature(target)?;
        if selected.application.is_valid() {
            let same_application = if self.closed_requirement_application(target).is_some() {
                self.closed_requirement_application_matches_call(target, application)
            } else {
                crate::static_applications::static_application_identities_equal(
                    self,
                    application,
                    self,
                    selected.application,
                )
                .is_ok_and(|same| same)
            };
            if !same_application {
                return None;
            }
        }
        // The executed target authenticates the original declaration and call
        // tuple above. Its caller-owned contract has already substituted the
        // complete telescope, including symbolic caller operands; consumers
        // must not infer that telescope again from runtime argument shapes.
        if let Some(signature) =
            self.public_requirement_contract(application, selected.declaration.symbol)
        {
            selected.contract = signature;
            selected.application = application;
            selected.selected_interface = self
                .expression_table
                .static_machine_application(application)
                .selected_interface;
        }
        Some(selected)
    }

    /// The exact original interface namespace of a selected public callable.
    /// A produced header keeps its selected presentation independently of its
    /// declaring parent. This reads original application custody, never an
    /// implicit receiver, provider choice or descendant search. Zero is absent.
    pub fn public_requirement_namespace_symbol(
        &self,
        target: symbols::SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> symbols::SymbolHandle {
        let Some(selected) = self.selected_public_trait_requirement_signature(target, application)
        else {
            return symbols::SymbolHandle::invalid();
        };
        if !application.is_valid() {
            let Ok(telescope) =
                crate::static_applications::StaticApplicationDeclaration::for_symbol(self, target)
            else {
                return symbols::SymbolHandle::invalid();
            };
            return if telescope.binders().next().is_none() && telescope.lifetimes().is_empty() {
                selected.owner.symbol
            } else {
                symbols::SymbolHandle::invalid()
            };
        }
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return symbols::SymbolHandle::invalid();
        }
        let original = self
            .expression_table
            .static_machine_application(application);
        if original.binding_declaration != selected.declaration.symbol
            || crate::static_applications::StaticApplicationDeclaration::for_application(
                self, original,
            )
            .is_err()
        {
            return symbols::SymbolHandle::invalid();
        }
        // Symbolic applications may still name the authored requirement rather
        // than an executable header. Their selected interface remains the
        // original presentation; the declaring parent cannot replace it.
        let presentation = if original.selected_interface.is_valid() {
            original.selected_interface
        } else {
            selected.owner.symbol
        };
        if selected.application.is_valid()
            && selected.selected_interface.is_valid()
            && selected.selected_interface != presentation
        {
            return symbols::SymbolHandle::invalid();
        }
        presentation
    }

    /// Rejoin the source qualifier to its exact selected callable declaration.
    /// Bodyless named operators and trait requirements have no machine state;
    /// limiting this query to machine bodies invents a read of their namespace
    /// during nested argument evaluation. Keep unknown or instance qualifiers
    /// observable, and leave actual operands on the ordinary ordered read path.
    pub fn call_has_no_runtime_receiver_for_target(
        &self,
        call: &crate::expression::TableCallExpression,
    ) -> bool {
        self.call_has_no_runtime_receiver_for_selected_target(call, call.target_symbol)
    }

    /// The caller supplies its already resolved occurrence target. This query
    /// retains the original operands, namespace and application; it neither
    /// chooses a declaration nor changes the call's source identity.
    pub fn call_has_no_runtime_receiver_for_selected_target(
        &self,
        call: &crate::expression::TableCallExpression,
        target_symbol: symbols::SymbolHandle,
    ) -> bool {
        if let Some(owner) = self
            .machine_by_symbol(target_symbol)
            .or_else(|| self.machine_holding_state(target_symbol))
        {
            return self.call_receiver_names_machine_owner(call, owner);
        }
        if crate::operator::selected_named_expression_call_has_no_runtime_receiver(
            self,
            call,
            target_symbol,
        ) {
            return true;
        }
        let slot = self.dynamic_slot_by_symbol(target_symbol);
        let requirement = slot.map_or(target_symbol, |slot| slot.requirement);
        let Some((owner, signature)) = self
            .selected_trait_requirement_signature(requirement, call.authored_machine_application)
        else {
            return false;
        };
        if slot.is_some_and(|slot| slot.declaring_trait != owner.symbol) {
            return false;
        }
        let parameters = self.state_signature_parameters(signature);
        let operand_count = match slot {
            Some(slot) if slot.data_parameters().len() == parameters.len() => slot.parameters.len(),
            Some(_) => return false,
            None => parameters.len(),
        };
        let arguments = self.expression_table.expression_handles(call.arguments);
        if !target_symbol.is_valid()
            || arguments.len() != call.arguments.count() as usize
            || arguments.len() != operand_count
        {
            return false;
        }
        let namespace = self
            .public_requirement_namespace_symbol(requirement, call.authored_machine_application);
        if !namespace.is_valid() {
            return false;
        }
        if !call.receiver.is_valid() {
            return true;
        }
        if !self.expression_table.expression_is_valid(call.receiver) {
            return false;
        }
        matches!(self.expression_table.expression(call.receiver),
            crate::expression::ExpressionNode::Name(path)
                if path.symbol == namespace)
    }

    /// A data-qualified call names its owner without evaluating a receiver.
    /// A value path, borrow, or other expression is still a runtime receiver.
    pub fn call_has_no_runtime_receiver(
        &self,
        call: &crate::expression::TableCallExpression,
        owner: &crate::machine::Machine,
        target: &crate::state::State,
    ) -> bool {
        if call.target_symbol != target.symbol {
            return false;
        }
        self.call_receiver_names_machine_owner(call, owner)
    }

    fn call_receiver_names_machine_owner(
        &self,
        call: &crate::expression::TableCallExpression,
        owner: &crate::machine::Machine,
    ) -> bool {
        if !call.receiver.is_valid() {
            return true;
        }
        if !owner.attached_data_symbol.is_valid()
            || !self.expression_table.expression_is_valid(call.receiver)
        {
            return false;
        }
        let crate::expression::ExpressionNode::Name(path) =
            self.expression_table.expression(call.receiver)
        else {
            return false;
        };
        path.symbol == owner.attached_data_symbol
    }
}
