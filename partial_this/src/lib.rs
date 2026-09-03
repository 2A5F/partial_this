//! Type-safe partial construction.
//!
//! The [`partial`] attribute macro lets you build a struct field-by-field,
//! while the type system guarantees that each field is initialized at most once
//! and that `done()` is only callable once every field has
//! been set.
//!
//! # Example
//!
//! ```rust
//! use partial_this::partial;
//!
//! #[partial]
//! #[derive(Debug)]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     let foo = Foo::partial(Box::new_uninit())
//!         .with_foo(1)
//!         .with_bar(1.0)
//!         .done();
//!     assert_eq!(foo.foo, 1);
//!     assert_eq!(foo.bar, 1.0);
//! }
//! ```
//!
//! # Multiple partial sources
//!
//! The initial storage can be a `Box`, an owned `MaybeUninit`, or a mutable
//! reference, producing `Box<T>`, `T`, or `&mut T` respectively:
//!
//! ```rust
//! use partial_this::partial;
//! use core::mem::MaybeUninit;
//!
//! #[partial]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     let a: Box<Foo> = Foo::partial(Box::new_uninit()).with_foo(1).with_bar(2.0).done();
//!     let b: Foo = Foo::partial(MaybeUninit::uninit()).with_foo(1).with_bar(2.0).done();
//!     let mut buf = MaybeUninit::uninit();
//!     let c: &mut Foo = Foo::partial(&mut buf).with_foo(1).with_bar(2.0).done();
//!     let _ = (a, b, c);
//! }
//! ```
//!
//! # Field initialization
//!
//! Each field's builder method can be called in **any order**, but **at most
//! once**:
//!
//! ```rust
//! use partial_this::partial;
//!
//! #[partial]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     // Fields can be set in any order.
//!     let p = Foo::partial(Box::new_uninit());
//!     let p = p.with_bar(1.0);
//!     let p = p.with_foo(1); // `foo` was not set yet, so this is allowed
//!     let foo = p.done();
//!     assert_eq!(foo.foo, 1);
//!     assert_eq!(foo.bar, 1.0);
//! }
//! ```
//!
//! # Field access
//!
//! After a field is initialized you can read it or mutate it:
//!
//! ```rust
//! use partial_this::partial;
//!
//! #[partial]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     let mut p = Foo::partial(Box::new_uninit()).with_foo(1);
//!     assert_eq!(*p.get_foo(), 1);
//!     p.set_foo(123);
//!     let foo = p.with_bar(2.0).done();
//!     assert_eq!(foo.foo, 123);
//! }
//! ```
//!
//! # Tuple and generic structs
//!
//! Tuple structs use `_0`, `_1`, ... as field method names; generic and
//! lifetime-parameterized structs are also supported:
//!
//! ```rust
//! use partial_this::partial;
//!
//! #[partial]
//! pub struct Bar(i32, f32);
//!
//! #[partial]
//! pub struct Pair<'a, T> {
//!     pub name: &'a str,
//!     pub value: T,
//! }
//!
//! fn main() {
//!     let bar = Bar::partial(Box::new_uninit()).with__0(1).with__1(2.0).done();
//!     assert_eq!(bar.0, 1);
//!     assert_eq!(bar.1, 2.0);
//!
//!     let pair = Pair::<String>::partial(Box::new_uninit())
//!         .with_name("x")
//!         .with_value(String::from("y"))
//!         .done();
//!     assert_eq!(pair.name, "x");
//!     assert_eq!(pair.value, "y");
//! }
//! ```
//!
//! # Others
//!
//! - **Multiple partial sources**: `Box::new_uninit()`, `MaybeUninit::uninit()`
//!   or `&mut MaybeUninit` all work as the initial storage.
//! - **Field initialization methods**: each field offers `with_field(value)`,
//!   `emplace_field(init)`, and `uninit_field()`; the builder methods can be
//!   called in any order, and at most once (a second call is a compile time error).
//! - **Field access methods**: after a field is initialized you can read it with
//!   `get_field()`, mutate it with `get_mut_field()`, or assign with
//!   `set_field(value)`.
//! - **`done()`**: finalizes the builder once every field is initialized, both
//!   as an inherent method and through the [`CtorComplete`] trait.
//! - **Drop safety**: partially-built values drop already-initialized fields in
//!   reverse order of initialization (the last field set is dropped first).
//! - **Field visibility**: the builder type `PartialStructName` is re-exported for a
//!   struct `StructName`, and field methods are available directly on the builder.
//!
//! # Safety
//!
//! The unsafe code in this crate is self-contained: it writes initialized fields
//! into a [`MaybeUninit`] buffer and only assumes the target value is initialized
//! after every field has been written.

