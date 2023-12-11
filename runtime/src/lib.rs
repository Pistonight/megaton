#![no_std]
///
#[repr(C, packed(1))]
#[derive(Debug)]
pub struct ModuleName<S> {
    unknown: u32,
    /// A 4-byte integer indicating the length of the module name (does not include null byte)
    pub len: u32,
    /// The module name
    pub name: S,
    null: u8,
}
impl<S> ModuleName<S> {
    pub const fn new(len: u32, name: S) -> Self {
        Self {
            unknown: 0,
            len,
            name,
            null: 0,
        }
    }
}
static_assertions::assert_eq_size!(ModuleName<[u8; 10]>, [u8; 19]);

/// Rust side initialization, called before rust's main
pub fn bootstrap_rust() {
}

/// Re-exports all proc macros
pub use megaton_proc_macros::*;
