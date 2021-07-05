use super::task::{TaskRef, Task};
use rayon::join;

pub struct Context<'r, 'task, T> {
    data: &'r T,
    tasks: &'r Vec<Task<'task, T>>
}

impl<'r, 'task, T: Sync> Context<'r, 'task, T> {
    pub fn new(data: &'r T, tasks: &'r Vec<Task<'task, T>>) -> Self {
        tasks.iter().for_each(|task| task.init());
        Self { data, tasks }
    }

    fn lock(&self, borrow: &TaskRef<'r, 'task, T>) {
        borrow.task()
            .lockable_deps()
            .iter()
            .for_each(|task| self.tasks[task.id()].lock());
    }

    fn execute(&self, borrow: &mut TaskRef<'r, 'task, T>) {
        borrow.execute(self.data)
    }

    fn unlock<'a>(&self, borrow: &'a TaskRef<'r, 'task, T>) -> impl Iterator<Item=TaskRef<'r, 'task, T>> + Send + 'a {
        let tasks = self.tasks;
        
        borrow.task()
            .unlockable_deps()
            .iter()
            .filter(move |task| tasks[task.id()].unlock())
            .map(move |task| tasks[task.id()].take())
            .filter(|task| task.is_some())
            .map(|task| task.unwrap())
    }

    fn take_unlocked(&self) -> impl Iterator<Item=TaskRef<'r, 'task, T>> + Send + 'r {
        self.tasks.iter()
            .map(|task| task.take())
            .filter(|task| task.is_some())
            .map(|task| task.unwrap())
    }

    fn run_iterator(&self, mut iter: impl Iterator<Item=TaskRef<'r, 'task, T>> + Send) {
        if let Some(mut task) = iter.next() {
            self.lock(&task);

            let tail = move || self.run_iterator(iter);
            let head = move || {
                self.execute(&mut task);
                self.run_iterator(self.unlock(&task));
            };

            join(head, tail);
        }
    }

    pub fn run(&self) {
        self.run_iterator(self.take_unlocked())
    }
}