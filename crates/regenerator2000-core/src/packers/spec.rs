//! Declarative packer specification types, address sources, and emulation hook facades.
//!
//! This module defines the data-driven model for 6502 binary packers:
//!
//! - [`Pat`] and the [`sig!`](crate::sig) macro express byte patterns with wildcards,
//!   bitmasks, and value sets.
//! - [`Anchor`] and [`Signature`] describe where and how signatures are matched in a
//!   [`LoadedImage`].
//! - [`AddrSource`], [`PointerGuard`], and [`Hints`] declaratively express static and
//!   runtime address overrides (depacker address, output range start/end, entry point,
//!   forced entry, and exact-return semantics).
//! - [`PatchAddr`] and [`MemoryPatch`] express conditional or unconditional pre-emulation
//!   RAM patches.
//! - [`PackerSpec`] combines signatures, hints, patches, an optional per-position
//!   `resolve` escape hatch, an optional `hooks` factory, and a self-test `fixture`.
//! - [`PackerHooks`], [`StepContext`], [`RefineContext`], and [`Bounds`] provide a narrow,
//!   encapsulated emulation-time hook facade that hides CPU and bus internals.

use std::fmt::Debug;
use std::ops::RangeInclusive;

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;

use crate::state::types::{System, TargetSystem};
use crate::unpacker::UnpackerMemory;
use crate::unpacker::machine::Phase;

/// Stable identifier for a registered [`PackerSpec`] (e.g. `"eagle"`, `"exomizer.v3"`).
pub type PackerId = &'static str;

/// Returns `true` if `id` is a valid, canonical [`PackerId`].
///
/// A valid identifier is non-empty, starts with an ASCII lowercase letter or digit,
/// and contains only ASCII lowercase alphanumeric characters, `'_'`, `'.'`, or `'-'`,
/// with no consecutive or trailing separators.
///
/// # Examples
///
/// ```
/// use regenerator2000_core::packers::spec::is_valid_packer_id;
///
/// assert!(is_valid_packer_id("eagle"));
/// assert!(is_valid_packer_id("exomizer.v3"));
/// assert!(is_valid_packer_id("1001_card_cruncher.acm"));
/// assert!(!is_valid_packer_id(""));
/// assert!(!is_valid_packer_id("Eagle"));
/// assert!(!is_valid_packer_id("bad id"));
/// ```
#[must_use]
pub fn is_valid_packer_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    let mut prev_sep = false;
    for &b in rest {
        if b.is_ascii_lowercase() || b.is_ascii_digit() {
            prev_sep = false;
        } else if matches!(b, b'_' | b'.' | b'-') {
            if prev_sep {
                return false;
            }
            prev_sep = true;
        } else {
            return false;
        }
    }
    !prev_sep
}

// ===========================================================================
// Signature patterns (`Pat`, `sig!`, `Anchor`, `Signature`)
// ===========================================================================

/// One element of a [`Signature`] byte pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pat {
    /// Matches only the exact byte value.
    Byte(u8),
    /// Wildcard: matches any byte value (`0x00..=0xFF`).
    Any,
    /// Bitmask match: holds when `(byte & mask) == eq` (e.g. TBC `& 0xFD == 0x84`).
    Mask {
        /// Bitmask applied to the candidate byte before comparison.
        mask: u8,
        /// Expected value after masking.
        eq: u8,
    },
    /// Set match: holds when `byte` equals any element in the slice (e.g. Exomizer `0x34 | 0x50`).
    OneOf(&'static [u8]),
}

impl Pat {
    /// Returns `true` if `byte` satisfies this pattern element.
    ///
    /// # Examples
    ///
    /// ```
    /// use regenerator2000_core::packers::spec::Pat;
    ///
    /// assert!(Pat::Byte(0x78).matches(0x78));
    /// assert!(!Pat::Byte(0x78).matches(0x79));
    /// assert!(Pat::Any.matches(0x00));
    /// assert!(Pat::Mask { mask: 0xFD, eq: 0x84 }.matches(0x86));
    /// assert!(Pat::OneOf(&[0x34, 0x50]).matches(0x50));
    /// assert!(!Pat::OneOf(&[]).matches(0x00));
    /// ```
    #[must_use]
    pub fn matches(self, byte: u8) -> bool {
        match self {
            Self::Byte(expected) => byte == expected,
            Self::Any => true,
            Self::Mask { mask, eq } => (byte & mask) == eq,
            Self::OneOf(choices) => choices.contains(&byte),
        }
    }
}

/// Constructs a `&'static [Pat]` slice from byte literals, wildcards (`_` or `__`),
/// `OneOf` slices (`[a, b]`), and `Mask` pairs (`(mask, eq)`).
///
/// Tokens may be separated by spaces or commas.
///
/// # Examples
///
/// ```
/// use regenerator2000_core::packers::spec::Pat;
/// use regenerator2000_core::sig;
///
/// const S: &[Pat] = sig![0x78, 0xA2, __, [0x34, 0x50], (0xFD, 0x84)];
/// assert_eq!(
///     S,
///     &[
///         Pat::Byte(0x78),
///         Pat::Byte(0xA2),
///         Pat::Any,
///         Pat::OneOf(&[0x34, 0x50]),
///         Pat::Mask { mask: 0xFD, eq: 0x84 },
///     ]
/// );
/// ```
#[macro_export]
macro_rules! sig {
    (@munch [ $($acc:tt)* ]) => {
        &[ $($acc)* ]
    };
    (@munch [ $($acc:tt)* ] , $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* ] $($rest)*)
    };
    (@munch [ $($acc:tt)* ] _ $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* $crate::packers::spec::Pat::Any, ] $($rest)*)
    };
    (@munch [ $($acc:tt)* ] __ $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* $crate::packers::spec::Pat::Any, ] $($rest)*)
    };
    (@munch [ $($acc:tt)* ] [ $($choice:expr),* $(,)? ] $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* $crate::packers::spec::Pat::OneOf(&[ $($choice),* ]), ] $($rest)*)
    };
    (@munch [ $($acc:tt)* ] ( $mask:expr , $eq:expr ) $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* $crate::packers::spec::Pat::Mask { mask: $mask, eq: $eq }, ] $($rest)*)
    };
    (@munch [ $($acc:tt)* ] $byte:literal $($rest:tt)*) => {
        $crate::sig!(@munch [ $($acc)* $crate::packers::spec::Pat::Byte($byte), ] $($rest)*)
    };
    ($($tokens:tt)*) => {
        $crate::sig!(@munch [] $($tokens)*)
    };
}

pub use crate::sig;

/// Where a [`Signature`] is searched within a [`LoadedImage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    /// Positive offset from [`System::default_basic_start`] (e.g. `$0801 + offset` on C64).
    BasicStart {
        /// Unsigned byte offset added to `system.default_basic_start()`.
        offset: u16,
    },
    /// Fixed absolute 16-bit memory address.
    Absolute(u16),
    /// Inclusive absolute address range searched in ascending order (e.g. `0x0810..=0x0840`).
    Window(RangeInclusive<u16>),
    /// Forward (ascending) scan starting at `image.load_addr` up to `load_addr + max_offset`.
    ScanFromLoad {
        /// Maximum offset past `load_addr` to test as a signature start position.
        max_offset: usize,
    },
    /// Backward (descending) scan from `image.load_end - pattern.len()` down by at most
    /// `max_offset` bytes, bounded below by `image.load_addr`.
    ScanBackFromEnd {
        /// Maximum backward distance from the highest valid start position.
        max_offset: usize,
    },
    /// Ordered list of candidate anchors tried in sequence.
    Candidates(&'static [Anchor]),
}

impl Anchor {
    /// Appends every candidate start address for a pattern of length `pattern_len`
    /// in `image` to `out`, in the exact probe order of this anchor.
    pub fn collect_positions(
        &self,
        image: &LoadedImage<'_>,
        pattern_len: usize,
        out: &mut Vec<u16>,
    ) {
        let _: Option<()> = self.find_position(image, pattern_len, |addr| {
            out.push(addr);
            None
        });
    }

    /// Calls `predicate` on each candidate start address for a pattern of length `pattern_len`
    /// in `image` in probe order, returning `Some(R)` on the first candidate where
    /// `predicate` returns `Some(R)`.
    ///
    /// Performs zero heap allocations.
    #[must_use]
    pub fn find_position<R, F>(
        &self,
        image: &LoadedImage<'_>,
        pattern_len: usize,
        mut predicate: F,
    ) -> Option<R>
    where
        F: FnMut(u16) -> Option<R>,
    {
        self.find_position_ref(image, pattern_len, &mut predicate)
    }

    fn find_position_ref<R, F>(
        &self,
        image: &LoadedImage<'_>,
        pattern_len: usize,
        predicate: &mut F,
    ) -> Option<R>
    where
        F: FnMut(u16) -> Option<R>,
    {
        match self {
            Self::BasicStart { offset } => {
                let addr = image.system.default_basic_start().checked_add(*offset)?;
                predicate(addr)
            }
            Self::Absolute(addr) => predicate(*addr),
            Self::Window(range) => {
                let start = *range.start();
                let end = *range.end();
                if start <= end {
                    for addr in start..=end {
                        if let Some(res) = predicate(addr) {
                            return Some(res);
                        }
                    }
                }
                None
            }
            Self::ScanFromLoad { max_offset } => {
                let start = usize::from(image.load_addr);
                let limit_from_offset = start.saturating_add(*max_offset);
                let limit_from_end = usize::from(image.load_end).saturating_sub(pattern_len);
                let upper = limit_from_offset
                    .min(limit_from_end)
                    .min(usize::from(u16::MAX));
                if start <= upper {
                    for p in start..=upper {
                        if let Ok(addr) = u16::try_from(p)
                            && let Some(res) = predicate(addr)
                        {
                            return Some(res);
                        }
                    }
                }
                None
            }
            Self::ScanBackFromEnd { max_offset } => {
                let end_bound = usize::from(image.load_end);
                if end_bound < pattern_len {
                    return None;
                }
                let upper = (end_bound - pattern_len).min(usize::from(u16::MAX));
                let lower = usize::from(image.load_addr).max(upper.saturating_sub(*max_offset));
                if lower <= upper {
                    for p in (lower..=upper).rev() {
                        if let Ok(addr) = u16::try_from(p)
                            && let Some(res) = predicate(addr)
                        {
                            return Some(res);
                        }
                    }
                }
                None
            }
            Self::Candidates(anchors) => {
                for anchor in *anchors {
                    if let Some(res) = anchor.find_position_ref(image, pattern_len, predicate) {
                        return Some(res);
                    }
                }
                None
            }
        }
    }
}

/// A byte pattern anchored by an [`Anchor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Where to look for `pattern` in memory.
    pub anchor: Anchor,
    /// Sequence of byte patterns that must match contiguously starting at the anchor position.
    pub pattern: &'static [Pat],
}

