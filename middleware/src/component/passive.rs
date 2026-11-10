//! Passive component trait and invocation types.
//!
//! Passive components are invoked synchronously by external callers.
//! They process requests and return responses without autonomous execution.
//!
//! # Traceability
//! - REQ-CMP-010: Passive components are invoked synchronously
//! - REQ-CMP-011: Passive component invoke() returns Result<Response, InvokeError>

#![forbid(unsafe_code)]

use crate::error::InvokeError;

/// Types of invocations supported by passive components.
///
/// Passive components can handle three categories of operations:
/// - Commands: Operations that modify state
/// - Queries: Operations that read state without modification
/// - Updates: Configuration or parameter updates
///
/// # Examples
///
/// ```
/// use middleware::component::Invocation;
///
/// fn describe(invocation: Invocation) -> (&'static str, u16) {
///     match invocation {
///         Invocation::Command(cmd_id) => ("command", cmd_id),
///         Invocation::Query(query_id) => ("query", query_id),
///         Invocation::Update(param_id) => ("update", param_id),
///     }
/// }
///
/// assert_eq!(describe(Invocation::Command(7)), ("command", 7));
/// assert_eq!(describe(Invocation::Query(1)), ("query", 1));
/// assert_eq!(describe(Invocation::Update(3)), ("update", 3));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invocation {
    /// Execute a command that may modify component state
    Command(u16),

    /// Execute a query that reads state without modification
    Query(u16),

    /// Update a configuration parameter
    Update(u16),
}

/// Response from passive component invocation.
///
/// Responses include the invocation type that was processed and
/// optional payload data. The payload is a fixed-size buffer to
/// maintain no_std compatibility.
///
/// # Examples
///
/// ```
/// use middleware::component::{Invocation, Response, ResponseStatus};
///
/// let response = Response {
///     invocation: Invocation::Query(1),
///     status: ResponseStatus::Success,
///     payload: [0; 32],
///     payload_len: 0,
/// };
/// assert_eq!(response, Response::success(Invocation::Query(1)));
///
/// let with_data = Response::with_payload(Invocation::Query(1), &[0xAB, 0xCD]).unwrap();
/// assert_eq!(&with_data.payload[..with_data.payload_len], &[0xAB, 0xCD]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The invocation that produced this response
    pub invocation: Invocation,

    /// Status of the invocation
    pub status: ResponseStatus,

    /// Response payload data
    pub payload: [u8; 32],

    /// Valid length of payload data
    pub payload_len: usize,
}

impl Response {
    /// Create a success response with no payload.
    pub fn success(invocation: Invocation) -> Self {
        Self {
            invocation,
            status: ResponseStatus::Success,
            payload: [0; 32],
            payload_len: 0,
        }
    }

    /// Create a success response with payload.
    ///
    /// # Errors
    ///
    /// Returns error if payload exceeds 32 bytes.
    pub fn with_payload(invocation: Invocation, payload: &[u8]) -> Result<Self, InvokeError> {
        let mut response = Self::success(invocation);
        response
            .payload
            .get_mut(..payload.len())
            .ok_or(InvokeError::ProcessingFailed)?
            .copy_from_slice(payload);
        response.payload_len = payload.len();
        Ok(response)
    }

    /// Create an error response.
    pub fn error(invocation: Invocation, status: ResponseStatus) -> Self {
        Self {
            invocation,
            status,
            payload: [0; 32],
            payload_len: 0,
        }
    }
}

/// Status codes for invocation responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseStatus {
    /// Invocation succeeded
    Success = 0,

    /// Invalid parameters provided
    InvalidParameters = 1,

    /// Component not in valid state
    InvalidState = 2,

    /// Operation not supported
    NotSupported = 3,

    /// Processing failed
    ProcessingFailed = 4,
}

