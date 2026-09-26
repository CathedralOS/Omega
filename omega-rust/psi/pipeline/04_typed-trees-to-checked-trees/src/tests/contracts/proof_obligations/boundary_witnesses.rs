use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::{parse_typed_trees, parse_typed_trees_with_service};

#[test]
fn boundary_witness_survives_disjoint_projected_alias_frame() {
    let source = r#"
        pub boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            size: u32;
            other: u32;
        }

        data Main {
            fw: Binding<Firmware>;
            cell: Cell;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cell.size);
            self.touch_other_through_projection();
            self.small = self.cell.size;
        }

        machine Main::touch_other_through_projection(&mut self) {
            let cell_alias: &mut Cell = &mut self.cell;
            let other: &mut u32 = &mut cell_alias.other;
            other = 9;
        }
    "#;

    lower_typed_trees(
        parse_typed_trees_with_service(source),
        &CheckingRequest::settled(),
    )
    .expect("an exact projected-alias frame should preserve a witness on a disjoint sibling");
}

#[test]
fn boundary_witness_dies_under_direct_member_after_index_frame() {
    let source = r#"
        pub boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            value: u32;
        }

        data Main {
            fw: Binding<Firmware>;
            cells: [Cell; 2];
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cells[0].value);
            self.touch_direct_member_after_index();
            self.small = self.cells[0].value;
        }

        machine Main::touch_direct_member_after_index(&mut self) {
            let value: &mut u32 = &mut self.cells[1].value;
            value = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(
        parse_typed_trees_with_service(source),
        &CheckingRequest::settled(),
    )
    .expect_err(
        "the direct member-after-index frame must invalidate an overlapping collection witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn incoming_guard_conjuncts_jointly_bound_operands_after_disjoint_write() {
    let source = r#"
        data Main {
            scratch: i32;
            position: i32 [0..=8];
            direction: i32;
            result: i32 [0..=9];
        }

        machine Main::main(&mut self) {
            transition self.scratch == 0 && self.direction >= 0 && self.direction <= 1 {
                true -> update()
                false -> done()
            }
            state update(&mut self) {
                self.scratch = 1;
                self.result = self.position + self.direction;
            }
            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("surviving lower and upper conjuncts must jointly bound the operand");
}

#[test]
fn bounded_byte_domain_projection_proves_the_requested_predicate_independently() {
    let source = r#"
        pub boundary trait Sink {
            machine write(text: [u8] in Text);
        }

        pub domain [u8]::Text
        requires
            no_nul(self);

        pub domain [u8; 4]::Text
        requires
            valid_utf8(self);

        data Main {
            sink: Binding<Sink>;
            text: [u8; 4] in Text;
        }

        machine Main::main(&mut self) reaches Sink {
            self.text = "G\x00te";
            self.sink.write(self.text);
        }
    "#;

    lower_typed_trees(
        parse_typed_trees_with_service(&source.replace(r"G\x00te", "Gate")),
        &CheckingRequest::settled(),
    )
    .expect("known bytes may independently establish both domain predicates");
    let Err(diagnostics) = lower_typed_trees(
        parse_typed_trees_with_service(source),
        &CheckingRequest::settled(),
    ) else {
        panic!("a carrier projection must not conflate different domain theories");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call write")
                && diagnostic.message.contains("self.text in [u8]::Text")
        }),
        "expected the mismatched slice-domain requirement to remain unproven, got {diagnostics:#?}"
    );
}
