pub mod analysis;

use crate::Executable;
use self::analysis::TimelineAnalyzer;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::time::Instant;
use std::hash::Hash;

pub struct WrappedTask<N, F> {
    sender: Sender<TimelineEvent<N>>,
    name: N,
    func: F
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TimelineEvent<N> {
    Start(N, Instant),
    End(N, Instant)
}

impl<N> TimelineEvent<N> {

    pub fn name(&self) -> &N {
        match self {
            TimelineEvent::Start(name, _) => name,
            TimelineEvent::End(name, _) => name,
        }
    }

    pub fn time(&self) -> Instant {
        match self {
            TimelineEvent::Start(_, time) => *time,
            TimelineEvent::End(_, time) => *time
        }
    }
}

pub struct TimelineReader<N> {
    sender: Sender<TimelineEvent<N>>,
    receiver: Receiver<TimelineEvent<N>>
}

pub struct TimelineIterator<N> {
    receiver: Receiver<TimelineEvent<N>>
}

impl<N: Clone> TimelineReader<N> {

    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }

    pub fn wrap<'a, T: Sync, F: Executable<T>>(&self, name: N, func: F) -> WrappedTask<N, F> {
        WrappedTask { sender: self.sender.clone(), name, func }
    }

    pub fn collect(self) -> TimelineIterator<N> {
        TimelineIterator { receiver: self.receiver }
    }
}

impl<N: Clone + Eq + Hash> TimelineReader<N> {
    pub fn analyze(self) -> TimelineAnalyzer<N> {
        self.collect().collect()
    }
}

impl<N: Clone, T: Sync, F: Executable<T>> Executable<T> for WrappedTask<N, F> {

    fn run(&mut self, data: &T) {
        let _ = self.sender.send(TimelineEvent::Start(self.name.clone(), Instant::now()));
        self.func.run(data);
        let _ = self.sender.send(TimelineEvent::End(self.name.clone(), Instant::now()));
    }
}

impl<N> Iterator for TimelineIterator<N> {
    type Item = TimelineEvent<N>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.try_recv() {
            Ok(e) => Some(e),
            Err(_) => None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::{TimelineReader, TimelineEvent};
    use crate::Executable;

    #[test]
    fn reader() {
        fn start(event: Option<TimelineEvent<&str>>, name: &str) {
            let event = event.unwrap_or_else(|| panic!("iterator is empty: expected Start({})", name));
            if let TimelineEvent::Start(event_name, _) = event {
                assert_eq!(event_name, name, "unexpected name")
            } else {
                panic!("unexpected end")
            }
        }

        fn end(event: Option<TimelineEvent<&str>>, name: &str) {
            let event = event.unwrap_or_else(|| panic!("iterator is empty: expected End({})", name));
            if let TimelineEvent::End(event_name, _) = event {
                assert_eq!(event_name, name, "unexpected name")
            } else {
                panic!("unexpected start")
            }
        }

        let reader = TimelineReader::new();
        let closure = |_: &()| {};

        reader.wrap("a", closure).run(&());
        reader.wrap("b", closure).run(&());
        reader.wrap("c", closure).run(&());
        reader.wrap("d", closure).run(&());
        reader.wrap("e", closure).run(&());

        let mut iter = reader.collect();

        start(iter.next(), "a");
        end(iter.next(), "a");
        start(iter.next(), "b");
        end(iter.next(), "b");
        start(iter.next(), "c");
        end(iter.next(), "c");
        start(iter.next(), "d");
        end(iter.next(), "d");
        start(iter.next(), "e");
        end(iter.next(), "e");
        assert_eq!(iter.next(), None, "expected end of iterator")
    }
}