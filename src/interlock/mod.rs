pub mod builder;
mod cell;
mod context;
mod task;

use crate::Executable;
use self::builder::InterlockBuilder;
use self::context::Context;
use self::task::Task;
use std::hash::Hash;
use std::fmt::{Debug, Formatter};
use std::fmt;
use std::iter::FromIterator;

pub fn builder<'task, T: Sync, R: Eq + Hash>() -> InterlockBuilder<'task, T, R> {
    InterlockBuilder::new()
}

pub struct InterlockExecutor<'task, T> {
    tasks: Vec<Task<'task, T>>
}

impl<'task, T: Sync> FromIterator<Task<'task, T>> for InterlockExecutor<'task, T> {

    fn from_iter<I: IntoIterator<Item=Task<'task, T>>>(iter: I) -> Self {
        let tasks: Vec<_> = iter.into_iter().collect();
        Self { tasks }
    }
}

impl<'task, T: Sync> Executable<T> for InterlockExecutor<'task, T> {

    fn run(&mut self, data: &T) {
        Context::new(data, &self.tasks).run()
    }
}

impl<'task, T> Debug for InterlockExecutor<'task, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Interlock [")?;
        for task in self.tasks.iter() {
            writeln!(f, "    Task #{}: (init={:?}, lock={:?}, unlock={:?})", task.id().id(), task.initial_count(), task.lockable_deps(), task.unlockable_deps())?;
        }
        write!(f, "]")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::analysis::{TimelineAnalyzer, TimelineOrder};
    use crate::test::TimelineReader;

    fn order(n: &TimelineAnalyzer<&str>, a: &str, b: &str) -> TimelineOrder {
        let task_a = n.first(&a).unwrap_or_else(|| panic!("task '{}' was not executed", a));
        let task_b = n.first(&b).unwrap_or_else(|| panic!("task '{}' was not executed", b));

        assert_eq!(n.count(&a), 1, "task '{}' was executed multiple times", a);
        assert_eq!(n.count(&b), 1, "task '{}' was executed multiple times", b);

        task_a.order_to(task_b)
    }

    fn mutex(n: &TimelineAnalyzer<&str>, a: &str, b: &str) {
        assert_ne!(order(n, a, b), TimelineOrder::Parallel, "tasks '{}' and '{}' were executed in parallel when they should not", a, b)
    }

    fn dep(n: &TimelineAnalyzer<&str>, a: &str, b: &str) {
        assert_eq!(order(n, a, b), TimelineOrder::After, "task '{}' depends on '{}' but they were executed out of order", b, a)
    }

    #[test]
    fn the_ultimate_test() {
        let closure = |_: &()| {};

        let reader = TimelineReader::new();
        let mut builder = builder();

        let a_task = builder.add(reader.wrap("a", closure), &[1u32], &[0u32], &[]);
        let b_task = builder.add(reader.wrap("b", closure), &[0u32], &[1u32], &[]);
        let c_task = builder.add(reader.wrap("c", closure), &[1u32], &[2u32], &[a_task, b_task]);
        let d_task = builder.add(reader.wrap("d", closure), &[0u32, 2u32], &[3u32], &[a_task]);
        let e_task = builder.add(reader.wrap("e", closure), &[], &[4u32], &[d_task]);
        let _f_task = builder.add(reader.wrap("f", closure), &[6u32], &[5u32], &[e_task, c_task]);
        let _g_task = builder.add(reader.wrap("g", closure), &[], &[6u32], &[d_task, c_task]);
        let _h_task = builder.add(reader.wrap("h", closure), &[], &[7u32], &[c_task]);

        let mut exec = builder.build();
        exec.run(&());

        let analyzer = reader.analyze();

        mutex(&analyzer, "a", "b");
        mutex(&analyzer, "a", "d");
        mutex(&analyzer, "c", "d");
        mutex(&analyzer, "b", "c");
        mutex(&analyzer, "f", "g");

        dep(&analyzer, "a", "c");
        dep(&analyzer, "b", "c");
        dep(&analyzer, "a", "d");
        dep(&analyzer, "d", "e");
        dep(&analyzer, "c", "f");
        dep(&analyzer, "e", "f");
        dep(&analyzer, "d", "g");
        dep(&analyzer, "c", "g");
        dep(&analyzer, "c", "h");
    }
}