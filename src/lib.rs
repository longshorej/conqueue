use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

struct QueueHead<T> {
    element: T,
    next: *mut QueueHead<T>,
}

struct QueueRoot<T> {
    in_queue: Arc<AtomicPtr<QueueHead<T>>>,
    out_queue: *mut QueueHead<T>,
}

pub struct QueueSender<T> {
    in_queue: Arc<AtomicPtr<QueueHead<T>>>,
}

impl<T> QueueSender<T> {
    pub fn push(&self, element: T) {
        let mut new = Box::into_raw(Box::new(QueueHead {
            element,
            next: ptr::null_mut(),
        }));

        loop {
            unsafe {
                let in_queue = self.in_queue.load(Ordering::SeqCst);

                (*new).next = in_queue;

                if self
                    .in_queue
                    .compare_and_swap(in_queue, new, Ordering::SeqCst)
                    == in_queue
                {
                    break;
                }
            }
        }
    }
}

impl<T> Clone for QueueSender<T> {
    fn clone(&self) -> Self {
        Self {
            in_queue: self.in_queue.clone(),
        }
    }
}

unsafe impl<T> Sync for QueueSender<T> {}

unsafe impl<T> Send for QueueSender<T> {}

pub struct QueueReceiver<T> {
    root: QueueRoot<T>,
}

impl<T> QueueReceiver<T> {
    pub fn pop(&mut self) -> Option<T> {
        if self.root.out_queue.is_null() {
            loop {
                let mut head = self.root.in_queue.load(Ordering::SeqCst);

                if head.is_null() {
                    break;
                }

                if self
                    .root
                    .in_queue
                    .compare_and_swap(head, ptr::null_mut(), Ordering::SeqCst)
                    == head
                {
                    while !head.is_null() {
                        unsafe {
                            let next = (*head).next;
                            (*head).next = self.root.out_queue;
                            self.root.out_queue = head;
                            head = next;
                        }
                    }

                    break;
                }
            }
        }

        if self.root.out_queue.is_null() {
            None
        } else {
            unsafe {
                let head = Box::from_raw(self.root.out_queue);
                self.root.out_queue = head.next;
                Some(head.element)
            }
        }
    }
}

pub struct Queue;

impl Queue {
    /// Create a new queue, returning a sender
    /// and receiver pair.
    ///
    /// Senders may be cloned to allow multiple
    /// producers, but only a single receiver
    /// may exist.
    pub fn unbounded<T>() -> (QueueSender<T>, QueueReceiver<T>) {
        let in_queue = Arc::new(AtomicPtr::new(ptr::null_mut()));

        let root = QueueRoot {
            in_queue: in_queue.clone(),
            out_queue: ptr::null_mut(),
        };

        let sender = QueueSender { in_queue };

        let receiver = QueueReceiver { root };

        (sender, receiver)
    }
}

#[cfg(test)]
mod tests {
    use crate::Queue;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time;

    #[test]
    fn test_single_thread() {
        let (tx, mut rx) = Queue::unbounded();

        tx.push(1);
        tx.push(2);
        tx.push(3);
        tx.push(4);

        assert_eq!(rx.pop(), Some(1));
        assert_eq!(rx.pop(), Some(2));
        assert_eq!(rx.pop(), Some(3));
        assert_eq!(rx.pop(), Some(4));

        tx.push(4);
        tx.push(3);
        tx.push(2);
        tx.push(1);

        assert_eq!(rx.pop(), Some(4));
        assert_eq!(rx.pop(), Some(3));
        assert_eq!(rx.pop(), Some(2));
        assert_eq!(rx.pop(), Some(1));
    }

    #[test]
    fn test_multiple_threads() {
        let (tx, mut rx) = Queue::unbounded();

        tx.push(1);
        tx.push(2);
        tx.push(3);
        tx.push(4);

        assert_eq!(rx.pop(), Some(1));
        assert_eq!(rx.pop(), Some(2));
        assert_eq!(rx.pop(), Some(3));
        assert_eq!(rx.pop(), Some(4));

        tx.push(4);
        tx.push(3);
        tx.push(2);
        tx.push(1);

        assert_eq!(rx.pop(), Some(4));
        assert_eq!(rx.pop(), Some(3));
        assert_eq!(rx.pop(), Some(2));
        assert_eq!(rx.pop(), Some(1));
    }

    #[test]
    fn test_drop() {
        struct MyStruct {
            sum: Arc<AtomicUsize>,
            value: usize,
        }

        impl Drop for MyStruct {
            fn drop(&mut self) {
                self.sum.fetch_add(self.value, Ordering::SeqCst);
            }
        }

        let sum = Arc::new(AtomicUsize::new(0));

        let (tx, mut rx) = Queue::unbounded();

        tx.push(MyStruct {
            sum: sum.clone(),
            value: 1,
        });
        tx.push(MyStruct {
            sum: sum.clone(),
            value: 2,
        });
        tx.push(MyStruct {
            sum: sum.clone(),
            value: 3,
        });

        assert_eq!(rx.pop().map(|s| s.value), Some(1));
        assert_eq!(rx.pop().map(|s| s.value), Some(2));
        assert_eq!(rx.pop().map(|s| s.value), Some(3));
        assert_eq!(rx.pop().map(|s| s.value), None);

        assert_eq!(sum.load(Ordering::SeqCst), 6);
    }

    #[test]
    #[ignore]
    fn benchmark() {
        let num_items = 1_000_000;

        for n in [1, 2, 4, 8, 16, 32].iter() {
            for _ in 0..1 {
                run(num_items, *n);
            }

            let start = time::Instant::now();

            run(num_items, *n);

            let duration = start.elapsed();

            let ns_per = duration.as_nanos() / (num_items as u128 * *n as u128);

            println!(
                "producers={}, taken={}ms, ns_per={}",
                n,
                duration.as_millis(),
                ns_per
            );
        }
    }

    fn run(items: usize, num_producers: usize) {
        let (tx, mut rx) = Queue::unbounded();
        let done = Arc::new(AtomicUsize::new(0));

        for _ in 0..num_producers {
            let done = done.clone();
            let queue = tx.clone();

            thread::spawn(move || {
                for n in 0..items {
                    queue.push(n);
                }

                done.fetch_add(1, Ordering::SeqCst);
            });
        }

        while done.load(Ordering::SeqCst) != num_producers {
            while let Some(_) = rx.pop() {}
        }
    }
}
