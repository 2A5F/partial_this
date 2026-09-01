use std::{
    marker::{PhantomData, PhantomPinned},
    mem::{ManuallyDrop, MaybeUninit},
};

use typenum::{False, True};

pub trait PartialThis<Src> {
    type Output;
    fn partial(this: Src) -> Self::Output;
}

pub trait DonePartial {
    type Output;

    fn done(self) -> Self::Output;
}

pub mod chain {
    use super::*;

    #[derive(Debug)]
    pub struct ScopedThis<'a, T>(&'a mut MaybeUninit<T>);

    impl<'a, T> ScopedThis<'a, T> {
        pub const fn new(this: &'a mut MaybeUninit<T>) -> Self {
            Self(this)
        }
    }

    #[derive(Debug)]
    pub struct ValueThis<T>(MaybeUninit<T>);

    impl<T> ValueThis<T> {
        pub const fn new(this: MaybeUninit<T>) -> Self {
            Self(this)
        }
    }

    #[derive(Debug)]
    pub struct BoxedThis<T>(Box<MaybeUninit<T>>);

    impl<T> BoxedThis<T> {
        pub const fn new(this: Box<MaybeUninit<T>>) -> Self {
            Self(this)
        }
    }

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
        pub const fn keep(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    impl<F, N> Field<true, F, N>
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr,
    {
        pub const fn init(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    impl<F, N> Field<false, F, N>
    where
        F: traits::Field<Target = N::Target>,
        N: traits::ThisPtr,
    {
        pub const fn uninit(next: N) -> Self {
            Self(next, PhantomData)
        }
    }

    pub mod traits {
        use typenum::{IsEqual, U0, Unsigned};

        use super::*;

        impl<'a, T> DonePartial for ScopedThis<'a, T> {
            type Output = &'a mut T;

            fn done(self) -> Self::Output {
                unsafe { self.0.assume_init_mut() }
            }
        }

        impl<T> DonePartial for ValueThis<T> {
            type Output = T;

            fn done(self) -> Self::Output {
                unsafe { self.0.assume_init() }
            }
        }

        impl<T> DonePartial for BoxedThis<T> {
            type Output = Box<T>;

            fn done(self) -> Self::Output {
                unsafe { self.0.assume_init() }
            }
        }

        impl<F, N> DonePartial for super::Field<true, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
            N: DonePartial,
        {
            type Output = N::Output;

            fn done(self) -> Self::Output {
                unsafe {
                    let this = ManuallyDrop::new(self);
                    let n = core::ptr::read(&this.0);
                    n.done()
                }
            }
        }

        pub trait Field {
            type Target;
            type Type;
            type Id: Unsigned;

            unsafe fn drop<const INIT: bool>(this: &mut MaybeUninit<Self::Target>);

            unsafe fn init(this: &mut MaybeUninit<Self::Target>, value: Self::Type);

            unsafe fn get(this: &MaybeUninit<Self::Target>) -> &Self::Type;
            unsafe fn get_mut(this: &mut MaybeUninit<Self::Target>) -> &mut Self::Type;
        }

        pub trait ThisPtr {
            type Target;
            type Id: Unsigned;
            type NextId: Unsigned;

            fn this(this: &Self) -> &MaybeUninit<Self::Target>;
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target>;
        }

        impl<'a, T> ThisPtr for ScopedThis<'a, T> {
            type Id = U0;
            type NextId = U0;
            type Target = T;

            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                &this.0
            }
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                &mut this.0
            }
        }

        impl<T> ThisPtr for ValueThis<T> {
            type Id = U0;
            type NextId = U0;
            type Target = T;

            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                &this.0
            }
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                &mut this.0
            }
        }

        impl<T> ThisPtr for BoxedThis<T> {
            type Id = U0;
            type NextId = U0;
            type Target = T;

            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                &this.0
            }
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                &mut this.0
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

            fn this(this: &Self) -> &MaybeUninit<Self::Target> {
                N::this(&this.0)
            }
            fn this_mut(this: &mut Self) -> &mut MaybeUninit<Self::Target> {
                N::this_mut(&mut this.0)
            }
        }

        impl<const INIT: bool, F, N> Drop for super::Field<INIT, F, N>
        where
            F: Field<Target = N::Target>,
            N: ThisPtr,
        {
            fn drop(&mut self) {
                unsafe { F::drop::<INIT>(N::this_mut(&mut self.0)) };
            }
        }

        pub trait MapInit<A, I, C>: ThisPtr {
            type Result: ThisPtr<Target = Self::Target>;
            type Next;
            type NextResult;

            unsafe fn map_init(this: Self, value: A) -> Self::Result;
        }

        impl<'a, A, I, T> MapInit<A, I, False> for ScopedThis<'a, T> {
            type Result = Self;
            type Next = Self;
            type NextResult = Self;

            unsafe fn map_init(_this: Self, _value: A) -> Self::Result {
                unreachable!("never")
            }
        }

        impl<A, I, T> MapInit<A, I, False> for ValueThis<T> {
            type Result = Self;
            type Next = Self;
            type NextResult = Self;

            unsafe fn map_init(_this: Self, _value: A) -> Self::Result {
                unreachable!("never")
            }
        }

        impl<A, I, T> MapInit<A, I, False> for BoxedThis<T> {
            type Result = Self;
            type Next = Self;
            type NextResult = Self;

            unsafe fn map_init(_this: Self, _value: A) -> Self::Result {
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

            unsafe fn map_init(this: Self, value: A) -> Self::Result {
                unsafe {
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::keep(N::map_init(n, value))
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

            unsafe fn map_init(mut this: Self, value: A) -> Self::Result {
                unsafe {
                    F::init(Self::this_mut(&mut this), value);
                    let this = ManuallyDrop::new(this);
                    let n = core::ptr::read(&this.0);
                    super::Field::init(n)
                }
            }
        }

        pub trait GetField<A, I, C>: ThisPtr {
            unsafe fn get<'a>(this: &'a Self) -> &'a A;
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
            unsafe fn get<'a>(this: &'a Self) -> &'a A {
                unsafe { N::get(&this.0) }
            }

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
            unsafe fn get<'a>(this: &'a Self) -> &'a A {
                unsafe { F::get(N::this(&this.0)) }
            }

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

    use std::mem::MaybeUninit;

    use crate::{DonePartial, PartialThis};

    pub use should_be_generated_by_macro::*;
    mod should_be_generated_by_macro {
        use crate::{
            PartialThis,
            chain::{self, BoxedThis, ScopedThis, ValueThis},
        };

        use super::*;

        impl PartialThis<std::mem::MaybeUninit<Self>> for Foo {
            type Output = chain::Field<
                false,
                fields::bar,
                chain::Field<false, fields::foo, chain::ValueThis<Foo>>,
            >;

            fn partial(this: std::mem::MaybeUninit<Self>) -> Self::Output {
                chain::Field::uninit(chain::Field::uninit(ValueThis::new(this)))
            }
        }

        impl PartialThis<Box<std::mem::MaybeUninit<Self>>> for Foo {
            type Output = chain::Field<
                false,
                fields::bar,
                chain::Field<false, fields::foo, chain::BoxedThis<Foo>>,
            >;

            fn partial(this: Box<std::mem::MaybeUninit<Self>>) -> Self::Output {
                chain::Field::uninit(chain::Field::uninit(BoxedThis::new(this)))
            }
        }

        impl<'a> PartialThis<&'a mut std::mem::MaybeUninit<Self>> for Foo {
            type Output = chain::Field<
                false,
                fields::bar,
                chain::Field<false, fields::foo, chain::ScopedThis<'a, Foo>>,
            >;

            fn partial(this: &'a mut std::mem::MaybeUninit<Self>) -> Self::Output {
                chain::Field::uninit(chain::Field::uninit(ScopedThis::new(this)))
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
                type Id = typenum::U1;

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
                type Id = typenum::U2;

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
            use typenum::{U1, U2};

            pub trait Foo_uninit_foo<T> {
                type Output;
                fn foo(self, value: i32) -> Self::Output;
            }
            pub trait Foo_uninit_bar<T> {
                type Output;
                fn bar(self, value: f32) -> Self::Output;
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
            }
        }

        pub use inited_fields::*;
        mod inited_fields {
            use super::*;
            use crate::chain::{self, traits::GetField};
            use typenum::{U1, U2};

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
