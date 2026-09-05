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

/// Generates queue types that forward trait methods to their elements.
///
/// The queue types are declared in the attribute arguments after the trait
/// name. `Single` stores one element, while `Multi` stores a front element and
/// the remainder of the queue. An optional first type declares the empty queue.
///
/// ```rust,ignore
/// use orx_meta::queue;
///
/// #[queue(Queue; Single, Multi)]
/// pub trait Fun {
///     fn work(&self);
///
///     fn work_with_number(&self, number: usize);
/// }
///
/// impl<X: Fn()> Fun for X {
///     fn work(&self) {
///         self();
///     }
///
///     fn work_with_number(&self, number: usize) {
///         for _ in 0..number {
///             self();
///         }
///     }
/// }
///
/// #[test]
/// fn forwards_methods_for_single_and_multi_queues() {
///     let output = std::cell::RefCell::new(Vec::new());
///     let first_output = &output;
///     let second_output = &output;
///     let queue = Single::new(move || first_output.borrow_mut().push("hey"))
///         .push(move || second_output.borrow_mut().push("there"));
///
///     queue.work();
///     queue.work_with_number(2);
///
///     assert_eq!(
///         output.into_inner(),
///         vec!["hey", "there", "hey", "hey", "there", "there"]
///     );
/// }
/// ```
#[proc_macro_attribute]
pub fn queue(args: TokenStream, input: TokenStream) -> TokenStream {
    queue::queue(args, input)
}
