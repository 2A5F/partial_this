//! Type-safe partial construction.
//!
//! The [`partial`] attribute macro lets you build a struct field-by-field,
//! while the type system guarantees that each field is initialized at most once
//! and that [`done`](DonePartial::done) is only callable once every field has
//! been set.
//!
//! # Example
//!
//! ```rust
//! use partial_this::{partial, DonePartial, PartialThis};
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
//!         .foo(1)
//!         .bar(1.0)
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
//! use partial_this::{partial, DonePartial, PartialThis};
//! use core::mem::MaybeUninit;
//!
//! #[partial]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     let a: Box<Foo> = Foo::partial(Box::new_uninit()).foo(1).bar(2.0).done();
//!     let b: Foo = Foo::partial(MaybeUninit::uninit()).foo(1).bar(2.0).done();
//!     let mut buf = MaybeUninit::uninit();
//!     let c: &mut Foo = Foo::partial(&mut buf).foo(1).bar(2.0).done();
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
//! use partial_this::{partial, DonePartial, PartialThis};
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
//!     let p = p.bar(1.0);
//!     let p = p.foo(1); // `foo` was not set yet, so this is allowed
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
//! use partial_this::{partial, DonePartial, PartialThis};
//!
//! #[partial]
//! pub struct Foo {
//!     pub foo: i32,
//!     pub bar: f32,
//! }
//!
//! fn main() {
//!     let mut p = Foo::partial(Box::new_uninit()).foo(1);
//!     assert_eq!(*p.foo(), 1);
//!     *p.foo_mut() = 123;
//!     let foo = p.bar(2.0).done();
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
//! use partial_this::{partial, DonePartial, PartialThis};
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
//!     let bar = Bar::partial(Box::new_uninit())._0(1)._1(2.0).done();
//!     assert_eq!(bar.0, 1);
//!     assert_eq!(bar.1, 2.0);
//!
//!     let pair = Pair::<String>::partial(Box::new_uninit())
//!         .name("x")
//!         .value(String::from("y"))
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
//! - **Field initialization methods**: each field has a builder method that can
//!   be called in any order, and at most once (a second call is a compile time error).
//! - **Field access methods**: after a field is initialized you can read it with
//!   `field()` or get a mutable reference with `field_mut()`.
//! - **`done()`**: finalizes the builder once every field is initialized.
//! - **Drop safety**: partially-built values drop already-initialized fields in
//!   declaration order.
//! - **Field visibility**: `pub` fields are re-exported; private fields are
//!   collected into a nested `private` module.
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
use core::{
    marker::{PhantomData, PhantomPinned},
    mem::{ManuallyDrop, MaybeUninit},
};

