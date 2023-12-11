use proc_macro::TokenStream;


/// Put this on the entry point (main function) of your module
/// to help generate the necessary code to make it work.
///
/// # Example
/// ```rust
/// #[megaton::bootstrap)]
/// #[module("my-module")] // MUST SPECIFY
/// #[abort(data)] // try to use data abort to abort (read from bad address)
/// #[abort("my_abort")] // call custom abort handler implemented in C
/// // abort handlers take no arg
///
/// #[panic(abort)] // when panic, try to cause null pointer exception to abort
/// #[panic(C("my_handler"))] // call custom C function when panic, message, filename, line number
/// // will be passed as arg
/// #[alloc(panic)] // panic when trying to allocate memory
/// #[alloc(bss(0x5000), oom(abort))] // use megaton framework's fake heap
/// #[alloc(C, oom(panic))] // only bind alloc to C malloc/free
/// #[alloc(malloc = "", free = "", oom(panic))] // bind alloc to custom C functions
/// fn main() {
///    // ...
/// }
/// ```
///
/// # Attributes
/// ## `module`
/// Required. Specify the module name.
///
/// `#[module("my-module")]` generates:
/// - `megaton_module_name()` function to let C code access the module name struct.
/// - `module_name()` function lets Rust code access the module name as a `&'static str`.
///
/// ## `abort`
/// Required. Specify abort handling behavior
#[proc_macro_attribute]
pub fn bootstrap(_attr: TokenStream, item: TokenStream) -> TokenStream {
    bootstrap::bootstrap_impl(item)
}
mod bootstrap;
