use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = Vec::new();
    io::stdin().read_to_end(&mut source)?;
    let observation = source_files_to_tokens::observation::encode(&source);
    io::stdout().write_all(&observation)?;
    Ok(())
}
