#[macro_use]
extern crate criterion;

use conqueue::Queue;
use criterion::black_box;
use criterion::{BatchSize, Criterion, ParameterizedBenchmark};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time;

fn run(num_producers: usize) {
    let queue = Queue::new();
    let done = Arc::new(AtomicUsize::new(0));

    for _ in 0..num_producers {
        let done = done.clone();
        let queue = queue.clone();

        thread::spawn(move || {
            for n in 0..1_000_000 {
                queue.push(n);
            }

            done.fetch_add(1, Ordering::SeqCst);
        });
    }

    while done.load(Ordering::SeqCst) != num_producers {
        while let Some(_) = queue.pop() {}
    }
}

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench(
        "performance",
        ParameterizedBenchmark::new(
            "run",
            |b, param| b.iter(move || run(*param)),
            vec![1, 4, 16, 128],
        )
        .measurement_time(time::Duration::from_secs(30)),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
