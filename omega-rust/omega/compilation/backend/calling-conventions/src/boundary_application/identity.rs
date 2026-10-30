//! Canonical application identity shared by producer and source-free admission.
//! The expected commitment comes from the independently admitted selected method;
//! hashing an untrusted payload alone does not authenticate that payload.
use super::*;
use crate::{BoundaryNativeParameterOrigin, BoundaryValueClass};

impl BoundaryCallingApplication {
    pub fn recomputed_commitment(&self) -> Result<[u8; 32], String> {
        let native_parameters = self
            .signature
            .native_parameters
            .span(self.signature.native_parameter_rows)
            .ok_or("native telescope has stale row storage")?;
        let mut strong =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.boundary-plan-application.v6");
        strong.string(&self.requirement_identity);
        strong.byte(match self.target.architecture {
            target::Architecture::Aarch64 => 1,
            target::Architecture::X86_64 => 2,
        });
        strong.byte(match self.target.object_format {
            target::ObjectFormat::Elf => 1,
            target::ObjectFormat::MachO => 2,
            target::ObjectFormat::Coff => 3,
        });
        strong.u64(self.target.pointer_size as u64);
        strong.u64(self.target.pointer_alignment as u64);
        strong.u64(self.opaque_applications.len() as u64);
        for representation in &self.opaque_applications {
            strong.raw(&representation.shape_root.to_le_bytes());
            strong.raw(&representation.application_report_fingerprint.to_le_bytes());
            strong.raw(&representation.conformance_application_commitment);
            strong.raw(&representation.representation_schema_version.to_le_bytes());
            strong.byte(1);
            strong.byte(1);
            strong.byte(match representation.checked_semantic_copy {
                false => 1,
                true => 2,
            });
            strong.raw(&representation.selected_application_commitment);
        }
        strong.u64(self.signature.shapes.len() as u64);
        for shape in &self.signature.shapes {
            match shape.class {
                BoundaryValueClass::Integer => strong.byte(1),
                BoundaryValueClass::Float => strong.byte(2),
                BoundaryValueClass::Reference => strong.byte(3),
                BoundaryValueClass::FixedArray { element, length } => {
                    strong.byte(4);
                    strong.raw(&element.to_le_bytes());
                    strong.raw(&length.to_le_bytes());
                }
                BoundaryValueClass::Record {
                    first_field,
                    field_count,
                } => {
                    strong.byte(5);
                    strong.raw(&first_field.to_le_bytes());
                    strong.raw(&field_count.to_le_bytes());
                }
            }
            strong.raw(&shape.byte_size.to_le_bytes());
            strong.raw(&shape.alignment.to_le_bytes());
        }
        strong.u64(self.signature.fields.len() as u64);
        for field in &self.signature.fields {
            strong.raw(&field.shape.to_le_bytes());
            strong.raw(&field.byte_offset.to_le_bytes());
        }
        strong.u64(self.signature.parameters.len() as u64);
        for parameter in &self.signature.parameters {
            strong.raw(&parameter.to_le_bytes());
        }
        match self.signature.result {
            Some(result) => {
                strong.byte(1);
                strong.raw(&result.to_le_bytes());
            }
            None => strong.byte(0),
        }
        strong.u64(native_parameters.len() as u64);
        for parameter in native_parameters {
            strong.raw(&parameter.identity.get().to_le_bytes());
            strong.raw(&parameter.native_ordinal.to_le_bytes());
            match parameter.origin {
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                    strong.byte(1);
                    strong.raw(&formal_ordinal.to_le_bytes());
                }
                BoundaryNativeParameterOrigin::PrivateCallback {
                    binder,
                    requirement,
                } => {
                    strong.byte(2);
                    strong.raw(&binder.get().to_le_bytes());
                    strong.raw(&requirement.get().to_le_bytes());
                }
            }
            match parameter.shape {
                BoundaryNativeParameterShape::Semantic(root) => {
                    strong.byte(1);
                    strong.raw(&root.to_le_bytes());
                }
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => {
                    strong.byte(2);
                    strong.raw(&byte_size.to_le_bytes());
                    strong.raw(&alignment.to_le_bytes());
                }
            }
        }
        let catalog = self
            .signature
            .callback_layouts
            .span(self.signature.callback_layout_rows)
            .ok_or("callback catalog has stale entry storage")?;
        strong.len(catalog.len());
        for entry in catalog {
            entry.field_offset(self.target)?;
            entry.encode(&mut strong)?;
        }
        let bindings = self
            .signature
            .callback_bindings
            .span(self.signature.callback_binding_rows)
            .ok_or("callback application has stale binder correspondence")?;
        strong.len(bindings.len());
        for (position, binding) in bindings.iter().enumerate() {
            if binding.requirement_identity.is_empty()
                || crate::callback_requirement_id(&binding.requirement_identity)
                    != binding.requirement
                || bindings[..position].iter().any(|prior| {
                    prior.binder == binding.binder
                        || prior.static_machine_ordinal >= binding.static_machine_ordinal
                })
            {
                return Err("callback application has invalid exact binder correspondence".into());
            }
            strong.raw(&binding.binder.get().to_le_bytes());
            strong.raw(&binding.requirement.get().to_le_bytes());
            strong.raw(&binding.static_machine_ordinal.to_le_bytes());
            strong.string(&binding.requirement_identity);
            strong.byte(u8::from(binding.invokes_synchronously));
        }
        if bindings.len() != self.callback_context.binders.len()
            || bindings.iter().any(|binding| {
                self.callback_context
                    .binders
                    .iter()
                    .filter(|row| {
                        row.binder == binding.binder && row.requirement == binding.requirement
                    })
                    .count()
                    != 1
            })
        {
            return Err("callback application changed its complete binder correspondence".into());
        }
        strong.raw(&self.plan.contract_commitment_digest());
        let commitment: [u8; 32] = strong.finish().into();
        Ok(commitment)
    }
}