impl Signature {
    /// Returns `true` if `self.pattern` matches `mem` starting at `addr`.
    ///
    /// Returns `false` (without panicking) if `addr + self.pattern.len()` exceeds `mem.len()`
    /// or overflows the 16-bit address space.
    #[must_use]
    pub fn matches_at(&self, mem: &[u8], addr: u16) -> bool {
        let start = usize::from(addr);
        let Some(end) = start.checked_add(self.pattern.len()) else {
            return false;
        };
        // Reject patterns that cross the 64 KB address boundary ($FFFF -> $10000).
        if end > 0x1_0000 {
            return false;
        }
        let Some(slice) = mem.get(start..end) else {
            return false;
        };
        slice
            .iter()
            .zip(self.pattern.iter())
            .all(|(&byte, &pat)| pat.matches(byte))
    }

    /// Returns the first candidate position of `self.anchor` where `self.pattern` matches `image`.
    #[must_use]
    pub fn find_first(&self, image: &LoadedImage<'_>) -> Option<u16> {
        self.anchor
            .find_position(image, self.pattern.len(), |addr| {
                self.matches_at(image.mem, addr).then_some(addr)
            })
    }
}

// ===========================================================================
// Address sources, guards, hints, and patches
// ===========================================================================

/// 6502 CPU register selector for [`AddrSource::PointerPlusRegister`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    /// Accumulator (`A`).
    A,
    /// Index register `X`.
    X,
    /// Index register `Y`.
    Y,
}

/// Acceptance policy for a post-emulation pointer read ([`AddrSource::PointerAfterRun`]).
///
/// Named variants keep [`AddrSource`] and [`Hints`] `PartialEq + Eq` without comparing
/// function pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerGuard {
    /// Accept the adjusted pointer value unconditionally.
    None,
    /// Generic R2000 rule: requires `raw > start_addr` and
    /// `adjusted >= start_addr && adjusted <= detected_end.saturating_add(slack)`.
    NearDetectedEnd {
        /// Maximum allowed positive distance past `detected_end` (typically `512`).
        slack: u16,
    },
    /// McCracken Compressor rule: requires `raw > start_addr` and
    /// `raw < 0xFFFF` (with `adjusted >= start_addr`).
    McCrackenEndOfProgram,
}

impl PointerGuard {
    /// Returns `true` if a pointer with raw LE value `raw` and adjusted value `adjusted`
    /// is accepted given the current `start_addr` and `detected_end`.
    #[must_use]
    pub const fn accepts(
        self,
        raw: u16,
        adjusted: u16,
        start_addr: u16,
        detected_end: u16,
    ) -> bool {
        match self {
            Self::None => true,
            Self::NearDetectedEnd { slack } => {
                raw > start_addr
                    && adjusted >= start_addr
                    && adjusted <= detected_end.saturating_add(slack)
            }
            Self::McCrackenEndOfProgram => {
                raw > start_addr && raw < 0xFFFF && adjusted >= start_addr
            }
        }
    }
}

/// Declarative source for a 16-bit address hint in [`Hints`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrSource {
    /// Fixed 16-bit constant address.
    Const(u16),
    /// Signed offset from [`System::default_basic_start`] (Ruling B4).
    ///
    /// For example, on C64 (`default_basic_start() == $0801`), `BasicStart(0)` resolves
    /// to `$0801` and `BasicStart(-1)` resolves to `$0800`. Out-of-range offsets that
    /// underflow `< 0` or overflow `> $FFFF` resolve to `None`.
    BasicStart(i16),
    /// Little-endian 16-bit word read from `mem` at `anchor_addr + from_match`.
    OperandLe {
        /// Signed byte offset from the primary signature match address (`anchor_addr`).
        from_match: isize,
    },
    /// Little-endian 16-bit word read from `mem` at a fixed absolute address.
    AbsoluteLe(u16),
    /// Little-endian 16-bit word read from `mem[ptr_addr]` and `mem[ptr_addr.wrapping_add(1)]`
    /// after emulation finishes, adjusted by `adjust` and checked against `guard`.
    PointerAfterRun {
        /// Zero-page or RAM address of the low byte (high byte at `ptr_addr.wrapping_add(1)`).
        ptr_addr: u16,
        /// Signed adjustment added to the raw pointer (e.g. `-1` to convert exclusive to inclusive).
        adjust: i16,
        /// Acceptance guard checked before overriding the bound.
        guard: PointerGuard,
    },
    /// Little-endian 16-bit word read from `mem[ptr_addr]` and `mem[ptr_addr.wrapping_add(1)]`
    /// after emulation finishes, plus the value of CPU register `reg` and signed `adjust`.
    PointerPlusRegister {
        /// Zero-page or RAM address of the low byte.
        ptr_addr: u16,
        /// CPU register added to the 16-bit pointer.
        reg: Reg,
        /// Signed adjustment added after the register offset.
        adjust: i16,
    },
    /// Little-endian 16-bit word at `ptr_addr` sampled only on the **first** hit at the
    /// depacker PC, adjusted by `adjust`.
    PointerAtDepacker {
        /// Zero-page or RAM address of the low byte.
        ptr_addr: u16,
        /// Signed adjustment added to the sampled pointer.
        adjust: i16,
    },
    /// Maximum value of the little-endian 16-bit word at `ptr_addr` sampled across hits at
    /// the depacker PC.
    MonitoredMax {
        /// Zero-page or RAM address of the low byte.
        ptr_addr: u16,
    },
    /// Minimum value of the little-endian 16-bit word at `ptr_addr` sampled across hits at
    /// the depacker PC.
    MonitoredMin {
        /// Zero-page or RAM address of the low byte.
        ptr_addr: u16,
    },
}

/// Reads a little-endian `u16` from `mem` at `ptr_addr` (low byte) and
/// `ptr_addr.wrapping_add(1)` (high byte, Ruling R19).
#[must_use]
pub fn read_u16_le_wrapping(mem: &[u8], ptr_addr: u16) -> Option<u16> {
    let lo = *mem.get(usize::from(ptr_addr))?;
    let hi = *mem.get(usize::from(ptr_addr.wrapping_add(1)))?;
    Some(u16::from_le_bytes([lo, hi]))
}

/// Runtime sampler for [`AddrSource::PointerAtDepacker`], [`AddrSource::MonitoredMax`],
/// and [`AddrSource::MonitoredMin`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PointerMonitor {
    first_at_depacker: Option<u16>,
    monitored_max: Option<u16>,
    monitored_min: Option<u16>,
}

impl PointerMonitor {
    /// Creates an empty [`PointerMonitor`] with no recorded samples.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            first_at_depacker: None,
            monitored_max: None,
            monitored_min: None,
        }
    }

    /// Samples the little-endian 16-bit word at `ptr_addr` from `mem` when execution
    /// reaches the depacker PC.
    ///
    /// - Records `first_at_depacker` only on the first sample (`T3.0.27`).
    /// - Updates `monitored_max` and `monitored_min` on every sample (`T3.0.28`).
    pub fn sample_at_depacker(&mut self, mem: &[u8], ptr_addr: u16) {
        let Some(val) = read_u16_le_wrapping(mem, ptr_addr) else {
            return;
        };
        if self.first_at_depacker.is_none() {
            self.first_at_depacker = Some(val);
        }
        self.monitored_max = Some(self.monitored_max.map_or(val, |cur| cur.max(val)));
        self.monitored_min = Some(self.monitored_min.map_or(val, |cur| cur.min(val)));
    }

    /// Returns the value captured on the first depacker hit, if any.
    #[must_use]
    pub const fn first_at_depacker(&self) -> Option<u16> {
        self.first_at_depacker
    }

    /// Returns the maximum sampled value across depacker hits, if any.
    #[must_use]
    pub const fn monitored_max(&self) -> Option<u16> {
        self.monitored_max
    }

    /// Returns the minimum sampled value across depacker hits, if any.
    #[must_use]
    pub const fn monitored_min(&self) -> Option<u16> {
        self.monitored_min
    }
}

impl AddrSource {
    /// Resolves a detection-time (static) [`AddrSource`] against `image` and `anchor_addr`.
    ///
    /// Returns `None` for runtime sources (`PointerAfterRun`, `PointerPlusRegister`,
    /// `PointerAtDepacker`, `MonitoredMax`, `MonitoredMin`) or if an offset/operand is
    /// out of bounds.
    #[must_use]
    pub fn resolve_static(self, image: &LoadedImage<'_>, anchor_addr: u16) -> Option<u16> {
        match self {
            Self::Const(addr) => Some(addr),
            Self::BasicStart(offset) => {
                let base = i32::from(image.system.default_basic_start());
                let target = base + i32::from(offset);
                u16::try_from(target).ok()
            }
            Self::OperandLe { from_match } => {
                let target = isize::try_from(anchor_addr).ok()?.checked_add(from_match)?;
                let idx = usize::try_from(target).ok()?;
                image.read_u16_le(idx)
            }
            Self::AbsoluteLe(addr) => image.read_u16_le(usize::from(addr)),
            Self::PointerAfterRun { .. }
            | Self::PointerPlusRegister { .. }
            | Self::PointerAtDepacker { .. }
            | Self::MonitoredMax { .. }
            | Self::MonitoredMin { .. } => None,
        }
    }

    /// Resolves a post-emulation (runtime) [`AddrSource`] using final `mem`, exit `registers`,
    /// current `(start_addr, detected_end)` bounds, and `monitor`.
    #[must_use]
    pub fn resolve_post_run(
        self,
        mem: &[u8],
        registers: Registers,
        start_addr: u16,
        detected_end: u16,
        monitor: &PointerMonitor,
    ) -> Option<u16> {
        match self {
            Self::Const(addr) => Some(addr),
            Self::PointerAfterRun {
                ptr_addr,
                adjust,
                guard,
            } => {
                let raw = read_u16_le_wrapping(mem, ptr_addr)?;
                let adjusted = raw.wrapping_add_signed(adjust);
                guard
                    .accepts(raw, adjusted, start_addr, detected_end)
                    .then_some(adjusted)
            }
            Self::PointerPlusRegister {
                ptr_addr,
                reg,
                adjust,
            } => {
                let raw = read_u16_le_wrapping(mem, ptr_addr)?;
                let with_reg = raw.wrapping_add(u16::from(registers.get(reg)));
                Some(with_reg.wrapping_add_signed(adjust))
            }
            Self::PointerAtDepacker { adjust, .. } => monitor
                .first_at_depacker()
                .map(|raw| raw.wrapping_add_signed(adjust)),
            Self::MonitoredMax { .. } => monitor.monitored_max(),
            Self::MonitoredMin { .. } => monitor.monitored_min(),
            Self::BasicStart(_) | Self::OperandLe { .. } | Self::AbsoluteLe(_) => None,
        }
    }
}

