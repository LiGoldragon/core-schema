# core-schema

`core-schema` owns the stringless **Encoded** schema layer and its bidirectional
`TextualSchema` view. Its public model surface uses `Encoded*` names with no legacy
aliases or compatibility bridges.

## S1 slicing

- `Identifier` is a closed namespace variant with a namespace-local `u16`
  allocation. `core-schema` owns `IdentifierNamespace::Schema`; it never
  reconstructs flat identifiers or converts between namespaces.
- A generic `NameTable` has the home namespace chosen by its owner.
  `core-schema`-owned tables have a Schema home slice; a consumer composes completed
  foreign slices with `NameTable::compose`, which borrows them without copying,
  flattening, renumbering, or legacy fallback behavior.
- The existing positional field-name ban remains: fields are bare type references
  and equal field types are distinguished only by their position.
- `EncodedSchema` keeps ordered interface alternatives and carries closed
  `StreamingRelation` data without source-spelling or alias surfaces.

The universe bridge continues to derive positional constructor signatures from
Encoded layouts and validates authored structural tables against those signatures.
Names remain outside Encoded content identity, so a name-table change cannot alter
an Encoded value's content hash.

## Dependency pin

All Protos machinery crates resolve at immutable pushed revision
`5eeb79f17559b7c395690304fa5b4a91cb36d45c`. Cargo.lock records the same revision;
the Nix build consumes that lockfile.

## Build and test

```sh
nix flake check --no-link --print-build-logs
cargo test
```