#![cfg_attr(not(test), no_std)]

extern crate alloc;
extern crate self as partial_this;

use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::mem::MaybeUninit;

pub use typenum;

/// Generates a type-safe partial construction for a struct.
///
/// Applying the attribute to a struct emits a module that contains one marker
/// struct per field, alongside an inherent `partial` constructor on the struct
/// and a `Partial<N>` builder type (re-exported as `PartialFoo`).
///
/// # Example
///
/// ```
/// use partial_this::partial;
///
/// #[partial]
/// #[derive(Debug)]
/// pub struct Foo {
///     pub a: i32,
///     pub b: f32,
/// }
///
/// fn main() {
///     let foo = Foo::partial(Box::new_uninit())
///         .with_a(1)
///         .with_b(2.0)
///         .done();
///     assert_eq!(foo.a, 1);
///     assert_eq!(foo.b, 2.0);
/// }
/// ```
///
/// # Multiple partial sources
///
/// `Box::new_uninit()`, `MaybeUninit::uninit()`, and `&mut MaybeUninit` are all
/// accepted as the initial storage, finalizing to `Box<T>`, `T`, or `&mut T`:
///
/// ```
/// use partial_this::partial;
/// use core::mem::MaybeUninit;
///
/// #[partial]
/// pub struct Foo {
///     pub a: i32,
///     pub b: f32,
/// }
///
/// fn main() {
///     let boxed: Box<Foo> = Foo::partial(Box::new_uninit()).with_a(1).with_b(2.0).done();
///     let owned: Foo = Foo::partial(MaybeUninit::uninit()).with_a(1).with_b(2.0).done();
///
///     let mut buf = MaybeUninit::uninit();
///     let borrowed: &mut Foo = Foo::partial(&mut buf).with_a(1).with_b(2.0).done();
///     let _ = (boxed, owned, borrowed);
/// }
/// ```
///
/// # Field initialization & access
///
/// Field builders can be called in any order, but at most once; after a field is
/// initialized you can read or mutate it:
///
/// ```
/// use partial_this::partial;
///
/// #[partial]
/// pub struct Foo {
///     pub a: i32,
///     pub b: f32,
/// }
///
/// fn main() {
///     let mut p = Foo::partial(Box::new_uninit());
///     let p = p.with_a(1);
///     assert_eq!(*p.get_a(), 1);
///     let mut p = p;
///     p.set_a(123);
///     let p = p.with_b(2.0);
///     let foo = p.done();
///     assert_eq!(foo.a, 123);
/// }
/// ```
///
/// # Tuple and generic structs
///
/// Tuple structs use `_0`, `_1`, ... as field names; generic and lifetime
/// structs are supported too:
///
/// ```
/// use partial_this::partial;
///
/// #[partial]
/// pub struct Bar(i32, f32);
///
/// #[partial]
/// pub struct Pair<'a, T> {
///     pub name: &'a str,
///     pub value: T,
/// }
///
/// fn main() {
///     let bar = Bar::partial(Box::new_uninit()).with__0(1).with__1(2.0).done();
///     assert_eq!(bar.0, 1);
///
///     let pair = Pair::<String>::partial(Box::new_uninit())
///         .with_name("x")
///         .with_value(String::from("y"))
///         .done();
///     assert_eq!(pair.name, "x");
///     assert_eq!(pair.value, "y");
/// }
/// ```
///
/// # Behavior
///
/// - **Field initialization** — each field offers `with_field(value)`,
///   `emplace_field(init)`, and `uninit_field()`; the builder methods can be
///   called in any order, but at most once (a second call is a compile error).
/// - **Field access** — after a field is initialized you can read it with
///   `get_field()`, mutate it with `get_mut_field()`, or assign with
///   `set_field(value)`.
/// - **`done()`** — finalizes the builder once every field is initialized, both
///   as an inherent method and through the [`CtorComplete`] trait.
/// - **Drop safety** — dropping an unfinished builder drops already-initialized
///   fields in reverse order of initialization (the last field set is dropped
///   first).
/// - **Field visibility** — the builder type `PartialStructName` is re-exported
///   for a struct `StructName`; field methods are available directly on it.
///
/// # Config
///
/// - `module = name` — name of the generated module. Defaults to a snake_case
///   variant of the struct name, e.g. `foo_partial` for `Foo`.
/// - `crate_name = name` — crate that exposes `ThisPtr`/`AnyUninit`/`typenum`.
///   Defaults to `partial_this`; set it to the dependency alias when the crate
///   is renamed in `Cargo.toml`.
/// - `pub_use = true|false` — whether to re-export the generated `PartialXxx`
///   builder type with `pub use`. Defaults to `true`; set to `false` to keep it
///   module-local. Even when `true`, if the struct or any field is not `pub`
///   (which would leak a private type into a public interface), the re-export
///   falls back to a module-local `use`.
pub use partial_this_macro::partial;

