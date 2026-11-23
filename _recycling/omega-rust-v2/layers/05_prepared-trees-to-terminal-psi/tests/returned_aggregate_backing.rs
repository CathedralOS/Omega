#[path = "support/source.rs"]
mod source;

use omega_psi::{BlockTerminator, MachineBody, Parameter, SuspensionSite};

#[test]
fn completed_aggregate_call_retains_the_exact_borrowed_home_at_a_later_wait() {
    let program = |home: &str, relay| {
        format!(
            "data Box<'r> {{ r:@'r u8; }}
         data Home [copy] {{ v:u8; }}
         machine pick<'r>(k:Box<'r>)->Box<'r> suspends {{ k }}
         machine wait() suspends {{}}
         machine read(left:Home,right:Home)->u8 suspends {{
             let seed = Box{{r:@{home}.v}};
             let item = {relay};
             suspend wait();
             $item.r
         }}"
        )
    };
    for (slot, home) in ["left", "right"].into_iter().enumerate() {
        let input = program(home, "suspend pick(seed)");
        let mut package = source::lower(&[("backing.omg", &input)]).unwrap();
        let observe = |p: &omega_psi::LoweredPackage| psi_handoff_consumer::returned_aggregate_backing(p, "read", "pick", "wait");
        spec_corpus::observe_handoff!(psi "psi-output/returned-aggregate-backing", assert_eq!(observe(&package), Some(slot)));

        let body = &mut package
            .machines
            .iter_mut()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let Parameter::Structural(other) = body.parameters[1 - slot] else {
            panic!("same-typed sibling home")
        };
        body.suspension_sites[1].retained_homes[0].place = other;
        assert_eq!(observe(&package), None);

        let direct = program(home, "seed");
        let package = source::lower(&[("direct.omg", &direct)]).unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let [wait] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let homes = psi_handoff_consumer::parked_copyable_homes(&package, body, wait).unwrap();
        let [backing] = homes.as_slice() else { panic!("one retained backing") };
        assert_eq!(
            (backing.formal, backing.needed_by_callee, backing.needed_after_completion),
            (Some(slot), false, true)
        );
    }
}

#[test]
fn completed_aggregate_call_follows_formal_fields_and_all_possible_returns() {
    let cases: [(&str, &[usize]); 4] = [
        ("seed.first", &[0]),
        ("seed.second", &[1]),
        ("transition flag {true -> seed.first, false -> seed.second,}", &[0, 1]),
        (
            "transition flag {true -> seed.first, false -> pick::<'r>(true,Seeds{first:seed.second,second:seed.first}),}",
            &[0, 1],
        ),
    ];
    for (relay, expected) in cases {
        // Both fields share one formal and region. Exact return paths can
        // narrow that frontier, and recursion can add a path via a new actual.
        let program = format!(
            "data Box<'r> [copy] {{ r:@'r u8; }}
             data Seeds<'r> [copy] {{ first:Box<'r>; second:Box<'r>; }}
             data Home [copy] {{ v:u8; }}
             machine pick<'r>(flag:bool,seed:Seeds<'r>)->Box<'r> {{ {relay} }}
             machine wait() suspends {{}}
             machine read(left:Home,right:Home,flag:bool)->u8 suspends {{
                 let seed = Seeds{{first:Box{{r:@left.v}},second:Box{{r:@right.v}}}};
                 let item = pick(flag,seed);
                 suspend wait();
                 $item.r
             }}"
        );
        let package = source::lower(&[("paths.omg", &program)]).unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let [wait] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let (homes, results) = resumed_storage(body, wait);
        assert_eq!(homes, expected, "{relay}");
        assert_eq!(results, 1, "the returned copyable Box also has a home");
    }
}

#[test]
fn completed_aggregate_call_reads_its_actual_before_the_actual_is_reassigned() {
    let program = "data Box<'r> [copy] { r:@'r u8; }
        data Home [copy] { v:u8; }
        machine pick<'r>(k:Box<'r>)->Box<'r> suspends { k }
        machine wait() suspends {}
        machine read(left:Home,right:Home)->u8 suspends {
            let mutable seed = Box{r:@left.v};
            let item = suspend pick(seed);
            seed.r = @right.v;
            suspend wait();
            $item.r
        }";
    let package = source::lower(&[("store.omg", program)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "read")
        .unwrap()
        .body;
    let [_, wait] = body.suspension_sites.as_slice() else {
        panic!("two crossings")
    };
    assert_eq!(resumed_storage(body, wait), (vec![0], 1));
}

#[test]
fn a_returned_cases_inactive_payload_does_not_add_a_backing_home() {
    for (case, field, slot) in [("A", "left", 0), ("B", "right", 1)] {
        let program = format!(
            "data Choice<'r> {{ case A(r:@'r u8); case B(r:@'r u8); }}
             data Pair<'r> [copy] {{ left:@'r u8; right:@'r u8; }}
             data Home [copy] {{ v:u8; }}
             machine pick<'r>(seed:Pair<'r>)->Choice<'r> {{ Choice::{case}{{r:seed.{field}}} }}
             machine consume<'r>(item:Choice<'r>) {{}}
             machine wait() suspends {{}}
             machine read(left:Home,right:Home) suspends {{
                 let seed = Pair{{left:@left.v,right:@right.v}};
                 let item = pick(seed);
                 suspend wait();
                 consume(item);
             }}"
        );
        let package = source::lower(&[("case.omg", &program)]).unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let [wait] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        assert_eq!(resumed_storage(body, wait), (vec![slot], 0));
    }
}

#[test]
fn terminal_completion_cleanup_preserves_the_returned_backing() {
    let program = "data Box<'r> [copy] { r:@'r u8; }
        data Home [copy] { v:u8; }
        data Token { v:u8; }
        data Lease dropped by release { mark:u8; }
        machine release(x:Lease) { let Lease{mark as _}=x; }
        data Choice { case A(left:Token,right:Lease); case B(left:Token,right:Token); }
        machine pick<'r>(seed:Box<'r>)->Box<'r> { seed }
        machine relay<'r>(keeper:Choice,seed:Box<'r>)->Box<'r> {
            transition { _ -> pick::<'r>(seed), }
        }
        machine wait() suspends {}
        machine read(left:Home,right:Home,keeper:Choice)->u8 suspends {
            let seed = Box{r:@left.v};
            let item = relay(keeper,seed);
            suspend wait();
            $item.r
        }";
    let package = source::lower(&[("completion.omg", program)]).unwrap();
    let relay = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap()
        .body;
    assert!(
        relay
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, BlockTerminator::TerminalCall { completion: Some(_), .. }))
    );
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "read")
        .unwrap()
        .body;
    let [wait] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    assert_eq!(resumed_storage(body, wait), (vec![0], 1));
}

fn resumed_storage(
    body: &MachineBody,
    site: &SuspensionSite,
) -> (Vec<usize>, usize) {
    let mut inputs = Vec::new();
    let mut results = 0;
    for home in &site.retained_homes {
        assert!(!home.needed_by_callee && home.needed_after_completion);
        if let Some(slot) = body
            .parameters
            .iter()
            .position(|formal| *formal == Parameter::Structural(home.place))
        {
            inputs.push(slot);
        } else {
            results += 1;
        }
    }
    (inputs, results)
}
