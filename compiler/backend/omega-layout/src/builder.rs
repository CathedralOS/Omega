use crate::packing::{PlannedField, pack_fields};
use crate::sizing::primitive_type_layout;
use crate::{
    DataLayout, DataShape, FieldLayout, LayoutPlan, MachineLayout, TypeLayout, VariantLayout,
};
use omega_checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::platform::Platform;
use omega_checked_trees::types::{PrimitiveType, TypeConstraint, TypeReference};
use omega_checked_trees::Program;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_target::NativeTarget;

pub fn build_layout_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<LayoutPlan, Diagnostic> {
    let mut builder = LayoutBuilder::new(program, target);

    for data_definition in program.data_definitions() {
        builder.layout_data_definition(data_definition.symbol)?;
    }

    for machine in program.machines() {
        builder.layout_machine(machine.symbol)?;
    }

    Ok(builder.finish())
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: Vec<SymbolHandle>,
    fields: Arena<FieldLayout>,
    machine_definitions: &'program [Machine],
    machine_layouts: Arena<MachineLayout>,
    machine_visiting: Vec<SymbolHandle>,
    platform_definitions: &'program [Platform],
    program: &'program Program,
    target: NativeTarget,
    type_constraints: &'program Arena<TypeConstraint>,
}

impl<'program> LayoutBuilder<'program> {
    fn new(program: &'program Program, target: NativeTarget) -> Self {
        Self {
            data_definitions: program.data_definitions(),
            data_layouts: Arena::new(),
            data_visiting: Vec::new(),
            fields: Arena::new(),
            machine_definitions: program.machines(),
            machine_layouts: Arena::new(),
            machine_visiting: Vec::new(),
            platform_definitions: program.platforms(),
            program,
            target,
            type_constraints: &program.type_constraints,
        }
    }

    fn finish(self) -> LayoutPlan {
        LayoutPlan {
            data_layouts: self.data_layouts,
            fields: self.fields,
            machine_layouts: self.machine_layouts,
        }
    }

    fn layout_data_definition(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some(data_layout) = self
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == symbol)
            .map(|(_, data_layout)| data_layout)
        {
            return Ok(data_layout.layout);
        }

        if self.data_visiting.contains(&symbol) {
            return Err(Diagnostic::error(format!(
                "recursive data layout is not supported yet for symbol {}",
                symbol.arena_index()
            )));
        }

        self.data_visiting.push(symbol);

        let definition = self.data_definition_by_symbol(symbol)?;
        let data_layout = self.compute_data_layout(definition)?;
        let layout = data_layout.layout;

        self.data_layouts.insert(data_layout);
        self.data_visiting.pop();

