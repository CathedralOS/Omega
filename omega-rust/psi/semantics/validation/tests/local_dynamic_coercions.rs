use validation::validate_program;

fn validate(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    validate_program(&crate::front_end::typed_program_result(source)?)
}

#[test]
fn resolves_dynamic_member_call_through_lifetime_applied_local_record() {
    validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Holder<'item> {
            handler: &'item dyn Shape;
        }

        data Reader {
            item: Item;
        }

        machine Reader::read<'item>(&self) {
            let erased: &'item dyn Shape = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: i32 = holder.handler.code();
        }
        "#,
    )
    .expect("a lifetime-applied record member must retain its dynamic trait type");
}

#[test]
fn rejects_boundary_trait_as_local_dynamic_descriptor() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: i32) reaches Console;
        }

        data Reader {}

        machine Reader::read(console: &dyn Console) {}
        "#,
    )
    .expect_err("a replaceable component boundary must not become a local dynamic table");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("local dynamic descriptors cannot cross a replaceable component boundary")
    }));
}

#[test]
fn admits_direct_same_trait_dynamic_parameter_forwarding() {
    validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        machine relay(value: &dyn Shape) -> i32 {
            let result: i32 = finish(value);
            transition { _ -> result }
        }

        machine finish(value: &dyn Shape) -> i32 {
            transition { _ -> value.code() }
        }
        "#,
    )
    .expect("an exact bare dynamic parameter may be passed onward unchanged");
}

#[test]
fn rejects_dynamic_parameter_forwarding_to_a_different_trait() {
    let diagnostics = validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        trait Other {
            machine code(&self) -> i32;
        }

        machine relay(value: &dyn Shape) -> i32 {
            let result: i32 = finish(value);
            transition { _ -> result }
        }

        machine finish(value: &dyn Other) -> i32 {
            transition { _ -> value.code() }
        }
        "#,
    )
    .expect_err("a parameter cannot change dynamic interfaces while forwarding");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "without one earlier exact compatible local conformance selection or dynamic parameter",
        )
    }));
}
