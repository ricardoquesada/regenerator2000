//! Domain error types for `regenerator2000-core`.

use crate::state::types::Addr;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Represents the core domain error hierarchy across all subsystems.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Represents an I/O failure with file path metadata.
    #[error("I/O error accessing {path}: {source}")]
    Io {
        /// File path associated with the I/O failure.
        path: PathBuf,
        /// Underlying standard I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Represents an invalid or out-of-bounds 6502 memory address.
    #[error("Invalid memory address: {0}")]
    InvalidAddress(Addr),

    /// Represents a binary unpacker or depacker failure.
    #[error("Unpack failure: {0}")]
    Unpack(#[from] UnpackError),

    /// Represents an assembly or HTML exporter failure.
    #[error("Export failure: {0}")]
    Export(#[from] ExportError),

    /// Represents a VICE monitor protocol client failure.
    #[error("VICE monitor error: {0}")]
    Vice(#[from] ViceError),

    /// Represents a project file persistence failure (`.regen2000proj`).
    #[error("Project file error: {0}")]
    Project(#[from] ProjectError),

    /// Represents a parse failure with a format identifier and message.
    #[error("Parse failure ({format}): {message}")]
    ParseFailed {
        /// File format or subsystem identifier.
        format: String,
        /// Diagnostic error message.
        message: String,
    },
}

/// Represents errors from binary unpacking and depacking heuristics.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnpackError {
    /// Indicates that the input buffer is empty.
    #[error("Empty input data")]
    EmptyData,

    /// Indicates that the BASIC header contains no `SYS` entry point.
    #[error("Could not find SYS entry point")]
    NoEntryPoint,

    /// Indicates that Phase 1 reached the instruction limit before finding the depacker.
    #[error("Phase 1 timeout: depacker not found")]
    Phase1Timeout,

    /// Indicates that Phase 2 reached the instruction limit before decompression finished.
    #[error("Phase 2 timeout: decompression did not finish")]
    Phase2Timeout,

    /// Indicates that decompression wrote no bytes to memory.
    #[error("No memory was modified during decompression")]
    NothingWritten,

    /// Indicates that the detected entry point lies outside the unpacked memory range.
    #[error(
        "Invalid unpacked range (${start_addr:04X}-${end_addr:04X}): entry point ${entry_point:04X} is outside range"
    )]
    InvalidAddressRange {
        /// First address of the unpacked region.
        start_addr: u16,
        /// Last address of the unpacked region.
        end_addr: u16,
        /// Detected execution entry point address.
        entry_point: u16,
    },

    /// Indicates an unknown signature or an unsupported packer format.
    #[error("Unsupported packer format signature: {0}")]
    UnknownSignature(String),

    /// Indicates a buffer underflow while reading the compressed stream.
    #[error("Decompression buffer underflow at offset ${0:04X}")]
    BufferUnderflow(usize),

    /// Indicates a corrupt payload header in the packed binary.
    #[error("Corrupt depacker payload header")]
    CorruptHeader,

    /// Indicates that unpacked output overlaps a reserved address range.
    #[error("Destination memory collision at range ${start:04X}-${end:04X}")]
    MemoryCollision {
        /// First address of the collision range.
        start: u16,
        /// Last address of the collision range.
        end: u16,
    },
}

/// Represents errors from assembly and HTML exporter backends.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExportError {
    /// Indicates a syntax generation error in the target assembler formatter.
    #[error("Formatter syntax error for target '{target}': {details}")]
    SyntaxError {
        /// Target assembler name, such as `64tass` or `acme`.
        target: String,
        /// Description of the syntax error.
        details: String,
    },

    /// Indicates a duplicate symbol definition in the output assembly scope.
    #[error("Symbol name collision: '{symbol}' defined at both ${addr1:04X} and ${addr2:04X}")]
    SymbolCollision {
        /// Conflicting symbol identifier.
        symbol: String,
        /// First address that defines the symbol.
        addr1: u16,
        /// Second address that defines the symbol.
        addr2: u16,
    },

    /// Indicates an output file write error.
    #[error("Output file I/O error at {path}: {source}")]
    Io {
        /// Destination file path.
        path: PathBuf,
        /// Underlying standard I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Represents errors from VICE binary monitor communication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ViceError {
    /// Indicates that the TCP connection to the VICE monitor socket failed.
    #[error("Failed to connect to VICE binary monitor at {address}: {source}")]
    ConnectionFailed {
        /// Target hostname and port string.
        address: String,
        /// Underlying network I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Indicates a protocol handshake byte mismatch.
    #[error("Protocol handshake failed: expected magic {expected:?}, got {got:?}")]
    HandshakeFailed {
        /// Expected handshake byte sequence.
        expected: Vec<u8>,
        /// Received handshake byte sequence.
        got: Vec<u8>,
    },

    /// Indicates that a monitor command timed out.
    #[error("Command execution timed out after {timeout_secs}s")]
    Timeout {
        /// Timeout limit in seconds.
        timeout_secs: u64,
    },

    /// Indicates that the VICE server returned an error status code.
    #[error("VICE returned error code 0x{code:02X}: {message}")]
    CommandRejected {
        /// Status code from the VICE monitor.
        code: u8,
        /// Error message from the VICE monitor.
        message: String,
    },

    /// Indicates a socket network I/O failure.
    #[error("Socket I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Indicates a malformed packet frame or an unexpected response packet.
    #[error("Unexpected response framing: {0}")]
    ProtocolFraming(String),
}

/// Represents errors from project file persistence (`.regen2000proj`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// Indicates that a newer application version created the project file.
    #[error(
        "Project was saved by a newer schema version {version} (max supported: {max_supported})"
    )]
    UnsupportedVersion {
        /// Schema version stored in the project file.
        version: u32,
        /// Maximum schema version supported by this build.
        max_supported: u32,
    },

    /// Indicates invalid JSON syntax during project deserialization.
    #[error("Project JSON deserialization failed for '{path}': {source}")]
    InvalidJson {
        /// Project file path.
        path: PathBuf,
        /// Underlying `serde_json` deserialization error.
        #[source]
        source: serde_json::Error,
    },

    /// Indicates a project file checksum mismatch.
    #[error("Project file checksum mismatch (expected {expected}, computed {actual})")]
    ChecksumMismatch {
        /// Expected hexadecimal checksum string.
        expected: String,
        /// Computed hexadecimal checksum string.
        actual: String,
    },

    /// Indicates a file I/O failure while loading or saving a project.
    #[error("Project file I/O error at {path}: {source}")]
    Io {
        /// Project file path.
        path: PathBuf,
        /// Underlying standard I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Indicates that no project path is set for the save operation.
    #[error("No project path set for save operation")]
    NoProjectPath,
}

/// Appends file path context to a [`std::io::Result`].
pub trait IoResultExt<T> {
    /// Attaches `path` context to a standard I/O result and produces a [`CoreError::Io`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Io`] with `path` and the original [`std::io::Error`] when `self` is `Err`.
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
