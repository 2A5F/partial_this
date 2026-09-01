use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

mod codegen;
mod config;
use config::PartialConfig;

/// Generates a type-safe partial construction for a struct.
///
/// Applying the attribute to a struct emits a module that implements
/// `PartialThis` for the struct, plus per-field builder (`uninit_*`) and
/// accessor (`inited_*`) traits.
///
/// # Example
///
/// ```ignore
/// use partial_this::{partial, DonePartial, PartialThis};
///
/// #[partial]
/// #[derive(Debug)]
/// pub struct Foo {
///     pub a: i32,
///     pub b: f32,
/// }
///
/// let foo = Foo::partial(Box::new_uninit())
///     .a(1)
///     .b(2.0)
///     .done();
/// ```
///
/// # Multiple partial sources
///
/// `Box::new_uninit()`, `MaybeUninit::uninit()`, and `&mut MaybeUninit` are all
/// accepted as the initial storage, finalizing to `Box<T>`, `T`, or `&mut T`:
///
/// ```ignore
/// let foo: Box<Foo> = Foo::partial(Box::new_uninit()).done();
/// let foo: Foo = Foo::partial(MaybeUninit::uninit()).done();
///
/// let mut buf = MaybeUninit::uninit();
/// let foo: &mut Foo = Foo::partial(&mut buf).done();
/// ```
///
/// # Field initialization & access
///
/// Field builders can be called in any order, but at most once; after a field is
/// initialized you can read or mutate it:
///
/// ```ignore
/// let p = Foo::partial(Box::new_uninit());
/// let p = p.a(1);
/// println!("{}", p.a());
///
/// let mut p = p;
/// *p.a_mut() = 123;
/// let p = p.b(2.0);
/// // let p = p.b(3.0); // error: field already initialized
/// let foo = p.done();
/// ```
///
/// # Tuple and generic structs
///
/// Tuple structs use `_0`, `_1`, ... as field names; generic and lifetime
/// structs are supported too:
///
/// ```ignore
/// #[partial]
/// pub struct Bar(i32, f32);
/// let bar = Bar::partial(Box::new_uninit())._0(1)._1(2.0).done();
///
/// #[partial]
/// pub struct Pair<'a, T> {
///     pub name: &'a str,
///     pub value: T,
/// }
/// let pair = Pair::<String>::partial(Box::new_uninit())
///     .name("x")
///     .value(String::from("y"))
///     .done();
/// ```
///
/// # Behavior
///
/// - **Field initialization** — each field's builder method can be called in
///   any order, but at most once; calling it twice is a compile error.
/// - **Field access** — after a field is initialized, `field()` borrows its
///   value and `field_mut()` returns a mutable reference.
/// - **`done()`** — finalizes the builder once every field is initialized.
/// - **Drop safety** — dropping an unfinished builder drops already-initialized
///   fields in declaration order.
/// - **Field visibility** — `pub` fields' traits are re-exported with `pub use`;
///   private fields' traits are collected into a nested `private` module and
///   brought into scope with a plain `use` (module-local by default).
///
/// # Config
///
/// - `module = name` — name of the generated module. Defaults to a snake_case
///   variant of the struct name, e.g. `foo_partial` for `Foo`.
/// - `crate_name = name` — crate that exposes `PartialThis`/`chain`/`typenum`.
///   Defaults to `partial_this`; set it to the dependency alias when the crate
///   is renamed in `Cargo.toml`.
/// - `pub_use = true|false` — whether to re-export the generated builder and
///   accessor traits. Defaults to `true`; set to `false` to require manual
///   imports.
#[proc_macro_attribute]
pub fn partial(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemStruct);
    let cfg = parse_macro_input!(attr as PartialConfig);

    match codegen::generate(&item, &cfg) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
