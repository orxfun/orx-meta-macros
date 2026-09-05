use proc_macro::TokenStream;

mod queue;

#[proc_macro_attribute]
pub fn queue(args: TokenStream, input: TokenStream) -> TokenStream {
    queue::queue(args, input)
}
