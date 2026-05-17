//! Internal logging facade.
//!
//! Routes `pe_info!` / `pe_warn!` / `pe_error!` to either the `tracing` crate
//! (when the `tracing` cargo feature is enabled) or the `log` crate (default).
//! Both backends accept the same `format!`-style call we use throughout the
//! crate, so callers don't need to care which one is active.
//!
//! Library code should always use these macros - never call `log::` or
//! `tracing::` macros directly - so consumers can pick a single logging
//! backend without dual-ingest.

#[cfg(feature = "tracing")]
pub(crate) use ::tracing::error as pe_error;
#[cfg(feature = "tracing")]
pub(crate) use ::tracing::info as pe_info;
#[cfg(feature = "tracing")]
pub(crate) use ::tracing::warn as pe_warn;

#[cfg(not(feature = "tracing"))]
pub(crate) use ::log::error as pe_error;
#[cfg(not(feature = "tracing"))]
pub(crate) use ::log::info as pe_info;
#[cfg(not(feature = "tracing"))]
pub(crate) use ::log::warn as pe_warn;
