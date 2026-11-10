//! Component and port code generation.
//!
//! Generated code here refers to the runtime crate by absolute path
//! (`::middleware::...`), so a crate expanding a model with ports or
//! components must depend on `middleware`. Models without them expand to
//! code with no runtime dependency.
//!
//! # Port messages
//! Every message or struct named as a `message_type` in the model's `ports`
//! list gets an `impl ::middleware::PortMessage`: fields are written in
//! declaration order as fixed-width little-endian bytes (`bool` as one byte,
//! 0 or 1), with a `WIRE_SIZE` constant. Encoding and decoding use only
//! checked slice access and return `SerializationError` instead of
//! panicking. Field types must be primitive numbers or `bool`, and
//! `WIRE_SIZE` must not exceed `::middleware::port::MAX_MESSAGE_SIZE`.
//!
//! # Components
//! For a component `Foo` the generator emits:
//! - `FooConfig`: an empty `ComponentConfig` (the model has no component
//!   configuration fields yet).
//! - `FooState`: a `ComponentState` holding `current`, the component's state
//!   value, or no fields if the component declares no state.
//! - `FooCommand`: an enum with one variant per command (PascalCase), plus
//!   `COUNT` and `ALL`.
//! - `FooPorts<..>`: one field per port, each generic parameter bounded by
//!   `InputPort<M>` or `OutputPort<M>` for the port's message type `M`.
//! - `Foo<..>`: holds config, state and ports. `with_ports(config, ports)`
//!   builds it in the initial state; `ports()` lends the ports; constants
//!   describe port names, directions and message types.
//! - `impl ::middleware::Component for Foo<..>`, `NAME` the component name
//!   and `ID` its 1-based position in the model. `Component::new` takes no
//!   ports, so it is implemented when every port type is `Default`
//!   (a disconnected handle) and builds `FooPorts::default()`.
//! - If the component names a `state_machine`, `handle(self, event, hooks)`
//!   runs that machine's `step` on the current state and returns the
//!   updated component with the outcome; on a hook error the component is
//!   returned unchanged alongside the error.
//!
//! The initial state is the state machine's `INITIAL_STATE` if one is
//! linked, otherwise the first variant of the state enum. Construction does
//! not run the initial state's `on_enter` action; hooks run only from
//! `handle`.
//!
//! # Traceability
//! - REQ-GEN-011: Generates structs, traits and implementations for components
//! - REQ-CMP-001: Components have associated Config, State and Ports types
//! - REQ-CMP-002: Components have static NAME and ID identifiers
//! - REQ-CMP-003: Component construction returns `Result<Self, InitError>`
//! - REQ-PRT-001: Port messages implement serialize/deserialize
//! - REQ-PRT-010: Input ports parameterized by message type
//! - REQ-PRT-020: Output ports parameterized by message type

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::naming::{pascal_ident, snake_ident};
use super::traces_doc;
use crate::model::{Component, Field, Model, PortDef};

/// Maximum serialized size the runtime accepts for one message.
///
/// Mirrors `middleware::port::MAX_MESSAGE_SIZE`; the codegen crate cannot
/// depend on the runtime crate, so the value is restated here.
const MAX_MESSAGE_SIZE: usize = 256;

/// Port direction parsed from the model's `direction` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Receives messages.
    Input,
    /// Sends messages.
    Output,
}

impl Direction {
    /// Parses a model direction string.
    fn parse(port: &PortDef) -> Result<Self, String> {
        match port.direction.as_str() {
            "input" => Ok(Self::Input),
            "output" => Ok(Self::Output),
            other => Err(format!(
                "Port '{}' direction '{}' must be 'input' or 'output'",
                port.name, other
            )),
        }
    }
}

/// Wire encoding of a primitive field type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireType {
    /// Fixed-width number written with `to_le_bytes`; holds the byte width.
    Number(usize),
    /// One byte, 0 or 1.
    Bool,
}

