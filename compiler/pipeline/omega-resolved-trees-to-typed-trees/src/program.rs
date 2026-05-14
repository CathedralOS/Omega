use crate::data::lower_data_definition;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::platform::lower_platform;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees::ResolvedTrees;
use omega_typed_trees::Program as TypedProgram;

pub fn lower_resolved_trees(resolved_program: &ResolvedTrees) -> Result<TypedProgram, Diagnostic> {
    let mut lowerer = Lowerer {
        program: TypedProgram::default(),
        source_program: resolved_program,
    };

    for invariant_definition in &resolved_program.invariant_definitions {
        let invariant_definition = lower_invariant_definition(&mut lowerer, invariant_definition)?;
        lowerer.program.invariant_definitions.push(invariant_definition);
    }

    for data_definition in &resolved_program.data_definitions {
        let data_definition = lower_data_definition(&mut lowerer, data_definition)?;
        lowerer.program.data_definitions.push(data_definition);
    }

    for machine in &resolved_program.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.program.machines.push(machine);
    }

    for platform in &resolved_program.platforms {
        let platform = lower_platform(&mut lowerer, platform)?;
        lowerer.program.platforms.push(platform);
    }

    lowerer.finish()
}

pub fn lower_program(resolved_program: &ResolvedTrees) -> Result<TypedProgram, Diagnostic> {
    lower_resolved_trees(resolved_program)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) program: TypedProgram,
    pub(crate) source_program: &'source ResolvedTrees,
}

impl Lowerer<'_> {
    pub(crate) fn finish(mut self) -> Result<TypedProgram, Diagnostic> {
        self.program.symbols = self.source_program.symbols.clone();
        self.program.rebuild_tables();
        Ok(self.program)
    }
}

#[cfg(test)]
mod tests {
    use super::lower_resolved_trees;
    use omega_source_files_to_tokens::Lexer;
    use omega_syntax_trees_to_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn lowers_dungeon_style_machine_program() {
        let source = r#"
        data Inventory {
            gold: u32[exact];
        }

        machine Inventory::clear {
            pub entry(&mut self, inventory: &mut Inventory) {
                inventory.gold = 0;
            }
        }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
        let program = lower_resolved_trees(&resolved_program).expect("lowering should succeed");

        assert_eq!(program.data_definitions.len(), 1);
        assert_eq!(program.machines.len(), 1);
        assert_eq!(program.machines[0].states.len(), 1);
        assert!(program.symbols.find_child_by_name(program.symbols.root(), "u32").is_some());
    }
}
