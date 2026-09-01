# PartialThis

Type-safe partial construction

```rust

#[derive(Debug)]
pub struct Foo {
    pub foo: i32,
    pub bar: f32,
}

fn test()
{
    let p = Foo::partial(Box::new_uninit());
    let p = p.foo(1);
    let p = p.bar(1.0);
    let foo: Box<T> = p.done();
    println!("{:?}", foo);
}
```

- Field initialization methods. Can be called in any order and at most once.
  ```rust
  // ok
  let p = p.foo(1);
  let p = p.bar(1.0);
  ```
  ```rust
  // ok
  let p = p.bar(1.0);
  let p = p.foo(1);
  ```
  ```rust
  let p = p.foo(1);
  let p = p.foo(1); // error
  ```
- Field access methods. Can be called after initialization.
  ```rust
  let p = p.foo(1);
  println!("{}", p.foo());
  ```
  ```rust
  let mut p = p.foo(1);
  *p.foo_mut() = 123；
  ```
- Done method.Can be called after all fields are initialized.
  ```rust
  let foo = p.done();
  ```
- Drop safe. Will call fields `drop` according to the field initialization status and fields declare order.
  ```rust
  struct Some {
    field0: ...,
    field1: ...,
    field2: ...,
    ...
  }
  let p = Some::partial(Box::new_uninit());
  let p = p.field0(...);
  let p = p.field1(...);
  drop(p); // drop(field1) -> drop(field0) -> drop(Box<MaybeUninit<Some>>)
  ```
- Multi overloads
  ```rust
  let foo: Box<T> = Foo::partial(Box::new_uninit()).done();

  let foo: Foo = Foo::partial(MaybeUninit::uninit()).done();
  
  let mut foo = MaybeUninit::uninit();
  let foo: &mut Foo = Foo::partial(&mut foo).done();
  ```
