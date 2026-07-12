//! Generic packer strategy implementation for signature-only packers.

use super::{Packer, PackerInfo};

/// Generic packer implementation for signature-matched packers with static metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplePacker {
    /// Information describing the packer.
    pub info: PackerInfo,
}

impl SimplePacker {
    /// Creates a new [`SimplePacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo) -> Self {
        Self { info }
    }
}

impl Packer for SimplePacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }
}
