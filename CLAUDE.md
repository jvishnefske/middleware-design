# Architectural Principles
MVP functional requirements are in design.md check off when requirements are validated and provide traceability.
If you think you need unsafe, you are doing something wrong.
All modules need rustdoc to match implementation.
The system's design philosophy mandates several core behaviors to manage complexity and enforce correctness. The architecture requires **immutable-by-default** data structures, modeling all state transitions as immutable transformations that return new instances or utilize the **Builder pattern** for complex construction. A "Functional Core, Imperative Shell" architecture is required to keep state-based logic separate from I/O operations and other side effects. Memory and ownership must be managed without reliance on complex object graphs, favoring a **linear or tree-like ownership structure** over mechanisms like `Arc` or `Mutex`. The design must leverage Rust's **type system** to enforce rules and invariants, for instance, by using custom types for units or `enum` variants for closed sets of states. Functions that can fail must return **`Result<T, E>`** to ensure the caller explicitly handles all possible outcomes.
do not put code in any markdown. code needs to be regression tested
Run multiple Task invocations with maximum parallelism in a SINGLE message.
do not use foo/mod.rs. foo.rs can just be foo.rs, and the submodule is still foo/bar.rs
