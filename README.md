# core-schema

`core-schema` owns the stringless encoded schema data family and its bidirectional
`TextualSchema` view.

## What it delivers

- Encoded schema declarations carry namespace-variant `Identifier` values with a
  `u16` local allocation, never flat name indexes or embedded strings. Names live
  in composed NameTables and are excluded from encoded content identity.
- `CoreUniverse` derives positional constructor signatures from encoded layouts and
  validates the authored StructureTree against them.
- `TextualSchema` reads source through its StructureTree and emits canonical text
  through that same tree. Struct fields are bare expected types in positional slots;
  no field-label form exists.
- `CoreVariant` is one ordered alternative algebra with an optional single typed
  payload. Unit and payload variants preserve their declaration order and therefore
  their discriminant order.
- `StreamingRelation` is reusable encoded protocol data: opening input variant,
  acknowledgement output variant, token type, event type, and close-token type.
  A signal projection will generate the existing streaming frame topology from this
  relation; no Spirit-only path and no source spelling are introduced here.

## Layout and migration

Version 0.5.0 is a deliberate layout break. Layout 4 replaced the flat identifier
with a namespace enum carrying `u16` locals; layout 5 adds ordered streaming
relations to encoded schemas. Existing stored schema packages are regenerated with
their paired NameTable under the new producer-to-consumer train. They are not read
as sliced archives. The current layout's content identity is pinned by
`tests/content_hash_witness.rs`.

## Dependencies

All Protos machinery dependencies pin the same producer revision:
`c7510d35ae9126ea89c43aea51a11a0801a4f408`.

## Build and test

```sh
nix flake check
cargo test
```

`nix flake check` is the durable gate; `cargo test` is inner-loop evidence.
