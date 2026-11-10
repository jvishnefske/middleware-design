//! # Error Types
//!
//! Comprehensive error types for the middleware framework.
//!
//! # Traceability
//!
//! - REQ-PLT-010: All fallible operations return Result<T, E>
//! - REQ-PLT-011: Error types are exhaustive enums
//!
//! Every enum here is a closed set of payload-free variants, apart from
//! [`FrameworkError`], which only wraps the other enums. None is
//! `#[non_exhaustive]`; `checks/nostd-gen/tests/platform.rs` matches each
//! one from outside the crate without a wildcard, so adding a variant or the
//! attribute breaks that build until the change is reviewed.

/// Top-level framework error type.
///
/// # Traceability
///
/// - REQ-PLT-010: Result-based error handling
/// - REQ-PLT-011: Exhaustive enum variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FrameworkError {
    /// Component initialization failed
    Init(InitError),
    /// Lifecycle transition failed
    Lifecycle(LifecycleError),
    /// Port operation failed
    Port(PortError),
    /// State machine processing failed
    StateMachine(StateMachineError),
    /// Serialization/deserialization failed
    Serialize(SerializeError),
    /// Control block operation failed
    Control(ControlError),
    /// Component invocation failed
    Invoke(InvokeError),
    /// Component cycle execution failed
    Cycle(CycleError),
}

/// Component initialization errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InitError {
    /// Invalid configuration parameter
    InvalidConfig,
    /// Required resource unavailable
    ResourceUnavailable,
    /// Dependency initialization failed
    DependencyFailed,
    /// Hardware initialization failed
    HardwareFailed,
}

/// Component lifecycle state transition errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    /// Transition not allowed from current state
    InvalidTransition,
    /// State entry action failed
    EntryActionFailed,
    /// State exit action failed
    ExitActionFailed,
    /// Lifecycle hook returned error
    HookFailed,
}

/// Port communication errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PortError {
    /// Send buffer full
    SendBufferFull,
    /// Receive buffer empty
    ReceiveBufferEmpty,
    /// Timeout waiting for data
    Timeout,
    /// Invalid message format
    InvalidMessage,
    /// Port not connected
    NotConnected,
}

/// Passive component invocation errors.
///
/// # Traceability
///
/// - REQ-CMP-011: Passive component invoke() returns Result<Response, InvokeError>
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InvokeError {
    /// Invalid invocation parameters
    InvalidParameters,
    /// Component not in valid state for invocation
    InvalidState,
    /// Operation not supported by component
    NotSupported,
    /// Internal processing error
    ProcessingFailed,
}

/// Active component cycle execution errors.
///
/// # Traceability
///
/// - REQ-CMP-021: Active components implement run_cycle() returning Result<CycleResult, CycleError>
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CycleError {
    /// Component not ready to run
    NotReady,
    /// Input data unavailable
    InputUnavailable,
    /// Processing timeout occurred
    Timeout,
    /// Internal computation error
    ComputationError,
}

/// Port message receive errors.
///
/// # Traceability
///
/// - REQ-PRT-011: Sync input ports support blocking recv with timeout
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RecvError {
    /// No message available (timeout or would block)
    NoMessage,
    /// Channel disconnected
    Disconnected,
    /// Deserialization failed
    DeserializationFailed,
}

/// Port message send errors.
///
/// # Traceability
///
/// - REQ-PRT-021: Output ports return Result<(), SendError> on send
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SendError {
    /// Channel full (would block)
    Full,
    /// Channel disconnected
    Disconnected,
    /// Serialization failed
    SerializationFailed,
}

/// Message serialization/deserialization errors.
///
/// # Traceability
///
/// - REQ-PRT-001: Port messages implement serialize/deserialize to/from byte buffers
/// - REQ-PRT-002: Serialization errors are returned as Result
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SerializationError {
    /// Buffer too small for serialization
    BufferTooSmall,
    /// Invalid data format
    InvalidFormat,
    /// Encoding error
    EncodingError,
    /// Decoding error
    DecodingError,
}

/// State machine processing errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
/// - REQ-FSM-032: [`StateMachineError::InvalidTimeStep`] from
///   `TimedGuard::update`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StateMachineError {
    /// No valid transition for event in current state
    NoValidTransition,
    /// Guard condition evaluation failed
    GuardFailed,
    /// Transition action failed
    ActionFailed,
    /// Invalid state
    InvalidState,
    /// Invalid event
    InvalidEvent,
    /// Time step is negative, NaN or infinite
    InvalidTimeStep,
}

/// Serialization/deserialization errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SerializeError {
    /// Buffer too small for serialized data
    BufferTooSmall,
    /// Invalid CBOR format
    InvalidFormat,
    /// Unsupported message type
    UnsupportedType,
    /// Encoding failed
    EncodeFailed,
    /// Decoding failed
    DecodeFailed,
}

