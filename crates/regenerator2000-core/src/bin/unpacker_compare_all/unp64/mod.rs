//! Runs the `unp64` binary: locates the executable, probes its version, and
//! runs it on one fixture with a unique temporary output file.
//!
//! `parse` holds pure functions that parse `unp64` console output and saved
//! files. This module adds process execution, file cleanup, and pure helpers
//! for binary candidates, command-line arguments, and temporary paths.

pub mod parse;
mod runner;

pub use self::parse::Unp64Outcome;
#[cfg(test)]
pub use self::runner::{ProbeOutput, interpret_version_probe, unp64_args};
pub use self::runner::{UNP64_ENV, Unp64, Unp64Error, binary_candidates, tmp_output_path};
