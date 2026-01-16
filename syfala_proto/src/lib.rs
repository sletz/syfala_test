//! Implementation of a simple protocol for real-time audio communication and discovery
#![no_std]

extern crate alloc;

mod format;
mod messages;

pub use format::*;
pub use messages::*;
