//! Atomic record establishment over the interpreter's existing structural storage.
//! Constructors stage child storage and custody before publishing the parent.
//! Linear claims relocate by identity and typed path; the affine frontier owns
//! only their disjoint complement. Records, arrays and cases share that staged
//! ownership transaction in `record/operands.rs`, so nesting neither remints a
//! child's claim nor disposes it as an affine subtree.
//! Carrier disposal multiplicity is not copyability: a consumed shared
//! reference can enter an unrestricted aggregate. Psi admits the constructor;
//! the transaction transfers actual debts and claims, not the parent's label.
//! `record/storage.rs` stages the complete typed payload for constructors and
//! independent owned call/block arguments; copying only scalar fields would
//! drop nested array backing, case discriminators and primitive cells.
//! Initialized homes and resource custody have distinct lifetimes. A partial
//! transfer retires its selected subtree, not the scalar siblings of its home.
//! Completion retires remaining frame bindings after checked cleanup has settled
//! every affine obligation and linear claim; outgoing backing remains separate.
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralRuntimePlace;
use crate::values::StructuralScalarRuntimeField;
use crate::values::TerminalStructuralValue;
use std::collections::BTreeMap;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralFieldType;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;

mod byte_fields;
mod operands;
pub(crate) mod storage;
mod structural_array;
use storage::{ConstructorStorage, ConstructorValue};

impl TerminalExecution<'_> {
    pub(crate) fn establish_record(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        fields: &[terminal_psi::RecordFieldInitializer],
    ) -> Result<(), TerminalInterpretError> {
        // Whole-result memberships are Psi's checked construction judgments.
        // The constructor establishes storage, not those judgments; retaining
        // them must not weaken the actual-child qualification/custody joins.
        let declaration = self
            .program
            .structural_types()
            .get(&result.structural_type)
            .ok_or_else(malformed)?;
        let StructuralTypeShape::Record {
            fields: declarations,
            ..
        } = &declaration.shape
        else {
            return Err(malformed());
        };
        let declarations = declarations
            .iter()
            .filter(|field| !field.relevance.is_erased());
        if fields.len() != declarations.clone().count() {
            return Err(malformed());
        }
        let mut value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        let mut staged = BTreeMap::new();
        let mut storage = ConstructorStorage::default();

        let mut consumed = Vec::new();
        let mut claim_operands = Vec::new();
        let mut frontier = self.live_affine_frontier.clone();
        for (binding, declaration) in fields.iter().zip(declarations) {
            if binding.field != declaration.id || declaration.relevance.is_erased() {
                return Err(malformed());
            }
            match &binding.value {
                terminal_psi::RecordFieldValue::Scalar { value: operand, .. } => {
                    let scalar = self.values.get(operand).copied().ok_or_else(malformed)?;
                    if declaration.field_type.scalar_type() != Some(scalar.scalar_type())
                        || !terminal_scalar_belongs_to_type(scalar)
                    {
                        return Err(malformed());
                    }
                    staged.insert(
                        StructuralScalarRuntimeField::member(
                            StructuralRuntimePlace::from(&value),
                            binding.field,
                        ),
                        scalar,
                    );
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    claim_operands.push((
                        argument,
                        vec![StructuralPathSegment::Field(declaration.identity.clone())],
                    ));
                    let child = ConstructorValue::resolve(self, argument)?;
                    // The child already completed in authored order. Rebase
                    // its actual memberships, never the destination field's
                    // declared requirements, onto this new record occurrence.
                    if matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
                        child.append_memberships(
                            &[StructuralPathSegment::Field(declaration.identity.clone())],
                            &mut value.projected_qualifications,
                        );
                    }
                    if let Some(bytes) = self.prepare_constructor_byte_field(
                        declaration,
                        argument,
                        &child,
                        &mut frontier,
                        &mut consumed,
                    )? {
                        storage.stage_byte_field(
                            crate::values::StructuralByteSequenceRuntimeField {
                                parent: StructuralRuntimePlace::from(&value),
                                namespace: crate::values::StructuralFieldNamespace::Member,
                                field: binding.field,
                            },
                            bytes,
                        );
                        continue;
                    }
                    if argument.access != StructuralAccess::Owned
                        || declaration.field_type
                            != StructuralFieldType::Structural(child.structural_type())
                    {
                        return Err(malformed());
                    }
                    let transfers_custody =
                        self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
                    let mut member_path = value.path.clone();
                    member_path.push(StructuralPathSegment::Field(declaration.identity.clone()));
                    storage.stage(
                        self,
                        argument,
                        &child,
                        &StructuralRuntimePlace {
                            opaque_identity: value.opaque_identity,
                            path: member_path,
                        },
                        transfers_custody,
                    )?;
                }
            }
        }
        value.projected_qualifications.sort();
        value.projected_qualifications.dedup();
        if result
            .projected_qualifications
            .iter()
            .any(|row| !value.projected_qualifications.contains(row))
        {
            return Err(malformed());
        }
        let claims =
            self.stage_constructor_claims(result, &claim_operands, &mut frontier, &|current| {
                storage.case_at_projection(self, &value, &current.path)
            })?;
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        self.structural_scalar_fields.extend(staged);
        storage.publish(self)?;
        self.structural_values.insert(result.place, value);
        Ok(())
    }
    /// Give each independent owned copy its own storage identity. Composite
    /// payloads are staged here; byte binding already detached the live prefix.
    /// Both must stop naming the caller's storage before a reference can capture
    /// the new home. Otherwise a nested call can resolve the callee's loan to
    /// the suspended caller's independent snapshot. Borrowed descriptors retain
    /// their original backing identity.
    /// The values and cursor belong to the uncommitted call or successor edge;
    /// source storage remains untouched even in swaps.
    pub(crate) fn stage_owned_value_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        values: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
        cursor: &mut Option<u64>,
    ) -> Result<ConstructorStorage, TerminalInterpretError> {
        let mut payload = ConstructorStorage::default();
        for parameter in parameters {
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            {
                continue;
            }
            let types = self.program.structural_types();
            let declaration = types
                .get(&parameter.structural_type)
                .ok_or_else(malformed)?;
            let composite = declaration.shape.is_composite_shape();
            let primitive = match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
                _ => None,
            };
            if !composite && primitive.is_none() && !declaration.is_bounded_owned_byte_sequence() {
                continue;
            }
            let value = values.get_mut(&parameter.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
            )?;
            let source = StructuralRuntimePlace::from(&*value);
            let identity = self.local_structural_identities.allocate_staged(cursor)?;
            if let Some(scalar_type) = primitive {
                let (storage, actual, shared_view) =
                    self.primitive_view_access(value, &[], scalar_type, false)?;
                let scalar = self.read_primitive_storage(
                    parameter.place,
                    storage,
                    actual,
                    scalar_type,
                    shared_view,
                )?;
                payload.stage_scalar_endpoint(
                    &crate::primitive_storage::PrimitiveStorageEndpoint::Scalar(
                        StructuralRuntimePlace {
                            opaque_identity: identity,
                            path: Vec::new(),
                        },
                    ),
                    scalar,
                )?;
            } else if composite {
                payload.stage_subtree(
                    self,
                    &source,
                    &StructuralRuntimePlace {
                        opaque_identity: identity,
                        path: Vec::new(),
                    },
                    false,
                )?;
            }
            value.opaque_identity = identity;
            value.path.clear();
        }
        Ok(payload)
    }
}