pub use uninit_this::*;
mod uninit_this {
    use super::*;

    /// A source of uninitialized storage for a value of type
    /// [`Target`](Self::Target).
    ///
    /// Implemented for `MaybeUninit<T>`, `&mut MaybeUninit<T>`,
    /// `Box<MaybeUninit<T>>`, `Rc<MaybeUninit<T>>` and `Arc<MaybeUninit<T>>`.
    /// Implementations expose the underlying [`MaybeUninit`](core::mem::MaybeUninit)
    /// and allow assuming the storage has been initialized.
    pub trait AnyUninit {
        /// The type to be constructed.
        type Target;

        /// The type produced once [`assume_init`](Self::assume_init) is called.
        type Inited;

        /// Borrows the underlying uninitialized storage.
        ///
        /// # Safety
        ///
        /// The storage must not have been initialized yet.
        unsafe fn get(&self) -> &MaybeUninit<Self::Target>;

        /// Mutably borrows the underlying uninitialized storage.
        ///
        /// # Safety
        ///
        /// The storage must not have been initialized yet.
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target>;

        /// Assumes the storage has been initialized and returns the value.
        ///
        /// # Safety
        ///
        /// Every field must have been initialized before the value is assumed.
        unsafe fn assume_init(self) -> Self::Inited;
    }

    impl<'a, T> AnyUninit for &'a mut MaybeUninit<T> {
        type Target = T;

        type Inited = &'a mut T;

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn assume_init(self) -> Self::Inited {
            unsafe { self.assume_init_mut() }
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get(&self) -> &MaybeUninit<Self::Target> {
            self
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
            self
        }
    }
    impl<T> AnyUninit for MaybeUninit<T> {
        type Target = T;
        type Inited = T;

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn assume_init(self) -> Self::Inited {
            unsafe { self.assume_init() }
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get(&self) -> &MaybeUninit<Self::Target> {
            self
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
            self
        }
    }
    impl<T> AnyUninit for Box<MaybeUninit<T>> {
        type Target = T;
        type Inited = Box<T>;

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn assume_init(self) -> Self::Inited {
            unsafe { self.assume_init() }
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get(&self) -> &MaybeUninit<Self::Target> {
            self
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
            self
        }
    }
    impl<T> AnyUninit for Rc<MaybeUninit<T>> {
        type Target = T;
        type Inited = Rc<T>;

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn assume_init(self) -> Self::Inited {
            unsafe { self.assume_init() }
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get(&self) -> &MaybeUninit<Self::Target> {
            self
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
            Rc::get_mut(self).unwrap()
        }
    }
    impl<T> AnyUninit for Arc<MaybeUninit<T>> {
        type Target = T;
        type Inited = Arc<T>;

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn assume_init(self) -> Self::Inited {
            unsafe { self.assume_init() }
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get(&self) -> &MaybeUninit<Self::Target> {
            self
        }

        #[cfg_attr(not(debug_assertions), inline(always))]
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
            Arc::get_mut(self).unwrap()
        }
    }
}

/// Provides access to the underlying uninitialized storage of a builder node.
#[doc(hidden)]
pub trait ThisPtr {
    /// The struct type being constructed.
    type Target;

    /// Borrows the underlying `MaybeUninit` storage.
    fn this(&self) -> &MaybeUninit<Self::Target>;

    /// Mutably borrows the underlying `MaybeUninit` storage.
    fn this_mut(&mut self) -> &mut MaybeUninit<Self::Target>;
}

impl<U: AnyUninit> ThisPtr for U {
    type Target = U::Target;

    #[cfg_attr(not(debug_assertions), inline(always))]
    fn this(&self) -> &MaybeUninit<Self::Target> {
        unsafe { U::get(self) }
    }

