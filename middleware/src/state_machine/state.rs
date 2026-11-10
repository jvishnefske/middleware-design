//! State trait and state lifecycle management.
//!
//! States in this framework are represented as enum variants, typically with
//! `#[repr(u8)]` or `#[repr(u16)]` for efficient storage and serialization.
//!
//! # Traceability
//! - REQ-FSM-001: States represented as enums with repr(u8) or repr(u16)
//! - REQ-FSM-002: States support on_enter and on_exit actions

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
///     fn on_enter(&self, context: &mut Self::Context) -> Result<(), ()> {
///         match self {
///             MyState::Idle => {
///                 context.reset_count += 1;
///                 Ok(())
///             }
///             _ => Ok(())
///         }
///     }
///
///     fn on_exit(&self, _context: &mut Self::Context) -> Result<(), ()> {
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
    /// `Ok(())` on success, `Err(())` if entry action failed
    fn on_enter(&self, context: &mut Self::Context) -> Result<(), ()>;

    /// Called when exiting this state.
    ///
    /// # Parameters
    /// - `context`: Mutable reference to the state machine context
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(())` if exit action failed
    fn on_exit(&self, context: &mut Self::Context) -> Result<(), ()>;
}

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

        fn on_enter(&self, context: &mut Self::Context) -> Result<(), ()> {
            context.enter_count += 1;
            Ok(())
        }

        fn on_exit(&self, context: &mut Self::Context) -> Result<(), ()> {
            context.exit_count += 1;
            Ok(())
        }
    }

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
}
