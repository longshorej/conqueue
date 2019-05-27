use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct ConNode<T> {
    element: T,
    next: *mut ConNode<T>,
}

struct ConQueueInner<T> {
    front: *mut ConNode<T>,
    back: *mut ConNode<T>,
}

struct ConQueueLock<T> {
    inner: *mut ConQueueInner<T>,
    lock: AtomicBool,
}

impl<T> Drop for ConQueueLock<T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.inner);
        }
    }
}

pub struct Queue<T> {
    lock: Arc<ConQueueLock<T>>,
    inner: *mut ConQueueInner<T>,
}

unsafe impl<T> Send for Queue<T> {}
unsafe impl<T> Sync for Queue<T> {}

impl<T> Queue<T> {
    pub fn new() -> Self {
        let inner = Box::into_raw(Box::new(ConQueueInner {
            front: ptr::null_mut(),
            back: ptr::null_mut(),
        }));

        Self {
            lock: Arc::new(ConQueueLock {
                inner,
                lock: AtomicBool::new(false),
            }),
            inner,
        }
    }

    pub fn pop(&self) -> Option<T> {
        while self
            .lock
            .lock
            .compare_and_swap(false, true, Ordering::Acquire)
        {
        }

        let element = unsafe {
            if !(*self.inner).front.is_null() {
                let next = (*(*self.inner).front).next;
                let mut node = Box::from_raw((*self.inner).front);
                let element = node.element;

                node.next = ptr::null_mut();

                if !next.is_null() {
                    (*self.inner).front = next;
                } else {
                    (*self.inner).front = ptr::null_mut();
                    (*self.inner).back = ptr::null_mut();
                }

                Some(element)
            } else {
                None
            }
        };

        self.lock.lock.store(false, Ordering::Release);

        element
    }

    pub fn push(&self, element: T) {
        let node = Box::into_raw(Box::new(ConNode {
            element: element,
            next: ptr::null_mut(),
        }));

        while self
            .lock
            .lock
            .compare_and_swap(false, true, Ordering::Acquire)
        {
        }

        unsafe {
            if !(*self.inner).back.is_null() {
                (*(*self.inner).back).next = node;
                (*self.inner).back = node;

                if (*self.inner).front.is_null() {
                    (*self.inner).front = node;
                }
            } else {
                (*self.inner).back = node;
                (*self.inner).front = node;
            }
        }

        self.lock.lock.store(false, Ordering::Release);
    }
}

impl<T> Clone for Queue<T> {
    fn clone(&self) -> Self {
        Self {
            lock: self.lock.clone(),
            inner: self.inner,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Queue;
    use criterion::{BatchSize, Criterion, ParameterizedBenchmark};
    use criterion::black_box;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time;


    #[test]
    fn test_single_thread() {
        let queue = Queue::new();

        queue.push(1);
        queue.push(2);
        queue.push(3);
        queue.push(4);

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(4));

        queue.push(4);
        queue.push(3);
        queue.push(2);
        queue.push(1);

        assert_eq!(queue.pop(), Some(4));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(1));
    }

    #[test]
    #[ignore]
    fn benchmark() {
        let num_items = 1_000_000;

        for n in [1, 2, 4, 8, 16, 32].iter() {
            for _ in 0 .. 1 {
                run(num_items, *n);
            }

            let start = time::Instant::now();

            run(num_items, *n);

            let duration = start.elapsed();

            let ns_per = duration.as_nanos() / (num_items as u128 * *n as u128);

            println!("producers={}, taken={}ms, ns_per={}", n, duration.as_millis(), ns_per);
        }
    }

    fn run(items: usize, num_producers: usize) {
        let queue = Queue::new();
        let done = Arc::new(AtomicUsize::new(0));

        for _ in 0 .. num_producers {
            let done = done.clone();
            let queue = queue.clone();

            thread::spawn(move || {
                for n in 0 .. items {
                    queue.push(n);
                }

                done.fetch_add(1, Ordering::SeqCst);
            });
        }

        while done.load(Ordering::SeqCst) != num_producers {
            while let Some(_) = queue.pop() {}
        }
    }
}
