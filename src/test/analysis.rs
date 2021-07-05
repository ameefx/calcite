use std::time::Duration;
use std::iter::FromIterator;
use std::hash::Hash;
use std::collections::HashMap;
use super::TimelineEvent;

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug)]
pub enum TimelineOrder {
    Before,
    Parallel,
    After
}

#[derive(Eq, PartialEq, Clone, Hash, Debug)]
pub struct TimelineTask<N> {
    name: N,
    start: Duration,
    length: Duration
}

impl<N> TimelineTask<N> {

    pub fn new(name: N,
               start: Duration,
               length: Duration) -> Self {
        Self { name, start, length }
    }

    pub fn name(&self) -> &N {
        &self.name
    }

    pub fn start(&self) -> Duration {
        self.start
    }

    pub fn end(&self) -> Duration {
        self.start + self.length
    }

    pub fn len(&self) -> Duration {
        self.length
    }

    pub fn order_to(&self, task: &Self) -> TimelineOrder {
        if task.start() < self.end() && self.start() < task.end() {
            TimelineOrder::Parallel
        } else if task.start() > self.start() {
            TimelineOrder::After
        } else {
            TimelineOrder::Before
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimelineAnalyzer<N> {
    tasks: Vec<TimelineTask<N>>
}

impl<N: PartialEq> TimelineAnalyzer<N> {

    pub fn single<'a>(&'a self, name: &'a N) -> Option<&'a TimelineTask<N>> {
        let mut iter = self.get(name);
        match iter.next() {
            Some(value) =>
                match iter.next() {
                    Some(_) => None,
                    None => Some(value)
                },

            None => None
        }
    }

    pub fn first<'a>(&'a self, name: &'a N) -> Option<&'a TimelineTask<N>> {
        self.get(name).next()
    }

    pub fn last<'a>(&'a self, name: &'a N) -> Option<&'a TimelineTask<N>> {
        self.get(name).last()
    }

    pub fn count(&self, name: &N) -> usize {
        self.get(name).count()
    }

    pub fn has(&self, name: &N) -> bool {
        self.get(name).next().is_some()
    }

    pub fn get<'a>(&'a self, name: &'a N) -> impl Iterator<Item=&TimelineTask<N>> + 'a {
        self.iter().filter(move |t| &t.name == name)
    }

    pub fn iter(&self) -> impl Iterator<Item=&TimelineTask<N>> + '_ {
        self.tasks.iter()
    }

    pub fn len(&self) -> Duration {
        self.iter()
            .map(|t| t.end())
            .max()
            .unwrap_or(Duration::from_millis(0))
    }

    pub fn serial_len(&self) -> Duration {
        self.iter()
            .map(|t| t.len())
            .sum()
    }

    pub fn efficiency(&self) -> f64 {
        self.serial_len().as_secs_f64() / self.len().as_secs_f64()
    }

    pub fn threads(&self) -> usize {
        let mut counter = Vec::new();

        fn find_slot(counter: &Vec<Duration>, start: Duration) -> Option<usize> {
            for (idx, end) in counter.iter().enumerate() {
                if &start >= end {
                    return Some(idx);
                }
            }

            None
        }

        for task in self.iter() {
            match find_slot(&counter, task.start()) {
                Some(id) => counter[id] = task.end(),
                None => counter.push(task.end())
            }
        }

        counter.len()
    }
}

impl<N> FromIterator<TimelineTask<N>> for TimelineAnalyzer<N> {
    fn from_iter<T: IntoIterator<Item=TimelineTask<N>>>(iter: T) -> Self {
        let mut tasks: Vec<_> = iter.into_iter().collect();
        tasks.sort_by_key(|t| t.start());

        Self { tasks }
    }
}

impl<N: Eq + Hash> FromIterator<TimelineEvent<N>> for TimelineAnalyzer<N> {
    fn from_iter<T: IntoIterator<Item=TimelineEvent<N>>>(iter: T) -> Self {
        let events: Vec<TimelineEvent<N>> = iter.into_iter().collect();
        match events.iter().min_by_key(|e| e.time()) {
            Some(start) => {
                let min = start.time();
                let mut pending = HashMap::new();
                let mut tasks = Vec::new();

                for event in events {
                    match event {
                        TimelineEvent::Start(name, time) => {
                            assert!(pending.insert(name, time).is_none(), "task analysis: start with duplicate name")
                        },

                        TimelineEvent::End(name, end) => {
                            let start = pending.remove(&name).expect("task analysis: unmatched end");
                            let start = start - min;
                            let end = end - min;

                            tasks.push(TimelineTask::new(name, start, end - start))
                        }
                    }
                }

                if !pending.is_empty() {
                    panic!("task analysis: unmatched start")
                }

                tasks.into_iter().collect()
            },

            None => Vec::<TimelineTask<_>>::new().into_iter().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use std::ops::Add;

    #[test]
    fn task_order() {
        let a = TimelineTask::new((), Duration::from_millis(0), Duration::from_millis(10));
        let b = TimelineTask::new((), Duration::from_millis(5), Duration::from_millis(10));
        let c = TimelineTask::new((), Duration::from_millis(10), Duration::from_millis(10));
        let d = TimelineTask::new((), Duration::from_millis(15), Duration::from_millis(10));
        let e = TimelineTask::new((), Duration::from_millis(10), Duration::from_millis(0));

        assert_eq!(a.order_to(&b), TimelineOrder::Parallel);
        assert_eq!(a.order_to(&c), TimelineOrder::After);
        assert_eq!(a.order_to(&e), TimelineOrder::After);

        assert_eq!(b.order_to(&c), TimelineOrder::Parallel);
        assert_eq!(b.order_to(&d), TimelineOrder::After);
        assert_eq!(b.order_to(&e), TimelineOrder::Parallel);

        //inverse cases
        assert_eq!(b.order_to(&a), TimelineOrder::Parallel);
        assert_eq!(e.order_to(&a), TimelineOrder::Before);
        assert_eq!(c.order_to(&a), TimelineOrder::Before);

        assert_eq!(c.order_to(&b), TimelineOrder::Parallel);
        assert_eq!(d.order_to(&b), TimelineOrder::Before);
        assert_eq!(e.order_to(&b), TimelineOrder::Parallel);
    }

    fn construct_analyzer() -> TimelineAnalyzer<&'static str> {
        use TimelineEvent::{Start, End};

        let now = Instant::now();
        let instant = |t| now.add(Duration::from_millis(t));

        vec![
            Start   ("a", instant(0)), // ---a
            Start   ("b", instant(0)), // -b |
            End     ("b", instant(5)), // -+ |
            Start   ("c", instant(5)), // ---|-c
            End     ("a", instant(10)),// ---+ |
            Start   ("e", instant(10)),// ---e |
            End     ("c", instant(15)),// ---|-+
            Start   ("d", instant(15)),// -d |
            Start   ("f", instant(15)),// -|-|-f
            End     ("d", instant(20)),// -+ | |
            End     ("e", instant(20)),// ---+ |
            End     ("f", instant(30)),// -----+
            Start   ("a", instant(30)),// ---a [*]
            Start   ("g", instant(30)),// -g |
            End     ("g", instant(35)),// -+ |
            End     ("a", instant(40)),// ---+
            Start   ("b", instant(40)),// -b   [*]
            End     ("b", instant(40)),// -+
        ].into_iter().collect()
    }

    #[test]
    fn analyzer_construct() {
        construct_analyzer();
    }

    #[test]
    fn analyzer_single() {
        let a = construct_analyzer();

        assert!(a.single(&"a").is_none());
        assert!(a.single(&"b").is_none());
        assert!(a.single(&"c").is_some());
        assert!(a.single(&"d").is_some());
        assert!(a.single(&"e").is_some());
        assert!(a.single(&"f").is_some());
        assert!(a.single(&"g").is_some());
        assert!(a.single(&"x").is_none());
        assert!(a.single(&"y").is_none());
        assert!(a.single(&"z").is_none());
    }

    #[test]
    fn analyzer_first() {
        let a = construct_analyzer();

        assert!(a.first(&"a").is_some());
        assert!(a.first(&"b").is_some());
        assert!(a.first(&"c").is_some());
        assert!(a.first(&"d").is_some());
        assert!(a.first(&"e").is_some());
        assert!(a.first(&"f").is_some());
        assert!(a.first(&"g").is_some());
        assert!(a.first(&"x").is_none());
        assert!(a.first(&"y").is_none());
        assert!(a.first(&"z").is_none());
    }

    #[test]
    fn analyzer_count() {
        let a = construct_analyzer();

        assert_eq!(a.count(&"a"), 2);
        assert_eq!(a.count(&"b"), 2);
        assert_eq!(a.count(&"c"), 1);
        assert_eq!(a.count(&"d"), 1);
        assert_eq!(a.count(&"e"), 1);
        assert_eq!(a.count(&"f"), 1);
        assert_eq!(a.count(&"g"), 1);
        assert_eq!(a.count(&"x"), 0);
        assert_eq!(a.count(&"y"), 0);
        assert_eq!(a.count(&"z"), 0);
    }

    #[test]
    fn analyzer_tasks() {
        let a = construct_analyzer();
        let task = |name, start, end| Some(TimelineTask::new(name, Duration::from_millis(start), Duration::from_millis(end - start)));

        assert_eq!(a.first(&"a"), task("a", 0, 10).as_ref());
        assert_eq!(a.first(&"b"), task("b", 0, 5).as_ref());
        assert_eq!(a.first(&"c"), task("c", 5, 15).as_ref());
        assert_eq!(a.first(&"d"), task("d", 15, 20).as_ref());
        assert_eq!(a.first(&"e"), task("e", 10, 20).as_ref());
        assert_eq!(a.first(&"f"), task("f", 15, 30).as_ref());
        assert_eq!(a.first(&"g"), task("g", 30, 35).as_ref());

        assert_eq!(a.last(&"a"), task("a", 30, 40).as_ref());
        assert_eq!(a.last(&"b"), task("b", 40, 40).as_ref());
    }
}