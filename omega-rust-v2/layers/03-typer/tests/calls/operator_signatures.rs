// Three type positions have five distinct patterns of binder reuse. Two declarations
// are duplicates exactly when they use the same pattern, even through an alias.
#[test]
fn generated_named_and_operator_signatures_agree_on_binder_reuse() {
    let patterns = [
        ("A", ["A", "A", "A"], "X", ["X", "X", "X"]),
        ("A,B", ["A", "A", "B"], "Y,X", ["X", "X", "Y"]),
        ("A,B", ["A", "B", "A"], "Y,X", ["X", "Y", "X"]),
        ("A,B", ["A", "B", "B"], "Y,X", ["X", "Y", "Y"]),
        ("A,B,C", ["A", "B", "C"], "Z,Y,X", ["X", "Y", "Z"]),
    ];
    let alias = "data Id<T> = T;";
    let mut distinct = vec![("alias.omg".to_owned(), alias.to_owned())];
    for (i, (left_binders, [a, b, c], _, _)) in patterns.iter().enumerate() {
        for (j, (_, _, right_binders, [x, y, z])) in patterns.iter().enumerate() {
            for operator in [false, true] {
                // A fresh nominal pair keeps independent operator families apart in the batch.
                let family = format!("pair_{i}_{j}_{}", if operator { "token" } else { "named" });
                let first = format!("first_{family}");
                let second = if operator { format!("second_{family}") } else { first.clone() };
                let token = if operator { "+ " } else { "" };
                let source = format!(
                    "data {family}<L,R>{{}} machine {token}{first}<{left_binders}>(p:{family}<{a},{b}>,v:{c}){{}} machine {token}{second}<{right_binders}>(p:Id<{family}<{x},{y}>>,v:{z}){{}}"
                );
                let file = format!("{family}.omg");
                if i == j {
                    let error = check(&[("alias.omg", alias), (&file, &source)])
                        .err()
                        .unwrap_or_else(|| panic!("accepted duplicate in {file}: {source}"));
                    let expected = if operator {
                        "duplicate normalized operator binding".to_owned()
                    } else {
                        format!("duplicate typed operand signature for {first}")
                    };
                    assert_eq!(error.file, file, "{source}");
                    assert_eq!(error.message, expected, "{source}");
                } else {
                    distinct.push((file, source));
                }
            }
        }
    }
    let files = distinct
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect::<Vec<_>>();
    let stage = check(&files).unwrap_or_else(|e| {
        let source = files.iter().find(|(p, _)| *p == e.file).unwrap();
        panic!("{e:?}\n{}", source.1);
    });
    assert_eq!(stage.signatures.len(), 80); // Both declarations survived in each of the 40 distinct pairs.
}
