# core-schema

`core-schema` owns the stringless **Core** schema layer and its bidirectional
`TextualSchema` view. The public API remains `Core*` throughout this S1 slicing
step; the immediately following rename train will rename that API separately.

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
- `CoreSchema` keeps ordered interface alternatives and carries closed
  `StreamingRelation` data without source-spelling or alias surfaces.

The universe bridge continues to derive positional constructor signatures from
Core layouts and validates authored structural tables against those signatures.
Names remain outside Core content identity, so a name-table change cannot alter a
Core value's content hash.

## Dependency pin

All Protos machinery crates resolve at immutable pushed revision
`290f2a1c5a9ae2bb2769d7dcd1722c056b85a5d4`. Cargo.lock records the same revision;
the Nix build consumes that lockfile.

## Build and test

```sh
nix flake check --no-link --print-build-logs
cargo test
```