/// Trait for passive components.
///
/// Passive components respond to external invocations synchronously.
/// They do not have autonomous execution cycles and only perform
/// work when explicitly invoked.
///
/// # Examples
///
/// ```
/// use middleware::InvokeError;
/// use middleware::component::{Invocation, PassiveComponent, Response, ResponseStatus};
///
/// struct MyPassiveComponent;
///
/// impl PassiveComponent for MyPassiveComponent {
///     fn invoke(&self, invocation: Invocation) -> Result<Response, InvokeError> {
///         match invocation {
///             Invocation::Query(0) => Ok(Response::success(invocation)),
///             _ => Err(InvokeError::NotSupported),
///         }
///     }
/// }
///
/// let component = MyPassiveComponent;
/// let response = component.invoke(Invocation::Query(0)).unwrap();
/// assert_eq!(response.status, ResponseStatus::Success);
/// assert_eq!(component.invoke(Invocation::Command(1)), Err(InvokeError::NotSupported));
/// ```
pub trait PassiveComponent {
    /// Invoke an operation on the component.
    ///
    /// The component processes the invocation and returns a response.
    /// This method should be deterministic and complete within a
    /// bounded time.
    ///
    /// # Errors
    ///
    /// Returns `InvokeError` if the invocation cannot be processed:
    /// - `InvalidParameters`: Invocation parameters are invalid
    /// - `InvalidState`: Component state prevents processing
    /// - `NotSupported`: Invocation type not supported
    /// - `ProcessingFailed`: Internal processing error
    fn invoke(&self, invocation: Invocation) -> Result<Response, InvokeError>;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#[cfg(test)]
mod tests {
    use super::*;

    struct TestPassiveComponent {
        value: u32,
    }

    impl PassiveComponent for TestPassiveComponent {
        fn invoke(&self, invocation: Invocation) -> Result<Response, InvokeError> {
            match invocation {
                Invocation::Query(0) => {
                    // Return current value as payload
                    let payload = self.value.to_le_bytes();
                    Response::with_payload(invocation, &payload)
                }
                Invocation::Command(_) => Ok(Response::success(invocation)),
                _ => Err(InvokeError::NotSupported),
            }
        }
    }

    #[test]
    fn invocation_types() {
        let cmd = Invocation::Command(1);
        let query = Invocation::Query(2);
        let update = Invocation::Update(3);

        assert_ne!(cmd, query);
        assert_ne!(query, update);
    }

    #[test]
    fn response_success() {
        let inv = Invocation::Query(0);
        let resp = Response::success(inv);

        assert_eq!(resp.invocation, inv);
        assert_eq!(resp.status, ResponseStatus::Success);
        assert_eq!(resp.payload_len, 0);
    }

    #[test]
    fn response_with_payload() {
        let inv = Invocation::Query(0);
        let payload = [1, 2, 3, 4];
        let resp = Response::with_payload(inv, &payload).expect("Should create response");

        assert_eq!(resp.payload_len, 4);
        assert_eq!(&resp.payload[..4], &payload);
    }

    #[test]
    fn response_payload_too_large() {
        let inv = Invocation::Query(0);
        let payload = [0u8; 64];
        let result = Response::with_payload(inv, &payload);

        assert!(result.is_err());
    }

    #[test]
    fn response_error() {
        let inv = Invocation::Command(1);
        let resp = Response::error(inv, ResponseStatus::InvalidState);

        assert_eq!(resp.status, ResponseStatus::InvalidState);
    }

    #[test]
    fn passive_component_query() {
        let component = TestPassiveComponent { value: 42 };
        let result = component.invoke(Invocation::Query(0));

        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status, ResponseStatus::Success);
        assert_eq!(resp.payload_len, 4);
    }

    #[test]
    fn passive_component_command() {
        let component = TestPassiveComponent { value: 0 };
        let result = component.invoke(Invocation::Command(1));

        assert!(result.is_ok());
    }

    #[test]
    fn passive_component_not_supported() {
        let component = TestPassiveComponent { value: 0 };
        let result = component.invoke(Invocation::Update(0));

        assert_eq!(result, Err(InvokeError::NotSupported));
    }
}
