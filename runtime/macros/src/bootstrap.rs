use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{ItemFn, Meta, LitStr, Attribute, parenthesized, LitInt, punctuated::Punctuated, Token, Ident};

type TokenStream2 = proc_macro2::TokenStream;
/// Implementation of the `#[megaton::bootstrap]` attribute.
pub fn bootstrap_impl(item: TokenStream) -> TokenStream {
    let mut parsed = syn::parse_macro_input!(item as ItemFn);

    let mut expanded = TokenStream2::new();

    // process attributes
    let mut found_module_name = false;
    let mut found_abort = false;
    let mut keep_attrs = Vec::new();

    for attr in std::mem::take(&mut parsed.attrs) {
        if let Meta::List(list) = &attr.meta {
            if list.path.is_ident("module") {
                found_module_name = true;
                let module_name=TokenStream2::from(declare_module_name(list.tokens.clone().into()));
                expanded.extend(module_name);
            } else if list.path.is_ident("abort") {
                found_abort = true;
                let abort_handler = TokenStream2::from(declare_abort_handler(&attr));
                expanded.extend(abort_handler);
            } 
            continue;
        }
        keep_attrs.push(attr);
    }

    if !found_module_name {
        panic!("Missing module name!. Please add #[module(\"...\")].");
    }

    if !found_abort {
        panic!("Missing abort handler!. Please add #[abort(...)]. If you are unsure, add `#[abort(code(-1))]`");
    }

    let main_name = &parsed.sig.ident;

    // generate bootstrap
    let megaton_rust_main = quote::quote! {
        #[no_mangle]
        pub extern "C" fn megaton_rust_main() {
            // Rust side initialization
            megaton::bootstrap_rust();
            // Call main
            #main_name();
        }
    };

    expanded.extend(megaton_rust_main);
    for attr in keep_attrs {
        expanded.extend(quote::quote! { #attr });
    }

    let vis = parsed.vis;
    let sig = parsed.sig;
    let block = parsed.block;

    expanded.extend(quote::quote! {
        #vis #sig #block
    });

    expanded.into()
}

pub fn declare_module_name(attr: TokenStream) -> TokenStream {
    let literal = syn::parse_macro_input!(attr as LitStr);
    let value = literal.value();
    let len = value.len();
    let mut byte_array = TokenStream2::new();
    for byte in value.bytes() {
        byte_array.extend(quote::quote! { #byte, });
    }

    
    let out = quote::quote! {
        #[link_section = ".nx-module-name"]
        #[used]
        static NX_MODULE_NAME: megaton::ModuleName<[u8; #len]> = 
            megaton::ModuleName::new(#len as u32, [#byte_array]);
        #[no_mangle]
        pub extern "C" fn megaton_module_name() -> *const megaton::ModuleName<[u8; #len]> {
            &NX_MODULE_NAME as *const _
        }
        pub const fn module_name() -> &'static str {
            #literal
        }
    };

    out.into()
}

pub fn declare_abort_handler(attr: &Attribute) -> TokenStream {

    let nested = match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
        Ok(nested) => nested,
        Err(e) => panic!("Error parsing abort attribute: {}", e),
    };

    let mut code: Option<i32> = None;
    let mut handler: Option<String> = None;

    for meta in nested {
        match meta {
            Meta::List(meta) if meta.path.is_ident("code") => {
                if code.is_some() {
                    panic!("`code` in abort attribute can only be specified once");
                }
                let tokens: TokenStream = meta.tokens.into();
                let lit = syn::parse_macro_input!(tokens as LitInt);
                match lit.base10_parse() {
                    Ok(n) => code = Some(n),
                    Err(_) => panic!("`code` in abort attribute must be an integer literal"),
                }
            }
            Meta::NameValue(meta) if meta.path.is_ident("handler") => {
                if handler.is_some() {
                    panic!("`handler` in abort attribute can only be specified once");
                }
                let tokens: TokenStream = meta.value.into_token_stream().into();
                let lit = syn::parse_macro_input!(tokens as LitStr);
                handler = Some(lit.value());
            }
            _ => panic!("Unknown abort attribute! Please see documentation"),
        }
    }

    let handler = handler.unwrap_or("megaton_default_abort".to_string());
    let handler = match syn::parse_str::<Ident>(&handler) {
        Ok(ident) => ident,
        Err(_) => panic!("Invalid abort handler name"),
    };
    // default abort handler
    let out = quote::quote! {
        extern "C" {
            fn #handler(code: i32) -> !;
        }
        #[no_mangle]
        pub extern "C" fn megaton_abort() {
            unsafe { #handler(#code) }
        }
    };

    out.into()

}


