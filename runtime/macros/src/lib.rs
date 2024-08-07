use proc_macro::TokenStream;


/// Put this on the entry point (main function) of your module
/// to bootstrap the module.
///
/// # Example
/// ```rust
/// #[megaton::bootstrap)]
/// #[module("my-module")]
/// #[abort(code(-1))]
///
/// #[panic(print, abort)] // when panic, try to cause null pointer exception to abort
/// #[panic(print, handler = "my_handler")] // call custom C function when panic, message, filename, line number
/// // will be passed as arg
/// #[alloc(panic)] // panic when trying to allocate memory
/// #[alloc(bss(0x5000))] // use megaton framework's fake heap
/// #[alloc(C)] // only bind alloc to C malloc/free
/// #[alloc(malloc = "", free = "")] // bind alloc to custom C functions
///
/// #[stdio(web(5000), init) // bind stdio to websocket on port 5000
/// #[stdio(tcp(5000), init) // bind stdio to tcp socket on port 5000
/// #[stdio(in = "read_in", out = "write_out")] // bind stdio to custom C functions
/// #[stdio(none)] // no stdio
/// fn main() {
///    // ...
/// }
/// ```
///
/// # Attributes
/// The bootstrap proc macro parses the other attributes to generate
/// the bootstrap code. The attributes can be hard to remember, but
/// there is one rule:
///
/// - If the attribute is meant to specify a binding to a C function,
///   it will be in the `name = "value"` form. For example, `#[alloc(malloc = "my_malloc", free =
///   "my_free")]`
/// - The other attributes are either `name(value)` or `name`, depending on whether it expects an
///   agrument.
///
/// ## `module`
/// Required. Specify the module name.
///
/// `#[module("my-module")]` generates:
/// - `megaton_module_name()` function to let C code access the module name struct.
/// - `module_name()` function lets Rust code access the module name as a `&'static str`.
///
/// ## `abort`
/// Required. Specify abort handling behavior.
///
/// Aborting happens both when the program wants to exit normally and when the program panics.
/// The abort handler takes in an int argument as the exit code.
///
/// There are two accepted forms of abort handling (X is an int literal indicating the exit code on
/// abort)
/// - `#[abort(code(X))]` - Use the default abort handler. The default abort handler will write
/// the code to X28 and then cause a data abort (invalid memory access) to exit.
/// - `#[abort(handler = "my_abort", code(X))]` - Call the C function `my_abort` when aborting, and
/// pass X as the argument. The function should not return.
#[proc_macro_attribute]
pub fn bootstrap(_attr: TokenStream, item: TokenStream) -> TokenStream {
    bootstrap::bootstrap_impl(item)
}
mod bootstrap;
