//! Scheduler for emitting events at a specific time in the future.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::event::Event;

#[cfg(test)]
static TEST_EVENTS: once_cell::sync::Lazy<std::sync::Mutex<Vec<Event>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(Vec::new()));

#[cfg(test)]
pub fn test_take_events() -> Vec<Event> {
    let mut g = TEST_EVENTS.lock().unwrap();
    let v = g.clone();
    g.clear();
    v
}

#[cfg(test)]
pub fn test_clear_events() {
    TEST_EVENTS.lock().unwrap().clear();
}

/// ID uniquely identifying a timer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimerId {
    topic: Topic,
    window_id: Option<WindowId>,
}

impl TimerId {
    pub fn new(topic: Topic, window_id: WindowId) -> Self {
        Self { topic, window_id: Some(window_id) }
    }

    /// Create a TimerId without associating it to a specific window (useful in tests)
    #[cfg(test)]
    pub fn new_anonymous(topic: Topic) -> Self {
        Self { topic, window_id: None }
    }
}

/// Available timer topics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Topic {
    SelectionScrolling,
    DelayedSearch,
    BlinkCursor,
    BlinkTimeout,
    Frame,
    // Debounced Blocks Search typing
    BlocksSearchTyping,
    // Debounced Workflows Search typing
    WorkflowsSearchTyping,
    // Retain workflows progress overlay briefly after completion
    WorkflowsProgressRetain,
    // Autosave workspace sessions periodically
    WorkspaceSessionAutosave,
    // Debounced AI inline suggestion trigger
    AiInlineTyping,
}

/// Event scheduled to be emitted at a specific time.
pub struct Timer {
    pub deadline: Instant,
    pub event: Event,
    pub id: TimerId,

    interval: Option<Duration>,
}

/// Scheduler tracking all pending timers.
pub struct Scheduler {
    timers: VecDeque<Timer>,
    event_proxy: EventLoopProxy<Event>,
}

impl Scheduler {
    pub fn new(event_proxy: EventLoopProxy<Event>) -> Self {
        Self { timers: VecDeque::new(), event_proxy }
    }

    #[cfg(test)]
    pub fn test_new_with_proxy(event_proxy: EventLoopProxy<Event>) -> Self {
        Self::new(event_proxy)
    }

    /// Process all pending timers.
    ///
    /// If there are still timers pending after all ready events have been processed, the closest
    /// pending deadline will be returned.
    pub fn update(&mut self) -> Option<Instant> {
        let now = Instant::now();

        while !self.timers.is_empty() && self.timers[0].deadline <= now {
            if let Some(timer) = self.timers.pop_front() {
                // Automatically repeat the event.
                if let Some(interval) = timer.interval {
                    self.schedule(timer.event.clone(), interval, true, timer.id);
                }

                let _ = self.event_proxy.send_event(timer.event.clone());
                #[cfg(test)]
                {
                    TEST_EVENTS.lock().unwrap().push(timer.event);
                }
            }
        }

        self.timers.front().map(|timer| timer.deadline)
    }

    /// Schedule a new event.
    pub fn schedule(&mut self, event: Event, interval: Duration, repeat: bool, timer_id: TimerId) {
        let deadline = Instant::now() + interval;

        // Get insert position in the schedule.
        let index = self
            .timers
            .iter()
            .position(|timer| timer.deadline > deadline)
            .unwrap_or(self.timers.len());

        // Set the automatic event repeat rate.
        let interval = if repeat { Some(interval) } else { None };

        self.timers.insert(index, Timer { interval, deadline, event, id: timer_id });
    }

    /// Cancel a scheduled event.
    pub fn unschedule(&mut self, id: TimerId) -> Option<Timer> {
        let index = self.timers.iter().position(|timer| timer.id == id)?;
        self.timers.remove(index)
    }

    /// Check if a timer is already scheduled.
    pub fn scheduled(&mut self, id: TimerId) -> bool {
        self.timers.iter().any(|timer| timer.id == id)
    }

    /// Remove all timers scheduled for a window.
    ///
    /// This must be called when a window is removed to ensure that timers on intervals do not
    /// stick around forever and cause a memory leak.
    pub fn unschedule_window(&mut self, window_id: WindowId) {
        self.timers.retain(|timer| timer.id.window_id != Some(window_id));
    }
}

