// The misspelled `gaurd` key must be rejected by the schema
// (`additionalProperties: false`), not silently ignored, which would turn a
// guarded transition into an unguarded one.
middleware_codegen::middleware_model!("../fixtures/schema_unknown_key.yaml");

fn main() {}