    #[cfg_attr(not(debug_assertions), inline(always))]
    fn this_mut(&mut self) -> &mut MaybeUninit<Self::Target> {
        unsafe { U::get_mut(self) }
    }
}

/// Finalizes a partially-constructed value once every field is initialized.
///
/// The code generated by [`partial`] implements this trait, exposing `done()`
/// through a public interface. `T` is the finalized type produced by
/// [`AnyUninit::Inited`] — for a struct `Foo` this is `Box<Foo>`, `Foo`, or
/// `&mut Foo` depending on the storage — and not the raw [`AnyUninit::Target`]
/// struct type. `T` is a generic parameter rather than an associated type, so
/// implementing the trait never forces the generated builder's private state to
/// appear in a public trait interface.
pub trait CtorComplete<T> {
    fn done(self) -> T;
}

#[cfg(test)]
#[allow(nonstandard_style)]
pub mod test_reference {
    #[derive(Debug)]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    pub use should_be_generated_by_macro::*;
    mod should_be_generated_by_macro {
        use crate::ThisPtr;

        #[derive(Debug)]
        pub struct foo<N: ThisPtr<Target = super::Foo>>(N);
        #[derive(Debug)]
        pub struct bar<N: ThisPtr<Target = super::Foo>>(N);

        impl<N: ThisPtr<Target = super::Foo>> ThisPtr for foo<N> {
            type Target = super::Foo;

            fn this(&self) -> &core::mem::MaybeUninit<Self::Target> {
                self.0.this()
            }

            fn this_mut(&mut self) -> &mut core::mem::MaybeUninit<Self::Target> {
                self.0.this_mut()
            }
        }

        impl<N: ThisPtr<Target = super::Foo>> ThisPtr for bar<N> {
            type Target = super::Foo;

            fn this(&self) -> &core::mem::MaybeUninit<Self::Target> {
                self.0.this()
            }

            fn this_mut(&mut self) -> &mut core::mem::MaybeUninit<Self::Target> {
                self.0.this_mut()
            }
        }

        impl<N: ThisPtr<Target = super::Foo>> Drop for foo<N> {
            fn drop(&mut self) {
                unsafe { core::ptr::drop_in_place(&mut (*self.0.this_mut().as_mut_ptr()).foo) };
            }
        }

        impl<N: ThisPtr<Target = super::Foo>> Drop for bar<N> {
            fn drop(&mut self) {
                unsafe { core::ptr::drop_in_place(&mut (*self.0.this_mut().as_mut_ptr()).bar) };
            }
        }

        pub use private::Partial as PartialFoo;

        mod private {
            #![allow(clippy::missing_safety_doc)]
            use super::*;
            use crate::{AnyUninit, ThisPtr};
            use core::mem::{ManuallyDrop, MaybeUninit};
            use core::ops::{BitAnd, BitOr};
            use typenum::{Or, Shleft, U0, U1, U2, U3};

            #[derive(Debug)]
            pub struct Partial<N>(N);

            impl super::super::Foo {
                pub fn partial<U>(this: U) -> private::Partial<U>
                where
                    U: crate::AnyUninit<Target = super::super::Foo>,
                {
                    private::Partial(this)
                }
            }

            pub trait State: ThisPtr<Target = super::super::Foo> {
                type Flags;
                type Inited;

                unsafe fn assume_init(self) -> Self::Inited;
            }

            impl<U: AnyUninit<Target = super::super::Foo>> State for U {
                type Flags = U0;

                type Inited = U::Inited;

                unsafe fn assume_init(self) -> Self::Inited {
                    unsafe { U::assume_init(self) }
                }
            }

            impl<N> State for foo<N>
            where
                N: State,
                N::Flags: BitOr<U1>,
            {
                type Flags = Or<N::Flags, U1>;
                type Inited = N::Inited;

                unsafe fn assume_init(self) -> Self::Inited {
                    unsafe {
                        let this = ManuallyDrop::new(self);
                        core::ptr::read(&this.0).assume_init()
                    }
                }
            }

            impl<N> State for bar<N>
            where
                N: State,
                N::Flags: BitOr<Shleft<U1, U1>>,
            {
                type Flags = Or<N::Flags, U2>;
                type Inited = N::Inited;

                unsafe fn assume_init(self) -> Self::Inited {
                    unsafe {
                        let this = ManuallyDrop::new(self);
                        core::ptr::read(&this.0).assume_init()
                    }
                }
            }

