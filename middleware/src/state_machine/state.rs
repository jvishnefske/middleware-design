//! State trait and state lifecycle management.
//!
//! States in this framework are represented as enum variants, typically with
//! `#[repr(u8)]` or `#[repr(u16)]` for efficient storage and serialization.
//!
//! # Traceability
//! - REQ-FSM-001: States represented as enums with repr(u8) or repr(u16)
//! - REQ-FSM-002: States support on_enter and on_exit actions

use crate::error::LifecycleError;

/// Trait representing a state in a finite state machine.
///
/// States have associated context data and lifecycle hooks for entry and exit actions.
///
/// # Type Parameters
/// - `Context`: The context type available during state entry/exit
///
/// # Examples
///
/// Basic state implementation:
/// ```
/// use middleware::state_machine::State;
/// use middleware::LifecycleError;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// #[repr(u8)]
/// enum MyState {
///     Idle = 0,
///     Running = 1,
///     Fault = 2,
/// }
///
/// struct MyContext {
///     reset_count: u32,
/// }
///
/// impl State for MyState {
///     type Context = MyContext;
///
///     fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
///         match self {
///             MyState::Idle => {
///                 context.reset_count += 1;
///                 Ok(())
///             }
///             _ => Ok(())
///         }
///     }
///
///     fn on_exit(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
///         Ok(())
///     }
/// }
///
/// let mut context = MyContext { reset_count: 0 };
/// assert_eq!(MyState::Idle.on_enter(&mut context), Ok(()));
/// assert_eq!(MyState::Running.on_enter(&mut context), Ok(()));
/// assert_eq!(context.reset_count, 1);
/// assert_eq!(MyState::Fault as u8, 2);
/// ```
pub trait State: Copy + PartialEq {
    /// Context type available to state lifecycle hooks.
    type Context;

    /// Called when entering this state.
    ///
    /// # Parameters
    /// - `context`: Mutable reference to the state machine context
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// Returns a [`LifecycleError`] (conventionally
    /// [`LifecycleError::EntryActionFailed`]) if the entry action failed. The
    /// state machine reports any such error as `FsmError::EntryFailed`.
    fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError>;

    /// Called when exiting this state.
    ///
    /// # Parameters
    /// - `context`: Mutable reference to the state machine context
    ///
    /// # Returns
    /// `Ok(())` on success.
    ///
    /// # Errors
    /// Returns a [`LifecycleError`] (conventionally
    /// [`LifecycleError::ExitActionFailed`]) if the exit action failed. The
    /// state machine reports any such error as `FsmError::ExitFailed`.
    fn on_exit(&self, context: &mut Self::Context) -> Result<(), LifecycleError>;
}

// test code: a panic is a test failure, which is the intended reporting mechanism;
// `as` casts read back enum discriminants, which is the property under test
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::as_conversions))]
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum TestState {
        Idle = 0,
        Active = 1,
        Fault = 2,
    }

    struct TestContext {
        enter_count: u32,
        exit_count: u32,
    }

    impl State for TestState {
        type Context = TestContext;

        fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.enter_count += 1;
            match self {
                TestState::Fault => Err(LifecycleError::EntryActionFailed),
                TestState::Idle | TestState::Active => Ok(()),
            }
        }

        fn on_exit(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.exit_count += 1;
            Ok(())
        }
    }

    /// Verifies: REQ-FSM-002
    #[test]
    fn test_state_lifecycle() {
        let mut context = TestContext {
            enter_count: 0,
            exit_count: 0,
        };

        let state = TestState::Idle;
        assert!(state.on_enter(&mut context).is_ok());
        assert_eq!(context.enter_count, 1);

        assert!(state.on_exit(&mut context).is_ok());
        assert_eq!(context.exit_count, 1);
    }

    #[test]
    fn test_state_transitions() {
        let mut context = TestContext {
            enter_count: 0,
            exit_count: 0,
        };

        let idle = TestState::Idle;
        let active = TestState::Active;

        idle.on_enter(&mut context).unwrap();
        assert_eq!(context.enter_count, 1);

        idle.on_exit(&mut context).unwrap();
        assert_eq!(context.exit_count, 1);

        active.on_enter(&mut context).unwrap();
        assert_eq!(context.enter_count, 2);
    }

    /// Verifies: REQ-FSM-002
    #[test]
    fn test_state_entry_failure_reported() {
        let mut context = TestContext {
            enter_count: 0,
            exit_count: 0,
        };

        assert_eq!(
            TestState::Fault.on_enter(&mut context),
            Err(LifecycleError::EntryActionFailed)
        );
        assert_eq!(context.enter_count, 1);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u16)]
    enum WideState {
        Low = 0x0001,
        High = 0xFFFF,
    }

    impl State for WideState {
        type Context = TestContext;

        fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.enter_count += 1;
            Ok(())
        }

        fn on_exit(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.exit_count += 1;
            Ok(())
        }
    }

    /// Accepts any `State`; compiling the calls below shows both a
    /// `repr(u8)` and a `repr(u16)` enum implement the trait.
    fn is_state<S: State>(_: S) {}

    /// `repr(u8)` and `repr(u16)` enums implement `State` and keep their
    /// declared size and discriminants (including the u16 maximum).
    /// Verifies: REQ-FSM-001
    #[test]
    fn test_state_repr_u8_and_u16() {
        is_state(TestState::Idle);
        is_state(WideState::High);
        assert_eq!(core::mem::size_of::<TestState>(), 1);
        assert_eq!(core::mem::size_of::<WideState>(), 2);
        assert_eq!(TestState::Idle as u8, 0);
        assert_eq!(TestState::Active as u8, 1);
        assert_eq!(TestState::Fault as u8, 2);
        assert_eq!(WideState::Low as u16, 1);
        assert_eq!(WideState::High as u16, u16::MAX);

        let mut context = TestContext {
            enter_count: 0,
            exit_count: 0,
        };
        WideState::Low.on_exit(&mut context).unwrap();
        WideState::High.on_enter(&mut context).unwrap();
        assert_eq!((context.enter_count, context.exit_count), (1, 1));
    }
}
