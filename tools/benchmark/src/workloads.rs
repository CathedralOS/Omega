/*
   Repeatable runtime workloads for calibrating and comparing native producers.
   The command supplies the amount of work and seed at runtime. Setup precedes
   the timer; the checksum consumes every computation after it. black_box keeps
   observable data dependencies while normal optimizations remain enabled.
   This executable is Rust reference material, not generated Omega evidence.
   The same timed-work protocol is usable by any native producer.
*/
use std::hint::black_box;
use std::time::Instant;

#[cfg(feature = "allocation-observation")]
#[global_allocator]
static ALLOCATOR: allocation_observation::ObservedAllocator<std::alloc::System> =
    allocation_observation::ObservedAllocator::new(std::alloc::System);

trait Transform {
    fn apply(&self, value: u64, position: u64) -> u64;
}
struct Add(u64);
struct Xor(u64);
impl Transform for Add {
    #[inline(never)]
    fn apply(&self, value: u64, position: u64) -> u64 {
        value.wrapping_add(position).wrapping_add(self.0)
    }
}
impl Transform for Xor {
    #[inline(never)]
    fn apply(&self, value: u64, position: u64) -> u64 {
        value ^ position ^ self.0
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 3 {
        return Err(
            "usage: omega-bench-workload <compute|memory|calls|allocations> <work-units> <seed>"
                .into(),
        );
    }
    let units: u64 = arguments[1]
        .parse()
        .map_err(|error| format!("work units: {error}"))?;
    let seed: u64 = arguments[2]
        .parse()
        .map_err(|error| format!("seed: {error}"))?;
    if !(1..=100_000_000).contains(&units) {
        return Err("work units must be 1..=100000000".into());
    }
    let mut memory: Vec<u64> = (0..65536)
        .map(|position| seed.wrapping_add(position))
        .collect();
    let transforms: [&dyn Transform; 2] = [&Add(seed), &Xor(seed)];
    let mut checksum = seed;
    let started = Instant::now();
    match arguments[0].as_str() {
        "compute" => {
            for position in 0..black_box(units) {
                checksum = black_box(
                    checksum
                        .rotate_left(7)
                        .wrapping_add(position.wrapping_mul(31)),
                );
            }
        }
        "memory" => {
            for position in 0..black_box(units) {
                let slot = ((position.wrapping_mul(17)) & 65535) as usize;
                let observed = black_box(memory[slot]);
                memory[slot] = observed.wrapping_add(position);
                checksum = checksum.wrapping_add(observed);
            }
        }
        "calls" => {
            for position in 0..black_box(units) {
                let selected = ((position ^ seed) & 1) as usize;
                checksum = black_box(transforms[selected]).apply(checksum, position);
            }
        }
        "allocations" => {
            for position in 0..black_box(units) {
                let length = 64 + (position & 255) as usize;
                let mut allocation = vec![seed as u8; black_box(length)];
                allocation[0] = position as u8;
                checksum = checksum
                    .wrapping_add(u64::from(allocation[0]))
                    .wrapping_add(u64::from(allocation[length - 1]));
                black_box(allocation.as_slice());
            }
        }
        _ => return Err("unknown workload".into()),
    }
    let elapsed = started.elapsed().as_nanos();
    if arguments[0] == "memory" {
        for value in black_box(&memory) {
            checksum = checksum.wrapping_add(*value);
        }
    }
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{units},\"checksum\":\"{}\"}}",
        black_box(checksum)
    );
    #[cfg(feature = "allocation-observation")]
    if let Some(path) = std::env::var_os("OMEGA_ALLOCATION_PROFILE") {
        ALLOCATOR
            .write_report(std::path::Path::new(&path))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