            impl<N> Partial<N>
            where
                N: ThisPtr<Target = super::super::Foo> + State,
                N::Flags: BitAnd<U1, Output = U0>,
            {
                pub fn uninit_foo(&mut self) -> &mut MaybeUninit<i32> {
                    unsafe {
                        let ptr: *mut _ = &mut (*self.0.this_mut().as_mut_ptr()).foo;
                        &mut *ptr.cast()
                    }
                }

                pub unsafe fn assume_init_foo(self) -> Partial<foo<N>> {
                    Partial(foo(self.0))
                }

                pub fn with_foo(mut self, value: i32) -> Partial<foo<N>> {
                    unsafe { core::ptr::write(&mut (*self.0.this_mut().as_mut_ptr()).foo, value) };
                    Partial(foo(self.0))
                }

                pub fn emplace_foo(
                    mut self,
                    init: impl for<'a> FnOnce(&'a mut MaybeUninit<i32>) -> &'a mut i32,
                ) -> Partial<foo<N>> {
                    _ = init(self.uninit_foo());
                    Partial(foo(self.0))
                }
            }

            impl<N> Partial<N>
            where
                N: ThisPtr<Target = super::super::Foo> + State,
                N::Flags: BitAnd<U2, Output = U0>,
            {
                pub fn uninit_bar(&mut self) -> &mut MaybeUninit<f32> {
                    unsafe {
                        let ptr: *mut _ = &mut (*self.0.this_mut().as_mut_ptr()).bar;
                        &mut *ptr.cast()
                    }
                }

                pub unsafe fn assume_init_bar(self) -> Partial<bar<N>> {
                    Partial(bar(self.0))
                }

                pub fn with_bar(mut self, value: f32) -> Partial<bar<N>> {
                    unsafe { core::ptr::write(&mut (*self.0.this_mut().as_mut_ptr()).bar, value) };
                    Partial(bar(self.0))
                }

                pub fn emplace_bar(
                    mut self,
                    init: impl for<'a> FnOnce(&'a mut MaybeUninit<f32>) -> &'a mut f32,
                ) -> Partial<bar<N>> {
                    _ = init(self.uninit_bar());
                    Partial(bar(self.0))
                }
            }

            impl<N> Partial<N>
            where
                N: ThisPtr<Target = super::super::Foo> + State,
                N::Flags: BitAnd<U1, Output = U1>,
            {
                pub fn set_foo(&mut self, value: i32) -> &mut Self {
                    *self.get_mut_foo() = value;
                    self
                }
                pub fn get_foo(&self) -> &i32 {
                    unsafe { &(*self.0.this().as_ptr()).foo }
                }
                pub fn get_mut_foo(&mut self) -> &mut i32 {
                    unsafe { &mut (*self.0.this_mut().as_mut_ptr()).foo }
                }
            }

            impl<N> Partial<N>
            where
                N: ThisPtr<Target = super::super::Foo> + State,
                N::Flags: BitAnd<U2, Output = U2>,
            {
                pub fn set_bar(&mut self, value: f32) -> &mut Self {
                    *self.get_mut_bar() = value;
                    self
                }
                pub fn get_bar(&self) -> &f32 {
                    unsafe { &(*self.0.this().as_ptr()).bar }
                }
                pub fn get_mut_bar(&mut self) -> &mut f32 {
                    unsafe { &mut (*self.0.this_mut().as_mut_ptr()).bar }
                }
            }

            impl<N> Partial<N>
            where
                N: State<Flags = U3>,
            {
                pub fn done(self) -> N::Inited {
                    unsafe { self.0.assume_init() }
                }
            }

            impl<N> crate::CtorComplete<N::Inited> for Partial<N>
            where
                N: State<Flags = U3>,
            {
                fn done(self) -> N::Inited {
                    self.done()
                }
            }
        }
    }

    #[test]
    fn test1() {
        let a = Foo::partial(Box::new_uninit());
        let mut a = a.with_foo(1);
        a.set_foo(123);
        let a = a.with_bar(456.0).done();
        assert_eq!(a.foo, 123);
        assert_eq!(a.bar, 456.0)
    }
}

#[cfg(test)]
pub mod test_macro {
    use super::*;

    #[partial]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    #[test]
    fn macro_generates_partial() {
        let a = Foo::partial(Box::new_uninit());
        let mut a = a.with_foo(1);
        a.set_foo(123);
        let a = a.with_bar(456.0).done();
        assert_eq!(a.foo, 123);
        assert_eq!(a.bar, 456.0);
    }
}
