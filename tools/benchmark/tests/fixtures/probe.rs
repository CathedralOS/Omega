//! A known-work child and adversarial process fixture for the benchmark instrument.

use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("--build-profile") => {
            let level = if std::env::var_os("PROBE_UNOPTIMIZED").is_some() {
                "0"
            } else {
                "1"
            };
            println!("{{\"schema\":\"omega-compiler-build-profile-v1\",\"opt_level\":\"{level}\"}}")
        }
        Some("--version") => println!("benchmark-probe fake compiler; not an Omega producer"),
        Some("plain") | None => println!("native ok"),
        Some("wrong-exit") => std::process::exit(7),
        Some("stall") => std::thread::sleep(Duration::from_secs(10)),
        Some("flood") => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 3 * 1024 * 1024]);
        }
        Some("mutate") => {
            std::fs::write(&arguments[1], "changed").unwrap();
            println!("native ok");
        }
        Some("delete") => {
            std::fs::remove_file(&arguments[1]).unwrap();
            println!("native ok");
        }
        Some("timed") => {
            let work_units: u64 = arguments[1].parse().unwrap();
            let mut accumulator: u64 = arguments[2].parse().unwrap();
            let started = Instant::now();
            for value in 0..black_box(work_units) {
                accumulator = black_box(accumulator.wrapping_add(value.wrapping_mul(value)));
            }
            let elapsed = started.elapsed().as_nanos();
            println!(
                "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{work_units},\"checksum\":\"{accumulator}\"}}"
            );
        }
        _ => {
            if std::env::var_os("PROBE_COMPILE_FAIL").is_some() {
                eprintln!("controlled compile failure");
                std::process::exit(4);
            }
            let position = arguments
                .iter()
                .position(|argument| argument == "--build-dir")
                .unwrap();
            let artifact = std::path::Path::new(&arguments[position + 1]).join(if cfg!(windows) {
                "native.exe"
            } else {
                "native"
            });
            std::fs::copy(std::env::current_exe().unwrap(), &artifact).unwrap();
            if std::env::var_os("PROBE_PUBLISH_OUTSIDE").is_some() {
                println!(
                    "published native output to {}",
                    std::env::current_exe().unwrap().display()
                );
            } else {
                println!("published native output to {}", artifact.display());
            }
            eprintln!("  1.25 ms  Stage fixture");
        }
    }
}
