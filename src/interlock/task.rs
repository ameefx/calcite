use crate::Executable;
use super::cell::{CountCell, CountRef};

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct TaskId(usize);

impl TaskId {

    pub(crate) fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn id(&self) -> usize {
        self.0
    }
}

pub struct Task<'a, T> {
    id: TaskId,
    task: CountCell<Box<dyn Executable<T> + Send + 'a>>,
    lock: Vec<TaskId>,
    unlock: Vec<TaskId>,
    initial: usize
}

pub struct TaskRef<'r, 'task, T> {
    task: &'r Task<'task, T>,
    borrow: CountRef<'r, Box<dyn Executable<T> + Send + 'task>>
}

impl<'r, 'task, T> TaskRef<'r, 'task, T> {

    pub fn task(&self) -> &Task<'task, T> {
        self.task
    }

    pub fn execute(&mut self, data: &T) {
        self.borrow.run(data);
    }
}

impl<'task, T> Task<'task, T> {
    pub fn new(id: TaskId, task: Box<dyn Executable<T> + Send + 'task>, lock: Vec<TaskId>, unlock: Vec<TaskId>, initial: usize) -> Self {
        Self { id, task: CountCell::new(task), lock, unlock, initial }
    }

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn init(&self) {
        self.task.reset(self.initial);
    }

    pub fn lock(&self) {
        self.task.lock()
    }

    pub fn unlock(&self) -> bool {
        self.task.unlock()
    }

    pub fn take(&self) -> Option<TaskRef<'_, 'task, T>> {
        self.task.take().map(|borrow| TaskRef { task: self, borrow })
    }

    pub fn initial_count(&self) -> usize {
        self.initial
    }

    pub fn lockable_deps(&self) -> &[TaskId] {
        self.lock.as_slice()
    }

    pub fn unlockable_deps(&self) -> &[TaskId] {
        self.unlock.as_slice()
    }
}