/// Address and execution hints contributed by a [`PackerSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hints {
    /// Depacker stub start address (`Phase::FindDepacker` → `Phase::Decompress` trigger).
    pub dep_addr: Option<AddrSource>,
    /// Unpacked output start address override.
    pub start_addr: Option<AddrSource>,
    /// Unpacked output end address override (constant, operand, or post-run pointer).
    pub end_addr: Option<AddrSource>,
    /// Unpacked program entry point override.
    pub entry_point: Option<AddrSource>,
    /// Forced initial entry point overriding the BASIC `SYS` line (unp64 `Forced`).
    pub forced_entry: Option<AddrSource>,
    /// When `true`, `Phase::Decompress` exits only when `pc == ret_addr` rather than
    /// `pc >= ret_addr` (unp64 `RtAFrc`).
    pub exact_return: bool,
}

impl Hints {
    /// All-empty [`Hints`] constant for struct update syntax (`..Hints::NONE`).
    pub const NONE: Self = Self {
        dep_addr: None,
        start_addr: None,
        end_addr: None,
        entry_point: None,
        forced_entry: None,
        exact_return: false,
    };
}

impl Default for Hints {
    fn default() -> Self {
        Self::NONE
    }
}

/// Target address for a pre-emulation [`MemoryPatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchAddr {
    /// Fixed absolute 16-bit memory address.
    Absolute(u16),
    /// Signed offset relative to the primary signature match address (`anchor_addr`).
    FromMatch(isize),
}

impl PatchAddr {
    /// Resolves this patch address to a 16-bit memory address given `anchor_addr`.
    #[must_use]
    pub fn resolve(self, anchor_addr: u16) -> Option<u16> {
        match self {
            Self::Absolute(addr) => Some(addr),
            Self::FromMatch(offset) => {
                let target = isize::try_from(anchor_addr).ok()?.checked_add(offset)?;
                u16::try_from(target).ok()
            }
        }
    }
}

/// A pre-emulation RAM modification declared by a [`PackerSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryPatch {
    /// Where to write `bytes`.
    pub addr: PatchAddr,
    /// Optional guard byte: when `Some(expected)`, the patch is applied only if the
    /// first target byte currently equals `expected` (e.g. Antiram conditional patch).
    pub expected: Option<u8>,
    /// Replacement byte sequence written directly into `mem` before the snapshot.
    pub bytes: &'static [u8],
}

impl MemoryPatch {
    /// Applies this patch to `mem` if its resolved address range lies within `mem`
    /// and `self.expected` (if any) matches the byte at the resolved start address.
    ///
    /// Returns `true` if the patch was applied.
    pub fn apply(&self, mem: &mut [u8], anchor_addr: u16) -> bool {
        if self.bytes.is_empty() {
            return false;
        }
        let Some(target_addr) = self.addr.resolve(anchor_addr) else {
            return false;
        };
        let start = usize::from(target_addr);
        let Some(end) = start.checked_add(self.bytes.len()) else {
            return false;
        };
        if end > 0x1_0000 {
            return false;
        }
        let Some(dest) = mem.get_mut(start..end) else {
            return false;
        };
        if let Some(expected_byte) = self.expected
            && dest[0] != expected_byte
        {
            return false;
        }
        dest.copy_from_slice(self.bytes);
        true
    }
}

// ===========================================================================
// LoadedImage, SignatureMatch, ResolvedSpec, PackerSpec
// ===========================================================================

/// Read-only view of a loaded binary image inspected during packer detection.
#[derive(Debug, Clone, Copy)]
pub struct LoadedImage<'a> {
    /// Memory buffer (typically a 64 KB system memory image).
    pub mem: &'a [u8],
    /// Load address (origin) of the binary.
    pub load_addr: u16,
    /// End address bound passed to packer detection.
    pub load_end: u16,
    /// Target machine system profile.
    pub system: &'a System,
}

impl<'a> LoadedImage<'a> {
    /// Creates a new [`LoadedImage`] view.
    #[must_use]
    pub const fn new(mem: &'a [u8], load_addr: u16, load_end: u16, system: &'a System) -> Self {
        Self {
            mem,
            load_addr,
            load_end,
            system,
        }
    }

    /// Creates a [`LoadedImage`] view for [`TargetSystem::C64`].
    #[must_use]
    pub const fn for_c64(mem: &'a [u8], load_addr: u16, load_end: u16) -> Self {
        Self {
            mem,
            load_addr,
            load_end,
            system: &TargetSystem::C64,
        }
    }

    /// Reads a single byte at `addr`, returning `None` if `addr >= self.mem.len()`.
    #[must_use]
    pub fn read_u8(&self, addr: usize) -> Option<u8> {
        self.mem.get(addr).copied()
    }

    /// Reads a little-endian `u16` at `addr`, returning `None` if `addr + 1 >= self.mem.len()`.
    #[must_use]
    pub fn read_u16_le(&self, addr: usize) -> Option<u16> {
        let end = addr.checked_add(2)?;
        let bytes = self.mem.get(addr..end)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Reads a little-endian `u32` at `addr`, returning `None` if `addr + 3 >= self.mem.len()`.
    #[must_use]
    pub fn read_u32_le(&self, addr: usize) -> Option<u32> {
        let end = addr.checked_add(4)?;
        let bytes = self.mem.get(addr..end)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

/// Detection-time output produced by a [`PackerSpec`] (either statically from its
/// fields or dynamically via [`PackerSpec::resolve`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSpec {
    /// Optional display-name override (when `None`, [`PackerSpec::name`] is used).
    pub name: Option<&'static str>,
    /// Effective hints for this match.
    pub hints: Hints,
    /// Effective pre-emulation memory patches for this match.
    pub patches: &'static [MemoryPatch],
}

impl ResolvedSpec {
    /// Creates a [`ResolvedSpec`] with `hints` and no display-name override or patches.
    #[must_use]
    pub const fn from_hints(hints: Hints) -> Self {
        Self {
            name: None,
            hints,
            patches: &[],
        }
    }

    /// Creates a [`ResolvedSpec`] with a display-name override and `hints`.
    #[must_use]
    pub const fn named(name: &'static str, hints: Hints) -> Self {
        Self {
            name: Some(name),
            hints,
            patches: &[],
        }
    }

    /// Creates a [`ResolvedSpec`] with a display-name override, `hints`, and pre-emulation `patches`.
    #[must_use]
    pub const fn with_patches(
        name: &'static str,
        hints: Hints,
        patches: &'static [MemoryPatch],
    ) -> Self {
        Self {
            name: Some(name),
            hints,
            patches,
        }
    }
}

/// Result of matching a [`PackerSpec`] against a [`LoadedImage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureMatch {
    /// Identifier of the matched [`PackerSpec`].
    pub spec_id: PackerId,
    /// Effective display name of the matched packer (`resolved.name.unwrap_or(spec.name)`).
    pub spec_name: &'static str,
    /// Matched address of the primary signature (`signatures[0]`, or `load_addr` if `signatures` is empty).
    pub anchor_addr: u16,
    /// Matched address for every signature in [`PackerSpec::signatures`], in slice order.
    pub match_addrs: Vec<u16>,
    /// Effective hints, patches, and optional name override for this match.
    pub resolved: ResolvedSpec,
}

impl SignatureMatch {
    /// Returns the matched address of signature `index`, if present.
    #[must_use]
    pub fn signature_addr(&self, index: usize) -> Option<u16> {
        self.match_addrs.get(index).copied()
    }
}

/// Returns `true` if `actual` matches the allowed system entry `allowed`.
///
/// For [`TargetSystem::C64`], this uses [`TargetSystem::is_c64`] (consistent with
/// [`RomTrapTable::for_system`](crate::unpacker::machine::RomTrapTable::for_system)
/// and [`C64Bus`](crate::unpacker::C64Bus)), NOT `PartialEq`, so a
/// `TargetSystem::Custom("Commodore 64")` does not masquerade as [`TargetSystem::C64`]
/// (`T3.0.31`).
#[must_use]
pub fn system_matches(allowed: &TargetSystem, actual: &TargetSystem) -> bool {
    match (allowed, actual) {
        (TargetSystem::C64, sys) => sys.is_c64(),
        (TargetSystem::Custom(a), TargetSystem::Custom(b)) => a.eq_ignore_ascii_case(b),
        (TargetSystem::Custom(_), _) | (_, TargetSystem::Custom(_)) => false,
        (a, b) => core::mem::discriminant(a) == core::mem::discriminant(b),
    }
}

/// Function pointer signature for [`PackerSpec::resolve`].
pub type ResolveFn = fn(&LoadedImage<'_>, &SignatureMatch) -> Option<ResolvedSpec>;

/// Function pointer signature for [`PackerSpec::hooks`] (Ruling B3).
pub type HooksFactory = fn(&LoadedImage<'_>, &SignatureMatch) -> Box<dyn PackerHooks>;

/// Declarative specification of a binary packer variant.
///
/// Note: Derives `Debug`, `Clone`, and `Copy`, but intentionally omits `PartialEq` / `Eq`
/// to avoid the `unpredictable_function_pointer_comparisons` lint on `resolve` and `hooks`.
/// Compare specs by [`Self::id`] or [`std::ptr::eq`].
#[derive(Debug, Clone, Copy)]
pub struct PackerSpec {
    /// Stable identifier (e.g. `"eagle"`, `"exomizer.v3"`).
    pub id: PackerId,
    /// Default human-readable display name (e.g. `"Eagle Cruncher"`).
    pub name: &'static str,
    /// Detection priority (strictly increasing across the registry; lower runs first).
    pub priority: u16,
    /// Target systems on which this spec is eligible to match.
    pub systems: &'static [TargetSystem],
    /// Optional upper bound on `image.load_addr` (e.g. `Some(0x0801)` or `Some(0x080B)`).
    pub load_addr_max: Option<u16>,
    /// Signatures that must ALL match before `resolve` (if any) is evaluated.
    pub signatures: &'static [Signature],
    /// Static hints used when `resolve` is `None`.
    pub hints: Hints,
    /// Static pre-emulation patches used when `resolve` is `None`.
    pub patches: &'static [MemoryPatch],
    /// Optional per-position detection escape hatch for computed operands, secondary scans,
    /// or position-dependent byte checks.
    ///
    /// Evaluated at *each candidate position* of `signatures[0].anchor` where all
    /// `signatures` match. Returning `None` advances to the next candidate position
    /// (ECA `continue` semantics, `T3.0.14`–`T3.0.15`).
    pub resolve: Option<ResolveFn>,
    /// Optional factory producing emulation-time [`PackerHooks`] (Ruling B3, `T3.0.36`).
    pub hooks: Option<HooksFactory>,
    /// Minimal synthetic payload bytes placed at `fixture_load_addr` for registry self-tests.
    pub fixture: &'static [u8],
    /// Load address at which `fixture` is placed in memory for self-tests.
    pub fixture_load_addr: u16,
}

impl PackerSpec {
    /// Evaluates whether this [`PackerSpec`] matches `image`.
    ///
    /// Checks:
    /// 1. `image.system` matches at least one entry in `self.systems` via [`system_matches`].
    /// 2. `image.load_addr <= max_load` if `self.load_addr_max` is `Some(max_load)`.
    /// 3. For each candidate position of `self.signatures[0]` where `signatures[0]` matches
    ///    and every secondary signature `self.signatures[1..]` also matches somewhere in
    ///    `image`, runs `self.resolve` (if `Some`). If `resolve` returns `None`, continues
    ///    scanning the remaining candidate positions of `signatures[0]`.
    #[must_use]
    pub fn detect(&self, image: &LoadedImage<'_>) -> Option<SignatureMatch> {
        if !self
            .systems
            .iter()
            .any(|allowed| system_matches(allowed, image.system))
        {
            return None;
        }

        if let Some(max_load) = self.load_addr_max
            && image.load_addr > max_load
        {
            return None;
        }

        let default_resolved = ResolvedSpec {
            name: Some(self.name),
            hints: self.hints,
            patches: self.patches,
        };

        let Some((first_sig, rest_sigs)) = self.signatures.split_first() else {
            // A spec with no declarative signatures relies exclusively on `resolve`.
            let resolve_fn = self.resolve?;
            let prelim = SignatureMatch {
                spec_id: self.id,
                spec_name: self.name,
                anchor_addr: image.load_addr,
                match_addrs: Vec::new(),
                resolved: default_resolved,
            };
            let resolved = resolve_fn(image, &prelim)?;
            return Some(SignatureMatch {
                spec_name: resolved.name.unwrap_or(self.name),
                resolved,
                ..prelim
            });
        };

        // Secondary signatures are independent of the primary anchor position, so we
        // only resolve them once after the first primary hit.
        let mut secondary_addrs: Option<Option<Vec<u16>>> = None;

        first_sig
            .anchor
            .find_position(image, first_sig.pattern.len(), |pos| {
                if !first_sig.matches_at(image.mem, pos) {
                    return None;
                }

                let sec_addrs = match &secondary_addrs {
                    Some(Some(addrs)) => addrs.as_slice(),
                    Some(None) => return None,
                    None => {
                        if rest_sigs.is_empty() {
                            secondary_addrs = Some(Some(Vec::new()));
                            &[]
                        } else {
                            let mut addrs = Vec::with_capacity(rest_sigs.len());
                            for sig in rest_sigs {
                                let Some(addr) = sig.find_first(image) else {
                                    secondary_addrs = Some(None);
                                    return None;
                                };
                                addrs.push(addr);
                            }
                            secondary_addrs.insert(Some(addrs)).as_deref()?
                        }
                    }
                };

                let mut match_addrs = Vec::with_capacity(1 + sec_addrs.len());
                match_addrs.push(pos);
                match_addrs.extend_from_slice(sec_addrs);

                let prelim = SignatureMatch {
                    spec_id: self.id,
                    spec_name: self.name,
                    anchor_addr: pos,
                    match_addrs,
                    resolved: default_resolved,
                };

                match self.resolve {
                    None => Some(prelim),
                    Some(resolve_fn) => {
                        let resolved = resolve_fn(image, &prelim)?;
                        Some(SignatureMatch {
                            spec_name: resolved.name.unwrap_or(self.name),
                            resolved,
                            ..prelim
                        })
                    }
                }
            })
    }

    /// Builds a 64 KB memory buffer with `self.fixture` loaded at `self.fixture_load_addr`,
    /// along with `(load_addr, load_end)` suitable for constructing a [`LoadedImage`].
    #[must_use]
    pub fn build_fixture_memory(&self) -> (Vec<u8>, u16, u16) {
        let mut mem = vec![0u8; 0x1_0000];
        let start = usize::from(self.fixture_load_addr);
        let copy_len = self.fixture.len().min(0x1_0000_usize.saturating_sub(start));
        mem[start..start + copy_len].copy_from_slice(&self.fixture[..copy_len]);
        let load_end = self.fixture_load_addr.wrapping_add(copy_len as u16);
        (mem, self.fixture_load_addr, load_end)
    }
}

// ===========================================================================
// Emulation-time hook facade (`PackerHooks`, `StepContext`, `RefineContext`)
// ===========================================================================

/// Value copy of the 6502 CPU registers exposed to [`StepContext`] and [`RefineContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Registers {
    /// Accumulator (`A`).
    pub a: u8,
    /// Index register `X`.
    pub x: u8,
    /// Index register `Y`.
    pub y: u8,
    /// Stack pointer (`SP`).
    pub sp: u8,
}

impl Registers {
    /// Returns the value of the requested [`Reg`].
    #[must_use]
    pub const fn get(self, reg: Reg) -> u8 {
        match reg {
            Reg::A => self.a,
            Reg::X => self.x,
            Reg::Y => self.y,
        }
    }
}

/// Encapsulated per-instruction context passed to [`PackerHooks::on_step`].
///
/// Provides a value copy of the CPU registers (`registers`), raw side-effect-free
/// RAM inspection ([`Self::peek`]), and phase-1 write-bitmap clearing
/// ([`Self::forget_writes`]) without exposing the `CPU` or `C64Bus` internals (Ruling B2).
pub struct StepContext<'a> {
    /// Current 6502 program counter.
    pub pc: u16,
    /// Current emulation stage ([`Phase::FindDepacker`] or [`Phase::Decompress`]).
    pub phase: Phase,
    /// Value copy of the CPU registers (`A`, `X`, `Y`, `SP`). Mutating this copy
    /// does not modify the emulated CPU (`T3.0.35`).
    pub registers: Registers,
    bus: &'a mut UnpackerMemory,
}

impl std::fmt::Debug for StepContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepContext")
            .field("pc", &self.pc)
            .field("phase", &self.phase)
            .field("registers", &self.registers)
            .finish_non_exhaustive()
    }
}

