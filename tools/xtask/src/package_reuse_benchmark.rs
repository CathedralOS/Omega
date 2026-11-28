//! Measure the original public filesystem program with empty stores, then an edited copy.
use crate::execution::{digest, write_json};
use crate::repository::git;
use serde_json::json;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const BASE: &str = "e6477bc08c36684b78cca182ba91bd012b4c5a4d";
const FIXTURE: &str = "source-rust/application/tests/fixtures/filesystem/run/public_values";
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn run(args: &[String]) -> std::result::Result<(), u8> {
    let [compiler, output] = args else {
        eprintln!("usage: cargo xtask measure-public-values RELEASE_OMEGA NEW_DIRECTORY");
        return Err(2);
    };
    measure(&crate::repo_root()?, compiler.as_ref(), output.as_ref()).map_err(|e| {
        eprintln!("measure-public-values: {e}");
        1
    })
}

fn files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            results.extend(files(&entry.path())?);
        } else {
            results.push(entry.path());
        }
    }
    Ok(results)
}

fn cpu_model() -> Option<String> {
    if cfg!(windows) {
        let row = Command::new("reg.exe")
            .args([
                "query",
                r"HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0",
                "/v",
                "ProcessorNameString",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&row.stdout);
        return text
            .lines()
            .find_map(|line| line.split_once("REG_SZ").map(|(_, v)| v.trim().to_owned()));
    }
    fs::read_to_string("/proc/cpuinfo")
        .ok()?
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find_map(|(key, value)| (key.trim() == "model name").then(|| value.trim().to_owned()))
}
fn measure(repo: &Path, compiler: &Path, output: &Path) -> Result<()> {
    let compiler = compiler.canonicalize()?;
    fs::create_dir(output)?;
    let output = output.canonicalize()?;
    let original = repo.join(FIXTURE);
    for name in ["main.omg", "build.omg"] {
        if fs::read(original.join(name))?
            != git(repo, &["show", &format!("{BASE}:{FIXTURE}/{name}")])?.as_bytes()
        {
            return Err(format!("original {name} differs from the benchmark base").into());
        }
    }
    let host = json!({
        "fixture_base": BASE,
        "head": git(repo, &["rev-parse", "HEAD"])?.trim(),
        "compiler": compiler, "compiler_sha256": digest(&compiler)?,
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "workers_available": std::thread::available_parallelism()?.get(),
        "cpu_model": cpu_model(),
        "rayon_num_threads": std::env::var("RAYON_NUM_THREADS").ok()
    });
    write_json(&output.join("host.json"), &host)?;
    let bench = Benchmark {
        compiler,
        output,
        library: repo.join("source/library"),
    };
    for n in 1..=3 {
        bench.trial(
            &format!("cold-{n}"),
            &original,
            &format!("cold-cache-{n}"),
            false,
        )?;
    }
    let copy = bench.output.join("source-copy");
    let tracked = git(repo, &["ls-files", "-z", "--", FIXTURE, "source/library"])?;
    for file in tracked.split_terminator('\0') {
        let dest = copy.join(file);
        fs::create_dir_all(dest.parent().ok_or("missing parent for fixture file")?)?;
        fs::copy(repo.join(file), dest)?;
    }
    let fixture = copy.join(FIXTURE);
    bench.trial("prepare", &fixture, "edited-cache", false)?;
    let main = fixture.join("main.omg");
    let original_text = fs::read_to_string(&main)?;
    for n in 1..=3 {
        let updated =
            original_text.replace("portable_copy.tmp", &format!("portable_edited_{n}.tmp"));
        if updated == original_text {
            return Err("fixture lacks its copy destination".into());
        }
        fs::write(&main, updated)?;
        bench.trial(&format!("edited-{n}"), &fixture, "edited-cache", false)?;
    }
    bench.trial("profile", &original, "profile-cache", true)
}

struct Benchmark {
    compiler: PathBuf,
    output: PathBuf,
    library: PathBuf,
}

impl Benchmark {
    fn trial(&self, label: &str, fixture: &Path, cache_name: &str, profile: bool) -> Result<()> {
        let run = self.output.join(label);
        let cwd = run.join("cwd");
        fs::create_dir_all(&cwd)?;
        let cache = self.output.join(cache_name);
        fs::create_dir_all(&cache)?;
        let before = files(&cache)?.len();
        let product = run.join("product");
        let build = fixture.join("build.omg");
        let mut compile = Command::new(&self.compiler);
        compile
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .env(
                if cfg!(windows) {
                    "LOCALAPPDATA"
                } else {
                    "XDG_CACHE_HOME"
                },
                &cache,
            )
            .env("OMEGA_LIBRARY_ROOT", &self.library)
            .args(["build", "--target", "windows_x86_64", "--build-dir"])
            .arg(&product);
        if profile {
            compile.arg("--timings");
        }
        compile.arg(&build);
        // Wait on the process itself. Sampling its status on a fixed interval
        // would quantize the elapsed time around the one-second acceptance.
        let start = Instant::now();
        let out = compile.output()?;
        let elapsed = start.elapsed().as_nanos() as u64;
        fs::write(run.join("stdout.txt"), &out.stdout)?;
        fs::write(run.join("stderr.txt"), &out.stderr)?;
        let mut row = json!({
            "case": label, "arguments": compile.get_args().map(|v| v.to_string_lossy()).collect::<Vec<_>>(),
            "cwd": cwd, "cache": cache, "cache_files_before": before, "cache_files_after": files(&cache)?.len(),
            "returncode": out.status.code(), "elapsed_ns": elapsed,
            "build_sha256": digest(&build)?, "main_sha256": digest(&fixture.join("main.omg"))?
        });
        let result = run.join("result.json");
        write_json(&result, &row)?;
        if !out.status.success() {
            return Err(format!(
                "{label}: compiler failed; see {}",
                run.join("stderr.txt").display()
            )
            .into());
        }
        let images: Vec<_> = files(&product)?
            .into_iter()
            .filter(|p| p.extension().is_some_and(|s| s == "exe"))
            .collect();
        let [image] = images.as_slice() else {
            return Err(format!("{label}: expected one PE executable").into());
        };
        let bytes = fs::read(image)?;
        let Some(offset) = bytes.get(60..64) else {
            return Err("incomplete DOS header".into());
        };
        let pe = u32::from_le_bytes(offset.try_into()?) as usize;
        let header = bytes.get(pe..).ok_or("missing PE header")?;
        if !bytes.starts_with(b"MZ")
            || header.get(..6) != Some(b"PE\0\0\x64\x86")
            || header.get(24..26) != Some(&[0x0b, 0x02])
        {
            return Err(format!("{label}: expected x86-64 PE32+").into());
        }
        row["image"] = json!(image);
        row["image_bytes"] = json!(bytes.len());
        row["image_sha256"] = json!(digest(image)?);
        if cfg!(windows) {
            let native = Command::new(image)
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .output()?;
            fs::write(run.join("native.stdout"), &native.stdout)?;
            fs::write(run.join("native.stderr"), &native.stderr)?;
            row["native_exit"] = json!(native.status.code());
            write_json(&result, &row)?;
            if !native.status.success()
                || !native.stdout.is_empty()
                || !native.stderr.is_empty()
                || fs::read_dir(&cwd)?.next().is_some()
            {
                return Err(format!("{label}: filesystem program failed its native oracle").into());
            }
        }
        write_json(&result, &row)?;
        println!(
            "{label} {:.6} s, {} bytes, {}",
            elapsed as f64 / 1e9,
            bytes.len(),
            image.display()
        );
        Ok(())
    }
}
