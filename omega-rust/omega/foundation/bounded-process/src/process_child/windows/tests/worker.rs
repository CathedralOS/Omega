use std::hint::black_box;
use std::time::{Duration, Instant};

use super::support::{WORKER_MODE, WORKER_VALUE, worker_command};

fn parse_worker_values(value: &str, expected: usize) -> Vec<usize> {
    let values = value
        .split(',')
        .map(|field| field.parse::<usize>().expect("parse worker value"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), expected, "unexpected worker value shape");
    values
}

fn touch_memory(bytes: usize, hold_millis: usize) {
    let mut memory = Vec::new();
    memory
        .try_reserve_exact(bytes)
        .expect("worker memory reservation should remain below its expected limit");
    memory.resize(bytes, 0x5a_u8);
    black_box(&memory);
    std::thread::sleep(Duration::from_millis(
        u64::try_from(hold_millis).expect("hold duration fits u64"),
    ));
}

fn wait_for_all(children: &mut [std::process::Child]) -> bool {
    let mut every_child_succeeded = true;
    for child in children {
        every_child_succeeded &= child.wait().is_ok_and(|status| status.success());
    }
    every_child_succeeded
}

#[test]
fn job_limit_worker() {
    let Ok(mode) = std::env::var(WORKER_MODE) else {
        return;
    };
    let value = std::env::var(WORKER_VALUE).expect("limited worker value");
    match mode.as_str() {
        "hold" => {
            let millis = value.parse::<u64>().expect("parse hold duration");
            std::thread::sleep(Duration::from_millis(millis));
        }
        "fanout" => {
            let values = parse_worker_values(&value, 2);
            let expected_success = values[1] != 0;
            let mut children = Vec::new();
            let mut every_spawn_succeeded = true;
            for _ in 0..values[0] {
                match worker_command("hold", "750").spawn() {
                    Ok(child) => children.push(child),
                    Err(_) => every_spawn_succeeded = false,
                }
            }
            let every_child_succeeded = wait_for_all(&mut children);
            assert_eq!(
                every_spawn_succeeded && every_child_succeeded,
                expected_success,
                "active-process worker observed an unexpected fanout result"
            );
        }
        "touch" => {
            let values = parse_worker_values(&value, 2);
            touch_memory(values[0], values[1]);
        }
        "aggregate-memory" => {
            let values = parse_worker_values(&value, 3);
            let expected_success = values[2] != 0;
            let mut children = (0..values[0])
                .filter_map(|_| {
                    worker_command("touch", &format!("{},750", values[1]))
                        .spawn()
                        .ok()
                })
                .collect::<Vec<_>>();
            let every_child_spawned = children.len() == values[0];
            let child_statuses_succeeded = wait_for_all(&mut children);
            let every_child_succeeded = every_child_spawned && child_statuses_succeeded;
            assert_eq!(
                every_child_succeeded, expected_success,
                "aggregate-memory worker observed an unexpected child result"
            );
        }
        "spin" => {
            let deadline = Instant::now()
                + Duration::from_millis(value.parse::<u64>().expect("parse spin duration"));
            let mut value = 0_u64;
            while Instant::now() < deadline {
                value = black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
            }
            black_box(value);
        }
        "spin-until-terminated" => {
            let mut value = 0_u64;
            loop {
                value = black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
            }
        }
        _ => panic!("unknown Windows Job test worker mode `{mode}`"),
    }
}
