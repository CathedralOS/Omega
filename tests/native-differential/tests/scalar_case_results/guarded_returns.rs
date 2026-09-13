//! Ordered source exits keep their selected result and the original borrowed sum.

use super::{NativeTarget, membership, produce_source, publish};

#[test]
fn ordered_case_constructor_composes_with_borrowed_getter() {
    let source = format!(
        "{ALIGNMENT}
        machine Alignment::from(size: i32) -> Alignment {{
            transition size {{
                1 -> (Alignment::Byte)
                2 -> (Alignment::Word)
                4 -> (Alignment::DoubleWord)
                8 -> (Alignment::QuadWord)
                _ -> (Alignment::Byte)
            }}
        }}
        machine evaluate(size: i32) -> u64 {{
            let alignment: Alignment = Alignment::from(size);
            alignment.get_stride()
        }}"
    );
    let artifact = produce_source("evaluate", &source);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        "#include <stdint.h>\n
        extern uint64_t omega_entry(int32_t);
        int main(void) {
            const int32_t sizes[] = {INT32_MIN,-1,0,1,2,3,4,7,8,9,INT32_MAX};
            for (unsigned position=0; position<11; ++position) {
                int32_t size=sizes[position];
                uint64_t expected=(size==2 || size==4 || size==8) ? (uint64_t)size : 1;
                if (omega_entry(size)!=expected) return 1;
            }
            return 0;
        }",
    );
}

#[test]
fn ordered_case_returns_execute_only_selected_payload_effects() {
    let artifact = produce_source(
        "choose",
        "
        data Choice [copy] {
            case First(value: u64);
            case Second(value: u64);
            case Last(value: u64);
        }
        machine replace(value: &mut u64, next: u64) -> u64 {
            let before: u64 = value;
            value = next;
            before
        }
        machine choose(selector: i32, value: &mut u64) -> Choice {
            transition {
                selector < 2 -> (Choice::First { value: replace(&mut value, 11) ^ replace(&mut value, 12) })
                selector < 4 -> (Choice::Second { value: replace(&mut value, 21) ^ replace(&mut value, 22) })
                _ -> (Choice::Last { value: replace(&mut value, 31) ^ replace(&mut value, 32) })
            }
        }",
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(&artifact, "#include <stdint.h>\n
        struct choice { uint32_t tag; uint32_t padding; uint64_t value; };
        extern struct choice omega_entry(int32_t, uint64_t *);
        int main(void) {
            const int32_t selectors[] = {INT32_MIN,0,1,2,3,4,INT32_MAX};
            for (unsigned position=0; position<7; ++position) {
                int32_t selector=selectors[position];
                uint32_t expected=selector<2 ? 0 : selector<4 ? 1 : 2;
                uint64_t first=11+10*expected;
                uint64_t initial=UINT64_C(0x8123456789abcdef), value=initial;
                struct choice result=omega_entry(selector,&value);
                if (result.tag!=expected || result.value!=(initial^first) || value!=first+1) return 1;
            }
            return 0;
        }");
}

const ALIGNMENT: &str = "
data Alignment [copy] {
    case Byte;
    case Word;
    case DoubleWord;
    case QuadWord;
}
machine Alignment::get_stride(&self) -> u64 [1..=8] {
    transition self {
        Alignment::Byte -> (1)
        Alignment::Word -> (2)
        Alignment::DoubleWord -> (4)
        Alignment::QuadWord -> (8)
    }
}";

#[test]
fn borrowed_sum_guarded_returns_preserve_declared_result_range() {
    let artifact = produce_source("Alignment::get_stride", ALIGNMENT);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let getter = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        getter.contract.ensures.len(),
        1,
        "the declared range remains a normal-result obligation"
    );
    let observations = getter
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralCaseMembership { source, case } => {
                Some((source, case))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(observations.len(), 4, "every source guard is retained");
    assert!(
        observations
            .iter()
            .all(|(source, _)| *source == getter.structural_parameters[0].place)
    );
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(const uint32_t *);\nint main(void) { const uint64_t expected[] = {1,2,4,8}; for (uint32_t tag=0; tag<4; ++tag) { uint32_t retained=tag; for (unsigned repeat=0; repeat<3; ++repeat) { if (omega_entry(&retained)!=expected[tag] || retained!=tag) return 1; } } return 0; }",
    );
}

#[test]
fn ordered_scalar_returns_preserve_first_selected_guard_and_fallback() {
    let artifact = produce_source(
        "select",
        "
        machine select(value: u64) -> u64 [1..=8] {
            transition {
                value < 2 -> (1)
                value < 4 -> (2)
                value < 8 -> (4)
                _ -> (8)
            }
        }",
    );
    membership::execute(
        &artifact,
        "#include <stdint.h>\nextern uint64_t omega_entry(uint64_t);\nint main(void) { const uint64_t inputs[] = {0,1,2,3,4,7,8,UINT64_MAX}; const uint64_t expected[] = {1,1,2,2,4,4,8,8}; for (unsigned ordinal=0; ordinal<8; ++ordinal) if (omega_entry(inputs[ordinal])!=expected[ordinal]) return 1; return 0; }",
    );
}
