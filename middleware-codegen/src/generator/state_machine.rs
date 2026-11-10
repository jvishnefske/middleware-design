//! State machine code generation.
//!
//! Each model state machine becomes a module named after it, containing:
//!
//! - `INITIAL_STATE` and `TRANSITION_COUNT` constants.
//! - `Guard` and `Action` enums: one variant per distinct guard name and per
//!   distinct action name (transition actions, `on_enter` and `on_exit`
//!   share the `Action` set). An FSM without any yields an empty enum.
//! - A `Transition` row struct and the compile-time `TRANSITIONS` table, each
//!   row carrying `Option<Guard>` and `Option<Action>`.
//! - `const fn find_transition`, `on_enter` and `on_exit` lookups. `on_enter`
//!   and `on_exit` are exhaustive matches over every variant of the state
//!   enum, so they cannot panic.
//! - A `Hooks` trait with an associated `Error` type, one
//!   `fn <guard>(&self) -> bool` per guard and one
//!   `fn <action>(&mut self) -> Result<(), Self::Error>` per action. The
//!   user implements it to supply side effects.
//! - `Outcome` (`Transitioned { from, to }`, `Remained`, `Ignored`) and
//!   `StepError<E>` (`ExitFailed`, `ActionFailed`, `EntryFailed`), mirroring
//!   the runtime `TransitionResult` and `FsmError` so generated machines need
//!   no runtime crate.
//! - `fn step(current, event, &mut hooks) -> Result<(State, Outcome), StepError<_>>`.
//!
//! `step` scans `TRANSITIONS` once, in order. The first row matching the
//! current state and event whose guard is absent or returns `true` fires:
//! the source state's `on_exit`, then the row's action, then the target
//! state's `on_enter`. If rows matched but every guard was false the result
//! is `Remained`; if no row matched it is `Ignored`. In both cases no action
//! runs and the state is returned unchanged. On a hook error, `step` returns
//! the error and the caller keeps its previous state. The scan is bounded by
//! `TRANSITION_COUNT`; `step` is pure apart from the calls into `Hooks`,
//! which is the imperative shell.
//!
//! # Traceability
//! - REQ-GEN-002: State machine structure
//! - REQ-FSM-002: on_enter and on_exit actions
//! - REQ-FSM-011: Optional guard conditions
//! - REQ-FSM-012: Optional transition actions
//! - REQ-FSM-013: Compile-time constant transition tables

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::naming::{pascal_ident, snake_ident};
use super::traces_doc;
use crate::model::{EnumDef, StateDef, StateMachine};

/// Generates code for every state machine in the model.
///
/// # Errors
/// Returns the first error from [`generate_state_machine`].
pub fn generate_state_machines(
    machines: &[StateMachine],
    enums: &[EnumDef],
) -> Result<TokenStream, String> {
    let machine_tokens = machines
        .iter()
        .map(|fsm| generate_state_machine(fsm, enums))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#machine_tokens)* })
}

/// Names of the hooks a state machine needs, in first-use order.
#[derive(Debug, Default, PartialEq, Eq)]
struct HookNames {
    /// Distinct guard names.
    guards: Vec<String>,
    /// Distinct action names (transition, on_enter and on_exit).
    actions: Vec<String>,
}

/// Appends `name` to `names` unless already present, returning the new list.
fn with_unique(names: Vec<String>, name: &str) -> Vec<String> {
    if names.iter().any(|n| n == name) {
        names
    } else {
        names.into_iter().chain(core::iter::once(name.to_string())).collect()
    }
}

/// Collects guard and action names from a state machine definition.
///
/// # Errors
/// Returns an error if a name is used both as a guard and as an action,
/// since the two have different hook signatures.
fn collect_hook_names(fsm: &StateMachine) -> Result<HookNames, String> {
    let guards = fsm
        .transitions
        .iter()
        .filter_map(|t| t.guard.as_deref())
        .fold(Vec::new(), with_unique);
    let actions = fsm
        .states
        .iter()
        .flat_map(|s| [s.on_enter.as_deref(), s.on_exit.as_deref()])
        .chain(fsm.transitions.iter().map(|t| t.action.as_deref()))
        .flatten()
        .fold(Vec::new(), with_unique);
    match guards.iter().find(|g| actions.contains(g)) {
        Some(clash) => Err(format!(
            "State machine '{}': '{}' is used as both a guard and an action",
            fsm.name, clash
        )),
        None => Ok(HookNames { guards, actions }),
    }
}

/// Looks up an enum definition by name.
fn find_enum<'a>(enums: &'a [EnumDef], name: &str, fsm: &str, role: &str) -> Result<&'a EnumDef, String> {
    enums
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("State machine '{}': {} type '{}' is not a model enum", fsm, role, name))
}

