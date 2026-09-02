//! Integration test verifying that partially-built values drop already-initialized
//! fields in the reverse order of initialization (the last field set is dropped
//! first), not in declaration order. It also exercises custom (non-prelude)
//! field types in the generated builder.

use partial_this::{PartialThis, partial};
use std::cell::RefCell;

thread_local! {
    static DROP_ORDER: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

struct Droppable(&'static str);

impl Drop for Droppable {
    fn drop(&mut self) {
        DROP_ORDER.with(|v| v.borrow_mut().push(self.0));
    }
}

#[partial]
pub struct S {
    a: Droppable,
    b: Droppable,
}

#[test]
fn drops_in_reverse_init_order() {
    DROP_ORDER.with(|v| v.borrow_mut().clear());

    let p = S::partial(Box::new_uninit());
    let p = p.a(Droppable("a"));
    let p = p.b(Droppable("b"));
    drop(p);

    let order = DROP_ORDER.with(|v| v.borrow().clone());
    assert_eq!(order, vec!["b", "a"]);
}
