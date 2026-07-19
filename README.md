# core-schema

The first **real** stringless Core schema layer of the next-generation NOTA
language family, and the first **real** Textual form, `TextualSchema`.

Slice one of the language-family proof-of-concept delivered four foundation
crates — [`content-identity`], [`name-table`], [`raw-discovery`],
[`structural-codec`] — but its universe was a *synthetic* fixture: ids that keyed
no real Core layout. This crate makes the Core layer real, and connects it to the
structural-form kernel through a universe bridge that closes structural-codec's
one deferred deviation.

## What it delivers

- **Stringless `EncodedSchema` value types.** `EncodedType { Newtype | Struct |
  Enumeration }`, modelled on `schema-language`'s proven `EncodedType`. Every name is
  an `Identifier` into a `NameTable`; type references dispatch **by kind and
  projection** (`EncodedReference`: scalar leaves, `Plain`, and the
  Vector/Optional/ScopeOf/Map/Bytes projections), never by a head string. Content
  identity is blake3 over the stringless rkyv bytes with the NameTable excluded, so
  **a rename is hash-stable by construction** (proven in `tests/identity.rs`).

- **The universe bridge** (`EncodedUniverse`). A set of `EncodedSchema` declarations
  forms a `structural-codec` Core universe: one `ScopedEncodedTypeId` per type, one
  `EncodedConstructorId` per constructor, and each constructor's `PositionalSignature`
  **derived from the Core layout** — the ordered ids of its fields' referenced
  types. `EncodedUniverse::validate_table` proves every authored codec signature in a
  structural table equals the Core field signature, and a mismatched table fails
  loudly. This closes structural-codec's deferred *signature-vs-Core validation*
  (previously "no Core layout in the PoC").

- **`TextualSchema`, the first real Textual form.** Real schema TEXT
  (`CommitSequence.{ Integer }`, `DatabaseMarker.{ CommitSequence StateDigest
  secretDigest.StateDigest }`) decodes — through raw-discovery and the trusted
  evaluator — into real `EncodedSchema` values with a real `NameTable`, and encodes
  back to identical canonical text. The `Field` disjoint alternatives (an elided
  name derived from the type versus an explicit `name.Type`) work against the real
  Core layout, with the derived-name rule (`name-table`) deciding elision.

- **The four conformance laws re-proven over the real Core** (`tests/laws.rs`):
  round-trip-core, round-trip-canonical, interning atomicity, and identity
  preservation across two textual revisions — now over a table whose signatures are
  validated against the Core layout, not a synthetic fixture — plus the
  rename-stability test from the identity ruling.

## Dependencies

Consumed across repositories by **pinned git rev** (the green path while the
release train is assembled), exactly as slice one's crates consume each other:

| crate | rev |
| --- | --- |
| `content-identity` | `6cc0408cdb96f174cc8fdf6ca23420038de28450` |
| `name-table` | `c3237f77c087e6feab49d6cf34971cebc14a11e6` |
| `raw-discovery` | `a4e8c6df84e6a487ca6fe2f3641f9bafd0b0d8c8` |
| `structural-codec` | `104f92454a5ba88b376fa706a9fe38c4a4b65ee0` |

## Build & test

`nix flake check` is the gate (build, test, clippy, fmt, doc). `cargo test` is the
inner loop.

## Relationship to the existing stack

Greenfield by design — see `ARCHITECTURE.md`. This crate models the proven Core
shapes of `schema-language`/`schema`/`schema-rust` in the new stringless
discipline; it does **not** edit them. Convergence with those repositories happens
later on the release train.

[`content-identity`]: https://github.com/LiGoldragon/content-identity
[`name-table`]: https://github.com/LiGoldragon/name-table
[`raw-discovery`]: https://github.com/LiGoldragon/raw-discovery
[`structural-codec`]: https://github.com/LiGoldragon/structural-codec
