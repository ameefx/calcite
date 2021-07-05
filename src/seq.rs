use crate::Executable;

/**
 Executes two tasks sequentially, that is, one task executes then another one.
 Tasks are expected to run on the same thread as the caller.
*/
pub struct Seq<Q1, Q2> {
    head: Q1,
    tail: Q2
}

impl<T, Q1: Executable<T>, Q2: Executable<T>> Executable<T> for Seq<Q1, Q2> {

    fn run(&mut self, data: &T) {
        self.head.run(data);
        self.tail.run(data);
    }
}

impl<Q1, Q2> Seq<Q1, Q2> {

    pub fn new(head: Q1, tail: Q2) -> Self {
        Self { head, tail }
    }
}