impl<'a> StepContext<'a> {
    /// Creates a [`StepContext`] from `bus`, `pc`, `phase`, and `registers`.
    #[must_use]
    pub fn new(bus: &'a mut UnpackerMemory, pc: u16, phase: Phase, registers: Registers) -> Self {
        Self {
            pc,
            phase,
            registers,
            bus,
        }
    }

    /// Creates a [`StepContext`] snapshot from the live `cpu` and `phase`.
    #[must_use]
    pub fn from_cpu(cpu: &'a mut CPU<UnpackerMemory, Nmos6502>, phase: Phase) -> Self {
        let pc = cpu.registers.program_counter;
        let registers = Registers {
            a: cpu.registers.accumulator,
            x: cpu.registers.index_x,
            y: cpu.registers.index_y,
            sp: cpu.registers.stack_pointer.0,
        };
        Self {
            pc,
            phase,
            registers,
            bus: &mut cpu.memory,
        }
    }

    /// Reads the raw RAM byte at `addr` directly from `C64Bus::mem` without going
    /// through `Bus::get_byte` (Ruling B2, `T3.0.32`–`T3.0.33`).
    ///
    /// Never advances `total_cycles`, never steps CIA1/CIA2 timers, and always reads
    /// underlying RAM even at ROM or I/O addresses.
    #[must_use]
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.mem.get(usize::from(addr)).copied().unwrap_or(0)
    }

    /// Reads a little-endian `u16` from raw RAM at `addr` and `addr.wrapping_add(1)`
    /// without side effects.
    #[must_use]
    pub fn peek_u16_le(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.peek(addr), self.peek(addr.wrapping_add(1))])
    }

    /// Clears write-tracking flags in `C64Bus::written` for every address in `range`,
    /// leaving `C64Bus::written_phase2` completely untouched (Ruling B2, `T3.0.34`).
    pub fn forget_writes(&mut self, range: RangeInclusive<u16>) {
        let start = usize::from(*range.start());
        let end = usize::from(*range.end());
        if start <= end {
            let clamped_end = end.min(self.bus.written.len().saturating_sub(1));
            if let Some(slice) = self.bus.written.get_mut(start..=clamped_end) {
                slice.fill(false);
            }
        }
    }

    /// Test-only accessor for `engine.rs` invariant unit tests (`T2.3.2`–`T2.3.4`).
    #[cfg(test)]
    pub(crate) fn bus_mut(&mut self) -> &mut UnpackerMemory {
        self.bus
    }
}

/// Read-only post-emulation state passed to [`PackerHooks::refine`].
#[derive(Debug, Clone, Copy)]
pub struct RefineContext<'a> {
    /// Final 64 KB emulated RAM image.
    pub mem: &'a [u8],
    /// Pre-emulation 64 KB RAM snapshot.
    pub snapshot: &'a [u8],
    /// Per-byte write tracking across both emulation phases (`written`).
    pub written: &'a [bool],
    /// Target machine system profile.
    pub system: &'a System,
    /// CPU registers at the moment emulation stopped.
    pub registers: Registers,
}

/// Mutable output range bounds and entry point refined by [`PackerHooks::refine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Inclusive start address of the unpacked output range.
    pub start_addr: u16,
    /// Inclusive end address of the unpacked output range.
    pub end_addr: u16,
    /// Resolved program entry point.
    pub entry_point: u16,
}