pub use typenum;
use typenum::{False, True};

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
    pub trait UninitThis {
        /// The type to be constructed.
        type Target;

        /// The type produced once [`assume_init`](Self::assume_init) is called.
        type Inited;

        /// Borrows the underlying uninitialized storage.
        unsafe fn get(&self) -> &MaybeUninit<Self::Target>;

        /// Mutably borrows the underlying uninitialized storage.
        unsafe fn get_mut(&mut self) -> &mut MaybeUninit<Self::Target>;

        /// Assumes the storage has been initialized and returns the value.
        unsafe fn assume_init(self) -> Self::Inited;
    }

    impl<'a, T> UninitThis for &'a mut MaybeUninit<T> {
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
    impl<T> UninitThis for MaybeUninit<T> {
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
    impl<T> UninitThis for Box<MaybeUninit<T>> {
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
    impl<T> UninitThis for Rc<MaybeUninit<T>> {
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
    impl<T> UninitThis for Arc<MaybeUninit<T>> {
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

/// A struct that can be partially constructed.
///
/// Implemented by the [`partial`] macro for a struct.
/// [`partial`](Self::partial) starts the builder with some uninitialized
/// storage (`this`), returning a [`chain::Field`] chain that can be filled
/// field-by-field and finally finalized with [`DonePartial::done`].
pub trait PartialThis<Src> {
    /// The builder type returned from [`partial`](Self::partial).
    type Output;

    /// Starts building a value of `Self` inside the given uninitialized storage.
    fn partial(this: Src) -> Self::Output;
}

/// Finalizes a partial builder once every field has been initialized.
///
/// Implemented for [`UninitThis`] sources and for [`chain::Field`] chains whose
/// every field has been initialized (tracked at the type level).
pub trait DonePartial {
    /// The finalized, fully-initialized value.
    type Output;

    /// Assumes the target value is initialized and returns it.
    fn done(self) -> Self::Output;
}

/// The type-level builder chain used by the `partial` macro.
///
/// Each struct field is represented as a [`Field`](crate::chain::Field) node;
/// nodes are nested to form a linked list whose type encodes both the next field
/// to initialize and which fields are currently initialized. The
/// [`traits`](crate::chain::traits) module provides the field descriptors and
/// chain-walking logic.
pub mod chain {
    use super::*;

    /// A node in the partial-construction chain.
    ///
    /// - `const INIT` tracks (at the type level) whether this node's field has
    ///   been initialized.
    /// - `F` is the field descriptor (implements [`traits::Field`]).
    /// - `N` is the rest of the chain (the already-built part).
    #[derive(Debug)]
    pub struct Field<const INIT: bool, F, N>(N, PhantomData<(F, PhantomPinned)>)
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr;

    impl<const INIT: bool, F, N> Field<INIT, F, N>
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr,
    {
        /// Wraps `next` as a chain node
        #[cfg_attr(not(debug_assertions), inline(always))]
        pub const fn keep(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    impl<F, N> Field<true, F, N>
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr,
    {
        /// Marks the field as initialized and wraps the rest of the chain.
        #[cfg_attr(not(debug_assertions), inline(always))]
        pub const fn init(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    impl<F, N> Field<false, F, N>
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr,
    {
        /// Marks the field as uninitialized and wraps the rest of the chain.
        #[cfg_attr(not(debug_assertions), inline(always))]
        pub const fn uninit(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    /// Traits that describe struct fields and drive the chain-walking logic.
    pub mod traits {
        use typenum::{IsEqual, U0, Unsigned};

        use super::*;

        impl<U: UninitThis> DonePartial for U {
            type Output = U::Inited;

            fn done(self) -> Self::Output {
                unsafe { self.assume_init() }
            }
        }

        impl<F, N> DonePartial for super::Field<true, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
            N: DonePartial,
        {
            type Output = N::Output;

            #[cfg_attr(not(debug_assertions), inline(always))]
            fn done(self) -> Self::Output {
                unsafe {
                    let this = ManuallyDrop::new(self);
                    let n = core::ptr::read(&this.0);
                    n.done()
                }
            }
        }

        /// Describes a single struct field for the partial builder.
        ///
        /// Implemented by the `partial` macro for each field of the struct.
        pub trait Field {
            /// The struct type that owns this field.
            type Target;
            /// The field's type.
            type Type;
            /// A unique type-level id for this field ([`typenum`] unsigned).
            type Id: Unsigned;

            /// Drops the field's value if the type-level `INIT` flag is set.
            unsafe fn drop<const INIT: bool>(this: &mut MaybeUninit<Self::Target>);

            /// Writes `value` into the field.
            unsafe fn init(this: &mut MaybeUninit<Self::Target>, value: Self::Type);

            /// Borrows the initialized field value.
            unsafe fn get(this: &MaybeUninit<Self::Target>) -> &Self::Type;

            /// Mutably borrows the initialized field value.
            unsafe fn get_mut(this: &mut MaybeUninit<Self::Target>) -> &mut Self::Type;
        }

        /// Provides access to the underlying uninitialized storage of a chain node.
        pub trait ThisPtr {
            /// The struct type being constructed.
            type Target;
            /// The field id of this node.
            type Id: Unsigned;
            /// The field id of the next node in the chain.
            type NextId: Unsigned;

            /// Borrows the underlying `MaybeUninit` storage.
            fn this(this: &Self) -> &MaybeUninit<Self::Target>;

            /// Mutably borrows the underlying `MaybeUninit` storage.
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target>;
        }

        impl<U: UninitThis> ThisPtr for U {
            type Id = U0;
            type NextId = U0;
            type Target = U::Target;

            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                unsafe { this.get() }
            }
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                unsafe { this.get_mut() }
            }
        }

        impl<const INIT: bool, F, N> ThisPtr for super::Field<INIT, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
        {
            type Id = F::Id;
            type NextId = N::Id;
            type Target = N::Target;

            #[cfg_attr(not(debug_assertions), inline(always))]
            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                N::this(&this.0)
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                N::this_mut(&mut this.0)
            }
        }

        impl<const INIT: bool, F, N> Drop for super::Field<INIT, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
        {
            #[cfg_attr(not(debug_assertions), inline(always))]
            fn drop(&mut self) {
                unsafe { F::drop::<INIT>(N::this_mut(&mut self.0)) };
            }
        }

        /// Drives a field-initialization operation across the chain.
        ///
        /// Given the value type `A`, the field id `I`, and the current chain
        /// context `C`, it returns a chain where the field has been initialized.
        pub trait MapInit<A, I, C>: ThisPtr {
            /// The resulting chain after initialization.
            type Result: ThisPtr<Target = Self::Target>;
            /// The rest of the chain.
            type Next;
            /// The rest of the chain after initialization.
            type NextResult;

            /// Initializes the field with `value`, returning the new chain.
            unsafe fn map_init(this: Self, value: A) -> Self::Result;

            /// Marks the field initialized without writing a value.
            unsafe fn assume_init(this: Self) -> Self::Result;
        }

        impl<A, I, T, U: UninitThis<Target = T>> MapInit<A, I, False> for U {
            type Result = Self;
            type Next = Self;
            type NextResult = Self;

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn map_init(_this: Self, _value: A) -> Self::Result {
                unreachable!("never")
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn assume_init(_this: Self) -> Self::Result {
                unreachable!("never")
            }
        }

        impl<const INIT: bool, A, F, I, N> MapInit<A, I, False> for super::Field<INIT, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
            F::Id: IsEqual<I, Output = False>,
            I: IsEqual<Self::NextId>,
            I: IsEqual<N::Id>,
            N: MapInit<A, I, <I as IsEqual<Self::NextId>>::Output>,
        {
            type Result = super::Field<INIT, F, N::Result>;
            type Next = N;
            type NextResult = N::Result;

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn map_init(this: Self, value: A) -> Self::Result {
                unsafe {
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::keep(N::map_init(n, value))
                }
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn assume_init(this: Self) -> Self::Result {
                unsafe {
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::keep(N::assume_init(n))
                }
            }
        }

        impl<A, F, I, N> MapInit<A, I, True> for super::Field<false, F, N>
        where
            F: Field<Target = N::Target, Type = A>,
            N: ThisPtr,
            F::Id: IsEqual<I, Output = True>,
            N: MapInit<A, I, False>,
        {
            type Result = super::Field<true, F, N>;
            type Next = N;
            type NextResult = N;

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn map_init(mut this: Self, value: A) -> Self::Result {
                unsafe {
                    F::init(Self::this_mut(&mut this), value);
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::init(n)
                }
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn assume_init(this: Self) -> Self::Result {
                unsafe {
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::init(n)
                }
            }
        }

        /// Reads an already-initialized field's value from the chain.
        ///
        /// Given the value type `A`, the field id `I` and the chain context `C`,
        /// returns a reference to the initialized field.
        pub trait GetField<A, I, C>: ThisPtr {
            /// Borrows the initialized field value.
            unsafe fn get<'a>(this: &'a Self) -> &'a A;

            /// Mutably borrows the initialized field value.
            unsafe fn get_mut<'a>(this: &'a mut Self) -> &'a mut A;
        }

        impl<const INIT: bool, A, F, I, N> GetField<A, I, False> for super::Field<INIT, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
            F::Id: IsEqual<I, Output = False>,
            I: IsEqual<Self::NextId>,
            I: IsEqual<N::Id>,
            N: GetField<A, I, <I as IsEqual<Self::NextId>>::Output>,
        {
            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn get<'a>(this: &'a Self) -> &'a A {
                unsafe { N::get(&this.0) }
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn get_mut<'a>(this: &'a mut Self) -> &'a mut A {
                unsafe { N::get_mut(&mut this.0) }
            }
        }

        impl<A, F, I, N> GetField<A, I, True> for super::Field<true, F, N>
        where
            F: Field<Target = N::Target, Type = A>,
            N: ThisPtr,
            F::Id: IsEqual<I, Output = True>,
        {
            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn get<'a>(this: &'a Self) -> &'a A {
                unsafe { F::get(N::this(&this.0)) }
            }

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn get_mut<'a>(this: &'a mut Self) -> &'a mut A {
                unsafe { F::get_mut(N::this_mut(&mut this.0)) }
            }
        }
    }
}

#[cfg(test)]
#[allow(nonstandard_style)]
pub mod test {
    #[derive(Debug)]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    use crate::{DonePartial, PartialThis};
    use core::mem::MaybeUninit;

    pub use should_be_generated_by_macro::*;
    mod should_be_generated_by_macro {
        use crate::{
            PartialThis, UninitThis,
            chain::{self},
        };

        use super::*;

        impl<U: UninitThis<Target = Foo>> PartialThis<U> for Foo {
            type Output = chain::Field<false, fields::bar, chain::Field<false, fields::foo, U>>;

            fn partial(this: U) -> Self::Output {
                chain::Field::uninit(chain::Field::uninit(this))
            }
        }

        mod fields {
            use crate::chain::traits::Field;

            #[derive(Debug)]
            pub struct foo;
            #[derive(Debug)]
            pub struct bar;

            impl Field for foo {
                type Target = super::Foo;
                type Type = i32;
                type Id = crate::typenum::U1;

                unsafe fn drop<const INIT: bool>(this: &mut std::mem::MaybeUninit<Self::Target>) {
                    if INIT {
                        unsafe { core::ptr::drop_in_place(&mut (*this.as_mut_ptr()).foo) };
                    }
                }

                unsafe fn init(this: &mut std::mem::MaybeUninit<Self::Target>, value: Self::Type) {
                    unsafe { core::ptr::write(&mut (*this.as_mut_ptr()).foo, value) }
                }

                unsafe fn get(this: &std::mem::MaybeUninit<Self::Target>) -> &Self::Type {
                    unsafe { &(*this.as_ptr()).foo }
                }

                unsafe fn get_mut(
                    this: &mut std::mem::MaybeUninit<Self::Target>,
                ) -> &mut Self::Type {
                    unsafe { &mut (*this.as_mut_ptr()).foo }
                }
            }

            impl Field for bar {
                type Target = super::Foo;
                type Type = f32;
                type Id = crate::typenum::U2;

                unsafe fn drop<const INIT: bool>(this: &mut std::mem::MaybeUninit<Self::Target>) {
                    if INIT {
                        unsafe { core::ptr::drop_in_place(&mut (*this.as_mut_ptr()).bar) };
                    }
                }

                unsafe fn init(this: &mut std::mem::MaybeUninit<Self::Target>, value: Self::Type) {
                    unsafe { core::ptr::write(&mut (*this.as_mut_ptr()).bar, value) }
                }

                unsafe fn get(this: &std::mem::MaybeUninit<Self::Target>) -> &Self::Type {
                    unsafe { &(*this.as_ptr()).bar }
                }

                unsafe fn get_mut(
                    this: &mut std::mem::MaybeUninit<Self::Target>,
                ) -> &mut Self::Type {
                    unsafe { &mut (*this.as_mut_ptr()).bar }
                }
            }
        }

        pub use uninit_fields::*;
        mod uninit_fields {
            use super::*;
            use crate::chain::{self, traits::MapInit};
            use crate::typenum::{U1, U2};

            pub trait Foo_uninit_foo<T> {
                type Output;
                fn foo(self, value: i32) -> Self::Output;
                fn assume_init_foo(self) -> Self::Output;
            }
            pub trait Foo_uninit_bar<T> {
                type Output;
                fn bar(self, value: f32) -> Self::Output;
                fn assume_init_bar(self) -> Self::Output;
            }

            impl<const INIT: bool, F, N, C> Foo_uninit_foo<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = Foo>,
                F: chain::traits::Field<Target = Foo>,
                Self: MapInit<i32, U1, C>,
            {
                type Output = <Self as MapInit<i32, U1, C>>::Result;

                fn foo(self, value: i32) -> Self::Output {
                    unsafe { Self::map_init(self, value) }
                }

                fn assume_init_foo(self) -> Self::Output {
                    unsafe { Self::assume_init(self) }
                }
            }

            impl<const INIT: bool, F, N, C> Foo_uninit_bar<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = Foo>,
                F: chain::traits::Field<Target = Foo>,
                Self: MapInit<f32, U2, C>,
            {
                type Output = <Self as MapInit<f32, U2, C>>::Result;

                fn bar(self, value: f32) -> Self::Output {
                    unsafe { Self::map_init(self, value) }
                }

                fn assume_init_bar(self) -> Self::Output {
                    unsafe { Self::assume_init(self) }
                }
            }
        }

        pub use inited_fields::*;
        mod inited_fields {
            use super::*;
            use crate::chain::{self, traits::GetField};
            use crate::typenum::{U1, U2};

            pub trait Foo_inited_foo<T> {
                fn foo(&self) -> &i32;
                fn foo_mut(&mut self) -> &mut i32;
            }
            pub trait Foo_inited_bar<T> {
                fn bar(&self) -> &f32;
                fn bar_mut(&mut self) -> &mut f32;
            }

            impl<const INIT: bool, F, N, C> Foo_inited_foo<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = Foo>,
                F: chain::traits::Field<Target = Foo>,
                Self: GetField<i32, U1, C>,
            {
                fn foo(&self) -> &i32 {
                    unsafe { Self::get(self) }
                }

                fn foo_mut(&mut self) -> &mut i32 {
                    unsafe { Self::get_mut(self) }
                }
            }

            impl<const INIT: bool, F, N, C> Foo_inited_bar<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = Foo>,
                F: chain::traits::Field<Target = Foo>,
                Self: GetField<f32, U2, C>,
            {
                fn bar(&self) -> &f32 {
                    unsafe { Self::get(self) }
                }

                fn bar_mut(&mut self) -> &mut f32 {
                    unsafe { Self::get_mut(self) }
                }
            }
        }
    }

    #[test]
    fn test1() {
        let foo = Foo::partial(Box::new_uninit());
        let mut a = foo.foo(1);
        *a.foo_mut() = 123;
        let foo = a.bar(456.0).done();
        println!("r: {:?}", foo);
    }

    #[test]
    fn test2() {
        let mut foo = MaybeUninit::uninit();
        let foo = Foo::partial(&mut foo);
        let mut a = foo.foo(1);
        *a.foo_mut() = 123;
        let foo = a.bar(456.0).done();
        println!("r: {:?}", foo);
    }

    #[test]
    fn test3() {
        let foo = Foo::partial(MaybeUninit::uninit());
        let mut a = foo.foo(1);
        *a.foo_mut() = 123;
        let foo = a.bar(456.0).done();
        println!("r: {:?}", foo);
    }
}

#[cfg(test)]
#[allow(nonstandard_style)]
pub mod test1 {
    use super::*;

    #[partial]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    #[test]
    fn macro_generates_partial() {
        let foo = Foo::partial(Box::new_uninit());
        let mut a = foo.foo(1);
        *a.foo_mut() = 123;
        let foo = a.bar(456.0).done();
        assert_eq!(foo.foo, 123);
        assert_eq!(foo.bar, 456.0);
    }
}
