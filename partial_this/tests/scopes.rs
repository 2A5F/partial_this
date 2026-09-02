//! Integration tests covering name-resolution / scope edge cases in the
//! generated module, e.g. field names that collide with sibling modules and
//! module-qualified field types.

mod module_name_conflict {
    use partial_this::{PartialThis, partial};

    mod a {}

    #[partial]
    pub struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds_with_conflicting_module_name() {
        let p = Foo::partial(Box::new_uninit());
        let foo = p.a(1).done();
        assert_eq!(foo.a, 1);
    }
}

mod module_qualified_field_type {
    use partial_this::{PartialThis, partial};

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
        let h = p.v(inner::Value(7)).done();
        assert_eq!(h.v.0, 7);
    }
}

mod custom_local_field_type {
    use partial_this::{PartialThis, partial};

    pub struct Local(pub i32);

    #[partial]
    pub struct Blob {
        pub data: Local,
    }

    #[test]
    fn builds_with_local_field_type() {
        let p = Blob::partial(Box::new_uninit());
        let b = p.data(Local(9)).done();
        assert_eq!(b.data.0, 9);
    }
}

// `pub` struct whose fields are all `pub`: builder is re-exported with `pub use`.
mod all_pub {
    use partial_this::{PartialThis, partial};

    #[partial]
    pub struct Foo {
        pub a: i32,
        pub b: f32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).a(1).b(2.0).done();
        assert_eq!((foo.a, foo.b), (1, 2.0));
    }
}

// `pub` struct with a private field: builder re-export is module-local, no leak.
mod mixed_visibility {
    use partial_this::{PartialThis, partial};

    #[partial]
    pub struct Foo {
        pub a: i32,
        b: f32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).a(1).b(2.0).done();
        assert_eq!((foo.a, foo.b), (1, 2.0));
    }
}

// `pub_use = false` forces a module-local builder re-export.
mod pub_use_false_config {
    use partial_this::{PartialThis, partial};

    #[partial(pub_use = false)]
    pub struct Foo {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let foo = Foo::partial(Box::new_uninit()).a(1).done();
        assert_eq!(foo.a, 1);
    }
}