#[cfg(all(test, feature = "blocks"))]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration as StdDuration;

    #[test]
    fn debounced_event_is_dispatched_after_deadline() {
        // Build EventLoop on any thread (Linux: X11/Wayland)
        let mut builder = winit::event_loop::EventLoop::<crate::event::Event>::with_user_event();
        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::EventLoopBuilderExtWayland;
            use winit::platform::x11::EventLoopBuilderExtX11;
            // Disambiguate: set for both Wayland and X11 backends
            EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
            EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        }
        let event_loop = builder.build().expect("failed to build event loop");
        let proxy = event_loop.create_proxy();
        let mut scheduler = Scheduler::test_new_with_proxy(proxy);

        // Schedule event 50ms from now
        let evt = crate::event::Event::new(
            crate::event::EventType::BlocksSearchPerform("test".into()),
            None,
        );
        // Use an anonymous TimerId that doesn't require a WindowId
        let tid = TimerId::new_anonymous(Topic::BlocksSearchTyping);

        super::test_clear_events();
        scheduler.schedule(evt, StdDuration::from_millis(50), false, tid);
        sleep(StdDuration::from_millis(80));
        scheduler.update();
        let events = super::test_take_events();
        assert!(events
            .iter()
            .any(|e| matches!(e.payload(), crate::event::EventType::BlocksSearchPerform(_))));

        // Integration: when components are missing, BlocksSearchPerform should post empty results
        #[cfg(feature = "blocks")]
        {
            crate::event::test_posted_events::clear();
            let cfg = crate::config::UiConfig::default();
            let cli = crate::cli::Options::default();
            let mut proc = crate::event::Processor::new(cfg, cli, &event_loop);
            let win = winit::window::WindowId::dummy();
            let evt = crate::event::Event::new(
                crate::event::EventType::BlocksSearchPerform("abc".to_string()),
                win,
            );
            proc.handle_user_event_for_test(evt);
            let posted = crate::event::test_posted_events::take();
            assert_eq!(posted.len(), 1);
            match &posted[0] {
                crate::event::EventType::BlocksSearchResults(items) => assert!(items.is_empty()),
                other => panic!("unexpected event: {:?}", other),
            }
        }

        // Subtest: debounce reschedules and emits only latest
        // Use a single TimerId and reschedule
        let tid = TimerId::new_anonymous(Topic::BlocksSearchTyping);

        super::test_clear_events();

        // Schedule first event far in the future
        let evt1 = crate::event::Event::new(
            crate::event::EventType::BlocksSearchPerform("first".into()),
            None,
        );
        scheduler.schedule(evt1, StdDuration::from_millis(200), false, tid);

        // Before it fires, unschedule and schedule a new one sooner
        sleep(StdDuration::from_millis(50));
        let _ = scheduler.unschedule(tid);
        let evt2 = crate::event::Event::new(
            crate::event::EventType::BlocksSearchPerform("second".into()),
            None,
        );
        scheduler.schedule(evt2, StdDuration::from_millis(60), false, tid);

        // Wait long enough for only the second to fire
        sleep(StdDuration::from_millis(120));
        scheduler.update();
        let events = super::test_take_events();
        // Ensure only one event and it's the 'second'
        let mut seen_first = false;
        let mut seen_second = false;
        for e in &events {
            if let crate::event::EventType::BlocksSearchPerform(s) = e.payload() {
                if s == "first" {
                    seen_first = true;
                }
                if s == "second" {
                    seen_second = true;
                }
            }
        }
        assert!(!seen_first, "debounced (first) event should have been canceled");
        assert!(seen_second, "latest event should have fired");
        assert_eq!(events.len(), 1, "only latest event should be dispatched");

        // Additional subtest: simulate typing/backspace debounce end-to-end
        super::test_clear_events();
        let win = winit::window::WindowId::dummy();
        crate::event::schedule_blocks_search_for_test(&mut scheduler, win, "a".into());
        sleep(StdDuration::from_millis(50));
        crate::event::schedule_blocks_search_for_test(&mut scheduler, win, "ab".into());
        sleep(StdDuration::from_millis(50));
        crate::event::schedule_blocks_search_for_test(&mut scheduler, win, "a".into());
        sleep(StdDuration::from_millis(300));
        scheduler.update();
        let events = super::test_take_events();
        assert_eq!(events.len(), 1);
        match events[0].payload() {
            crate::event::EventType::BlocksSearchPerform(s) => assert_eq!(s, "a"),
            other => panic!("unexpected event: {:?}", other),
        }
    }
}
