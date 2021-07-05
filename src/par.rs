use crate::Executable;
use rayon::join;

/**
 Executes two tasks in parallel, that is, one task _may_ begin at the same time as another, but there is no guarantee.
 You cannot use thread-local data as the tasks may be executed on a different thread to caller.

 Uses `rayon::join` internally.
*/
pub struct Par<Q1, Q2> {
    head: Q1,
    tail: Q2
}

impl<T: Send + Sync, Q1: Executable<T> + Send, Q2: Executable<T> + Send> Executable<T> for Par<Q1, Q2> {

    fn run(&mut self, data: &T) {
        let head = &mut self.head;
        let tail = &mut self.tail;

        let head = move || head.run(data);
        let tail = move || tail.run(data);

        join(head, tail);
    }
}

impl<Q1, Q2> Par<Q1, Q2> {

    pub fn new(head: Q1, tail: Q2) -> Self {
        Self { head, tail }
    }
}
