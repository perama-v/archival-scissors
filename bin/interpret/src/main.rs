use anyhow::Result;

pub(crate) mod cli;
pub(crate) mod context;
pub(crate) mod ether;
pub mod filter;
pub(crate) mod juncture;
pub(crate) mod opcode;
pub(crate) mod processed;

use clap::Parser;
use cli::AppArgs;
pub use filter::process_trace;
/// Produces a summary of a transaction trace by processing it as a stream
/// ```command
/// cargo run --release --example 09_use_proof | cargo run --release -p archors_interpret
/// ```
fn main() -> Result<()> {
    let args = AppArgs::parse();
    process_trace(args.trace_style);
    Ok(())
}
