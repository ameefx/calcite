use crate::Executable;
use super::InterlockExecutor;
use super::task::TaskId;
use std::borrow::Borrow;
use multimap::MultiMap;
use std::hash::Hash;
use std::collections::HashSet;

struct TaskBuilder<'task, T, R> {
    task: Box<dyn Executable<T> + Send + 'task>,
    dependencies: Vec<TaskId>,
    reads: Vec<R>,
    writes: Vec<R>
}

pub struct InterlockBuilder<'task, T, R> {
    tasks: Vec<TaskBuilder<'task, T, R>>
}

impl<'task, T: Sync, R: Eq + Hash> InterlockBuilder<'task, T, R> {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn add_box<D: Borrow<TaskId>>(&mut self, task: Box<dyn Executable<T> + Send + 'task>,
                                      reads: impl IntoIterator<Item=R>,
                                      writes: impl IntoIterator<Item=R>,
                                      deps: impl IntoIterator<Item=D>) -> TaskId {
        let id = TaskId::new(self.tasks.len());

        self.tasks.push(TaskBuilder {
            task,
            dependencies: deps.into_iter().map(|x| *x.borrow()).collect(),
            reads: reads.into_iter().collect(),
            writes: writes.into_iter().collect()
        });

        id
    }

    pub fn add<D: Borrow<TaskId>>(&mut self,
                                  task: impl Executable<T> + Send + 'task,
                                  reads: impl IntoIterator<Item=R>,
                                  writes: impl IntoIterator<Item=R>,
                                  deps: impl IntoIterator<Item=D>) -> TaskId {
        self.add_box(Box::new(task), reads, writes, deps)
    }

    pub fn build(self) -> InterlockExecutor<'task, T> {
        struct Task<'task, T> {
            task: Box<dyn Executable<T> + Send + 'task>,
            dependants: Vec<TaskId>,
            resource_locks: HashSet<TaskId>,
            initial: usize
        }

        impl<'task, T> Task<'task, T> {

            fn new(task: Box<dyn Executable<T> + Send + 'task>, initial: usize) -> Self {
                Self { task, initial, dependants: Vec::new(), resource_locks: HashSet::new() }
            }

            fn add_resource_lock(&mut self, id: TaskId) {
                self.resource_locks.insert(id);
            }

            fn add_dependant(&mut self, id: TaskId) {
                self.dependants.push(id);
            }

            fn build(self, id: TaskId) -> super::Task<'task, T> {
                let mut lock = Vec::with_capacity(self.resource_locks.len());
                let mut unlock = self.dependants; //why allocate new vec when i can do this??

                lock.extend(self.resource_locks.iter().map(|t| *t)); //cloning da iterator
                unlock.extend(self.resource_locks);

                super::Task::new(id, self.task, lock, unlock, self.initial)
            }
        }

        let mut tasks: Vec<Task<'task, T>> = Vec::with_capacity(self.tasks.len());

        let mut read_map = MultiMap::new();
        let mut write_map = MultiMap::new();

        for (id, task) in self.tasks.into_iter().enumerate().map(|(id, task)| (TaskId::new(id), task)) {
            tasks.push(Task::new(task.task, task.dependencies.len()));

            for read in task.reads {
                read_map.insert(read, id);
            }

            for write in task.writes {
                write_map.insert(write, id);
            }

            for dep in task.dependencies { //guaranteed to be added before that
                tasks[dep.id()].add_dependant(id); //add as a dependant
            }
        }

        //every WRITE locks every WRITE and every READ
        for (res, writes) in write_map.iter_all() {
            let reads = read_map.get_vec(res);

            for current in writes {
                for next in writes {
                    if current != next {
                        tasks[next.id()].add_resource_lock(*current);
                    }
                }

                if let Some(reads) = reads {
                    for next in reads {
                        if current != next {
                            tasks[next.id()].add_resource_lock(*current);
                        }
                    }
                }
            }
        }

        //every READ locks every WRITE
        for (res, reads) in read_map.iter_all() {
            if let Some(writes) = write_map.get_vec(res) {
                for current in reads {
                    for next in writes {
                        if current != next {
                            tasks[next.id()].add_resource_lock(*current);
                        }
                    }
                }
            }
        }

        tasks.into_iter()
            .enumerate()
            .map(|(id, t)| t.build(TaskId::new(id)))
            .collect()
    }
}