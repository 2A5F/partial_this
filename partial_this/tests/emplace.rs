//! Integration test for `emplace_field`, including building nested `#[partial]`
//! structs directly inside the parent's storage (placement).

use partial_this::{AnyUninit, partial};

#[partial]
#[derive(Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub fn new_in<U>(place: U, x: i32, y: i32) -> U::Inited
    where
        U: AnyUninit<Target = Self>,
    {
        Self::partial(place).with_x(x).with_y(y).done()
    }
}

#[partial]
#[derive(Debug)]
struct Line {
    start: Point,
    end: Point,
}

#[test]
fn emplaces_nested_struct_in_place() {
    // Build `Point` values directly inside `Line`'s storage, so they are
    // emplaced rather than copied from an intermediate value.
    let line = Line::partial(Box::new_uninit())
        .emplace_start(|slot| Point::partial(slot).with_x(1).with_y(2).done())
        .emplace_end(|slot| slot.write(Point { x: 3, y: 4 }))
        .done();

    assert_eq!(line.start, Point { x: 1, y: 2 });
    assert_eq!(line.end, Point { x: 3, y: 4 });
}

#[test]
fn emplaces_nested_struct_in_place_2() {
    // Build `Point` values directly inside `Line`'s storage, so they are
    // emplaced rather than copied from an intermediate value.
    let line = Line::partial(Box::new_uninit())
        .emplace_start(|slot| Point::new_in(slot, 1, 2))
        .emplace_end(|slot| slot.write(Point { x: 3, y: 4 }))
        .done();

    assert_eq!(line.start, Point { x: 1, y: 2 });
    assert_eq!(line.end, Point { x: 3, y: 4 });
}
