extern crate conqueue;

struct MyStruct {
    _value: usize,
}

impl Drop for MyStruct {
    fn drop(&mut self) {}
}

fn test_memory_leaks_one() {
    let (tx, mut rx) = conqueue::Queue::unbounded();

    for i in 1..100_000 {
        tx.push(MyStruct { _value: i });
    }

    while let Some(_) = rx.pop() {}

    for i in 1..100_000 {
        tx.push(MyStruct { _value: i });
    }

    drop(rx);
    drop(tx);
}

fn test_memory_leaks_two() {
    let (tx, mut rx) = conqueue::Queue::unbounded();

    for i in 1..100_000 {
        tx.push(MyStruct { _value: i });
    }

    while let Some(_) = rx.pop() {}

    for i in 1..100_000 {
        tx.push(MyStruct { _value: i });
    }

    drop(tx);
    drop(rx);
}

fn main() {
    test_memory_leaks_one();
    test_memory_leaks_two();
}
