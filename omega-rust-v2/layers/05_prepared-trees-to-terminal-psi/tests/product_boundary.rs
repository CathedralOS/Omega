//! Guard public products against direct and transitive dependencies on compiler implementations.
use std::process::Command;

fn check_dependencies(
    root: &str,
    allowed: &[&str],
) {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "--all-features",
            "--target",
            "all",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
            "--package",
            root,
        ])
        .output()
        .expect("read the resolved dependency graph");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let graph = String::from_utf8(output.stdout).unwrap();
    // Cargo displays local path packages with their checkout location.
    for package in graph
        .lines()
        .filter(|row| row.trim_end_matches(" (*)").contains(" ("))
    {
        let name = package.split_whitespace().next().unwrap();
        assert!(name == root || allowed.contains(&name), "{root} links unexpected local dependency {package}");
    }
}

#[test]
fn the_downstream_client_builds_with_only_the_psi_product_and_shared_contracts() {
    check_dependencies("omega-program-contracts", &[]);
    check_dependencies("omega-target-contract", &["omega-program-contracts"]);
    check_dependencies("omega-psi", &["omega-program-contracts", "omega-target-contract"]);
    check_dependencies("psi-handoff-consumer", &["omega-psi", "omega-program-contracts", "omega-target-contract"]);
}

#[test]
fn source_products_depend_only_on_the_contracts_and_products_they_carry() {
    let mut earlier = vec!["omega-program-contracts"];
    for product in [
        "omega-tokens",
        "omega-syntax",
        "omega-resolved",
        "omega-typed",
        "omega-prepared",
    ] {
        if product == "omega-typed" {
            earlier.push("omega-target-contract");
        }
        check_dependencies(product, &earlier);
        earlier.push(product);
        match product {
            "omega-tokens" => check_dependencies("omega-lexer", &earlier),
            "omega-syntax" => check_dependencies("omega-parser", &earlier),
            "omega-resolved" => check_dependencies("omega-resolver", &earlier),
            _ => {}
        }
    }
    check_dependencies("tree-handoff-consumer", &earlier);
    earlier.push("omega-psi");
    check_dependencies("omega-lowering", &earlier);
}
