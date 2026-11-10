//! Event trait for state machine triggers.
//!
//! Events represent stimuli that can cause state transitions in a finite state machine.
//! They can carry optional associated data.
//!
//! # Traceability
//! - REQ-FSM-003: Events represented as enums with optional associated data

/// Trait representing an event that can trigger state transitions.
///
/// Events are typically represented as enum variants and can carry associated data.
///
/// # Examples
///
/// Simple event implementation:
/// ```
/// use middleware::state_machine::Event;
///
/// // `f32` payloads rule out `Eq`; `Event` only needs `Copy + PartialEq`.
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum ControlEvent {
///     Start,
///     Stop,
///     Error,
///     ThresholdExceeded(f32),
/// }
///
/// impl Event for ControlEvent {
///     const NAME: &'static str = "ControlEvent";
/// }
///
/// assert_eq!(ControlEvent::NAME, "ControlEvent");
/// assert_eq!(ControlEvent::ThresholdExceeded(1.5), ControlEvent::ThresholdExceeded(1.5));
/// assert_ne!(ControlEvent::ThresholdExceeded(1.5), ControlEvent::ThresholdExceeded(2.0));
/// assert_ne!(ControlEvent::Start, ControlEvent::Stop);
/// assert_ne!(ControlEvent::Stop, ControlEvent::Error);
/// ```
pub trait Event: Copy + PartialEq {
    /// Human-readable name of the event type.
    ///
    /// This is used for debugging, logging, and telemetry.
    const NAME: &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestEvent {
        Start,
        Stop,
        Reset,
    }

    impl Event for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    #[test]
    fn test_event_name() {
        assert_eq!(TestEvent::NAME, "TestEvent");
    }

    #[test]
    fn test_event_equality() {
        let e1 = TestEvent::Start;
        let e2 = TestEvent::Start;
        let e3 = TestEvent::Stop;

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum DataEvent {
        Threshold(f32),
        Timeout(u32),
    }

    impl Event for DataEvent {
        const NAME: &'static str = "DataEvent";
    }

    #[test]
    fn test_event_with_data() {
        let e1 = DataEvent::Threshold(3.14);
        let e2 = DataEvent::Threshold(3.14);
        let e3 = DataEvent::Threshold(2.71);

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }
}
