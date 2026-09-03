//! Integration tests covering name-resolution / scope edge cases in the
//! generated module, e.g. field names that collide with sibling modules and
//! module-qualified field types.

mod module_name_conflict {
    use partial_this::partial;

    mod a {}

    #[partial]
    pub struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds_with_conflicting_module_name() {
        let p = Foo::partial(Box::new_uninit());
        let foo = p.with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

mod module_qualified_field_type {
    use partial_this::partial;

    mod inner {
        pub struct Value(pub i32);
    }

    #[partial]
    pub struct Holder {
        pub v: inner::Value,
    }

    #[test]
    fn builds_with_module_qualified_field_type() {
        let p = Holder::partial(Box::new_uninit());
        let h = p.with_v(inner::Value(7)).done();
        assert_eq!(h.v.0, 7);
    }
}

mod custom_local_field_type {
    use partial_this::partial;

    pub struct Local(pub i32);

    #[partial]
    pub struct Blob {
        pub data: Local,
    }

    #[test]
    fn builds_with_local_field_type() {
        let p = Blob::partial(Box::new_uninit());
        let b = p.with_data(Local(9)).done();
        assert_eq!(b.data.0, 9);
    }
}

// `pub` struct whose fields are all `pub`: builder is re-exported with `pub use`.
mod all_pub {
    use partial_this::partial;

    #[partial]
    pub struct Foo {
        pub a: i32,
        pub b: f32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).with_b(2.0).done();
        assert_eq!((foo.a, foo.b), (1, 2.0));
    }
}

// `pub` struct with a private field: builder re-export is module-local, no leak.
mod mixed_visibility {
    use partial_this::partial;

    #[partial]
    pub struct Foo {
        pub a: i32,
        b: f32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).with_b(2.0).done();
        assert_eq!((foo.a, foo.b), (1, 2.0));
    }
}

// `pub_use = false` forces a module-local builder re-export.
mod pub_use_false_config {
    use partial_this::partial;

    #[partial(pub_use = false)]
    pub struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

// `pub_use = false` combined with a private struct.
mod pub_use_false_private_struct {
    use partial_this::partial;

    #[partial(pub_use = false)]
    struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

// `pub_use = false` combined with a pub(crate) struct.
mod pub_use_false_pub_crate_struct {
    use partial_this::partial;

    #[partial(pub_use = false)]
    pub(crate) struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

// `pub(crate)` struct: generated items are crate-visible, no public leak.
mod pub_crate_struct {
    use partial_this::partial;

    #[partial]
    pub(crate) struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

// Private struct: the builder is `pub(in ...)` scoped to the struct's module,
// so `Foo::partial` works but nothing leaks outside.
mod private_struct {
    use partial_this::partial;

    #[partial]
    struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).with_a(1).done();
        assert_eq!(foo.a, 1);
    }
}

// `pub` struct with a private field whose type is private: no public leak.
mod private_field_type {
    use partial_this::partial;

    struct Hidden(i32);

    #[partial]
    pub struct Foo {
        a: Hidden,
        pub b: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit())
            .with_a(Hidden(3))
            .with_b(1)
            .done();
        let Foo { a, b } = *foo;
        assert_eq!(a.0, 3);
        assert_eq!(b, 1);
    }
}