/// Checks that `variant` names a variant of `enum_def`.
fn require_variant(enum_def: &EnumDef, variant: &str, fsm: &str) -> Result<(), String> {
    if enum_def.variants.iter().any(|v| v == variant) {
        Ok(())
    } else {
        Err(format!(
            "State machine '{}': '{}' is not a variant of '{}'",
            fsm, variant, enum_def.name
        ))
    }
}

/// Validates that every state and transition refers to existing enum variants
/// and that no state is defined twice.
fn validate_references(fsm: &StateMachine, states: &EnumDef, events: &EnumDef) -> Result<(), String> {
    fsm.states
        .iter()
        .try_for_each(|s| require_variant(states, &s.name, &fsm.name))?;
    require_variant(states, &fsm.initial_state, &fsm.name)?;
    fsm.transitions.iter().try_for_each(|t| {
        require_variant(states, &t.from, &fsm.name)?;
        require_variant(states, &t.to, &fsm.name)?;
        require_variant(events, &t.event, &fsm.name)
    })?;
    match fsm
        .states
        .iter()
        .enumerate()
        .find(|(i, s)| fsm.states[..*i].iter().any(|p| p.name == s.name))
    {
        Some((_, dup)) => Err(format!("State machine '{}': state '{}' defined twice", fsm.name, dup.name)),
        None => Ok(()),
    }
}

