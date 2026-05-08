use crate::layout::packing::{PlannedField, pack_fields};
use crate::layout::sizing::primitive_type_layout;
use crate::target::NativeTarget;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_layout::{DataLayout, DataShape, FieldLayout, LayoutPlan, MachineLayout, TypeLayout};
use omega_typed_program::Program;
use omega_typed_program::data::{DataDefinition, DataMember, DataShapeKind};
use omega_typed_program::machine::Machine;
use omega_typed_program::types::{PrimitiveType, TypeConstraint, TypeReference};

pub(super) struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: Vec<String>,
    fields: Arena<FieldLayout>,
    machine_definitions: &'program [Machine],
    machine_layouts: Arena<MachineLayout>,
    machine_visiting: Vec<String>,
    target: NativeTarget,
    type_constraints: &'program Arena<TypeConstraint>,
}

impl<'program> LayoutBuilder<'program> {
    pub(super) fn new(program: &'program Program, target: NativeTarget) -> Self {
        Self {
            data_definitions: &program.data_definitions,
            data_layouts: Arena::new(),
            data_visiting: Vec::new(),
            fields: Arena::new(),
            machine_definitions: &program.machines,
            machine_layouts: Arena::new(),
            machine_visiting: Vec::new(),
            target,
            type_constraints: &program.type_constraints,
        }
    }

    pub(super) fn finish(self) -> LayoutPlan {
        LayoutPlan {
            data_layouts: self.data_layouts,
            fields: self.fields,
            machine_layouts: self.machine_layouts,
        }
    }

    pub(super) fn layout_data_definition(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(data_layout) = self
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == name)
            .map(|(_, data_layout)| data_layout)
        {
            return Ok(data_layout.layout);
        }

        if self.data_visiting.iter().any(|visiting| visiting == name) {
            return Err(Diagnostic::error(format!(
                "recursive data layout is not supported yet for `{name}`"
            )));
        }

        self.data_visiting.push(name.to_owned());

        let definition = self.data_definition(name)?;
        let data_layout = self.compute_data_layout(definition)?;
        let layout = data_layout.layout;

        self.data_layouts.insert(data_layout);
        self.data_visiting.pop();

        Ok(layout)
    }

    pub(super) fn layout_machine(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(machine_layout) = self
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.name == name)
            .map(|(_, machine_layout)| machine_layout)
        {
            return Ok(machine_layout.layout);
        }

        if self
            .machine_visiting
            .iter()
            .any(|visiting| visiting == name)
        {
            return Err(Diagnostic::error(format!(
                "recursive machine layout is not supported yet for `{name}`"
            )));
        }

        self.machine_visiting.push(name.to_owned());

        let machine = self.machine_definition(name)?;
        let machine_layout = self.compute_machine_layout(machine)?;
        let layout = machine_layout.layout;

        self.machine_layouts.insert(machine_layout);
        self.machine_visiting.pop();

        Ok(layout)
    }

    fn compute_data_layout(
        &mut self,
        definition: &DataDefinition,
    ) -> Result<DataLayout, Diagnostic> {
        if definition.shape_kind() == DataShapeKind::Enum {
            let variants = definition
                .members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant.name.clone()),
                    DataMember::Field(_) => None,
                })
                .collect();

            return Ok(DataLayout {
                name: definition.name.clone(),
                shape: DataShape::Enum { variants },
                layout: TypeLayout {
                    size: 4,
                    alignment: 4,
                },
            });
        }

        let fields = definition
            .members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                DataMember::Variant(_) => None,
            })
            .map(|field| {
                let layout = self.layout_type_reference(&field.type_reference)?;
                Ok(PlannedField {
                    name: field.name.clone(),
                    type_name: field
                        .type_reference
                        .display_name_with_constraints(self.type_constraints),
                    layout,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let (fields, layout) = pack_fields(fields);
        let fields = self.fields.insert_many(fields);

        Ok(DataLayout {
            name: definition.name.clone(),
            shape: DataShape::Record { fields },
            layout,
        })
    }

    fn compute_machine_layout(&mut self, machine: &Machine) -> Result<MachineLayout, Diagnostic> {
        let mut fields = Vec::new();

        for owned_data in &machine.owned_data {
            fields.push(PlannedField {
                name: owned_data.name.clone(),
                type_name: owned_data
                    .type_reference
                    .display_name_with_constraints(self.type_constraints),
                layout: self.layout_type_reference(&owned_data.type_reference)?,
            });
        }

        for contained_object in &machine.contains {
            if self.machine_definition(&contained_object.type_name).is_ok() {
                fields.push(PlannedField {
                    name: contained_object.name.clone(),
                    type_name: contained_object.type_name.to_string(),
                    layout: self.layout_machine(&contained_object.type_name)?,
                });
            }
        }

        let (fields, layout) = pack_fields(fields);
        let fields = self.fields.insert_many(fields);

        Ok(MachineLayout {
            name: machine.name.clone(),
            fields,
            layout,
        })
    }

    fn layout_type_reference(
        &mut self,
        type_reference: &TypeReference,
    ) -> Result<TypeLayout, Diagnostic> {
        match type_reference {
            TypeReference::Constrained { base_type, .. } => self.layout_type_reference(base_type),
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                let element_layout = self.layout_type_reference(element_type)?;

                Ok(TypeLayout {
                    size: element_layout.size * length,
                    alignment: element_layout.alignment,
                })
            }
            TypeReference::Generic { base_name, .. } => Err(Diagnostic::error(format!(
                "native layout for generic type `{base_name}` is not implemented yet"
            ))),
            TypeReference::Named(name) => self.layout_named_type(name),
            TypeReference::Unit => Ok(TypeLayout {
                size: 0,
                alignment: 1,
            }),
        }
    }

    fn layout_named_type(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(primitive_type) = PrimitiveType::from_name(name) {
            return Ok(primitive_type_layout(self.target, primitive_type));
        }

        self.layout_data_definition(name)
    }

    fn data_definition(&self, name: &str) -> Result<&'program DataDefinition, Diagnostic> {
        self.data_definitions
            .iter()
            .find(|definition| definition.name == name)
            .ok_or_else(|| Diagnostic::error(format!("unknown data type `{name}`")))
    }

    fn machine_definition(&self, name: &str) -> Result<&'program Machine, Diagnostic> {
        self.machine_definitions
            .iter()
            .find(|machine| machine.name == name)
            .ok_or_else(|| Diagnostic::error(format!("unknown machine `{name}`")))
    }
}
