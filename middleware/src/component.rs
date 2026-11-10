//! Component model for the middleware framework.
//!
//! This module defines the core abstractions for building components
//! in the middleware framework. Components can be passive (invoked
//! synchronously) or active (autonomous execution cycles).
//!
//! # Modules
//!
//! - `config`: Configuration traits and builders
//! - `state`: State management with immutable transformations
//! - `passive`: Passive component trait and invocation types
//! - `active`: Active component trait and scheduling
//! - `lifecycle`: Component lifecycle states and hooks
//!
//! # Traceability
//! - REQ-CMP-001: Components have associated Config, State, and Ports types
//! - REQ-CMP-002: Components have static NAME and ID identifiers
//! - REQ-CMP-003: Component construction returns Result<Self, InitError>

#![forbid(unsafe_code)]

pub mod config;
pub mod state;
pub mod passive;
pub mod active;
pub mod lifecycle;

use crate::error::InitError;

// Re-export key traits and types
pub use config::{ComponentConfig, ConfigBuilder};
pub use state::ComponentState;
pub use passive::{PassiveComponent, Invocation, Response, ResponseStatus};
pub use active::{ActiveComponent, Schedule, CycleResult};
pub use lifecycle::{Lifecycle, LifecycleState};

/// Type alias for component identifiers.
///
/// Component IDs are used to uniquely identify components within
/// a system. The u16 range supports up to 65535 components.
pub type ComponentId = u16;

/// Base trait for all components.
///
/// All components must provide associated types for configuration,
/// state, and port definitions. Components also have static metadata
/// including a name and unique identifier.
///
/// # Examples
///
/// ```
/// use middleware::InitError;
/// use middleware::component::{Component, ComponentConfig, ComponentId, ComponentState};
///
/// # #[derive(Debug, Clone, Default)]
/// # struct MyConfig { gain: u8 }
/// # impl ComponentConfig for MyConfig {}
/// # #[derive(Debug, Clone, Default, PartialEq)]
/// # struct MyState { cycles: u32 }
/// # impl ComponentState for MyState {}
/// # struct MyPorts;
/// struct MyComponent {
///     config: MyConfig,
///     state: MyState,
/// }
///
/// impl Component for MyComponent {
///     type Config = MyConfig;
///     type State = MyState;
///     type Ports = MyPorts;
///
///     const NAME: &'static str = "MyComponent";
///     const ID: ComponentId = 1;
///
///     fn new(config: Self::Config) -> Result<Self, InitError> {
///         Ok(Self {
///             config,
///             state: MyState::default(),
///         })
///     }
///
///     fn config(&self) -> &Self::Config {
///         &self.config
///     }
///
///     fn state(&self) -> &Self::State {
///         &self.state
///     }
/// }
///
/// let component = MyComponent::new(MyConfig { gain: 3 }).unwrap();
/// assert_eq!(MyComponent::NAME, "MyComponent");
/// assert_eq!(MyComponent::ID, 1);
/// assert_eq!(component.config().gain, 3);
/// assert_eq!(component.state(), &MyState::default());
/// ```
pub trait Component: Sized {
    /// Configuration type for this component
    type Config: ComponentConfig;

    /// State type for this component
    type State: ComponentState;

    /// Port definitions for this component
    type Ports;

    /// Human-readable component name
    const NAME: &'static str;

    /// Unique component identifier
    const ID: ComponentId;

    /// Construct a new component instance.
    ///
    /// Component construction may fail if the configuration is invalid
    /// or required resources are unavailable.
    ///
    /// # Errors
    ///
    /// Returns `InitError` if construction fails:
    /// - `InvalidConfig`: Configuration parameters invalid
    /// - `ResourceUnavailable`: Required resource not available
    /// - `HardwareFailure`: Hardware initialization failed
    /// - `Timeout`: Initialization timeout
    fn new(config: Self::Config) -> Result<Self, InitError>;

    /// Get reference to component configuration.
    fn config(&self) -> &Self::Config;

    /// Get reference to component state.
    fn state(&self) -> &Self::State;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default)]
    struct TestConfig {
        value: u32,
    }

    impl ComponentConfig for TestConfig {}

    #[derive(Debug, Clone)]
    struct TestState {
        counter: u32,
    }

    impl ComponentState for TestState {}

    struct TestPorts;

    struct TestComponent {
        config: TestConfig,
        state: TestState,
    }

    impl Component for TestComponent {
        type Config = TestConfig;
        type State = TestState;
        type Ports = TestPorts;

        const NAME: &'static str = "TestComponent";
        const ID: ComponentId = 42;

        fn new(config: Self::Config) -> Result<Self, InitError> {
            if config.value > 100 {
                return Err(InitError::InvalidConfig);
            }

            Ok(Self {
                config,
                state: TestState { counter: 0 },
            })
        }

        fn config(&self) -> &Self::Config {
            &self.config
        }

        fn state(&self) -> &Self::State {
            &self.state
        }
    }

    #[test]
    fn component_metadata() {
        assert_eq!(TestComponent::NAME, "TestComponent");
        assert_eq!(TestComponent::ID, 42);
    }

    #[test]
    fn component_new_success() {
        let config = TestConfig { value: 50 };
        let component = TestComponent::new(config);
        assert!(component.is_ok());
    }

    #[test]
    fn component_new_invalid_config() {
        let config = TestConfig { value: 150 };
        let result = TestComponent::new(config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e, InitError::InvalidConfig);
        }
    }

    #[test]
    fn component_accessors() {
        let config = TestConfig { value: 25 };
        let component = TestComponent::new(config).unwrap();

        assert_eq!(component.config().value, 25);
        assert_eq!(component.state().counter, 0);
    }
}
