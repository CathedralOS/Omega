#![allow(dead_code, unused_imports)]
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use serde_json::Value as Json;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod checking {
    #[path = "KNOWN_PATH"]
    pub(crate) mod known;
    #[path = "PROVE_PATH"]
    pub(crate) mod prove;
    pub(crate) fn run(
        context: &semantic_vocabulary::PropositionContext,
        values: &dyn known::ProofValues,
        assumptions: &[semantic_vocabulary::Proposition],
        axioms: &[semantic_vocabulary::Proposition],
        value: semantic_vocabulary::ValueId,
    ) -> (
        Option<semantic_vocabulary::IntegerValue>,
        Option<semantic_vocabulary::IntegerValue>,
    ) {
        let mut activity = prove::ProofSearchActivity::default();
        let start = std::time::Instant::now();
        let mut prover = prove::Prover::new(context, assumptions, axioms, values, &mut activity);
        eprintln!("PROVER_READY elapsed={:?}", start.elapsed());
        let result = prover.literal_bounds(value);
        drop(prover);
        eprintln!("ACTIVITY {activity:?}");
        result
    }
}

fn tag(n: &Json) -> &str {
    n[0].as_str().unwrap()
}
fn name(n: &Json) -> &str {
    n[1].as_str().unwrap()
}
fn list(n: &Json) -> &[Json] {
    assert_eq!(tag(n), "list");
    n[1].as_array().unwrap()
}
fn args<'a>(n: &'a Json, expected: &str) -> &'a [Json] {
    assert_eq!(tag(n), "call");
    assert_eq!(name(n), expected);
    n[2].as_array().unwrap()
}
fn field<'a>(n: &'a Json, key: &str) -> &'a Json {
    assert_eq!(tag(n), "struct");
    let mut rows = n[2]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row[0].as_str() == Some(key));
    let row = rows.next().unwrap();
    assert!(rows.next().is_none());
    &row[1]
}
fn atom(n: &Json) -> &str {
    assert_eq!(tag(n), "atom");
    name(n)
}
fn raw(n: &Json) -> u64 {
    atom(n).parse().unwrap()
}
fn value(n: &Json) -> ValueId {
    ValueId::new(raw(&args(n, "ValueId")[0])).unwrap()
}
fn integer(n: &Json) -> IntegerType {
    assert_eq!(name(n), "IntegerType");
    assert_eq!(atom(field(n, "carrier")), "Fixed");
    let sign = match atom(field(n, "sign")) {
        "Unsigned" => IntegerSign::Unsigned,
        "Signed" => IntegerSign::Signed,
        x => panic!("unknown sign {x}"),
    };
    IntegerType::new(
        sign,
        raw(&args(field(n, "width"), "Bits")[0]).try_into().unwrap(),
    )
    .unwrap()
}
fn scalar(n: &Json) -> ScalarType {
    if tag(n) == "atom" {
        assert_eq!(atom(n), "Boolean");
        ScalarType::Boolean
    } else {
        ScalarType::Integer(integer(&args(n, "Integer")[0]))
    }
}
fn int_value(n: &Json) -> IntegerValue {
    assert_eq!(tag(n), "call");
    match name(n) {
        "Unsigned" => IntegerValue::Unsigned(atom(&n[2][0]).parse().unwrap()),
        "Signed" => IntegerValue::Signed(atom(&n[2][0]).parse().unwrap()),
        x => panic!("unknown integer {x}"),
    }
}
fn boolean(n: &Json) -> bool {
    atom(n).parse().unwrap()
}
fn term(n: &Json) -> ScalarTerm {
    if tag(n) == "call" {
        return ScalarTerm::Boolean(boolean(&args(n, "Boolean")[0]));
    }
    let get = |key| field(n, key);
    match name(n) {
        "Value" => ScalarTerm::value(value(get("id")), scalar(get("scalar_type"))),
        "Integer" => {
            ScalarTerm::integer(integer(get("scalar_type")), int_value(get("value"))).unwrap()
        }
        "BooleanEqual" => ScalarTerm::BooleanEqual {
            left: Box::new(term(get("left"))),
            right: Box::new(term(get("right"))),
        },
        "IntegerLessThan" => ScalarTerm::IntegerLessThan {
            scalar_type: integer(get("scalar_type")),
            left: Box::new(term(get("left"))),
            right: Box::new(term(get("right"))),
        },
        "IntegerLessOrEqual" => ScalarTerm::IntegerLessOrEqual {
            scalar_type: integer(get("scalar_type")),
            left: Box::new(term(get("left"))),
            right: Box::new(term(get("right"))),
        },
        "ExactIntegerSubtract" => ScalarTerm::ExactIntegerSubtract {
            scalar_type: integer(get("scalar_type")),
            left: Box::new(term(get("left"))),
            right: Box::new(term(get("right"))),
        },
        "ExactIntegerAdd" => ScalarTerm::ExactIntegerAdd {
            scalar_type: integer(get("scalar_type")),
            left: Box::new(term(get("left"))),
            right: Box::new(term(get("right"))),
        },
        x => panic!("unknown term {x}"),
    }
}
fn prop(n: &Json) -> Proposition {
    if tag(n) == "atom" {
        return match atom(n) {
            "Truth" => Proposition::Truth,
            "Falsehood" => Proposition::Falsehood,
            x => panic!("unknown prop {x}"),
        };
    }
    if tag(n) == "struct" {
        assert_eq!(name(n), "Implication");
        return Proposition::Implication {
            premise: Box::new(prop(field(n, "premise"))),
            conclusion: Box::new(prop(field(n, "conclusion"))),
        };
    }
    let a = n[2].as_array().unwrap();
    match name(n) {
        "Equal" => Proposition::Equal(term(&a[0]), term(&a[1])),
        "LessThan" => Proposition::LessThan(term(&a[0]), term(&a[1])),
        "LessOrEqual" => Proposition::LessOrEqual(term(&a[0]), term(&a[1])),
        "Conjunction" => Proposition::Conjunction(list(&a[0]).iter().map(prop).collect()),
        "Disjunction" => Proposition::Disjunction(list(&a[0]).iter().map(prop).collect()),
        x => panic!("unknown proposition {x}"),
    }
}
fn place(n: &Json) -> StructuralPlaceKind {
    if tag(n) == "atom" {
        assert_eq!(atom(n), "Result");
        return StructuralPlaceKind::Result;
    }
    match name(n) {
        "Parameter" => StructuralPlaceKind::Parameter {
            position: raw(field(n, "position")).try_into().unwrap(),
        },
        "BlockParameter" => StructuralPlaceKind::BlockParameter {
            position: raw(field(n, "position")).try_into().unwrap(),
            block: BlockId::new(raw(&args(field(n, "block"), "BlockId")[0])).unwrap(),
        },
        "OperationResult" => StructuralPlaceKind::OperationResult {
            producer: OperationId::new(raw(&args(field(n, "producer"), "OperationId")[0])).unwrap(),
            structural_type: StructuralTypeId::new(raw(&args(
                field(n, "structural_type"),
                "StructuralTypeId",
            )[0]))
            .unwrap(),
        },
        x => panic!("unknown place {x}"),
    }
}
struct Values {
    rows: Vec<Option<(ValueId, Option<ScalarType>)>>,
    storage: Vec<ValueId>,
    types: arena::IdTable<ValueId, Option<ScalarType>>,
}
impl checking::known::ProofValues for Values {
    fn count(&self) -> usize {
        self.rows.len()
    }
    fn identity(&self, p: usize) -> Option<ValueId> {
        self.rows.get(p).copied().flatten().map(|r| r.0)
    }
    fn scalar_type(&self, v: ValueId) -> Option<ScalarType> {
        self.types.get(v).copied().flatten()
    }
    fn is_storage_content(&self, v: ValueId) -> bool {
        self.storage.binary_search(&v).is_ok()
    }
}
fn replay() -> Result<(), String> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("input directory"));
    let json: Json = serde_json::from_slice(
        &std::fs::read(root.join("inputs.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let context = &json["context"];
    let value_types = field(context, "value_types")[1]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (value(&r[0]), scalar(&r[1])))
        .collect::<Vec<_>>();
    let places = field(context, "structural_places")[1]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                PlaceId::new(raw(&args(&r[0], "PlaceId")[0])).unwrap(),
                place(&r[1]),
            )
        })
        .collect::<Vec<_>>();
    let context = PropositionContext::from_value_types_and_places(value_types, places).unwrap();
    let assumptions = list(&json["assumptions"])
        .iter()
        .map(prop)
        .collect::<Vec<_>>();
    let axioms = list(&json["axioms"]).iter().map(prop).collect::<Vec<_>>();
    let storage = list(&json["storage_values"])
        .iter()
        .map(value)
        .collect::<Vec<_>>();
    let proof_values = list(&json["proof_values"])
        .iter()
        .map(|n| {
            if tag(n) == "atom" {
                assert_eq!(atom(n), "None");
                return None;
            }
            let pair = list(&args(n, "Some")[0]);
            let ty = if tag(&pair[1]) == "atom" {
                assert_eq!(atom(&pair[1]), "None");
                None
            } else {
                Some(scalar(&args(&pair[1], "Some")[0]))
            };
            Some((value(&pair[0]), ty))
        })
        .collect::<Vec<_>>();
    assert!(list(&json["active_goals"]).is_empty());
    assert!(list(&json["active_cuts"]).is_empty());
    assert_eq!(atom(&json["case_branches"]), "64");
    let actual = format!(
        "sequence=4662\nmachine=MachineId(2)\nrange_value=ValueId(29)\npoint=Edge(EdgeId(51))\ncase_branches=64\nactive_goals=[]\nactive_cuts=[]\ncontext={context:#?}\nstorage_values={storage:?}\nassumptions={assumptions:#?}\naxioms={axioms:#?}\nproof_values={proof_values:#?}\n"
    );
    let expected = std::fs::read_to_string(root.join("expected.txt")).map_err(|e| e.to_string())?;
    if actual != expected {
        return Err("reconstructed query differs from captured Debug inputs".into());
    }
    drop(actual);
    drop(expected);
    drop(json);
    let types = arena::IdTable::from_rows_with_gaps(
        proof_values.iter().flatten().copied(),
        proof_values.len(),
    )
    .unwrap();
    let values = Values {
        rows: proof_values,
        storage,
        types,
    };
    eprintln!(
        "INPUT_MATCH values={} assumptions={} axioms={}",
        values.rows.len(),
        assumptions.len(),
        axioms.len()
    );
    let start = Instant::now();
    let result = checking::run(
        &context,
        &values,
        &assumptions,
        &axioms,
        ValueId::new(29).unwrap(),
    );
    eprintln!(
        "RANGE_RESULT elapsed={:?} bounds={result:?}",
        start.elapsed()
    );
    Ok(())
}
fn main() {
    let result = execution_admission::ExecutionBudget::with_workers(1)
        .map_err(|e| e.to_string())
        .and_then(|budget| budget.run(replay));
    if let Err(e) = result {
        eprintln!("REPLAY_ERROR {e}");
        std::process::exit(1);
    }
}