/// Builds the `Option<Action>` expression for an optional action name.
fn action_option(name: Option<&str>) -> Result<TokenStream, String> {
    match name {
        Some(a) => {
            let variant = pascal_ident(a, "action")?;
            Ok(quote! { Some(Action::#variant) })
        }
        None => Ok(quote! { None }),
    }
}

/// Builds an exhaustive `const fn` mapping every state variant to its
/// optional enter or exit action.
fn lifecycle_fn(
    fn_name: &Ident,
    doc: &str,
    state_type: &Ident,
    variants: &[String],
    states: &[StateDef],
    pick: fn(&StateDef) -> Option<&str>,
) -> Result<TokenStream, String> {
    let arms = variants
        .iter()
        .map(|v| {
            let variant = format_ident!("{}", v);
            let action = action_option(states.iter().find(|s| &s.name == v).and_then(pick))?;
            Ok(quote! { #state_type::#variant => #action })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(quote! {
        #[doc = #doc]
        pub const fn #fn_name(state: #state_type) -> Option<Action> {
            match state {
                #(#arms),*
            }
        }
    })
}

/// Generates the module for a single state machine.
///
/// # Errors
/// Returns an error if the state or event type is not a model enum, a state,
/// event or initial state is not a variant of its enum, a state is defined
/// twice, a hook name is not snake_case, or a name is used both as a guard
/// and as an action.
pub fn generate_state_machine(fsm: &StateMachine, enums: &[EnumDef]) -> Result<TokenStream, String> {
    let state_enum = find_enum(enums, &fsm.state_type, &fsm.name, "state")?;
    let event_enum = find_enum(enums, &fsm.events, &fsm.name, "event")?;
    validate_references(fsm, state_enum, event_enum)?;
    let hooks = collect_hook_names(fsm)?;

    let name = format_ident!("{}", fsm.name);
    let state_type = format_ident!("{}", fsm.state_type);
    let event_type = format_ident!("{}", fsm.events);
    let initial_state = format_ident!("{}", fsm.initial_state);
    let traces = traces_doc(&fsm.traces);
    let module_doc = format!("State machine `{}` generated from the model.", fsm.name);

    let guard_variants = hooks
        .guards
        .iter()
        .map(|g| pascal_ident(g, "guard"))
        .collect::<Result<Vec<_>, _>>()?;
    let guard_methods = hooks
        .guards
        .iter()
        .map(|g| snake_ident(g, "guard"))
        .collect::<Result<Vec<_>, _>>()?;
    let action_variants = hooks
        .actions
        .iter()
        .map(|a| pascal_ident(a, "action"))
        .collect::<Result<Vec<_>, _>>()?;
    let action_methods = hooks
        .actions
        .iter()
        .map(|a| snake_ident(a, "action"))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((i, v)) = guard_variants
        .iter()
        .enumerate()
        .find(|(i, v)| guard_variants[..*i].contains(v))
    {
        return Err(format!(
            "State machine '{}': guard '{}' collides with another guard as '{}'",
            fsm.name, hooks.guards[i], v
        ));
    }
    if let Some((i, v)) = action_variants
        .iter()
        .enumerate()
        .find(|(i, v)| action_variants[..*i].contains(v))
    {
        return Err(format!(
            "State machine '{}': action '{}' collides with another action as '{}'",
            fsm.name, hooks.actions[i], v
        ));
    }

    let transition_count = fsm.transitions.len();
    let rows = fsm
        .transitions
        .iter()
        .map(|t| {
            let from = format_ident!("{}", t.from);
            let event = format_ident!("{}", t.event);
            let to = format_ident!("{}", t.to);
            let guard = match t.guard.as_deref() {
                Some(g) => {
                    let variant = pascal_ident(g, "guard")?;
                    quote! { Some(Guard::#variant) }
                }
                None => quote! { None },
            };
            let action = action_option(t.action.as_deref())?;
            Ok(quote! {
                Transition {
                    from: #state_type::#from,
                    event: #event_type::#event,
                    to: #state_type::#to,
                    guard: #guard,
                    action: #action,
                }
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let on_enter_fn = lifecycle_fn(
        &format_ident!("on_enter"),
        "Action run when `state` is entered, if any.",
        &state_type,
        &state_enum.variants,
        &fsm.states,
        |s| s.on_enter.as_deref(),
    )?;
    let on_exit_fn = lifecycle_fn(
        &format_ident!("on_exit"),
        "Action run when `state` is exited, if any.",
        &state_type,
        &state_enum.variants,
        &fsm.states,
        |s| s.on_exit.as_deref(),
    )?;

    let guard_docs: Vec<String> = hooks
        .guards
        .iter()
        .map(|g| format!("Guard `{}`: returns `true` if the transition may fire.", g))
        .collect();
    let action_docs: Vec<String> = hooks
        .actions
        .iter()
        .map(|a| format!("Action `{}`.", a))
        .collect();

    // An empty enum has nothing to dispatch on, so its match has no arms and
    // the hooks argument goes unused.
    let check_guard_fn = if guard_variants.is_empty() {
        quote! {
            fn check_guard<H: Hooks>(guard: Guard, _hooks: &H) -> bool {
                match guard {}
            }
        }
    } else {
        quote! {
            fn check_guard<H: Hooks>(guard: Guard, hooks: &H) -> bool {
                match guard {
                    #(Guard::#guard_variants => hooks.#guard_methods()),*
                }
            }
        }
    };
    let run_action_fn = if action_variants.is_empty() {
        quote! {
            fn run_action<H: Hooks>(action: Action, _hooks: &mut H) -> Result<(), H::Error> {
                match action {}
            }
        }
    } else {
        quote! {
            fn run_action<H: Hooks>(action: Action, hooks: &mut H) -> Result<(), H::Error> {
                match action {
                    #(Action::#action_variants => hooks.#action_methods()),*
                }
            }
        }
    };

    Ok(quote! {
        #traces
        #[doc = #module_doc]
        pub mod #name {
            use super::*;

            /// Initial state.
            pub const INITIAL_STATE: #state_type = #state_type::#initial_state;

            /// Number of transitions.
            pub const TRANSITION_COUNT: usize = #transition_count;

            /// Named guard conditions used by this state machine.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum Guard {
                #(#[doc = #guard_docs] #guard_variants),*
            }

            /// Named actions used by this state machine (transition, enter and exit).
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum Action {
                #(#[doc = #action_docs] #action_variants),*
            }

            /// One row of the transition table.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct Transition {
                /// Source state.
                pub from: #state_type,
                /// Triggering event.
                pub event: #event_type,
                /// Target state.
                pub to: #state_type,
                /// Optional guard; the row fires only if it returns `true`.
                pub guard: Option<Guard>,
                /// Optional action run between exit and enter.
                pub action: Option<Action>,
            }

            impl Transition {
                /// Returns `true` if this row starts at `current` and is triggered by `event`.
                pub const fn matches(&self, current: #state_type, event: #event_type) -> bool {
                    self.from as usize == current as usize && self.event as usize == event as usize
                }
            }

            /// Transition table, evaluated in order.
            pub const TRANSITIONS: [Transition; #transition_count] = [
                #(#rows),*
            ];

            /// Returns the first row matching `current` and `event`, ignoring guards.
            pub const fn find_transition(current: #state_type, event: #event_type) -> Option<Transition> {
                let mut i = 0;
                while i < TRANSITION_COUNT {
                    if TRANSITIONS[i].matches(current, event) {
                        return Some(TRANSITIONS[i]);
                    }
                    i += 1;
                }
                None
            }

            #on_enter_fn

            #on_exit_fn

            /// Side effects and conditions supplied by the application.
            ///
            /// Guards take `&self` and must not have side effects. Actions take
            /// `&mut self` and report failure through `Self::Error`.
            pub trait Hooks {
                /// Error returned by a failing action.
                type Error;
                #(#[doc = #guard_docs] fn #guard_methods(&self) -> bool;)*
                #(#[doc = #action_docs] fn #action_methods(&mut self) -> Result<(), Self::Error>;)*
            }

            /// Result of processing one event.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum Outcome {
                /// A transition fired.
                Transitioned {
                    /// State before the transition.
                    from: #state_type,
                    /// State after the transition.
                    to: #state_type,
                },
                /// Matching rows exist but every guard returned `false`.
                Remained,
                /// No row matches the current state and event.
                Ignored,
            }

            /// An action failed while a transition was firing.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum StepError<E> {
                /// The source state's `on_exit` action failed.
                ExitFailed(E),
                /// The transition action failed.
                ActionFailed(E),
                /// The target state's `on_enter` action failed.
                EntryFailed(E),
            }

            #check_guard_fn

            #run_action_fn

            /// Fires `row` from `current`: exit, transition action, enter.
            fn fire<H: Hooks>(
                row: &Transition,
                current: #state_type,
                hooks: &mut H,
            ) -> Result<(#state_type, Outcome), StepError<H::Error>> {
                if let Some(action) = on_exit(current) {
                    run_action(action, hooks).map_err(StepError::ExitFailed)?;
                }
                if let Some(action) = row.action {
                    run_action(action, hooks).map_err(StepError::ActionFailed)?;
                }
                if let Some(action) = on_enter(row.to) {
                    run_action(action, hooks).map_err(StepError::EntryFailed)?;
                }
                Ok((row.to, Outcome::Transitioned { from: current, to: row.to }))
            }

            /// Processes `event` in state `current` and returns the next state.
            ///
            /// The first matching row whose guard passes fires (exit, action,
            /// enter). Returns `Remained` if rows matched but no guard passed and
            /// `Ignored` if none matched; the state is then unchanged.
            ///
            /// # Errors
            /// Returns a [`StepError`] if an exit, transition or enter action fails.
            pub fn step<H: Hooks>(
                current: #state_type,
                event: #event_type,
                hooks: &mut H,
            ) -> Result<(#state_type, Outcome), StepError<H::Error>> {
                let mut matched = false;
                for row in TRANSITIONS.iter() {
                    if row.matches(current, event) {
                        matched = true;
                        let allowed = match row.guard {
                            Some(guard) => check_guard(guard, hooks),
                            None => true,
                        };
                        if allowed {
                            return fire(row, current, hooks);
                        }
                    }
                }
                let outcome = if matched { Outcome::Remained } else { Outcome::Ignored };
                Ok((current, outcome))
            }
        }
    })
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
"#;

    fn with_fsm(fsm: &str) -> String {
        format!("{}state_machines:\n{}", BASE, fsm)
    }

    #[test]
    fn hook_names_are_collected_once_in_order() {
        let yaml = with_fsm(
            r#"
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
        on_exit: leave
      - name: B
        on_enter: arrive
    transitions:
      - from: A
        event: Go
        to: B
        guard: ready
        action: leave
"#,
        );
        let model = parse_model(&yaml).unwrap();
        let names = collect_hook_names(&model.state_machines[0]).unwrap();
        assert_eq!(names.guards, vec!["ready".to_string()]);
        assert_eq!(names.actions, vec!["leave".to_string(), "arrive".to_string()]);
    }

    #[test]
    fn guard_and_action_with_same_name_is_rejected() {
        let yaml = with_fsm(
            r#"
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
        on_exit: same
    transitions:
      - from: A
        event: Go
        to: B
        guard: same
"#,
        );
        let model = parse_model(&yaml).unwrap();
        let err = generate_state_machine(&model.state_machines[0], &model.types.enums).unwrap_err();
        assert!(err.contains("both a guard and an action"), "{}", err);
    }

    #[test]
    fn unknown_variant_is_rejected() {
        let yaml = with_fsm(
            r#"
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
    transitions:
      - from: A
        event: Go
        to: Missing
"#,
        );
        let model = parse_model(&yaml).unwrap();
        let err = generate_state_machine(&model.state_machines[0], &model.types.enums).unwrap_err();
        assert!(err.contains("'Missing' is not a variant"), "{}", err);
    }

    #[test]
    fn unknown_state_type_is_rejected() {
        let yaml = with_fsm(
            r#"
  - name: M
    state_type: Nope
    initial_state: A
    events: E
    states:
      - name: A
    transitions: []
"#,
        );
        let model = parse_model(&yaml).unwrap();
        let err = generate_state_machine(&model.state_machines[0], &model.types.enums).unwrap_err();
        assert!(err.contains("not a model enum"), "{}", err);
    }

    #[test]
    fn non_snake_case_hook_is_rejected() {
        let yaml = with_fsm(
            r#"
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
        on_enter: BadName
    transitions: []
"#,
        );
        let model = parse_model(&yaml).unwrap();
        let err = generate_state_machine(&model.state_machines[0], &model.types.enums).unwrap_err();
        assert!(err.contains("must be snake_case"), "{}", err);
    }
}
