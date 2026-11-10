//! Component configuration traits and utilities.
//!
//! This module defines traits for component configuration management.
//! Configurations are immutable and constructed via the Builder pattern.
//!
//! # Traceability
//! - REQ-CMP-001: Components have associated Config, State, and Ports types

#![forbid(unsafe_code)]

/// Trait for component configuration types.
///
/// All component configurations must implement Default to provide
/// safe baseline configurations. Configurations are immutable and
/// must be constructed before component initialization.
///
/// # Examples
///
/// ```
/// use middleware::component::ComponentConfig;
///
/// #[derive(Debug, Clone, Default)]
/// struct MotorControllerConfig {
///     max_rpm: u16,
///     pid_kp: f32,
/// }
///
/// impl ComponentConfig for MotorControllerConfig {}
///
/// // `Default` supplies the safe baseline configuration.
/// let config = MotorControllerConfig::default();
/// assert_eq!(config.max_rpm, 0);
/// assert_eq!(config.pid_kp, 0.0);
/// ```
pub trait ComponentConfig: Default + Clone {}

/// Trait for configuration builders following the Builder pattern.
///
/// Builders provide a fluent API for constructing complex configurations
/// with validation. All builder methods return a new builder instance,
/// maintaining immutability.
///
/// # Examples
///
/// ```
/// use middleware::component::{ComponentConfig, ConfigBuilder};
///
/// # #[derive(Debug, Clone, Default, PartialEq)]
/// # struct MotorControllerConfig { max_rpm: u16 }
/// # impl ComponentConfig for MotorControllerConfig {}
/// // The error type is chosen by each builder.
/// #[derive(Debug, PartialEq)]
/// enum ConfigError {
///     InvalidParameter,
/// }
///
/// struct MotorControllerConfigBuilder {
///     config: MotorControllerConfig,
/// }
///
/// impl MotorControllerConfigBuilder {
///     fn new() -> Self {
///         Self { config: MotorControllerConfig::default() }
///     }
///
///     fn max_rpm(self, max_rpm: u16) -> Self {
///         Self { config: MotorControllerConfig { max_rpm, ..self.config } }
///     }
/// }
///
/// impl ConfigBuilder for MotorControllerConfigBuilder {
///     type Config = MotorControllerConfig;
///     type Error = ConfigError;
///
///     fn build(self) -> Result<Self::Config, Self::Error> {
///         // Validate configuration
///         if self.config.max_rpm == 0 {
///             return Err(ConfigError::InvalidParameter);
///         }
///         Ok(self.config)
///     }
/// }
///
/// let config = MotorControllerConfigBuilder::new().max_rpm(3000).build();
/// assert_eq!(config, Ok(MotorControllerConfig { max_rpm: 3000 }));
/// assert_eq!(MotorControllerConfigBuilder::new().build(), Err(ConfigError::InvalidParameter));
/// ```
pub trait ConfigBuilder {
    /// The configuration type being built
    type Config: ComponentConfig;

    /// Error type for configuration validation
    type Error;

    /// Build and validate the final configuration.
    ///
    /// This method consumes the builder and returns either a valid
    /// configuration or an error if validation fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration parameters are invalid
    /// or inconsistent.
    fn build(self) -> Result<Self::Config, Self::Error>;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::expect_used))]
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default, PartialEq)]
    struct TestConfig {
        value: u32,
    }

    impl ComponentConfig for TestConfig {}

    #[derive(Debug)]
    enum TestConfigError {
        ValueTooLarge,
    }

    struct TestConfigBuilder {
        config: TestConfig,
    }

    impl TestConfigBuilder {
        fn new() -> Self {
            Self {
                config: TestConfig::default(),
            }
        }

        fn with_value(mut self, value: u32) -> Self {
            self.config.value = value;
            self
        }
    }

    impl ConfigBuilder for TestConfigBuilder {
        type Config = TestConfig;
        type Error = TestConfigError;

        fn build(self) -> Result<Self::Config, Self::Error> {
            if self.config.value > 100 {
                return Err(TestConfigError::ValueTooLarge);
            }
            Ok(self.config)
        }
    }

    #[test]
    fn config_has_default() {
        let config = TestConfig::default();
        assert_eq!(config.value, 0);
    }

    #[test]
    fn builder_success() {
        let config = TestConfigBuilder::new()
            .with_value(50)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.value, 50);
    }

    #[test]
    fn builder_validation_error() {
        let result = TestConfigBuilder::new()
            .with_value(150)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn config_is_clone() {
        let config1 = TestConfig { value: 42 };
        let config2 = config1.clone();
        assert_eq!(config1, config2);
    }
}
