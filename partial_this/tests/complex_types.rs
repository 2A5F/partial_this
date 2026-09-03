//! Integration tests covering complex field types, structs with lifetime
//! parameters, and generic structs.

#![allow(clippy::box_collection)]

mod complex_fields {
    use partial_this::partial;

    #[partial]
    pub struct Complex {
        pub name: &'static str,
        pub values: Vec<i32>,
        pub maybe: Option<String>,
        pub bytes: [u8; 4],
        pub boxed: Box<String>,
    }

    #[test]
    fn builds_complex() {
        let p = Complex::partial(Box::new_uninit());
        let a = p.with_name("hi");
        let b = a.with_values([1, 2, 3].to_vec());
        let c = b.with_maybe(Some("x".to_string()));
        let d = c.with_bytes([1, 2, 3, 4]);
        let r = d.with_boxed(Box::new("y".to_string())).done();
        assert_eq!(r.name, "hi");
        assert_eq!(r.values, vec![1, 2, 3]);
        assert_eq!(r.maybe, Some("x".to_string()));
        assert_eq!(r.bytes, [1, 2, 3, 4]);
        assert_eq!(&*r.boxed, "y");
    }
}

mod lifetime_struct {
    use partial_this::partial;

    #[partial]
    pub struct Borrowed<'a> {
        pub name: &'a str,
        pub len: usize,
    }

    #[test]
    fn builds_lifetime() {
        let owned = String::from("hello");
        let p = Borrowed::partial(Box::new_uninit());
        let a = p.with_name(owned.as_str());
        let r = a.with_len(owned.len()).done();
        assert_eq!(r.name, "hello");
        assert_eq!(r.len, 5);
    }
}

mod generic_struct {
    use partial_this::partial;

    #[partial]
    pub struct Generic<T> {
        pub value: T,
        pub count: usize,
    }

    #[test]
    fn builds_generic() {
        let p = Generic::<String>::partial(Box::new_uninit());
        let a = p.with_value("hello".to_string());
        let r = a.with_count(1).done();
        assert_eq!(r.value, "hello");
        assert_eq!(r.count, 1);
    }
}

mod bounded_generic {
    use partial_this::partial;

    #[partial]
    pub struct Bounded<T>
    where
        T: Clone,
    {
        pub value: T,
        pub extra: Vec<T>,
    }

    #[test]
    fn builds_bounded() {
        let p = Bounded::<String>::partial(Box::new_uninit());
        let a = p.with_value("x".to_string());
        let r = a.with_extra(vec!["y".to_string()]).done();
        assert_eq!(r.value, "x");
        assert_eq!(r.extra, vec!["y".to_string()]);
    }
}

mod lifetime_and_generic {
    use partial_this::partial;

    #[partial]
    pub struct Both<'a, T> {
        pub name: &'a str,
        pub value: T,
    }

    #[test]
    fn builds_both() {
        let owned = String::from("hi");
        let p = Both::<'_, String>::partial(Box::new_uninit());
        let a = p.with_name(owned.as_str());
        let r = a.with_value(String::from("there")).done();
        assert_eq!(r.name, "hi");
        assert_eq!(r.value, "there");
    }
}

mod generic_name_collision {
    use partial_this::partial;

    #[partial]
    pub struct Collision<F, N, C, U, const INIT: usize> {
        pub f: F,
        pub n: N,
        pub c: C,
        pub u: U,
        pub data: [u8; INIT],
    }

    #[test]
    fn builds() {
        let p = Collision::<String, i32, f64, char, 4>::partial(Box::new_uninit());
        let a = p.with_f(String::from("f"));
        let b = a.with_n(1);
        let c = b.with_c(2.0);
        let d = c.with_u('x');
        let r = d.with_data([0u8; 4]).done();
        assert_eq!(r.f, "f");
        assert_eq!(r.n, 1);
        assert_eq!(r.c, 2.0);
        assert_eq!(r.u, 'x');
        assert_eq!(r.data, [0u8; 4]);
    }
}

mod tuple_struct {
    use partial_this::partial;

    #[partial]
    pub struct Tuple(i32, f32);

    #[test]
    fn builds_tuple() {
        let p = Tuple::partial(Box::new_uninit());
        let mut a = p.with__0(1);
        a.set__0(123);
        let r = a.with__1(456.0).done();
        assert_eq!(r.0, 123);
        assert_eq!(r.1, 456.0);
    }
}

mod generic_tuple {
    use partial_this::partial;

    #[partial]
    pub struct GenTuple<T, U>(pub T, pub U);

    #[test]
    fn builds_generic_tuple() {
        let p = GenTuple::<String, i32>::partial(Box::new_uninit());
        let a = p.with__0(String::from("x"));
        let r = a.with__1(1).done();
        assert_eq!(r.0, "x");
        assert_eq!(r.1, 1);
    }
}

mod private_field {
    use partial_this::partial;

    #[partial]
    pub struct Mixed {
        pub a: i32,
        b: f32,
    }

    #[test]
    fn builds() {
        let p = Mixed::partial(Box::new_uninit());
        let x = p.with_a(1);
        let r = x.with_b(2.0).done();
        assert_eq!(r.a, 1);
        assert_eq!(r.b, 2.0);
    }
}

mod no_pub_use {
    use partial_this::partial;

    #[partial(pub_use = false)]
    pub struct NoExport {
        pub a: i32,
    }

    #[test]
    fn builds() {
        let p = NoExport::partial(Box::new_uninit());
        let r = p.with_a(1).done();
        assert_eq!(r.a, 1);
    }
}

mod private_module_export {
    use partial_this::partial;

    #[partial(pub_use = false)]
    pub struct Holder {
        pub a: i32,
        b: f32,
    }

    #[test]
    fn builds() {
        let p = Holder::partial(Box::new_uninit());
        let r = p.with_a(1).with_b(2.0).done();
        assert_eq!(r.a, 1);
        assert_eq!(r.b, 2.0);
    }
}