/// Optional emulation-time lifecycle hooks for a [`PackerSpec`].
///
/// Requires `Debug + Send` (`T3.0.37`), but intentionally does not require `Sync`
/// so hook state can live in a plain struct owned per unpack run.
pub trait PackerHooks: Debug + Send {
    /// Invoked once per instruction step during emulation.
    fn on_step(&mut self, _ctx: &mut StepContext<'_>) {}

    /// Invoked after emulation finishes to refine the output bounds or entry point.
    fn refine(&self, _ctx: &RefineContext<'_>, _bounds: &mut Bounds) {}
}

// ===========================================================================
// Unit Tests (`T3.0.1`–`T3.0.29`, `T3.0.31`–`T3.0.37`)
// ===========================================================================

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// T3.0.1: `Pat::Byte(v)` matches only the exact byte `v` across all 256 possible values.
    #[test]
    fn pat_byte_matches_only_exact_value() {
        for expected in 0u8..=u8::MAX {
            let pat = Pat::Byte(expected);
            for actual in 0u8..=u8::MAX {
                assert_eq!(
                    pat.matches(actual),
                    actual == expected,
                    "expected={expected:#04X}, actual={actual:#04X}"
                );
            }
        }
    }

    /// T3.0.2: `Pat::Any` matches all 256 byte values.
    #[test]
    fn pat_any_matches_all_values() {
        for actual in 0u8..=u8::MAX {
            assert!(Pat::Any.matches(actual), "actual={actual:#04X}");
        }
    }

    /// T3.0.3: `Pat::Mask` exhaustively matches iff `(byte & mask) == eq`, including TBC's `& 0xFD == 0x84`.
    #[test]
    fn pat_mask_exhaustive() {
        // TBC Multicompactor mask (`tbc.rs:10`): `(b & 0xFD) == 0x84` matches only 0x84 and 0x86.
        let tbc = Pat::Mask {
            mask: 0xFD,
            eq: 0x84,
        };
        let matched: Vec<u8> = (0u8..=u8::MAX).filter(|&b| tbc.matches(b)).collect();
        assert_eq!(matched, vec![0x84, 0x86]);

        // Zero mask with eq 0 matches everything; impossible eq (bits outside mask) matches nothing.
        let all = Pat::Mask {
            mask: 0x00,
            eq: 0x00,
        };
        let impossible = Pat::Mask {
            mask: 0xFD,
            eq: 0x86,
        };
        for b in 0u8..=u8::MAX {
            assert!(all.matches(b));
            assert!(!impossible.matches(b));
        }
    }

    /// T3.0.4: `Pat::OneOf` matches iff the byte is in the candidate slice (e.g. Exomizer `0x34 | 0x50`).
    #[test]
    fn pat_one_of_exhaustive() {
        let exo = Pat::OneOf(&[0x34, 0x50]);
        let cruel = Pat::OneOf(&[0x1A, 0x1F, 0x20]);
        for b in 0u8..=u8::MAX {
            assert_eq!(exo.matches(b), b == 0x34 || b == 0x50, "byte={b:#04X}");
            assert_eq!(
                cruel.matches(b),
                b == 0x1A || b == 0x1F || b == 0x20,
                "byte={b:#04X}"
            );
        }
    }

    /// T3.0.5: `Pat::OneOf(&[])` never matches any byte value.
    #[test]
    fn pat_one_of_empty_never_matches() {
        let empty = Pat::OneOf(&[]);
        for b in 0u8..=u8::MAX {
            assert!(!empty.matches(b), "byte={b:#04X}");
        }
    }

    /// T3.0.6: `sig!` expands byte literals, `_`/`__` wildcards, `[...]` alternatives, and `(mask, eq)` pairs.
    #[test]
    fn sig_macro_expands_bytes_and_wildcards() {
        const EAGLE: &[Pat] = sig![0x78 0xA2 __ 0xBD _ __ 0x9D 0x00 0x01 0xD0 0xFC 0x4C 0x00 0x01];
        assert_eq!(
            EAGLE,
            &[
                Pat::Byte(0x78),
                Pat::Byte(0xA2),
                Pat::Any,
                Pat::Byte(0xBD),
                Pat::Any,
                Pat::Any,
                Pat::Byte(0x9D),
                Pat::Byte(0x00),
                Pat::Byte(0x01),
                Pat::Byte(0xD0),
                Pat::Byte(0xFC),
                Pat::Byte(0x4C),
                Pat::Byte(0x00),
                Pat::Byte(0x01),
            ]
        );

        const MIXED: &[Pat] = sig![0x78, [0x34, 0x37], (0xFD, 0x84), __];
        assert_eq!(
            MIXED,
            &[
                Pat::Byte(0x78),
                Pat::OneOf(&[0x34, 0x37]),
                Pat::Mask {
                    mask: 0xFD,
                    eq: 0x84
                },
                Pat::Any,
            ]
        );
    }

    /// T3.0.7: A signature extending past the end of `mem` or across `$FFFF` neither matches nor panics.
    #[test]
    fn signature_past_end_of_memory_does_not_match_or_panic() {
        let sig = Signature {
            anchor: Anchor::Absolute(0x080D),
            pattern: sig![0x78, 0xA2, 0x00, 0xBD],
        };

        // Short buffer ending before 0x080D + 4.
        let short = vec![0x78; 0x080F];
        let image_short = LoadedImage::for_c64(&short, 0x0801, 0x080F);
        assert!(sig.find_first(&image_short).is_none());

        // 64 KB buffer with signature anchored at $FFFE requiring 4 bytes (crosses $FFFF).
        let mut full = vec![0u8; 0x1_0000];
        full[0xFFFE] = 0x78;
        full[0xFFFF] = 0xA2;
        let wrap_sig = Signature {
            anchor: Anchor::Absolute(0xFFFE),
            pattern: sig![0x78, 0xA2, 0x00, 0x00],
        };
        let image_full = LoadedImage::for_c64(&full, 0x0801, 0xFFFF);
        assert!(wrap_sig.find_first(&image_full).is_none());
    }

    /// T3.0.8: `Anchor::BasicStart` resolves relative to `system.default_basic_start()` on each system.
    #[test]
    fn anchor_basic_start_offset_per_system() {
        let sig = Signature {
            anchor: Anchor::BasicStart { offset: 0x0C },
            pattern: sig![0x78, 0xA9, 0x34],
        };

        let cases = [
            (TargetSystem::C64, 0x080D_u16),
            (TargetSystem::C128, 0x1C0D_u16),
            (TargetSystem::Vic20, 0x100D_u16),
            (TargetSystem::Plus4, 0x100D_u16),
            (TargetSystem::C16, 0x100D_u16),
            (TargetSystem::Pet20, 0x040D_u16),
            (TargetSystem::Pet40, 0x040D_u16),
            (TargetSystem::C1541, 0x000C_u16),
        ];

        for (system, expected_addr) in cases {
            let mut mem = vec![0u8; 0x1_0000];
            let idx = usize::from(expected_addr);
            mem[idx..idx + 3].copy_from_slice(&[0x78, 0xA9, 0x34]);
            let image = LoadedImage::new(&mem, system.default_basic_start(), 0x2000, &system);
            assert_eq!(sig.find_first(&image), Some(expected_addr), "{system}");

            // Placing bytes only at C64's $080D must not match on systems with a different BASIC start.
            if expected_addr != 0x080D {
                let mut wrong_mem = vec![0u8; 0x1_0000];
                wrong_mem[0x080D..0x0810].copy_from_slice(&[0x78, 0xA9, 0x34]);
                let wrong_img =
                    LoadedImage::new(&wrong_mem, system.default_basic_start(), 0x2000, &system);
                assert_eq!(sig.find_first(&wrong_img), None, "{system}");
            }
        }

        // Overflowing `default_basic_start() + offset` yields no match and does not panic.
        let overflow_sig = Signature {
            anchor: Anchor::BasicStart { offset: 0xFFFF },
            pattern: sig![0x00],
        };
        let mem = vec![0u8; 0x1_0000];
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x1000);
        assert_eq!(overflow_sig.find_first(&image), None);
    }

    /// T3.0.9: `Anchor::Absolute` matches only at the exact fixed address.
    #[test]
    fn anchor_absolute() {
        let sig = Signature {
            anchor: Anchor::Absolute(0x0810),
            pattern: sig![0x78, 0xA9, 0x34],
        };
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080F..0x0812].copy_from_slice(&[0x78, 0xA9, 0x34]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x1000);
        assert_eq!(sig.find_first(&image), None);

