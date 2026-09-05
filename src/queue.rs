use alloc::vec::Vec;
use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::parse_macro_input;
use syn::{Error, FnArg, Ident, ItemTrait, Pat, ReturnType, Token, TraitItem, TraitItemFn};

struct QueueArgs {
    queue: Ident,
    empty: Option<Ident>,
    single: Ident,
    multi: Ident,
}

impl Parse for QueueArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let queue = input.parse()?;
        input.parse::<Token![;]>()?;
        let first = input.parse()?;
        input.parse::<Token![,]>()?;
        let second = input.parse()?;
        let third = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after queue definition"));
        }

        let (empty, single, multi) = match third {
            Some(multi) => (Some(first), second, multi),
            None => (None, first, second),
        };

        Ok(Self {
            queue,
            empty,
            single,
            multi,
        })
    }
}

pub fn queue(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as QueueArgs);
    let trait_item = parse_macro_input!(input as ItemTrait);

    match expand(args, trait_item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand(args: QueueArgs, trait_item: ItemTrait) -> syn::Result<TokenStream2> {
    if !trait_item.generics.params.is_empty() || trait_item.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &trait_item.generics,
            "queue currently supports only non-generic traits",
        ));
    }

    let trait_name = &trait_item.ident;
    let orx_meta = orx_meta_path();
    let methods = trait_item
        .items
        .iter()
        .map(|item| match item {
            TraitItem::Fn(method) => method_to_impl(method),
            _ => Err(Error::new_spanned(
                item,
                "queue supports only trait methods",
            )),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let single_methods = methods.iter().map(|method| &method.single);
    let multi_methods = methods.iter().map(|method| &method.multi);
    let queue = args.queue;
    let empty = args.empty;
    let single = args.single;
    let multi = args.multi;

    let empty_queue = empty.as_ref().map(|empty| {
        let empty_methods = methods.iter().map(|method| &method.empty);
        quote! {
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
            pub struct #empty;

            impl #empty {
                /// Creates an empty queue.
                pub const fn new() -> Self {
                    Self
                }
            }

            impl #trait_name for #empty {
                #(#empty_methods)*
            }

            impl #queue for #empty {
                type PushBack<T> = #single<T>
                where
                    T: #trait_name;

                type Front = Self;
                type Back = Self;

                const LEN: usize = 0;

                fn push<T>(self, value: T) -> Self::PushBack<T>
                where
                    T: #trait_name,
                {
                    #single::new(value)
                }

                fn front(&self) -> &Self::Front {
                    self
                }

                fn front_mut(&mut self) -> &mut Self::Front {
                    self
                }

                fn into_front(self) -> Self::Front {
                    self
                }
            }
        }
    });

    Ok(quote! {
        #trait_item

        #orx_meta::define_queue!(
            elements => [#trait_name];
            queue => [#queue; #single, #multi];
        );

        #empty_queue

        impl<F: #trait_name> #trait_name for #single<F> {
            #(#single_methods)*
        }

        impl<F: #trait_name, B: #queue> #trait_name for #multi<F, B> {
            #(#multi_methods)*
        }
    })
}

fn orx_meta_path() -> TokenStream2 {
    match crate_name("orx-meta") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#name)
        }
        Err(_) => quote!(::orx_meta),
    }
}

struct MethodImpl {
    empty: TokenStream2,
    single: TokenStream2,
    multi: TokenStream2,
}

fn method_to_impl(method: &TraitItemFn) -> syn::Result<MethodImpl> {
    let signature = &method.sig;
    if signature.asyncness.is_some()
        || signature.constness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
    {
        return Err(Error::new_spanned(
            signature,
            "queue supports only safe, synchronous methods",
        ));
    }

    if signature.receiver().is_none() {
        return Err(Error::new_spanned(
            signature,
            "queue requires methods with a self receiver",
        ));
    }

    if !returns_unit(signature.output.clone()) {
        return Err(Error::new_spanned(
            &signature.output,
            "queue currently supports only methods returning `()`",
        ));
    }

    let arguments = signature
        .inputs
        .iter()
        .skip(1)
        .map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) if pattern.subpat.is_none() => Ok(pattern.ident.clone()),
                _ => Err(Error::new_spanned(
                    &argument.pat,
                    "method arguments must use simple identifier patterns",
                )),
            },
            FnArg::Receiver(receiver) => Err(Error::new_spanned(
                receiver,
                "only the first method argument may be a self receiver",
            )),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let name = &signature.ident;
    let signature = signature.clone();
    let single = quote! {
        #signature {
            self.f.#name(#(#arguments),*);
        }
    };
    let empty = quote! {
        #[allow(unused_variables)]
        #signature {}
    };
    let multi = quote! {
        #signature {
            self.f.#name(#(#arguments),*);
            self.b.#name(#(#arguments),*);
        }
    };

    Ok(MethodImpl {
        empty,
        single,
        multi,
    })
}

fn returns_unit(output: ReturnType) -> bool {
    match output {
        ReturnType::Default => true,
        ReturnType::Type(_, ty) => match *ty {
            syn::Type::Tuple(tuple) => tuple.elems.is_empty(),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn empty_queue_has_new_constructor() {
        let args: QueueArgs = syn::parse2(quote!(Queue; Empty, Single, Multi)).unwrap();
        let trait_item: ItemTrait = syn::parse2(quote! {
            trait Fun {
                fn work(&self);
            }
        })
        .unwrap();

        let output = expand(args, trait_item).unwrap().to_string();

        assert!(output.contains("pub const fn new () -> Self { Self }"));
    }

    #[test]
    fn generic_methods_are_supported() {
        let args: QueueArgs = syn::parse2(quote!(Queue; Single, Multi)).unwrap();
        let trait_item: ItemTrait = syn::parse2(quote! {
            trait GenericFun {
                fn generic_work<T: Default>(&self, _value: T);
            }
        })
        .unwrap();

        let result = expand(args, trait_item);
        // Should succeed, not error about non-generic methods
        assert!(result.is_ok());
    }

    #[test]
    fn generic_methods_with_where_clauses_are_supported() {
        let args: QueueArgs = syn::parse2(quote!(Queue; Single, Multi)).unwrap();
        let trait_item: ItemTrait = syn::parse2(quote! {
            trait GenericFun {
                fn generic_work<T>(&self, _value: T)
                where
                    T: Default;
            }
        })
        .unwrap();

        let result = expand(args, trait_item);
        // Should succeed, not error about non-generic methods
        assert!(result.is_ok());
    }

    #[test]
    fn lifetime_and_trait_bounds_are_supported() {
        let args: QueueArgs = syn::parse2(quote!(TaskQueue; TasksSingle, TasksMulti)).unwrap();
        let trait_item: ItemTrait = syn::parse2(quote! {
            trait ParFun {
                fn run<'s, 'env, 'scope>(self, scope: impl Scope<'s, 'env, 'scope>)
                where
                    'scope: 's,
                    'env: 'scope + 's,
                    Self: 'scope + 'env;
            }
        })
        .unwrap();

        let result = expand(args, trait_item);
        // Should succeed, supporting complex lifetime and trait bounds
        assert!(result.is_ok());
        let output = result.unwrap().to_string();
        // Verify that the generics are preserved in the output
        assert!(output.contains("'s") || output.contains("'scope"));
    }
}
