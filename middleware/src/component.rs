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

pub mod active;
pub mod config;
pub mod lifecycle;
pub mod passive;
pub mod state;

use crate::error::InitError;

// Re-export key traits and types
pub use active::{ActiveComponent, CycleResult, Schedule};
pub use config::{ComponentConfig, ConfigBuilder};
pub use lifecycle::{Lifecycle, LifecycleState};
pub use passive::{Invocation, PassiveComponent, Response, ResponseStatus};
pub use state::ComponentState;

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
    /// - `DependencyFailed`: A dependency failed to initialize
    /// - `HardwareFailed`: Hardware initialization failed
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

    /// Second component used to show NAME and ID are per type.
    struct OtherComponent {
        config: TestConfig,
        state: TestState,
    }

    impl Component for OtherComponent {
        type Config = TestConfig;
        type State = TestState;
        type Ports = ();

        const NAME: &'static str = "OtherComponent";
        const ID: ComponentId = ComponentId::MAX;

        fn new(config: Self::Config) -> Result<Self, InitError> {
            Ok(Self {
                config,
                state: TestState { counter: 1 },
            })
        }

        fn config(&self) -> &Self::Config {
            &self.config
        }

        fn state(&self) -> &Self::State {
            &self.state
        }
    }

    /// Compiles only if `C` names exactly these Config, State and Ports types.
    fn assert_associated_types<C>()
    where
        C: Component<Config = TestConfig, State = TestState, Ports = TestPorts>,
    {
    }

    /// Compiles only if the associated types meet the trait bounds.
    fn assert_bounds<C: Component>()
    where
        C::Config: ComponentConfig + Default + Clone,
        C::State: ComponentState + Clone,
    {
    }

    /// Reads metadata through the trait bound only, as a framework would.
    fn metadata<C: Component>() -> (&'static str, ComponentId) {
        (C::NAME, C::ID)
    }

    /// Associated types are checked at compile time by the generic helpers;
    /// the accessors return references of those associated types.
    /// Verifies: REQ-CMP-001
    #[test]
    fn component_has_config_state_and_ports_types() {
        assert_associated_types::<TestComponent>();
        assert_bounds::<TestComponent>();
        assert_bounds::<OtherComponent>();
        let component = TestComponent::new(TestConfig { value: 7 }).unwrap();
        let config: &<TestComponent as Component>::Config = component.config();
        let state: &<TestComponent as Component>::State = component.state();
        assert_eq!(config.value, 7);
        assert_eq!(state.counter, 0);
        let other = OtherComponent::new(TestConfig { value: 3 }).unwrap();
        assert_eq!(other.config().value, 3);
        assert_eq!(other.state().counter, 1);
        // Ports is a pure type-level association; a zero-sized marker is valid.
        assert_eq!(
            core::mem::size_of::<<TestComponent as Component>::Ports>(),
            0
        );
        assert_eq!(
            core::mem::size_of::<<OtherComponent as Component>::Ports>(),
            0
        );
    }

    /// NAME and ID are associated constants: they are usable in `const`
    /// context (so known at compile time, with no instance), differ per
    /// type, and the `ComponentId` boundary `u16::MAX` is representable.
    /// Verifies: REQ-CMP-002
    #[test]
    fn component_has_static_name_and_id() {
        const NAME: &str = TestComponent::NAME;
        const ID: ComponentId = TestComponent::ID;
        const OTHER_ID: ComponentId = OtherComponent::ID;
        assert_eq!(NAME, "TestComponent");
        assert_eq!(ID, 42);
        assert_eq!(OTHER_ID, u16::MAX);
        assert_eq!(metadata::<TestComponent>(), ("TestComponent", 42));
        assert_eq!(metadata::<OtherComponent>(), ("OtherComponent", u16::MAX));
        assert_ne!(metadata::<TestComponent>(), metadata::<OtherComponent>());
    }

    /// The constructor signature is pinned by coercion to a function pointer
    /// type; the test component's validity boundary (value <= 100) gives Ok
    /// at 0 and 100 and `InitError::InvalidConfig` at 101 and `u32::MAX`.
    /// Verifies: REQ-CMP-003
    #[test]
    fn component_new_returns_result_with_init_error() {
        let constructor: fn(TestConfig) -> Result<TestComponent, InitError> =
            <TestComponent as Component>::new;
        for value in [0, 100] {
            let component = constructor(TestConfig { value }).unwrap();
            assert_eq!(component.config().value, value);
        }
        for value in [101, u32::MAX] {
            assert!(matches!(
                constructor(TestConfig { value }),
                Err(InitError::InvalidConfig)
            ));
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
        assert!(matches!(result, Err(InitError::InvalidConfig)));
    }

    #[test]
    fn component_accessors() {
        let config = TestConfig { value: 25 };
        let component = TestComponent::new(config).unwrap();

        assert_eq!(component.config().value, 25);
        assert_eq!(component.state().counter, 0);
    }
}
