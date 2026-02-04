use anyhow::Result;
use vergen_gitcl::{Emitter, GitclBuilder};

fn main() -> Result<()> {
    // Emit the instructions
    Emitter::default()
        .add_instructions(&GitclBuilder::all_git()?)?
        .emit()?;
    Ok(())
}
