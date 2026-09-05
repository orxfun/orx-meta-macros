#![doc = include_str!("../README.md")]
#![warn(
    missing_docs,
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::float_cmp,
    clippy::float_cmp_const,
    clippy::missing_panics_doc,
    clippy::todo
)]
#![cfg_attr(not(test), no_std)]

use proc_macro::TokenStream;

extern crate alloc;

mod queue;

#[proc_macro_attribute]
pub fn queue(args: TokenStream, input: TokenStream) -> TokenStream {
    queue::queue(args, input)
}
