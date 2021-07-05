pub mod seq;
pub mod par;
pub mod interlock;
pub mod test;

/**
 This trait is the heard of that library.
 It represents a single indivisible unit of work that requires reference to `T` to run.
*/
pub trait Executable<T> {
    fn run(&mut self, data: &T);
}

impl<T, F: FnMut(&T)> Executable<T> for F {
    fn run(&mut self, data: &T) {
        (self)(data)
    }
}

pub fn seq<T, Q1: Executable<T>, Q2: Executable<T>>(first: Q1, second: Q2) -> seq::Seq<Q1, Q2> {
    seq::Seq::new(first, second)
}

pub fn par<T: Sync, Q1: Executable<T> + Send, Q2: Executable<T>+ Send>(first: Q1, second: Q2) -> par::Par<Q1, Q2> {
    par::Par::new(first, second)
}

#[macro_export]
macro_rules! par {
    ($e1:expr, $e2:expr) => {
        par($e1, $e2)
    };

    ($e:expr, $($es:expr),+) => {
        par($e, par!($($es),+))
    };
}

#[macro_export]
macro_rules! seq {
    ($e1:expr, $e2:expr) => {
        seq($e1, $e2)
    };

    ($e:expr, $($es:expr),+) => {
        seq($e, seq!($($es),+))
    };
}