//! Domain error types for `regenerator2000-core`.

use crate::state::types::Addr;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Primary core domain error hierarchy composing subsystem errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Contextual I/O failure containing exact path metadata.
    #[error("I/O error accessing {path}: {source}")]
    Io {
        /// The file path involved in the I/O failure.
        path: PathBuf,
        /// The underlying standard I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Invalid or out-of-bounds 6502/6510 memory address.
    #[error("Invalid memory address: {0}")]
    InvalidAddress(Addr),

    /// Binary unpacker / depacker failure.
    #[error("Unpack failure: {0}")]
    Unpack(#[from] UnpackError),

    /// Assembly source code exporter error.
    #[error("Export failure: {0}")]
    Export(#[from] ExportError),

    /// VICE monitor protocol client error.
    #[error("VICE monitor error: {0}")]
    Vice(#[from] ViceError),

    /// Project file persistence error (.regen2000proj).
    #[error("Project file error: {0}")]
    Project(#[from] ProjectError),

    /// Generic parse failure with format tag and diagnostic message.
    #[error("Parse failure ({format}): {message}")]
    ParseFailed {
        /// The file format or sub-system format identifier.
        format: String,
        /// The diagnostic error message.
        message: String,
    },
}

/// Strongly typed errors for binary unpacking and depacking heuristics.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnpackError {
    /// The input data is empty.
    #[error("Empty input data")]
    EmptyData,

    /// Could not find a SYS entry point in the BASIC header.
    #[error("Could not find SYS entry point")]
    NoEntryPoint,

    /// Phase 1 exceeded the instruction limit without finding the depacker.
    #[error("Phase 1 timeout: depacker not found")]
    Phase1Timeout,

    /// Phase 2 exceeded the instruction limit without finishing decompression.
    #[error("Phase 2 timeout: decompression did not finish")]
    Phase2Timeout,

    /// No memory was modified during decompression.
    #[error("No memory was modified during decompression")]
    NothingWritten,

    /// The detected entry point is outside the unpacked memory range.
    #[error(
        "Invalid unpacked range (${start_addr:04X}-${end_addr:04X}): entry point ${entry_point:04X} is outside range"
    )]
    InvalidAddressRange {
        /// Start address of unpacked region.
        start_addr: u16,
        /// End address of unpacked region.
        end_addr: u16,
        /// Entry point address.
        entry_point: u16,
    },

    /// Unknown signature or unsupported packer format.
    #[error("Unsupported packer format signature: {0}")]
    UnknownSignature(String),

    /// Buffer underflow during decompression stream reading.
    #[error("Decompression buffer underflow at offset ${0:04X}")]
    BufferUnderflow(usize),

    /// Corrupt payload header detected during unpack sequence.
    #[error("Corrupt depacker payload header")]
    CorruptHeader,

    /// Unpacked output memory overlaps reserved address ranges.
    #[error("Destination memory collision at range ${start:04X}-${end:04X}")]
    MemoryCollision {
        /// Start address of collision.
        start: u16,
        /// End address of collision.
        end: u16,
    },
}

/// Strongly typed errors for assembly/source code exporter backends.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExportError {
    /// Syntax generation failure in target formatter.
    #[error("Formatter syntax error for target '{target}': {details}")]
    SyntaxError {
        /// Target assembler name (e.g. 64tass, acme).
        target: String,
        /// Details of syntax breakdown.
        details: String,
    },

    /// Duplicate symbol definition in output assembly scope.
    #[error("Symbol name collision: '{symbol}' defined at both ${addr1:04X} and ${addr2:04X}")]
    SymbolCollision {
        /// Symbol identifier name.
        symbol: String,
        /// First address definition.
        addr1: u16,
        /// Second address definition.
        addr2: u16,
    },

    /// Output file write error.
    #[error("Output file I/O error at {path}: {source}")]
    Io {
        /// Destination file path.
        path: PathBuf,
        /// Standard I/O source error.
        #[source]
        source: std::io::Error,
    },
}

/// Strongly typed errors for VICE monitor protocol client communication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ViceError {
    /// Connection attempt to VICE monitor socket failed.
    #[error("Failed to connect to VICE binary monitor at {address}: {source}")]
    ConnectionFailed {
        /// Target IP/hostname and port string.
        address: String,
        /// Underlying network I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Protocol handshake exchange mismatch.
    #[error("Protocol handshake failed: expected magic {expected:?}, got {got:?}")]
    HandshakeFailed {
        /// Expected binary byte sequence.
        expected: Vec<u8>,
        /// Actual binary byte sequence received.
        got: Vec<u8>,
    },

    /// Command response timeout.
    #[error("Command execution timed out after {timeout_secs}s")]
    Timeout {
        /// Timeout limit in seconds.
        timeout_secs: u64,
    },

    /// Command explicitly rejected by VICE server.
    #[error("VICE returned error code 0x{code:02X}: {message}")]
    CommandRejected {
        /// Status code returned by VICE monitor.
        code: u8,
        /// Error explanation message.
        message: String,
    },

    /// Socket network I/O error.
    #[error("Socket I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Malformed framing or unexpected packet response.
    #[error("Unexpected response framing: {0}")]
    ProtocolFraming(String),
}

/// Strongly typed errors for project state persistence (.regen2000proj).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// Project file was generated by a newer version than supported.
    #[error(
        "Project was saved by a newer schema version {version} (max supported: {max_supported})"
    )]
    UnsupportedVersion {
        /// File schema version.
        version: u32,
        /// Maximum schema version supported by this build.
        max_supported: u32,
    },

    /// Invalid JSON format during deserialization.
    #[error("Project JSON deserialization failed for '{path}': {source}")]
    InvalidJson {
        /// Project file path.
        path: PathBuf,
        /// Underlying serde_json error.
        #[source]
        source: serde_json::Error,
    },

    /// Integrity check failure.
    #[error("Project file checksum mismatch (expected {expected}, computed {actual})")]
    ChecksumMismatch {
        /// Expected hex hash string.
        expected: String,
        /// Actual computed hex hash string.
        actual: String,
    },

    /// File I/O error during project load/save.
    #[error("Project file I/O error at {path}: {source}")]
    Io {
        /// Project file path.
        path: PathBuf,
        /// Standard I/O source error.
        #[source]
        source: std::io::Error,
    },

    /// No project path set for save operation.
    #[error("No project path set for save operation")]
    NoProjectPath,
}

/// Context extension trait for appending file paths to [`std::io::Result`].
pub trait IoResultExt<T> {
    /// Attaches path context to a standard I/O result, producing a [`CoreError::Io`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Io`] containing `path` and the original `std::io::Error` if `self` is `Err`.
    fn with_path<P: AsRef<Path>>(self, path: P) -> Result<T, CoreError>;
}

impl<T> IoResultExt<T> for std::result::Result<T, std::io::Error> {
    fn with_path<P: AsRef<Path>>(self, path: P) -> Result<T, CoreError> {
        self.map_err(|source| CoreError::Io {
            path: path.as_ref().to_path_buf(),
            source,
        })
    }
}
