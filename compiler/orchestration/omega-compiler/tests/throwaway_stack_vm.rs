// Throwaway test: compile and run the stack_vm sample.
// DELETE THIS FILE when done (after adding permanent differential test).

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use omega_interpreter::interpret;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
#[test]
fn throwaway_stack_vm_compiles_and_exits_70() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .join("samples")
        .join("stack_vm");
    let main_path = sample.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-stack-vm-throwaway-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    // Check with interpreter first.
    let checked = compile_to_checked(&main_path, None)
        .expect("stack_vm should compile to checked");
    let interp_outcome = interpret(&checked, b"");
    eprintln!("interpreter exit code: {}", interp_outcome.exit_code);
    eprintln!("interpreter error: {:?}", interp_outcome.error);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("stack_vm should compile");

    let output = Command::new(build_dir.join("omega-program.exe"))
        .output()
        .expect("stack_vm executable should run");

    let code = output.status.code().unwrap_or(-1);
    eprintln!("native exit code: {code}");
    eprintln!("stdout: {:?}", String::from_utf8_lossy(&output.stdout));
    eprintln!("stderr: {:?}", String::from_utf8_lossy(&output.stderr));

    eprintln!(
        "interpreter={} native={} match={}",
        interp_outcome.exit_code,
        code,
        interp_outcome.exit_code == code
    );

    let _ = std::fs::remove_dir_all(&build_dir);
}
