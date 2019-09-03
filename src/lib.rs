use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

struct QueueHead<T> {
    element: Option<T>,
    next: *mut QueueHead<T>,
}

/// A `QueueSender` is used to push items into
/// the queue.
///
/// It implements `Send` and `Sync`, thus allowing
/// multiple callers to concurrent push items.
pub struct QueueSender<T> {
    in_queue: Arc<AtomicPtr<QueueHead<T>>>,
}

impl<T> QueueSender<T> {
    /// Push the supplied element into the queue.
    pub fn push(&self, element: T) {
        let mut in_queue = ptr::null_mut();
        let mut new = Box::into_raw(Box::new(QueueHead {
            element: Some(element),
            next: in_queue,
        }));

        loop {
            match self
                .in_queue
                .compare_exchange(in_queue, new, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    return;
                }

                Err(actual) => {
                    in_queue = actual;

                    unsafe {
                        if !in_queue.is_null() && (*in_queue).element.is_none() {
                            Box::from_raw(new);
                            return;
                        }

                        (*new).next = in_queue;
                    }
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

impl<T> Drop for QueueSender<T> {
    fn drop(&mut self) {
        let mut in_queue = Arc::new(AtomicPtr::default());

        mem::swap(&mut in_queue, &mut self.in_queue);

        if let Ok(head) = Arc::try_unwrap(in_queue) {
            let head = head.swap(ptr::null_mut(), Ordering::SeqCst);

            if !head.is_null() {
                unsafe { Box::from_raw(head) };
            }
        }
    }
}

unsafe impl<T> Sync for QueueSender<T> {}

unsafe impl<T> Send for QueueSender<T> {}

/// A `QueueReceiver` is used to pop previously
/// pushed items from the queue.
pub struct QueueReceiver<T> {
    in_queue: Arc<AtomicPtr<QueueHead<T>>>,
    out_queue: *mut QueueHead<T>,
}

impl<T> QueueReceiver<T> {
    /// Pop an item from the queue. If the queue is
    /// empty, `None` is returned.
    pub fn pop(&mut self) -> Option<T> {
        if self.out_queue.is_null() {
            let mut head = ptr::null_mut();

            loop {
                match self.in_queue.compare_exchange(
                    head,
                    ptr::null_mut(),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        while !head.is_null() {
                            unsafe {
                                let next = (*head).next;
                                (*head).next = self.out_queue;
                                self.out_queue = head;
                                head = next;
                            }
                        }

                        break;
                    }

                    Err(actual) => {
                        head = actual;

                        if head.is_null() {
                            break;
                        }
                    }
                }
            }
        }

        if self.out_queue.is_null() {
            None
        } else {
            unsafe {
                let head = Box::from_raw(self.out_queue);
                self.out_queue = head.next;
                Some(head.element.unwrap())
            }
        }
    }
}

impl<T> Drop for QueueReceiver<T> {
    fn drop(&mut self) {
        let last = Box::into_raw(Box::new(QueueHead {
            element: None,
            next: ptr::null_mut(),
        }));

        let mut head = ptr::null_mut();

        loop {
            match self
                .in_queue
                .compare_exchange(head, last, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    while !head.is_null() {
                        let boxed = unsafe { Box::from_raw(head) };

                        head = boxed.next;
                    }

                    break;
                }

                Err(actual) => {
                    head = actual;
                }
            }
        }

        let mut in_queue = Arc::new(AtomicPtr::default());

        mem::swap(&mut in_queue, &mut self.in_queue);

        if let Ok(head) = Arc::try_unwrap(in_queue) {
            let head = head.swap(ptr::null_mut(), Ordering::SeqCst);

            if !head.is_null() {
                unsafe { Box::from_raw(head) };
            }
        }
    }
}

unsafe impl<T> Send for QueueReceiver<T> {}

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

        let receiver = QueueReceiver {
            in_queue: in_queue.clone(),
            out_queue: ptr::null_mut(),
        };

        let sender = QueueSender { in_queue };

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
    fn test_disconnected() {
        struct MyStruct {
            dropped: Arc<AtomicUsize>,
        }

        impl Drop for MyStruct {
            fn drop(&mut self) {
                self.dropped.fetch_add(1, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicUsize::new(0));

        let (tx, rx) = Queue::unbounded();

        for _ in 1..=10 {
            tx.push(MyStruct {
                dropped: dropped.clone(),
            });
        }

        drop(rx);

        for _ in 1..=10 {
            tx.push(MyStruct {
                dropped: dropped.clone(),
            });
        }

        assert_eq!(dropped.load(Ordering::SeqCst), 20);
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
        let num_items = 10_000_000;

        for n in [1, 2, 4, 8, 16, 32, 64, 128].iter() {
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