/// Control block operation errors.
///
/// # Traceability
///
/// - REQ-PLT-011: Exhaustive error variants
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// Invalid gain parameter
    InvalidGain,
    /// Invalid time step
    InvalidTimeStep,
    /// Numerical overflow detected
    Overflow,
    /// Division by zero
    DivisionByZero,
    /// Matrix dimension mismatch
    DimensionMismatch,
}

impl From<InitError> for FrameworkError {
    fn from(e: InitError) -> Self {
        FrameworkError::Init(e)
    }
}

impl From<LifecycleError> for FrameworkError {
    fn from(e: LifecycleError) -> Self {
        FrameworkError::Lifecycle(e)
    }
}

impl From<PortError> for FrameworkError {
    fn from(e: PortError) -> Self {
        FrameworkError::Port(e)
    }
}

impl From<StateMachineError> for FrameworkError {
    fn from(e: StateMachineError) -> Self {
        FrameworkError::StateMachine(e)
    }
}

impl From<SerializeError> for FrameworkError {
    fn from(e: SerializeError) -> Self {
        FrameworkError::Serialize(e)
    }
}

impl From<ControlError> for FrameworkError {
    fn from(e: ControlError) -> Self {
        FrameworkError::Control(e)
    }
}

impl From<InvokeError> for FrameworkError {
    fn from(e: InvokeError) -> Self {
        FrameworkError::Invoke(e)
    }
}

impl From<CycleError> for FrameworkError {
    fn from(e: CycleError) -> Self {
        FrameworkError::Cycle(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    #[test]
    fn test_error_conversion() {
        let init_err = InitError::InvalidConfig;
        let fw_err: FrameworkError = init_err.into();
        assert_eq!(fw_err, FrameworkError::Init(InitError::InvalidConfig));
    }

    #[test]
    fn test_error_clone() {
        let err = PortError::Timeout;
        let cloned = Clone::clone(&err);
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_error_debug() {
        let err = ControlError::Overflow;
        let debug_str = std::format!("{:?}", err);
        assert!(debug_str.contains("Overflow"));
    }

    /// Fails with the leaf error `e` when `fail` is set; stands in for a
    /// fallible subsystem call.
    fn leaf<E>(fail: bool, e: E) -> Result<(), E> {
        if fail {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// Runs one leaf of each kind with `?`, so each leaf error propagates
    /// into `FrameworkError` through its `From` impl.
    fn pipeline(failing: usize) -> Result<(), FrameworkError> {
        leaf(failing == 0, InitError::HardwareFailed)?;
        leaf(failing == 1, LifecycleError::HookFailed)?;
        leaf(failing == 2, PortError::NotConnected)?;
        leaf(failing == 3, StateMachineError::InvalidEvent)?;
        leaf(failing == 4, SerializeError::DecodeFailed)?;
        leaf(failing == 5, ControlError::DimensionMismatch)?;
        leaf(failing == 6, InvokeError::ProcessingFailed)?;
        leaf(failing == 7, CycleError::ComputationError)?;
        Ok(())
    }

    /// Fallible operations compose as `Result`: every leaf error lifts into
    /// the matching `FrameworkError` wrapper with `?` and keeps its value,
    /// and the all-success path yields `Ok`. Each expected value is written
    /// as an explicit variant, so the eight `From` impls are each checked
    /// against the variant they must produce.
    /// Verifies: REQ-PLT-010, REQ-PLT-011
    #[test]
    fn every_leaf_error_propagates_into_its_wrapper() {
        let expected = [
            FrameworkError::Init(InitError::HardwareFailed),
            FrameworkError::Lifecycle(LifecycleError::HookFailed),
            FrameworkError::Port(PortError::NotConnected),
            FrameworkError::StateMachine(StateMachineError::InvalidEvent),
            FrameworkError::Serialize(SerializeError::DecodeFailed),
            FrameworkError::Control(ControlError::DimensionMismatch),
            FrameworkError::Invoke(InvokeError::ProcessingFailed),
            FrameworkError::Cycle(CycleError::ComputationError),
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(pipeline(i), Err(*want), "leaf {i}");
        }
        assert_eq!(pipeline(expected.len()), Ok(()));
    }

    /// Wrapper variants are distinct: equal leaves under different wrappers
    /// are unequal, and a wrapper compares by its inner value.
    #[test]
    fn wrappers_distinguish_source_and_value() {
        assert_ne!(
            FrameworkError::Serialize(SerializeError::BufferTooSmall),
            FrameworkError::Port(PortError::SendBufferFull)
        );
        assert_ne!(
            FrameworkError::Init(InitError::InvalidConfig),
            FrameworkError::Init(InitError::ResourceUnavailable)
        );
    }
}
