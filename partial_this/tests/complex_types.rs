//! Integration tests covering complex field types, structs with lifetime
//! parameters, and generic structs.

mod complex_fields {
    use partial_this::{DonePartial, PartialThis, partial};

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
        let a = p.name("hi");
        let b = a.values([1, 2, 3].to_vec());
        let c = b.maybe(Some("x".to_string()));
        let d = c.bytes([1, 2, 3, 4]);
        let r = d.boxed(Box::new("y".to_string())).done();
        assert_eq!(r.name, "hi");
        assert_eq!(r.values, vec![1, 2, 3]);
        assert_eq!(r.maybe, Some("x".to_string()));
        assert_eq!(r.bytes, [1, 2, 3, 4]);
        assert_eq!(&*r.boxed, "y");
    }
}

mod lifetime_struct {
    use partial_this::{DonePartial, PartialThis, partial};

    #[partial]
    pub struct Borrowed<'a> {
        pub name: &'a str,
        pub len: usize,
    }

    #[test]
    fn builds_lifetime() {
        let owned = String::from("hello");
        let p = Borrowed::partial(Box::new_uninit());
        let a = p.name(owned.as_str());
        let r = a.len(owned.len()).done();
        assert_eq!(r.name, "hello");
        assert_eq!(r.len, 5);
    }
}

mod generic_struct {
    use partial_this::{DonePartial, PartialThis, partial};

    #[partial]
    pub struct Generic<T> {
        pub value: T,
        pub count: usize,
    }

    #[test]
    fn builds_generic() {
        let p = Generic::<String>::partial(Box::new_uninit());
        let a = p.value("hello".to_string());
        let r = a.count(1).done();
        assert_eq!(r.value, "hello");
        assert_eq!(r.count, 1);
    }
}

mod bounded_generic {
    use partial_this::{DonePartial, PartialThis, partial};

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
        let a = p.value("x".to_string());
        let r = a.extra(vec!["y".to_string()]).done();
        assert_eq!(r.value, "x");
        assert_eq!(r.extra, vec!["y".to_string()]);
    }
}

mod lifetime_and_generic {
    use partial_this::{DonePartial, PartialThis, partial};

    #[partial]
    pub struct Both<'a, T> {
        pub name: &'a str,
        pub value: T,
    }

    #[test]
    fn builds_both() {
        let owned = String::from("hi");
        let p = Both::<'_, String>::partial(Box::new_uninit());
        let a = p.name(owned.as_str());
        let r = a.value(String::from("there")).done();
        assert_eq!(r.name, "hi");
        assert_eq!(r.value, "there");
    }
}

mod generic_name_collision {
    use partial_this::{DonePartial, PartialThis, partial};

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
        let a = p.f(String::from("f"));
        let b = a.n(1);
        let c = b.c(2.0);
        let d = c.u('x');
        let r = d.data([0u8; 4]).done();
        assert_eq!(r.f, "f");
        assert_eq!(r.n, 1);
        assert_eq!(r.c, 2.0);
        assert_eq!(r.u, 'x');
        assert_eq!(r.data, [0u8; 4]);
    }
}
