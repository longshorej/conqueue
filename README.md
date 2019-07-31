# Conqueue

[![Crates.io](https://img.shields.io/crates/v/conqueue.svg?style=flat-square)](https://crates.io/crates/conqueue)
[![Crates.io](https://img.shields.io/crates/d/conqueue.svg?style=flat-square)](https://crates.io/crates/conqueue)

Conqueue is another multi-producer, single-consumer queue (MPSC) for the Rust programming language.

## Getting Started

To get started, add the following to your `Cargo.toml` file:

```toml
[dependencies]
conqueue = "0.1.0"
```

Then, at the root of your crate:

```rust
extern crate conqueue
```

Finally, create a sender/receiver pair. The sender may be cloned to
allow concurrent producers, and it is both `Send` and `Sync`.

```rust
let (tx1, mut rx) = conqueue::Queue::unbounded();
let tx2 = tx1.clone();

tx1.push(1);
tx2.push(2);

while let Some(value) = rx.pop() {
  println!("popped: {}", value);
}
```

## Release Notes

### 0.1.1 - 2019-07-30

* Senders should be `Send`

### 0.1.0 - 2019-07-30

* Initial release. Likely not production ready.

## Developer Notes

To run the tests, execute the following:

```bash
cargo test
```

To run a benchmark for the queue, execute the following:

```bash
cargo test --release -- --ignored --nocapture
```

## Inspiration

This code is largely based on [majek](https://github.com/majek)'s
implementation of Michael-Scott queue. You can find the
code [here](https://github.com/majek/dump/blob/master/msqueue/queue_lock_mutex.c)
and a blog post [here](https://idea.popcount.org/2012-09-11-concurrent-queue-in-c/).

## License

Conqueue is provided under the [MIT license](LICENSE).
