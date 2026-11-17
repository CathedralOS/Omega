mod tests {
    use super::*;

    #[test]
    fn actual_and_inherited_declarations_keep_their_source_correspondence() {
        for (_, text, name) in workloads() {
            let program = typed(&text);
            projected(&program, name).validate_correspondence().unwrap();
        }
    }

    #[test]
    fn wrong_package_missing_source_and_lost_order_refuse() {
        let program = typed(&family(2, true));
        let mut projection = projected(&program, "Service");
        let saved = projection.requirements[0];
        projection.requirements[0].symbol = symbols::SymbolHandle::invalid();
        assert_eq!(
            projection.validate_correspondence().unwrap_err().message,
            "service row lost its exact declared requirement"
        );
        projection.requirements[0] = saved;
        projection.requirements[0].declaring_trait = symbols::SymbolHandle::invalid();
        assert_eq!(
            projection.validate_correspondence().unwrap_err().message,
            "service row lost its exact declaring trait"
        );
        projection.requirements[0] = saved;
        Arc::make_mut(&mut projection.declaration).methods[0].requirement_owner_package_identity =
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([29; 32]).unwrap());
        assert_eq!(
            projection.validate_correspondence().unwrap_err().message,
            "service row has no unique exact inherited requirement"
        );
        projection.requirements.pop();
        assert_eq!(
            projection.validate_correspondence().unwrap_err().message,
            "service row lost its ordered source correspondence"
        );
    }

    #[test]
    fn equal_leaf_names_and_applied_parent_arguments_keep_exact_owners() {
        let program = typed(
            "boundary trait Parent<T> { machine read(value: T) -> T reaches none crashes none; }\nboundary trait Other { machine read(value: u64) -> u64 reaches none crashes none; }\nboundary trait Service: Parent<u64> { machine read(value: u32) -> u32 reaches none crashes none; }\n",
        );
        let mut projection = projected(&program, "Service");
        projection.validate_correspondence().unwrap();
        let other = projected(&program, "Other");
        projection.requirements[0] = other.requirements[0];
        assert_eq!(
            projection.validate_correspondence().unwrap_err().message,
            "service row has no unique exact inherited requirement"
        );
    }

    #[test]
    fn duplicate_canonical_requirements_refuse_for_one_and_many_queries() {
        for methods in [1, 2] {
            let mut program = typed(&family(methods, false));
            let (handle, declaration) = program
                .traits
                .iter()
                .find(|(_, row)| row.name.as_str() == "Service")
                .unwrap();
            let mut declaration = declaration.clone();
            let signature = program
                .trait_machine_signatures(&declaration)
                .first()
                .unwrap()
                .clone();
            program.push_trait_machine_signature(&mut declaration, signature);
            *program.traits.get_mut(handle) = declaration;
            let projection = projected(&program, "Service");
            assert_eq!(projection.declaration.methods.len(), methods);
            assert_eq!(
                projection.validate_correspondence().unwrap_err().message,
                "service row has no unique exact inherited requirement"
            );
        }
    }

    #[test]
    fn ambiguous_inherited_declarations_refuse() {
        let mut program = typed(&family(2, true));
        let parent = program
            .traits()
            .iter()
            .find(|row| row.name.as_str() == "Parent")
            .unwrap()
            .clone();
        program.push_trait_definition(parent);
        assert_eq!(
            projected(&program, "Service")
                .validate_correspondence()
                .unwrap_err()
                .message,
            "service inheritance has ambiguous declarations"
        );
    }
}