        Ok(layout)
    }

    fn layout_machine(&mut self, symbol: SymbolHandle) -> Result<TypeLayout, Diagnostic> {
        if let Some(machine_layout) = self
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == symbol)
            .map(|(_, machine_layout)| machine_layout)
        {
            return Ok(machine_layout.layout);
        }

        if self.machine_visiting.contains(&symbol) {
            return Err(Diagnostic::error(format!(
                "recursive machine layout is not supported yet for symbol {}",
                symbol.arena_index()
            )));
        }

        self.machine_visiting.push(symbol);

        let machine = self.machine_definition_by_symbol(symbol)?;
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
                    DataMember::Variant(variant) => Some(VariantLayout {
                        symbol: variant.symbol,
                        name: variant.name.clone(),
                    }),
                    DataMember::Field(_) => None,
                })
                .collect();

            return Ok(DataLayout {
                symbol: definition.symbol,
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
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_symbol: self.type_reference_symbol(&field.type_reference),
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
            symbol: definition.symbol,
            name: definition.name.clone(),
            shape: DataShape::Record { fields },
            layout,
        })
    }

    fn compute_machine_layout(&mut self, machine: &Machine) -> Result<MachineLayout, Diagnostic> {
        let mut fields = Vec::new();

        if let Some(data_definition) = self
            .data_definitions
            .iter()
            .find(|definition| definition.name == machine.name)
        {
            for member in &data_definition.members {
                let DataMember::Field(field) = member else {
                    continue;
                };

                fields.push(PlannedField {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_symbol: self.type_reference_symbol(&field.type_reference),
                    type_name: field
                        .type_reference
                        .display_name_with_constraints(self.type_constraints),
                    layout: self.layout_type_reference(&field.type_reference)?,
                });
            }
        }

        for owned_data in &machine.owned_data {
            fields.push(PlannedField {
                symbol: owned_data.symbol,
                name: owned_data.name.clone(),
                type_symbol: self.type_reference_symbol(&owned_data.type_reference),
                type_name: owned_data
                    .type_reference
                    .display_name_with_constraints(self.type_constraints),
                layout: self.layout_type_reference(&owned_data.type_reference)?,
            });
        }

        for contained_object in self.program.machine_contained_objects(machine) {
            if self
                .machine_definition_by_symbol(contained_object.type_symbol)
                .is_ok()
            {
                fields.push(PlannedField {
                    symbol: contained_object.symbol,
                    name: contained_object.name.clone(),
                    type_symbol: contained_object.type_symbol,
                    type_name: contained_object.type_name.to_string(),
                    layout: self.layout_machine(contained_object.type_symbol)?,
                });
            }
        }

        let (fields, layout) = pack_fields(fields);
        let fields = self.fields.insert_many(fields);

        Ok(MachineLayout {
            symbol: machine.symbol,
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
            TypeReference::Reference { .. } => Ok(TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            }),
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
            TypeReference::Slice { .. } => Ok(self.slice_layout()),
            TypeReference::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if let Some(layout) = builtin_named_layout(self.target, base_name) {
                    return Ok(layout);
                }

                if base_symbol.is_valid()
                    && let Ok(definition) = self.data_definition_by_symbol(*base_symbol)
                    && definition.shape_kind() == DataShapeKind::Enum
                {
                    return self.layout_data_definition(*base_symbol);
                }

                Err(Diagnostic::error(format!(
                    "native layout for generic type `{base_name}` is not implemented yet"
                )))
            }
            TypeReference::Named { symbol, name } => self.layout_named_type(*symbol, name),
            TypeReference::Unit => Ok(TypeLayout {
                size: 0,
                alignment: 1,
            }),
        }
    }

    fn slice_layout(&self) -> TypeLayout {
        TypeLayout {
            size: self.target.pointer_size * 2,
            alignment: self.target.pointer_alignment,
        }
    }
    fn layout_named_type(
        &mut self,
        symbol: SymbolHandle,
        name: &str,
    ) -> Result<TypeLayout, Diagnostic> {
        if let Some(primitive_type) = PrimitiveType::from_name(name) {
            return Ok(primitive_type_layout(self.target, primitive_type));
        }

        if let Some(layout) = builtin_named_layout(self.target, name) {
            return Ok(layout);
        }

        if !symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "non-primitive type `{name}` is missing a resolved symbol"
            )));
        }

        if self
            .data_definitions
            .iter()
            .any(|definition| definition.symbol == symbol)
        {
            return self.layout_data_definition(symbol);
        }

        if self
            .machine_definitions
            .iter()
            .any(|machine| machine.symbol == symbol)
        {
            return self.layout_machine(symbol);
        }

        if self
            .platform_definitions
            .iter()
            .any(|platform| platform.symbol == symbol)
        {
            return Ok(TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            });
        }

        Err(Diagnostic::error(format!(
            "unknown layout-bearing type `{name}` for symbol {}",
            symbol.arena_index()
        )))
    }

    fn type_reference_symbol(&self, type_reference: &TypeReference) -> SymbolHandle {
        match type_reference {
            TypeReference::Reference { referee, .. } => self.type_reference_symbol(referee),
            TypeReference::Constrained { base_type, .. } => self.type_reference_symbol(base_type),
            TypeReference::FixedArray { element_type, .. } => {
                self.type_reference_symbol(element_type)
            }
            TypeReference::Slice { .. } => SymbolHandle::invalid(),
            TypeReference::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if PrimitiveType::from_name(base_name).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *base_symbol
                }
            }
            TypeReference::Named { symbol, name } => {
                if PrimitiveType::from_name(name).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *symbol
                }
            }
            TypeReference::Unit => SymbolHandle::invalid(),
        }
    }

    fn data_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&'program DataDefinition, Diagnostic> {
        self.data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown data type symbol {}", symbol.arena_index()))
            })
    }

    fn machine_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Result<&'program Machine, Diagnostic> {
        self.machine_definitions
            .iter()
            .find(|machine| machine.symbol == symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown machine symbol {}", symbol.arena_index()))
            })
    }
}

fn builtin_named_layout(target: NativeTarget, name: &str) -> Option<TypeLayout> {
    if name == "IndexOf" {
        return Some(TypeLayout {
            size: target.pointer_size,
            alignment: target.pointer_alignment,
        });
    }

    match name {
        "Uint" => Some(TypeLayout {
            size: target.pointer_size,
            alignment: target.pointer_alignment,
        }),
        "Real" => Some(TypeLayout {
            size: 8,
            alignment: 8,
        }),
        _ => None,
    }
}