        mem.fill(0);
        mem[0x0810..0x0813].copy_from_slice(&[0x78, 0xA9, 0x34]);
        let image = LoadedImage::for_c64(&mem, 0x1000, 0x2000);
        assert_eq!(sig.find_first(&image), Some(0x0810));
    }

    /// T3.0.10: `Anchor::Window(0x0810..=0x0840)` includes both endpoints and scans ascending.
    #[test]
    fn anchor_window_bounds_inclusive_like_mc_cracken() {
        let sig = Signature {
            anchor: Anchor::Window(0x0810..=0x0840),
            pattern: sig![0xA9, __, 0x85, __, 0xA0, 0x00, 0xC6, 0xAF],
        };
        let bytes = [0xA9, 0x01, 0x85, 0x02, 0xA0, 0x00, 0xC6, 0xAF];

        for (pos, expected) in [
            (0x080F_usize, None),
            (0x0810_usize, Some(0x0810)),
            (0x0828_usize, Some(0x0828)),
            (0x0840_usize, Some(0x0840)),
            (0x0841_usize, None),
        ] {
            let mut mem = vec![0u8; 0x1_0000];
            mem[pos..pos + bytes.len()].copy_from_slice(&bytes);
            let image = LoadedImage::for_c64(&mem, 0x0801, 0x1000);
            assert_eq!(sig.find_first(&image), expected, "pos={pos:#06X}");
        }
    }

    /// T3.0.11: `Anchor::ScanFromLoad` scans forward from `load_addr` up to `load_addr + max_offset`.
    #[test]
    fn anchor_scan_from_load() {
        let sig = Signature {
            anchor: Anchor::ScanFromLoad { max_offset: 0x20 },
            pattern: sig![0xDE, 0xAD, 0xBE, 0xEF],
        };
        let pat = [0xDE, 0xAD, 0xBE, 0xEF];

        for (offset, expected) in [
            (0x00_usize, Some(0x0801)),
            (0x10_usize, Some(0x0811)),
            (0x20_usize, Some(0x0821)),
            (0x21_usize, None),
        ] {
            let mut mem = vec![0u8; 0x1_0000];
            let addr = 0x0801 + offset;
            mem[addr..addr + 4].copy_from_slice(&pat);
            let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
            assert_eq!(sig.find_first(&image), expected, "offset={offset:#04X}");
        }
    }

    /// T3.0.12: `Anchor::ScanBackFromEnd` scans descending from `load_end - pattern.len()`.
    #[test]
    fn anchor_scan_back_from_end_descending() {
        let sig = Signature {
            anchor: Anchor::ScanBackFromEnd { max_offset: 0x80 },
            pattern: sig![0xC8, 0xC0, [0x34, 0x50], 0xD0],
        };
        let mut mem = vec![0u8; 0x1_0000];
        // Place matches at both $0890 and $08D0 within the backward window from load_end = $0900.
        mem[0x0890..0x0894].copy_from_slice(&[0xC8, 0xC0, 0x34, 0xD0]);
        mem[0x08D0..0x08D4].copy_from_slice(&[0xC8, 0xC0, 0x50, 0xD0]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        // Descending scan finds the higher address ($08D0) first.
        assert_eq!(sig.find_first(&image), Some(0x08D0));

        // A match further back than `max_offset` ($08FD - $80 = $087D) is not reached.
        mem.fill(0);
        mem[0x0850..0x0854].copy_from_slice(&[0xC8, 0xC0, 0x34, 0xD0]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert_eq!(sig.find_first(&image), None);
    }

    /// T3.0.13: `Anchor::Candidates` probes candidate anchors in exact slice order.
    #[test]
    fn anchor_candidates_in_order() {
        const CANDIDATES: [Anchor; 2] = [Anchor::Absolute(0x080D), Anchor::Absolute(0x080F)];
        let sig = Signature {
            anchor: Anchor::Candidates(&CANDIDATES),
            pattern: sig![0x78, 0xA2],
        };

        // Only second candidate present -> matches 0x080F.
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080F..0x0811].copy_from_slice(&[0x78, 0xA2]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert_eq!(sig.find_first(&image), Some(0x080F));

        // Both candidates present -> first candidate (0x080D) wins.
        mem[0x080D..0x080F].copy_from_slice(&[0x78, 0xA2]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert_eq!(sig.find_first(&image), Some(0x080D));
    }

    /// T3.0.14: When `resolve` returns `None` at the first matching anchor position, scanning
    /// continues to the next matching position in the anchor window (ECA `continue` semantics).
    #[test]
    fn resolve_none_moves_to_next_position() {
        fn eca_like_resolve(
            image: &LoadedImage<'_>,
            sig_match: &SignatureMatch,
        ) -> Option<ResolvedSpec> {
            let p = usize::from(sig_match.anchor_addr);
            // Position-dependent check: require mem[p + 4] == (p & 0xFF) as u8.
            if image.read_u8(p + 4)? == (p & 0xFF) as u8 {
                Some(ResolvedSpec::from_hints(Hints {
                    dep_addr: Some(AddrSource::Const(sig_match.anchor_addr)),
                    ..Hints::NONE
                }))
            } else {
                None
            }
        }

        const SIGS: [Signature; 1] = [Signature {
            anchor: Anchor::Window(0x080D..=0x0830),
            pattern: sig![0x2A, 0x2A, 0x2A, 0x2A],
        }];
        let spec = PackerSpec {
            id: "test.eca_continue",
            name: "Test ECA",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: Some(eca_like_resolve),
            hooks: None,
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        let mut mem = vec![0u8; 0x1_0000];
        // Position $0810 matches the 4-byte signature, but fails position-dependent byte ($FF != $10).
        mem[0x0810..0x0814].copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
        mem[0x0814] = 0xFF;
        // Position $0822 matches both the signature and the position-dependent byte ($22).
        mem[0x0822..0x0826].copy_from_slice(&[0x2A, 0x2A, 0x2A, 0x2A]);
        mem[0x0826] = 0x22;

        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let matched = spec.detect(&image).expect("second candidate must match");
        assert_eq!(matched.anchor_addr, 0x0822);
        assert_eq!(
            matched.resolved.hints.dep_addr,
            Some(AddrSource::Const(0x0822))
        );
    }

    /// T3.0.15: When `resolve` returns `None` at every matching anchor position, the spec is rejected.
    #[test]
    fn resolve_none_everywhere_rejects_spec() {
        fn always_none(_: &LoadedImage<'_>, _: &SignatureMatch) -> Option<ResolvedSpec> {
            None
        }

        const SIGS: [Signature; 1] = [Signature {
            anchor: Anchor::Window(0x080D..=0x0820),
            pattern: sig![0x2A, 0x2A],
        }];
        let spec = PackerSpec {
            id: "test.reject",
            name: "Test Reject",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: Some(always_none),
            hooks: None,
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0810..0x0812].copy_from_slice(&[0x2A, 0x2A]);
        mem[0x0818..0x081A].copy_from_slice(&[0x2A, 0x2A]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert!(spec.detect(&image).is_none());
    }

    /// T3.0.16: `resolve` runs only after ALL declarative `signatures` match.
    #[test]
    fn resolve_runs_only_after_all_signatures_match() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn counting_resolve(_: &LoadedImage<'_>, m: &SignatureMatch) -> Option<ResolvedSpec> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Some(m.resolved)
        }

        const SIGS: [Signature; 2] = [
            Signature {
                anchor: Anchor::Absolute(0x080D),
                pattern: sig![0x78, 0xA9],
            },
            Signature {
                anchor: Anchor::Absolute(0x0820),
                pattern: sig![0x4C, 0x00, 0x01],
            },
        ];
        let spec = PackerSpec {
            id: "test.all_sigs_before_resolve",
            name: "Test",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: Some(counting_resolve),
            hooks: None,
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        CALLS.store(0, Ordering::SeqCst);
        let mut mem = vec![0u8; 0x1_0000];
        // Only signature 0 matches -> resolve must not be called.
        mem[0x080D..0x080F].copy_from_slice(&[0x78, 0xA9]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert!(spec.detect(&image).is_none());
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);

        // Both signatures match -> resolve is called exactly once.
        mem[0x0820..0x0823].copy_from_slice(&[0x4C, 0x00, 0x01]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert!(spec.detect(&image).is_some());
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    /// T3.0.17: A spec with multiple `Signature`s requires every one of them to match.
    #[test]
    fn multiple_signatures_all_required() {
        const SIGS: [Signature; 3] = [
            Signature {
                anchor: Anchor::Absolute(0x080D),
                pattern: sig![0x78, 0xA0, 0x00],
            },
            Signature {
                anchor: Anchor::Absolute(0x081D),
                pattern: sig![0x4C, 0x00, 0xFF],
            },
            Signature {
                anchor: Anchor::Absolute(0x082D),
                pattern: sig![0xA5, 0xAF, 0xC9],
            },
        ];
        let spec = PackerSpec {
            id: "test.multi_sig",
            name: "Multi Signature Packer",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080D..0x0810].copy_from_slice(&[0x78, 0xA0, 0x00]);
        mem[0x081D..0x0820].copy_from_slice(&[0x4C, 0x00, 0xFF]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert!(spec.detect(&image).is_none());

        mem[0x082D..0x0830].copy_from_slice(&[0xA5, 0xAF, 0xC9]);
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let m = spec.detect(&image).expect("all 3 signatures present");
        assert_eq!(m.anchor_addr, 0x080D);
        assert_eq!(m.match_addrs, vec![0x080D, 0x081D, 0x082D]);
        assert_eq!(m.signature_addr(1), Some(0x081D));
    }

    /// T3.0.18: Hints and name returned by `resolve` replace the static fields on `PackerSpec`.
    #[test]
    fn resolve_hints_replace_static_hints() {
        const PATCHES: [MemoryPatch; 1] = [MemoryPatch {
            addr: PatchAddr::Absolute(0x080B),
            expected: None,
            bytes: &[0xA2],
        }];
        fn custom_resolve(_: &LoadedImage<'_>, _: &SignatureMatch) -> Option<ResolvedSpec> {
            Some(ResolvedSpec {
                name: Some("Dynamic Variant"),
                hints: Hints {
                    dep_addr: Some(AddrSource::Const(0x0134)),
                    entry_point: Some(AddrSource::Const(0x2000)),
                    exact_return: true,
                    ..Hints::NONE
                },
                patches: &PATCHES,
            })
        }

        const SIGS: [Signature; 1] = [Signature {
            anchor: Anchor::Absolute(0x080D),
            pattern: sig![0x78],
        }];
        let spec = PackerSpec {
            id: "test.replace_hints",
            name: "Static Name",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints {
                dep_addr: Some(AddrSource::Const(0x0100)),
                start_addr: Some(AddrSource::BasicStart(0)),
                ..Hints::NONE
            },
            patches: &[],
            resolve: Some(custom_resolve),
            hooks: None,
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080D] = 0x78;
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let m = spec.detect(&image).expect("matches");
        assert_eq!(m.spec_name, "Dynamic Variant");
        assert_eq!(m.resolved.hints.dep_addr, Some(AddrSource::Const(0x0134)));
        assert_eq!(m.resolved.hints.start_addr, None);
        assert_eq!(
            m.resolved.hints.entry_point,
            Some(AddrSource::Const(0x2000))
        );
        assert!(m.resolved.hints.exact_return);
        assert_eq!(m.resolved.patches, &PATCHES);
    }

    /// T3.0.19: `AddrSource::BasicStart(i16)` resolves signed offsets from `default_basic_start()` (Ruling B4).
    #[test]
    fn addr_source_basic_start_signed_offsets() {
        let mem = [0u8; 16];
        let c64 = LoadedImage::for_c64(&mem, 0x0801, 0x0810);
        assert_eq!(
            AddrSource::BasicStart(-1).resolve_static(&c64, 0x080D),
            Some(0x0800)
        );
        assert_eq!(
            AddrSource::BasicStart(0).resolve_static(&c64, 0x080D),
            Some(0x0801)
        );
        assert_eq!(
            AddrSource::BasicStart(0x0C).resolve_static(&c64, 0x080D),
            Some(0x080D)
        );
        assert_eq!(
            AddrSource::BasicStart(-0x0801).resolve_static(&c64, 0x080D),
            Some(0x0000)
        );

        let c128 = LoadedImage::new(&mem, 0x1C01, 0x1C10, &TargetSystem::C128);
        assert_eq!(
            AddrSource::BasicStart(-1).resolve_static(&c128, 0x1C0D),
            Some(0x1C00)
        );
        assert_eq!(
            AddrSource::BasicStart(0).resolve_static(&c128, 0x1C0D),
            Some(0x1C01)
        );
    }

    /// T3.0.20: `AddrSource::BasicStart(i16)` returns `None` when the signed offset underflows `< 0` or overflows `> $FFFF`.
    #[test]
    fn addr_source_basic_start_out_of_range_is_none() {
        let mem = [0u8; 16];
        // C64 default_basic_start is $0801; -0x0802 underflows below $0000.
        let c64 = LoadedImage::for_c64(&mem, 0x0801, 0x0810);
        assert_eq!(
            AddrSource::BasicStart(-0x0802).resolve_static(&c64, 0x080D),
            None
        );
        assert_eq!(
            AddrSource::BasicStart(i16::MIN).resolve_static(&c64, 0x080D),
            None
        );

        // Drive systems have default_basic_start == $0000; any negative offset underflows.
        let c1541 = LoadedImage::new(&mem, 0x0000, 0x0010, &TargetSystem::C1541);
        assert_eq!(
            AddrSource::BasicStart(-1).resolve_static(&c1541, 0x0000),
            None
        );
        assert_eq!(
            AddrSource::BasicStart(0).resolve_static(&c1541, 0x0000),
            Some(0x0000)
        );
    }

    /// T3.0.21: `AddrSource::Const` returns its constant 16-bit address.
    #[test]
    fn addr_source_const() {
        let mem = [0u8; 16];
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0810);
        for addr in [0x0000_u16, 0x0002, 0x0100, 0x0801, 0xFF00, 0xFFFF] {
            assert_eq!(
                AddrSource::Const(addr).resolve_static(&image, 0x080D),
                Some(addr)
            );
        }
    }

    /// T3.0.22: `AddrSource::OperandLe` reads a little-endian word at `anchor_addr + from_match`.
    #[test]
    fn addr_source_operand_little_endian() {
        let mut mem = vec![0u8; 0x1_0000];
        // TinyCrunch entry at $080D + 0x29 = $0836: $0911 (LE: 11 09).
        mem[0x0836] = 0x11;
        mem[0x0837] = 0x09;
        // Negative offset at $080D - 5 = $0808: $1234 (LE: 34 12).
        mem[0x0808] = 0x34;
        mem[0x0809] = 0x12;

        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        assert_eq!(
            AddrSource::OperandLe { from_match: 0x29 }.resolve_static(&image, 0x080D),
            Some(0x0911)
        );
        assert_eq!(
            AddrSource::OperandLe { from_match: -5 }.resolve_static(&image, 0x080D),
            Some(0x1234)
        );
        assert_eq!(
            AddrSource::AbsoluteLe(0x0836).resolve_static(&image, 0x080D),
            Some(0x0911)
        );

        // Out-of-bounds negative and past-end offsets return None.
        assert_eq!(
            AddrSource::OperandLe {
                from_match: -0x0900
            }
            .resolve_static(&image, 0x080D),
            None
        );
        assert_eq!(
            AddrSource::OperandLe { from_match: 0 }.resolve_static(&image, 0xFFFF),
            None
        );
    }

    /// T3.0.23: `AddrSource::PointerAfterRun` reads LE word with `wrapping_add(1)` for high byte and applies `adjust`.
    #[test]
    fn pointer_after_run_reads_le_with_adjust_and_wraps() {
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x00FA] = 0x00;
        mem[0x00FB] = 0x32; // $3200
        let monitor = PointerMonitor::new();

        let src_minus_1 = AddrSource::PointerAfterRun {
            ptr_addr: 0x00FA,
            adjust: -1,
            guard: PointerGuard::None,
        };
        assert_eq!(
            src_minus_1.resolve_post_run(&mem, Registers::default(), 0x0800, 0x31FF, &monitor),
            Some(0x31FF)
        );

        let src_plus_2 = AddrSource::PointerAfterRun {
            ptr_addr: 0x00FA,
            adjust: 2,
            guard: PointerGuard::None,
        };
        assert_eq!(
            src_plus_2.resolve_post_run(&mem, Registers::default(), 0x0800, 0x31FF, &monitor),
            Some(0x3202)
        );

        // Wrapping at $FFFF: low byte at $FFFF, high byte at $0000 (Ruling R19).
        mem[0xFFFF] = 0x34;
        mem[0x0000] = 0x12; // $1234
        let src_wrap = AddrSource::PointerAfterRun {
            ptr_addr: 0xFFFF,
            adjust: -1,
            guard: PointerGuard::None,
        };
        assert_eq!(
            src_wrap.resolve_post_run(&mem, Registers::default(), 0x0800, 0x1233, &monitor),
            Some(0x1233)
        );
    }

    /// T3.0.24: `PointerGuard::NearDetectedEnd` enforces `raw > start_addr` and `adjusted <= detected_end + slack`.
    #[test]
    fn pointer_guard_near_detected_end() {
        let mut mem = vec![0u8; 0x1_0000];
        let monitor = PointerMonitor::new();
        let src = AddrSource::PointerAfterRun {
            ptr_addr: 0x00FA,
            adjust: -1,
            guard: PointerGuard::NearDetectedEnd { slack: 512 },
        };

        // Within slack: detected_end = $3000, slack = $0200 -> max adjusted = $3200 (raw = $3201).
        mem[0x00FA] = 0x01;
        mem[0x00FB] = 0x32;
        assert_eq!(
            src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x3000, &monitor),
            Some(0x3200)
        );

        // 1 byte beyond slack (raw = $3202, adjusted = $3201 > $3200) -> rejected.
        mem[0x00FA] = 0x02;
        mem[0x00FB] = 0x32;
        assert_eq!(
            src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x3000, &monitor),
            None
        );

        // At or below start_addr (raw = $0800) -> rejected.
        mem[0x00FA] = 0x00;
        mem[0x00FB] = 0x08;
        assert_eq!(
            src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x3000, &monitor),
            None
        );
    }

    /// T3.0.25: `PointerGuard::McCrackenEndOfProgram` matches legacy `mc_cracken.rs:42-45` (`start_addr < raw < $FFFF`).
    #[test]
    fn pointer_guard_mccracken_matches_legacy() {
        let mut mem = vec![0u8; 0x1_0000];
        let monitor = PointerMonitor::new();
        let src = AddrSource::PointerAfterRun {
            ptr_addr: 0x00AE,
            adjust: -1,
            guard: PointerGuard::McCrackenEndOfProgram,
        };

        for (raw, start_addr, expected) in [
            (0x0000_u16, 0x0800_u16, None),
            (0x0800_u16, 0x0800_u16, None),
            (0x0801_u16, 0x0800_u16, Some(0x0800)),
            (0x9D1A_u16, 0x0800_u16, Some(0x9D19)),
            (0xFFFE_u16, 0x0800_u16, Some(0xFFFD)),
            (0xFFFF_u16, 0x0800_u16, None),
        ] {
            let [lo, hi] = raw.to_le_bytes();
            mem[0x00AE] = lo;
            mem[0x00AF] = hi;
            assert_eq!(
                src.resolve_post_run(&mem, Registers::default(), start_addr, 0x1000, &monitor),
                expected,
                "raw={raw:#06X}, start={start_addr:#06X}"
            );
        }
    }

    /// T3.0.26: `AddrSource::PointerPlusRegister` adds the selected register and signed `adjust`.
    #[test]
    fn pointer_plus_register() {
        let mut mem = vec![0u8; 0x1_0000];
        mem[0x00FE] = 0xF0;
        mem[0x00FF] = 0x07; // $07F0
        let monitor = PointerMonitor::new();
        let regs = Registers {
            a: 0x05,
            x: 0x08,
            y: 0x10,
            sp: 0xF6,
        };

        let exo30 = AddrSource::PointerPlusRegister {
            ptr_addr: 0x00FE,
            reg: Reg::Y,
            adjust: 0,
        };
        assert_eq!(
            exo30.resolve_post_run(&mem, regs, 0x0800, 0x2000, &monitor),
            Some(0x0800)
        );

        let exo32 = AddrSource::PointerPlusRegister {
            ptr_addr: 0x00FE,
            reg: Reg::Y,
            adjust: 1,
        };
        assert_eq!(
            exo32.resolve_post_run(&mem, regs, 0x0800, 0x2000, &monitor),
            Some(0x0801)
        );

        let with_x = AddrSource::PointerPlusRegister {
            ptr_addr: 0x00FE,
            reg: Reg::X,
            adjust: -4,
        };
        assert_eq!(
            with_x.resolve_post_run(&mem, regs, 0x0800, 0x2000, &monitor),
            Some(0x07F4)
        );
    }

    /// T3.0.27: `AddrSource::PointerAtDepacker` samples only the first hit at the depacker PC.
    #[test]
    fn pointer_at_depacker_samples_first_hit_only() {
        let mut mem = vec![0u8; 0x1_0000];
        let mut monitor = PointerMonitor::new();
        let src = AddrSource::PointerAtDepacker {
            ptr_addr: 0x002D,
            adjust: -1,
        };

        // Before any hit -> None.
        assert_eq!(
            src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            None
        );

        // First hit at depacker: $4000.
        mem[0x002D] = 0x00;
        mem[0x002E] = 0x40;
        monitor.sample_at_depacker(&mem, 0x002D);

        // Subsequent hits change $002D/$002E to $5000 and $6000, which must be ignored.
        mem[0x002D] = 0x00;
        mem[0x002E] = 0x50;
        monitor.sample_at_depacker(&mem, 0x002D);
        mem[0x002E] = 0x60;
        monitor.sample_at_depacker(&mem, 0x002D);

        assert_eq!(
            src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            Some(0x3FFF)
        );
    }

    /// T3.0.28: `AddrSource::MonitoredMax` and `MonitoredMin` track extrema across depacker hits.
    #[test]
    fn monitored_max_and_min() {
        let mut mem = vec![0u8; 0x1_0000];
        let mut monitor = PointerMonitor::new();
        let max_src = AddrSource::MonitoredMax { ptr_addr: 0x00AE };
        let min_src = AddrSource::MonitoredMin { ptr_addr: 0x00AE };

        assert_eq!(
            max_src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            None
        );
        assert_eq!(
            min_src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            None
        );

        for sample in [0x2000_u16, 0x0850, 0x9D19, 0x1100, 0x4500] {
            let [lo, hi] = sample.to_le_bytes();
            mem[0x00AE] = lo;
            mem[0x00AF] = hi;
            monitor.sample_at_depacker(&mem, 0x00AE);
        }

        assert_eq!(
            max_src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            Some(0x9D19)
        );
        assert_eq!(
            min_src.resolve_post_run(&mem, Registers::default(), 0x0800, 0x2000, &monitor),
            Some(0x0850)
        );
    }

    /// T3.0.29: `Hints::NONE` has all `Option` fields set to `None` and `exact_return == false`.
    #[test]
    fn hints_none_is_all_none() {
        let none = Hints::NONE;
        assert_eq!(none.dep_addr, None);
        assert_eq!(none.start_addr, None);
        assert_eq!(none.end_addr, None);
        assert_eq!(none.entry_point, None);
        assert_eq!(none.forced_entry, None);
        assert!(!none.exact_return);
        assert_eq!(Hints::default(), Hints::NONE);
    }

    /// T3.0.31: `PackerSpec` systems gating uses `System::is_c64()`, not `PartialEq`.
    #[test]
    fn systems_gate_uses_is_c64_predicate() {
        const SIGS: [Signature; 1] = [Signature {
            anchor: Anchor::Absolute(0x080D),
            pattern: sig![0x78],
        }];
        let spec = PackerSpec {
            id: "test.c64_only",
            name: "C64 Only",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0x78],
            fixture_load_addr: 0x080D,
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080D] = 0x78;

        // Standard C64 matches.
        let c64_img = LoadedImage::new(&mem, 0x0801, 0x0900, &TargetSystem::C64);
        assert!(spec.detect(&c64_img).is_some());

        // Custom("Commodore 64") equals TargetSystem::C64 under PartialEq, but `is_c64()` is false.
        let custom_c64 = TargetSystem::Custom("Commodore 64".into());
        assert_eq!(custom_c64, TargetSystem::C64);
        assert!(!custom_c64.is_c64());
        let custom_img = LoadedImage::new(&mem, 0x0801, 0x0900, &custom_c64);
        assert!(spec.detect(&custom_img).is_none());
    }

    /// T3.0.32: `StepContext::peek` reads raw `C64Bus::mem` even when ROM or I/O is banked in (Ruling B2).
    #[test]
    fn step_context_peek_reads_raw_ram() {
        let basic_rom = Some(vec![0xAA; 0x2000]);
        let kernal_rom = Some(vec![0xBB; 0x2000]);
        let char_rom = Some(vec![0xCC; 0x1000]);
        let mut bus = UnpackerMemory::new(TargetSystem::C64, basic_rom, kernal_rom, char_rom);
        // Bank in BASIC ROM ($A000), KERNAL ROM ($E000), and I/O ($D000).
        bus.mem[0x00] = 0x2F;
        bus.mem[0x01] = 0x37;
        // Write distinct values to underlying RAM under ROM and I/O ranges.
        bus.mem[0xA000] = 0x11;
        bus.mem[0xD012] = 0x22;
        bus.mem[0xDC04] = 0x33;
        bus.mem[0xE000] = 0x44;
        bus.mem[0xFFFF] = 0x55;

        let ctx = StepContext::new(&mut bus, 0x080D, Phase::Decompress, Registers::default());
        assert_eq!(ctx.peek(0xA000), 0x11);
        assert_eq!(ctx.peek(0xD012), 0x22);
        assert_eq!(ctx.peek(0xDC04), 0x33);
        assert_eq!(ctx.peek(0xE000), 0x44);
        assert_eq!(ctx.peek(0xFFFF), 0x55);
    }

    /// T3.0.33: `StepContext::peek` does not advance `total_cycles` or CIA timers (Ruling B2).
    #[test]
    fn step_context_peek_does_not_advance_cycles_or_cia() {
        let mut bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
        bus.mem[0x00] = 0x2F;
        bus.mem[0x01] = 0x37;
        bus.total_cycles = 1234;
        let cia1_before = format!("{:?}", bus.cia1);
        let cia2_before = format!("{:?}", bus.cia2);

        let ctx = StepContext::new(&mut bus, 0x0100, Phase::Decompress, Registers::default());
        for addr in [
            0x0000_u16, 0x00FE, 0x0100, 0x080D, 0xA000, 0xD011, 0xD012, 0xDC04, 0xDC0D, 0xDD04,
            0xE000, 0xFFFF,
        ] {
            let _ = ctx.peek(addr);
        }

        assert_eq!(bus.total_cycles, 1234);
        assert_eq!(format!("{:?}", bus.cia1), cia1_before);
        assert_eq!(format!("{:?}", bus.cia2), cia2_before);
    }

    /// T3.0.34: `StepContext::forget_writes` clears `written` in `range` and never touches `written_phase2` (Ruling B2).
    #[test]
    fn forget_writes_clears_written_only_in_range() {
        let mut bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
        bus.written.fill(true);
        bus.written_phase2.fill(true);

        {
            let mut ctx =
                StepContext::new(&mut bus, 0x0021, Phase::Decompress, Registers::default());
            ctx.forget_writes(0x4000..=0x7FFF);
        }

        for addr in 0..0x1_0000 {
            let in_forgotten = (0x4000..=0x7FFF).contains(&addr);
            assert_eq!(bus.written[addr], !in_forgotten, "written[{addr:#06X}]");
            assert!(
                bus.written_phase2[addr],
                "written_phase2[{addr:#06X}] must never be cleared"
            );
        }
    }

    /// T3.0.35: `StepContext::registers` is a value copy; mutating it does not mutate the CPU.
    #[test]
    fn step_context_registers_are_a_copy() {
        let bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
        let mut cpu = CPU::new(bus, Nmos6502);
        cpu.registers.program_counter = 0x080D;
        cpu.registers.accumulator = 0x12;
        cpu.registers.index_x = 0x34;
        cpu.registers.index_y = 0x56;
        cpu.registers.stack_pointer.0 = 0xF6;

        {
            let mut ctx = StepContext::from_cpu(&mut cpu, Phase::FindDepacker);
            assert_eq!(
                ctx.registers,
                Registers {
                    a: 0x12,
                    x: 0x34,
                    y: 0x56,
                    sp: 0xF6,
                }
            );
            ctx.registers.a = 0xFF;
            ctx.registers.x = 0xEE;
            ctx.registers.y = 0xDD;
            ctx.registers.sp = 0x00;
            ctx.pc = 0xFFFF;
            assert_eq!(ctx.registers.a, 0xFF);
            assert_eq!(ctx.registers.x, 0xEE);
            assert_eq!(ctx.registers.y, 0xDD);
            assert_eq!(ctx.registers.sp, 0x00);
            assert_eq!(ctx.pc, 0xFFFF);
        }

        assert_eq!(cpu.registers.program_counter, 0x080D);
        assert_eq!(cpu.registers.accumulator, 0x12);
        assert_eq!(cpu.registers.index_x, 0x34);
        assert_eq!(cpu.registers.index_y, 0x56);
        assert_eq!(cpu.registers.stack_pointer.0, 0xF6);
    }

    /// T3.0.36: The `hooks` factory receives `(&LoadedImage, &SignatureMatch)` and can inspect image bytes (Ruling B3).
    #[test]
    fn hooks_factory_receives_image() {
        #[derive(Debug)]
        struct RelocHook {
            reloc_start: u16,
        }
        impl PackerHooks for RelocHook {
            fn refine(&self, _ctx: &RefineContext<'_>, bounds: &mut Bounds) {
                bounds.end_addr = self.reloc_start;
            }
        }

        fn make_hooks(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Box<dyn PackerHooks> {
            let q = usize::from(sig_match.anchor_addr);
            let reloc_start = image.read_u16_le(q + 0x10).unwrap_or(0);
            Box::new(RelocHook { reloc_start })
        }

        const SIGS: [Signature; 1] = [Signature {
            anchor: Anchor::BasicStart { offset: 0x0C },
            pattern: sig![0x78, 0xA2],
        }];
        let spec = PackerSpec {
            id: "test.hooks_factory",
            name: "Test Hooks",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: Some(0x0801),
            signatures: &SIGS,
            hints: Hints::NONE,
            patches: &[],
            resolve: None,
            hooks: Some(make_hooks),
            fixture: &[],
            fixture_load_addr: 0x0801,
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x080D] = 0x78;
        mem[0x080E] = 0xA2;
        mem[0x081D] = 0x00;
        mem[0x081E] = 0x45; // reloc_start = $4500 at q + 0x10
        let image = LoadedImage::for_c64(&mem, 0x0801, 0x0900);
        let sig_match = spec.detect(&image).expect("spec matches");
        let hooks = (spec.hooks.unwrap())(&image, &sig_match);

        let mut bounds = Bounds {
            start_addr: 0x0800,
            end_addr: 0x9FFF,
            entry_point: 0x080D,
        };
        let refine_ctx = RefineContext {
            mem: &mem,
            snapshot: &mem,
            written: &[false; 0x1_0000],
            system: &TargetSystem::C64,
            registers: Registers::default(),
        };
        hooks.refine(&refine_ctx, &mut bounds);
        assert_eq!(bounds.end_addr, 0x4500);
    }

    /// T3.0.37: `PackerHooks` is `Send` (and does not require `Sync`).
    #[test]
    fn packer_hooks_is_send() {
        #[derive(Debug)]
        struct NonSyncHook {
            // `Cell` is `Send` but NOT `Sync`.
            steps: Cell<u32>,
        }
        impl PackerHooks for NonSyncHook {
            fn on_step(&mut self, _ctx: &mut StepContext<'_>) {
                self.steps.set(self.steps.get() + 1);
            }
        }

        fn assert_send<T: Send>() {}
        assert_send::<Box<dyn PackerHooks>>();

        let mut boxed: Box<dyn PackerHooks> = Box::new(NonSyncHook {
            steps: Cell::new(0),
        });
        std::thread::scope(|s| {
            s.spawn(move || {
                let mut bus = UnpackerMemory::new(TargetSystem::C64, None, None, None);
                let mut ctx =
                    StepContext::new(&mut bus, 0x080D, Phase::FindDepacker, Registers::default());
                boxed.on_step(&mut ctx);
            });
        });
    }

    /// T3.6.1: [`MemoryPatch`] with [`PatchAddr::Absolute`] writes `bytes` at the exact fixed
    /// address regardless of `anchor_addr` and rejects out-of-range or empty patches.
    #[test]
    fn patch_absolute_applied() {
        let patch = MemoryPatch {
            addr: PatchAddr::Absolute(0x080B),
            expected: None,
            bytes: &[0xA2, 0x34],
        };
        let mut mem = vec![0xEA_u8; 0x1_0000];
        assert!(patch.apply(&mut mem, 0x080C));
        assert_eq!(mem[0x080A], 0xEA);
        assert_eq!(&mem[0x080B..=0x080C], &[0xA2, 0x34]);
        assert_eq!(mem[0x080D], 0xEA);

        // Multi-byte patch crossing the 64 KB boundary (`0xFFFF..0x1_0001`) is rejected cleanly.
        let overflow_patch = MemoryPatch {
            addr: PatchAddr::Absolute(0xFFFF),
            expected: None,
            bytes: &[0x01, 0x02],
        };
        assert!(!overflow_patch.apply(&mut mem, 0x080C));
        assert_eq!(mem[0xFFFF], 0xEA);

        // Empty replacement slice is a no-op returning `false`.
        let empty_patch = MemoryPatch {
            addr: PatchAddr::Absolute(0x080B),
            expected: None,
            bytes: &[],
        };
        assert!(!empty_patch.apply(&mut mem, 0x080C));
    }

    /// T3.6.2: [`PatchAddr::FromMatch`] resolves positive, zero, and negative signed offsets
    /// relative to `anchor_addr` and rejects offsets that underflow `< 0` or overflow `> $FFFF`.
    #[test]
    fn patch_from_match_signed_offset() {
        let anchor_addr: u16 = 0x080C;

        let neg_patch = MemoryPatch {
            addr: PatchAddr::FromMatch(-1),
            expected: None,
            bytes: &[0xA2],
        };
        let zero_patch = MemoryPatch {
            addr: PatchAddr::FromMatch(0),
            expected: None,
            bytes: &[0x78],
        };
        let pos_patch = MemoryPatch {
            addr: PatchAddr::FromMatch(0x10),
            expected: None,
            bytes: &[0x4C, 0x00, 0x20],
        };

        let mut mem = vec![0u8; 0x1_0000];
        assert!(neg_patch.apply(&mut mem, anchor_addr));
        assert_eq!(mem[0x080B], 0xA2);

        assert!(zero_patch.apply(&mut mem, anchor_addr));
        assert_eq!(mem[0x080C], 0x78);

        assert!(pos_patch.apply(&mut mem, anchor_addr));
        assert_eq!(&mem[0x081C..=0x081E], &[0x4C, 0x00, 0x20]);

        // Underflow (`0x080C - 0x0900 < 0`) and overflow (`0x080C + 0x1_0000 > 0xFFFF`) fail.
        assert_eq!(PatchAddr::FromMatch(-0x0900).resolve(anchor_addr), None);
        assert_eq!(PatchAddr::FromMatch(0x1_0000).resolve(anchor_addr), None);
        let underflow_patch = MemoryPatch {
            addr: PatchAddr::FromMatch(-0x0900),
            expected: None,
            bytes: &[0xEA],
        };
        assert!(!underflow_patch.apply(&mut mem, anchor_addr));
    }

    /// T3.6.3: [`MemoryPatch::expected`] acts as an "apply only if" guard on the first target
    /// byte; when the byte at the resolved address differs from `expected`, the patch is skipped.
    #[test]
    fn patch_expected_mismatch_is_skipped() {
        let patch = MemoryPatch {
            addr: PatchAddr::Absolute(0x0911),
            expected: Some(0x20),
            bytes: &[0x2C],
        };

        let mut mem = vec![0u8; 0x1_0000];
        mem[0x0911] = 0x4C; // `JMP`, not `JSR` ($20)
        assert!(!patch.apply(&mut mem, 0x080D));
        assert_eq!(mem[0x0911], 0x4C, "mismatched expected byte must not patch");

        mem[0x0911] = 0x20; // `JSR` ($20) matches `expected`
        assert!(patch.apply(&mut mem, 0x080D));
        assert_eq!(
            mem[0x0911], 0x2C,
            "matching expected byte must apply patch (`JSR` -> `BIT`)"
        );
    }
}
