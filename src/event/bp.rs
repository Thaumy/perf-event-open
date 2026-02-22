use std::io::Result;

use super::EventConfig;
use crate::ffi::bindings as b;

/// Hardware breakpoint event provided by the CPU.
///
/// Breakpoints can be read/write accesses to an address as well as
/// execution of an instruction address.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Breakpoint {
    /// Breakpoint type.
    pub ty: Type,
    /// Breakpoint address.
    pub addr: u64,
}

/// Breakpoint type.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    /// Count when we read the memory location.
    R(Len),
    /// Count when we write the memory location.
    W(Len),
    /// Count when we read or write the memory location.
    Rw(Len),
    /// Count when we execute code at the memory location.
    X,
}

/// Breakpoint size (in bytes).
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Len {
    /// 1 byte.
    _1,
    /// 2 bytes.
    _2,
    /// 3 bytes.
    ///
    /// Since `linux-4.10`: <https://github.com/torvalds/linux/commit/651be3cb085341a21847e47c694c249c3e1e4e5b>
    _3,
    /// 4 bytes.
    _4,
    /// 5 bytes.
    ///
    /// Since `linux-4.10`: <https://github.com/torvalds/linux/commit/651be3cb085341a21847e47c694c249c3e1e4e5b>
    _5,
    /// 6 bytes.
    ///
    /// Since `linux-4.10`: <https://github.com/torvalds/linux/commit/651be3cb085341a21847e47c694c249c3e1e4e5b>
    _6,
    /// 7 bytes.
    ///
    /// Since `linux-4.10`: <https://github.com/torvalds/linux/commit/651be3cb085341a21847e47c694c249c3e1e4e5b>
    _7,
    /// 8 bytes.
    _8,
}

impl Len {
    #[cfg(feature = "linux-4.10")]
    pub(crate) const fn as_bp_len(&self) -> Result<u64> {
        let bp_len = match self {
            Self::_1 => b::HW_BREAKPOINT_LEN_1,
            Self::_2 => b::HW_BREAKPOINT_LEN_2,
            Self::_3 => b::HW_BREAKPOINT_LEN_3,
            Self::_4 => b::HW_BREAKPOINT_LEN_4,
            Self::_5 => b::HW_BREAKPOINT_LEN_5,
            Self::_6 => b::HW_BREAKPOINT_LEN_6,
            Self::_7 => b::HW_BREAKPOINT_LEN_7,
            Self::_8 => b::HW_BREAKPOINT_LEN_8,
        };
        Ok(bp_len as _)
    }

    #[cfg(not(feature = "linux-4.10"))]
    pub(crate) fn as_bp_len(&self) -> Result<u64> {
        let bp_len = match self {
            Self::_1 => b::HW_BREAKPOINT_LEN_1,
            Self::_2 => b::HW_BREAKPOINT_LEN_2,
            Self::_4 => b::HW_BREAKPOINT_LEN_4,
            Self::_8 => b::HW_BREAKPOINT_LEN_8,
            _ => crate::config::unsupported!(),
        };
        Ok(bp_len as _)
    }
}

super::try_from!(Breakpoint, value, {
    let (bp_type, bp_len) = match &value.ty {
        Type::R(l) => (b::HW_BREAKPOINT_R, l.as_bp_len()?),
        Type::W(l) => (b::HW_BREAKPOINT_W, l.as_bp_len()?),
        Type::Rw(l) => (b::HW_BREAKPOINT_RW, l.as_bp_len()?),
        Type::X => (b::HW_BREAKPOINT_X, 0),
    };
    let event_cfg = EventConfig {
        ty: b::PERF_TYPE_BREAKPOINT,
        config: 0,
        config1: 0,
        config2: bp_len,
        config3: 0,
        bp_type,
    };
    Ok(Self(event_cfg))
});
