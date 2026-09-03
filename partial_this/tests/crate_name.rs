//! Integration tests verifying that the `#[partial]` macro emits code that
//! references the `partial_this` crate by name, so it works when used from an
//! external crate, and that the `crate_name` config option is honoured.

mod default_crate_name {
    use partial_this::partial;

    #[partial]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    #[test]
    fn builds_partially() {
        let p = Foo::partial(Box::new_uninit());
        let mut a = p.with_foo(1);
        a.set_foo(123);
        let r = a.with_bar(456.0).done();
        assert_eq!(r.foo, 123);
        assert_eq!(r.bar, 456.0);
    }
}

mod ident_crate_name {
    use partial_this::partial;

    #[partial(crate_name = partial_this)]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    #[test]
    fn builds_partially() {
        let p = Foo::partial(Box::new_uninit());
        let mut a = p.with_foo(1);
        a.set_foo(123);
        let r = a.with_bar(456.0).done();
        assert_eq!(r.foo, 123);
        assert_eq!(r.bar, 456.0);
    }
}

mod string_crate_name {
    use partial_this::partial;

    #[partial(crate_name = "partial_this")]
    pub struct Foo {
        pub foo: i32,
        pub bar: f32,
    }

    #[test]
    fn builds_partially() {
        let p = Foo::partial(Box::new_uninit());
        let mut a = p.with_foo(1);
        a.set_foo(123);
        let r = a.with_bar(456.0).done();
        assert_eq!(r.foo, 123);
        assert_eq!(r.bar, 456.0);
    }
}
