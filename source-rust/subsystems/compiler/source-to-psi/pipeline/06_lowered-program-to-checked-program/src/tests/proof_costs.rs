//! What the kernel spends judging source laws over native inductives: the
//! conversion steps of each law's certificate, on the declarations and laws
//! the prototype measurements used
//! (`wiki/drafts/designs/recursive_data_encoding_experiments.md`), here
//! written in source and carried through stage 05 and stage 06.

use super::seq_laws::{judged, signature, unproved};
use super::*;

/// Plain data `name` with one case per arity, each holding that many fields
/// of the data, the machine that copies it field by field, and the law that
/// the copy is the value, by induction, citing itself at every field.
fn copied(name: &str, arities: &[usize]) -> String {
    let fields = |arity: usize| -> Vec<String> { (0..arity).map(|f| format!("f{f}")).collect() };
    let mut source = format!("data {name} {{\n");
    for (case, &arity) in arities.iter().enumerate() {
        if arity == 0 {
            source.push_str(&format!("    case C{case};\n"));
        } else {
            let declared: Vec<_> = fields(arity)
                .iter()
                .map(|field| format!("{field}: {name}"))
                .collect();
            source.push_str(&format!("    case C{case}({});\n", declared.join(", ")));
        }
    }
    source.push_str("}\n\n");
    let copy = format!("copy_{}", name.to_lowercase());
    source.push_str(&format!(
        "machine {copy}(x: {name}) -> {name}\nterminates by x\n{{\n    transition x {{\n"
    ));
    let mut arms = Vec::new();
    for (case, &arity) in arities.iter().enumerate() {
        if arity == 0 {
            arms.push(format!("        {name}::C{case} -> {name}::C{case}"));
        } else {
            let names = fields(arity);
            let built: Vec<_> = names
                .iter()
                .map(|field| format!("{field}: {copy}({field})"))
                .collect();
            arms.push(format!(
                "        {name}::C{case} {{ {} }} -> {name}::C{case} {{ {} }}",
                names.join(", "),
                built.join(", ")
            ));
        }
    }
    source.push_str(&arms.join(",\n"));
    source.push_str("\n    }\n}\n\n");
    let law = format!("{copy}_id");
    source.push_str(&format!(
        "machine {law}(x: {name})\nterminates by x\nensures\n    |proof|::Identity::<{name}>({copy}(x), x)\n{{\n    transition x {{\n"
    ));
    let mut arms = Vec::new();
    let mut steps = Vec::new();
    for (case, &arity) in arities.iter().enumerate() {
        let names = fields(arity);
        if arity == 0 {
            arms.push(format!("        {name}::C{case} -> leaf()"));
        } else {
            arms.push(format!(
                "        {name}::C{case} {{ {} }} -> step{arity}({})",
                names.join(", "),
                names.join(", ")
            ));
            if !steps.contains(&arity) {
                steps.push(arity);
            }
        }
    }
    source.push_str(&arms.join(",\n"));
    source.push_str("\n    }\n    machine leaf() {\n    }\n");
    for arity in steps {
        let names = fields(arity);
        let formals: Vec<_> = names
            .iter()
            .map(|field| format!("{field}: {name}"))
            .collect();
        source.push_str(&format!(
            "    machine step{arity}({}) {{\n",
            formals.join(", ")
        ));
        for field in &names {
            source.push_str(&format!("        {law}({field});\n"));
        }
        source.push_str("    }\n");
    }
    source.push_str("}\n\n");
    source
}

/// A mutual block whose two members hold each other, its copies, and the two
/// laws proved together by the block's induction.
const MUTUAL: &str = "
data Expr {
    case Num;
    case Add(left: Expr, right: Expr);
    case Block(body: Stmt);
}

data Stmt {
    case Skip;
    case Run(expr: Expr);
    case Seq(first: Stmt, then: Stmt);
}

machine copy_expr(e: Expr) -> Expr
terminates by e
{
    transition e {
        Expr::Num -> Expr::Num,
        Expr::Add { left, right } -> Expr::Add { left: copy_expr(left), right: copy_expr(right) },
        Expr::Block { body } -> Expr::Block { body: copy_stmt(body) }
    }
}

machine copy_stmt(s: Stmt) -> Stmt
terminates by s
{
    transition s {
        Stmt::Skip -> Stmt::Skip,
        Stmt::Run { expr } -> Stmt::Run { expr: copy_expr(expr) },
        Stmt::Seq { first, then } -> Stmt::Seq { first: copy_stmt(first), then: copy_stmt(then) }
    }
}

machine copy_expr_id(e: Expr)
terminates by e
ensures
    |proof|::Identity::<Expr>(copy_expr(e), e)
{
    transition e {
        Expr::Num -> num(),
        Expr::Add { left, right } -> add(left, right),
        Expr::Block { body } -> block(body)
    }
    machine num() {
    }
    machine add(left: Expr, right: Expr) {
        copy_expr_id(left);
        copy_expr_id(right);
    }
    machine block(body: Stmt) {
        copy_stmt_id(body);
    }
}

machine copy_stmt_id(s: Stmt)
terminates by s
ensures
    |proof|::Identity::<Stmt>(copy_stmt(s), s)
{
    transition s {
        Stmt::Skip -> skip(),
        Stmt::Run { expr } -> run(expr),
        Stmt::Seq { first, then } -> seq(first, then)
    }
    machine skip() {
    }
    machine run(expr: Expr) {
        copy_expr_id(expr);
    }
    machine seq(first: Stmt, then: Stmt) {
        copy_stmt_id(first);
        copy_stmt_id(then);
    }
}
";

const SHAPES: [(&str, &[usize]); 3] = [
    ("Four", &[0, 0, 2, 2]),
    ("Eight", &[0, 0, 1, 1, 2, 2, 2, 3]),
    ("Twelve", &[0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3]),
];

fn program() -> String {
    let mut source = super::nat_semiring::library_declarations();
    for (name, arities) in SHAPES {
        source.push_str(&copied(name, arities));
    }
    source.push_str(MUTUAL);
    source
}

/// The laws measured, with the most conversion steps each may take: about
/// twice what each takes now, so a change that makes one much dearer fails.
const MEASURED: [(&str, u32); 8] = [
    ("add_zero_right", 80),
    ("add_assoc", 170),
    ("add_comm", 110),
    ("copy_four_id", 210),
    ("copy_eight_id", 420),
    ("copy_twelve_id", 620),
    ("copy_expr_id", 630),
    ("copy_stmt_id", 630),
];

#[test]
fn laws_over_native_inductives_stay_within_their_measured_costs() {
    let verdicts = judged(&program());
    let judgment = signature(&verdicts);
    let mut over = Vec::new();
    for (machine, ceiling) in MEASURED {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
        if law.conversion_steps > ceiling {
            over.push((machine, law.conversion_steps, ceiling));
        }
    }
    assert!(over.is_empty(), "laws over their measured cost: {over:?}");
}

#[test]
fn a_copy_that_swaps_two_cases_is_refused() {
    let source = program().replace(
        "Four::C2 { f0, f1 } -> Four::C2 {",
        "Four::C2 { f0, f1 } -> Four::C3 {",
    );
    assert_ne!(source, program(), "the copy is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "copy_four_id", "{law:?}");
}