impl WireType {
    /// Maps a model field type to its wire encoding.
    fn of(field_type: &str) -> Option<Self> {
        match field_type {
            "u8" | "i8" => Some(Self::Number(1)),
            "u16" | "i16" => Some(Self::Number(2)),
            "u32" | "i32" | "f32" => Some(Self::Number(4)),
            "u64" | "i64" | "f64" => Some(Self::Number(8)),
            "bool" => Some(Self::Bool),
            _ => None,
        }
    }

    /// Encoded width in bytes.
    fn width(self) -> usize {
        match self {
            Self::Number(n) => n,
            Self::Bool => 1,
        }
    }
}

/// Finds the fields of a message or struct type defined in the model.
fn message_fields<'a>(model: &'a Model, type_name: &str) -> Option<&'a [Field]> {
    model
        .messages
        .iter()
        .find(|m| m.name == type_name)
        .map(|m| m.fields.as_slice())
        .or_else(|| {
            model
                .types
                .structs
                .iter()
                .find(|s| s.name == type_name)
                .map(|s| s.fields.as_slice())
        })
}

/// Generates the `PortMessage` implementation for one message type.
fn generate_port_message(model: &Model, type_name: &str) -> Result<TokenStream, String> {
    let fields = message_fields(model, type_name).ok_or_else(|| {
        format!("Port message type '{}' is not a model message or struct", type_name)
    })?;
    let wire = fields
        .iter()
        .map(|f| {
            WireType::of(&f.field_type).map(|w| (f, w)).ok_or_else(|| {
                format!(
                    "Port message '{}' field '{}' has type '{}'; only integer, float and bool fields can be sent on a port",
                    type_name, f.name, f.field_type
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let offsets: Vec<usize> = wire
        .iter()
        .scan(0usize, |offset, (_, w)| {
            let start = *offset;
            *offset += w.width();
            Some(start)
        })
        .collect();
    let size: usize = wire.iter().map(|(_, w)| w.width()).sum();
    if size > MAX_MESSAGE_SIZE {
        return Err(format!(
            "Port message '{}' is {} bytes; the maximum is {}",
            type_name, size, MAX_MESSAGE_SIZE
        ));
    }

    let name = format_ident!("{}", type_name);
    let writes = wire.iter().zip(&offsets).map(|((f, w), &start)| {
        let field = format_ident!("{}", f.name);
        let end = start + w.width();
        let bytes = match w {
            WireType::Number(_) => quote! { self.#field.to_le_bytes() },
            WireType::Bool => quote! { [u8::from(self.#field)] },
        };
        quote! {
            buffer
                .get_mut(#start..#end)
                .ok_or(::middleware::SerializationError::BufferTooSmall)?
                .copy_from_slice(&#bytes);
        }
    });
    let reads = wire.iter().zip(&offsets).map(|((f, w), &start)| {
        let field = format_ident!("{}", f.name);
        match w {
            WireType::Number(width) => {
                let ty = format_ident!("{}", f.field_type);
                let end = start + width;
                quote! {
                    #field: #ty::from_le_bytes(
                        buffer
                            .get(#start..#end)
                            .and_then(|bytes| <[u8; #width]>::try_from(bytes).ok())
                            .ok_or(::middleware::SerializationError::InvalidFormat)?,
                    )
                }
            }
            WireType::Bool => quote! {
                #field: match buffer.get(#start) {
                    Some(0) => false,
                    Some(1) => true,
                    _ => return Err(::middleware::SerializationError::InvalidFormat),
                }
            },
        }
    });

    Ok(quote! {
        impl #name {
            /// Serialized size in bytes.
            pub const WIRE_SIZE: usize = #size;
        }

        impl ::middleware::PortMessage for #name {
            fn serialize(&self, buffer: &mut [u8]) -> Result<usize, ::middleware::SerializationError> {
                #(#writes)*
                Ok(#size)
            }

            fn deserialize(buffer: &[u8]) -> Result<Self, ::middleware::SerializationError> {
                Ok(Self {
                    #(#reads),*
                })
            }

            fn max_size() -> usize {
                #size
            }
        }
    })
}

/// Generates `PortMessage` implementations for every type used by a port.
///
/// # Errors
/// Returns an error if a port's direction is invalid, its name is not
/// snake_case, its message type is not a model message or struct, a field
/// type cannot be encoded, or the encoded size exceeds the runtime maximum.
pub fn generate_port_messages(model: &Model) -> Result<TokenStream, String> {
    model.ports.iter().try_for_each(|p| {
        snake_ident(&p.name, "port")?;
        Direction::parse(p).map(|_| ())
    })?;
    if let Some((_, dup)) = model
        .ports
        .iter()
        .enumerate()
        .find(|(i, p)| model.ports[..*i].iter().any(|q| q.name == p.name))
    {
        return Err(format!("Port '{}' is defined twice", dup.name));
    }
    let types: Vec<&str> = model
        .ports
        .iter()
        .map(|p| p.message_type.as_str())
        .fold(Vec::new(), |acc, t| {
            if acc.contains(&t) {
                acc
            } else {
                acc.into_iter().chain(core::iter::once(t)).collect()
            }
        });
    let impls = types
        .iter()
        .map(|t| generate_port_message(model, t))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#impls)* })
}

/// A component port resolved against the model's port list.
struct ResolvedPort<'a> {
    /// Port definition.
    def: &'a PortDef,
    /// Parsed direction.
    direction: Direction,
    /// Field name in the ports struct.
    field: Ident,
    /// Generic parameter naming the port implementation type.
    generic: Ident,
}

/// Resolves the port names a component lists in one direction.
fn resolve_ports<'a>(
    model: &'a Model,
    comp: &Component,
    names: &[String],
    expected: Direction,
) -> Result<Vec<ResolvedPort<'a>>, String> {
    names
        .iter()
        .map(|n| {
            let def = model.ports.iter().find(|p| &p.name == n).ok_or_else(|| {
                format!("Component '{}' uses undefined port '{}'", comp.name, n)
            })?;
            let direction = Direction::parse(def)?;
            if direction != expected {
                return Err(format!(
                    "Component '{}' lists port '{}' as {:?} but it is declared {}",
                    comp.name, n, expected, def.direction
                ));
            }
            Ok(ResolvedPort {
                def,
                direction,
                field: snake_ident(n, "port")?,
                generic: format_ident!("{}Port", pascal_ident(n, "port")?),
            })
        })
        .collect()
}

/// How a component's state is initialised and evolved.
enum StateBinding {
    /// No state declared.
    Stateless,
    /// A state enum with no linked machine; starts at the first variant.
    Fixed { ty: Ident, initial: Ident },
    /// Driven by a generated state machine module.
    Machine { ty: Ident, event: Ident, module: Ident },
}

/// Determines a component's state binding from its `state` and `state_machine`.
fn bind_state(model: &Model, comp: &Component) -> Result<StateBinding, String> {
    match (&comp.state, &comp.state_machine) {
        (_, Some(fsm_name)) => {
            let fsm = model
                .state_machines
                .iter()
                .find(|f| &f.name == fsm_name)
                .ok_or_else(|| {
                    format!("Component '{}' links undefined state machine '{}'", comp.name, fsm_name)
                })?;
            match &comp.state {
                Some(s) if s != &fsm.state_type => Err(format!(
                    "Component '{}' state '{}' differs from state machine '{}' state type '{}'",
                    comp.name, s, fsm.name, fsm.state_type
                )),
                _ => Ok(StateBinding::Machine {
                    ty: format_ident!("{}", fsm.state_type),
                    event: format_ident!("{}", fsm.events),
                    module: format_ident!("{}", fsm.name),
                }),
            }
        }
        (Some(s), None) => {
            let first = model
                .types
                .enums
                .iter()
                .find(|e| &e.name == s)
                .and_then(|e| e.variants.first())
                .ok_or_else(|| {
                    format!(
                        "Component '{}' state '{}' must be a model enum when no state machine is linked",
                        comp.name, s
                    )
                })?;
            Ok(StateBinding::Fixed {
                ty: format_ident!("{}", s),
                initial: format_ident!("{}", first),
            })
        }
        (None, None) => Ok(StateBinding::Stateless),
    }
}

/// Generates code for one component.
fn generate_component(model: &Model, comp: &Component, index: usize) -> Result<TokenStream, String> {
    let id = u16::try_from(index + 1)
        .map_err(|_| format!("Component '{}': too many components for a u16 ID", comp.name))?;
    let inputs = resolve_ports(model, comp, &comp.input_ports, Direction::Input)?;
    let outputs = resolve_ports(model, comp, &comp.output_ports, Direction::Output)?;
    let ports: Vec<ResolvedPort> = inputs.into_iter().chain(outputs).collect();
    if let Some((_, dup)) = ports
        .iter()
        .enumerate()
        .find(|(i, p)| ports[..*i].iter().any(|q| q.def.name == p.def.name))
    {
        return Err(format!("Component '{}' lists port '{}' twice", comp.name, dup.def.name));
    }
    let binding = bind_state(model, comp)?;
    let commands = comp
        .commands
        .iter()
        .map(|c| pascal_ident(c, "command"))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((i, _)) = commands
        .iter()
        .enumerate()
        .find(|(i, c)| commands[..*i].contains(c))
    {
        return Err(format!("Component '{}' command '{}' is duplicated", comp.name, comp.commands[i]));
    }

    let name = format_ident!("{}", comp.name);
    let name_str = comp.name.as_str();
    let config = format_ident!("{}Config", comp.name);
    let state = format_ident!("{}State", comp.name);
    let command = format_ident!("{}Command", comp.name);
    let ports_ty = format_ident!("{}Ports", comp.name);
    let traces = traces_doc(&comp.traces);
    let trace_ids: Vec<&str> = comp.traces.iter().map(String::as_str).collect();
    let trace_count = trace_ids.len();
    let is_active = comp.kind == "active";

    let generics: Vec<&Ident> = ports.iter().map(|p| &p.generic).collect();
    let fields: Vec<&Ident> = ports.iter().map(|p| &p.field).collect();
    let bounds: Vec<TokenStream> = ports
        .iter()
        .map(|p| {
            let g = &p.generic;
            let msg = format_ident!("{}", p.def.message_type);
            match p.direction {
                Direction::Input => quote! { #g: ::middleware::port::InputPort<#msg> },
                Direction::Output => quote! { #g: ::middleware::port::OutputPort<#msg> },
            }
        })
        .collect();
    let default_bounds: Vec<TokenStream> = generics
        .iter()
        .map(|g| quote! { #g: ::core::default::Default })
        .collect();
    let port_docs: Vec<String> = ports
        .iter()
        .map(|p| format!("Port `{}` ({}) carrying `{}`.", p.def.name, p.def.direction, p.def.message_type))
        .collect();
    let port_count = ports.len();
    let port_names: Vec<&str> = ports.iter().map(|p| p.def.name.as_str()).collect();
    let port_types: Vec<&str> = ports.iter().map(|p| p.def.message_type.as_str()).collect();
    let port_dirs: Vec<TokenStream> = ports
        .iter()
        .map(|p| match p.direction {
            Direction::Input => quote! { ::middleware::PortDirection::Input },
            Direction::Output => quote! { ::middleware::PortDirection::Output },
        })
        .collect();
    let command_count = commands.len();

    let (state_fields, initial_state) = match &binding {
        StateBinding::Stateless => (quote! {}, quote! { #state {} }),
        StateBinding::Fixed { ty, initial } => (
            quote! {
                /// Current state value.
                pub current: #ty,
            },
            quote! { #state { current: #ty::#initial } },
        ),
        StateBinding::Machine { ty, module, .. } => (
            quote! {
                /// Current state value.
                pub current: #ty,
            },
            quote! { #state { current: #module::INITIAL_STATE } },
        ),
    };

    let handle_fn = match &binding {
        StateBinding::Machine { event, module, .. } => {
            let doc = format!(
                "Processes `event` through state machine `{}` and returns the updated component.",
                module
            );
            quote! {
                #[doc = #doc]
                ///
                /// # Errors
                /// If a hook fails, returns this component unchanged together with the error.
                pub fn handle<H: #module::Hooks>(
                    self,
                    event: #event,
                    hooks: &mut H,
                ) -> Result<(Self, #module::Outcome), (Self, #module::StepError<H::Error>)> {
                    match #module::step(self.state.current, event, hooks) {
                        Ok((current, outcome)) => Ok((
                            Self { state: #state { current }, ..self },
                            outcome,
                        )),
                        Err(error) => Err((self, error)),
                    }
                }
            }
        }
        _ => quote! {},
    };

    let config_doc = format!("Configuration for component `{}`.", comp.name);
    let state_doc = format!("State of component `{}`.", comp.name);
    let command_doc = format!("Commands accepted by component `{}`.", comp.name);
    let ports_doc = format!("Typed ports of component `{}`.", comp.name);
    let comp_doc = format!(
        "Component `{}` ({}), holding its configuration, state and ports.",
        comp.name, comp.kind
    );

    Ok(quote! {
        #[doc = #config_doc]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct #config {}

        impl ::middleware::ComponentConfig for #config {}

        #[doc = #state_doc]
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct #state {
            #state_fields
        }

        impl ::middleware::ComponentState for #state {}

        #[doc = #command_doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #command {
            #(#commands),*
        }

        impl #command {
            /// Number of commands.
            pub const COUNT: usize = #command_count;
            /// All commands in model order.
            pub const ALL: [Self; #command_count] = [#(Self::#commands),*];
        }

        #[doc = #ports_doc]
        #[derive(Debug, Default)]
        pub struct #ports_ty<#(#generics),*>
        where
            #(#bounds),*
        {
            #(#[doc = #port_docs] pub #fields: #generics),*
        }

        #traces
        #[doc = #comp_doc]
        #[derive(Debug)]
        pub struct #name<#(#generics),*>
        where
            #(#bounds),*
        {
            config: #config,
            state: #state,
            ports: #ports_ty<#(#generics),*>,
        }

        impl<#(#generics),*> #name<#(#generics),*>
        where
            #(#bounds),*
        {
            /// `true` for an active component, `false` for a passive one.
            pub const IS_ACTIVE: bool = #is_active;
            /// Number of ports.
            pub const PORT_COUNT: usize = #port_count;
            /// Port names, inputs first, in model order.
            pub const PORT_NAMES: [&'static str; #port_count] = [#(#port_names),*];
            /// Port directions, aligned with `PORT_NAMES`.
            pub const PORT_DIRECTIONS: [::middleware::PortDirection; #port_count] = [#(#port_dirs),*];
            /// Port message type names, aligned with `PORT_NAMES`.
            pub const PORT_MESSAGE_TYPES: [&'static str; #port_count] = [#(#port_types),*];
            /// Requirements this component traces to.
            pub const TRACES: [&'static str; #trace_count] = [#(#trace_ids),*];

            /// Builds the component in its initial state with the given ports.
            pub fn with_ports(config: #config, ports: #ports_ty<#(#generics),*>) -> Self {
                Self { config, state: #initial_state, ports }
            }

            /// Returns the component's ports.
            pub fn ports(&self) -> &#ports_ty<#(#generics),*> {
                &self.ports
            }

            #handle_fn
        }

        impl<#(#generics),*> ::middleware::Component for #name<#(#generics),*>
        where
            #(#bounds,)*
            #(#default_bounds),*
        {
            type Config = #config;
            type State = #state;
            type Ports = #ports_ty<#(#generics),*>;

            const NAME: &'static str = #name_str;
            const ID: ::middleware::ComponentId = #id;

            fn new(config: Self::Config) -> Result<Self, ::middleware::InitError> {
                Ok(Self::with_ports(config, ::core::default::Default::default()))
            }

            fn config(&self) -> &Self::Config {
                &self.config
            }

            fn state(&self) -> &Self::State {
                &self.state
            }
        }
    })
}

/// Generates code for every component in the model.
///
/// # Errors
/// Returns an error if a component references an undefined port or state
/// machine, lists a port under the wrong direction or twice, declares a
/// state that conflicts with its state machine or cannot be initialised,
/// or has an invalid or duplicated command name.
pub fn generate_components(model: &Model) -> Result<TokenStream, String> {
    let tokens = model
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| generate_component(model, c, i))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#tokens)* })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_model;

    const BASE: &str = r#"
version: "1.0"
namespace: "t"
types:
  enums:
    - name: S
      repr: u8
      variants: [A, B]
    - name: E
      repr: u8
      variants: [Go]
  structs:
    - name: Nested
      fields:
        - name: inner
          type: S
messages:
  - name: Msg
    fields:
      - name: value
        type: f32
ports:
  - name: input_a
    direction: input
    message_type: Msg
  - name: output_a
    direction: output
    message_type: Msg
state_machines:
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
    transitions: []
"#;

    fn model_with(components: &str) -> Model {
        parse_model(&format!("{}components:\n{}", BASE, components)).unwrap()
    }

    #[test]
    fn undefined_port_is_rejected() {
        let model = model_with("  - name: C\n    kind: passive\n    input_ports: [missing]\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("undefined port 'missing'"), "{}", err);
    }

    #[test]
    fn port_direction_mismatch_is_rejected() {
        let model = model_with("  - name: C\n    kind: passive\n    input_ports: [output_a]\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("declared output"), "{}", err);
    }

    #[test]
    fn state_conflicting_with_machine_is_rejected() {
        let model = model_with("  - name: C\n    kind: active\n    state: E\n    state_machine: M\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("differs from state machine"), "{}", err);
    }

    #[test]
    fn undefined_state_machine_is_rejected() {
        let model = model_with("  - name: C\n    kind: active\n    state_machine: Nope\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("undefined state machine"), "{}", err);
    }

    #[test]
    fn struct_state_without_machine_is_rejected() {
        let model = model_with("  - name: C\n    kind: passive\n    state: Nested\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("must be a model enum"), "{}", err);
    }

    #[test]
    fn duplicate_command_is_rejected() {
        let model = model_with("  - name: C\n    kind: passive\n    commands: [arm, Arm]\n");
        let err = generate_components(&model).unwrap_err();
        assert!(err.contains("duplicated"), "{}", err);
    }

    #[test]
    fn non_primitive_port_field_is_rejected() {
        let mut model = model_with("  - name: C\n    kind: passive\n");
        model.ports[0].message_type = "Nested".to_string();
        let err = generate_port_messages(&model).unwrap_err();
        assert!(err.contains("field 'inner'"), "{}", err);
    }

    #[test]
    fn unknown_port_message_type_is_rejected() {
        let mut model = model_with("  - name: C\n    kind: passive\n");
        model.ports[0].message_type = "Unknown".to_string();
        let err = generate_port_messages(&model).unwrap_err();
        assert!(err.contains("not a model message or struct"), "{}", err);
    }

    #[test]
    fn invalid_port_direction_is_rejected() {
        let mut model = model_with("  - name: C\n    kind: passive\n");
        model.ports[0].direction = "sideways".to_string();
        let err = generate_port_messages(&model).unwrap_err();
        assert!(err.contains("must be 'input' or 'output'"), "{}", err);
    }

    #[test]
    fn wire_widths() {
        assert_eq!(WireType::of("f64").map(WireType::width), Some(8));
        assert_eq!(WireType::of("u16").map(WireType::width), Some(2));
        assert_eq!(WireType::of("bool").map(WireType::width), Some(1));
        assert_eq!(WireType::of("String"), None);
    }
}
