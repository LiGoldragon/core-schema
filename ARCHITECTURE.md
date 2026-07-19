# Architecture — core-schema

This document states the durable direction of `core-schema`: what it is, why it is
greenfield, and the design of its one load-bearing piece, the universe bridge. It
is the pickup point for the next agent on the language-family train.

## Position in the language family

The next-generation NOTA family is four foundation crates with strictly downward
dependencies, so stringless Core never depends on text:

```
content-identity  <-  name-table  <-  raw-discovery  <-  structural-codec
```

Slice one delivered those four with a **synthetic** fixture universe: type ids that
keyed no real Core layout, so the structural table's positional signatures were
hand-authored with nothing to check them against (structural-codec named this its
one deferred deviation: "signature-vs-Core validation deferred — no Core layout in
the PoC").

`core-schema` is slice two: the first **real** stringless Core layer and the first
**real** Textual form. It depends on all four foundation crates by pinned git rev
and closes the deferred deviation.

## The stringless Core layer

`EncodedType { Newtype | Struct | Enumeration }` is modelled one-for-one on
`schema-language`'s proven `EncodedType` (`schema-language/src/core.rs`). The
faithful shapes carried over:

- Every name is an `Identifier` into a `NameTable`; the declarations carry no
  strings. Content identity (`EncodedSchemaDomain`, blake3 over stringless rkyv bytes
  via `content-identity`'s `ContentHash::of_core`) excludes the NameTable, so a
  rename is hash-stable by construction — a structural edit moves the hash, a
  rename does not.
- `EncodedReference` dispatches **by kind and projection, never a head string**: the
  scalar leaves, `Plain(Identifier)`, and the `SingleTypeReferenceProjection {
  Vector | Optional | ScopeOf }` / `MultiTypeReferenceProjection { Map }` /
  `ValueReferenceProjection { Bytes }` applications lifted verbatim from the ground
  truth. "Generics lower by kind" is thus real in the type, not a convention.

## The universe bridge (the crux)

`EncodedUniverse` turns a set of `EncodedSchema` declarations into a structural-codec Core
universe:

- **Id allocation.** One `ScopedEncodedTypeId` per Core type — the scalar-leaf
  primitives, the `Field` meta-type, and each user declaration — in an explicit
  fixture universe (the "unit of one schema" question stays parked with the psyche,
  `primary-56d1.11`). One `EncodedConstructorId` per constructor: a product (newtype,
  struct) has one; a sum (enumeration) one per variant.
- **Signature derivation.** `EncodedUniverse::core_signature` derives, from the Core
  layout alone, each constructor's `PositionalSignature`: the ordered universe-type
  ids of its fields' **referenced** types. A newtype yields `[inner]`; the
  `DatabaseMarker` struct yields `[CommitSequence, StateDigest, StateDigest]`; a
  variant with a payload yields `[payload]`, without yields `[]`.
- **Validation — the deferred deviation, closed.** `EncodedUniverse::validate_table`
  walks an authored `AddressedStructuralTable` and proves every `ConstructorCodec`
  signature equals the Core field signature (and that constructor counts match). A
  mismatch is the loud, typed `UniverseError::SignatureMismatch`. The authored table
  and the Core-layout derivation are **independent**: the table's signatures are
  hand-authored (as a table author writes them) and checked against the Core truth,
  so the agreement test is real, not a tautology — `tests/universe_bridge.rs` proves
  both the agreement and the loud rejection of a corrupted table.

The table's `core_layout_identity` is the schema's own `EncodedSchema` content hash,
tying each structural table to the exact stringless Core it targets while the table
identity itself stays **excluded** from Core value identity (law 4).

### Two construction modes: offline fixture vs authority-provided

`EncodedUniverse` is built two ways, and the distinction is load-bearing for the
identity keystone (`primary-56d1.11`, design v2):

- **Local / offline mode** — `EncodedUniverseBuilder` interns names in call order and
  the caller assigns type ids (the `fixture` family's hardcoded fixture ids). This is
  the self-contained path the existing tests use. It is a **lean**: because interning
  is parse-order, two ingestions of one declared schema that parse its declarations in
  different orders assign different name indices and declaration orders, so their Core
  values — hence content identities — diverge. That is exactly the "same thing,
  re-ID'ed" defect the keystone forbids.
- **Authority-provided mode** — `EncodedUniverse::from_assignment(universe, members)`
  takes a central-authority-minted universe id and a set of `AssignedMember`s (each a
  declared name, its authority-assigned local, and its kind). It registers members in
  ascending assigned-local order, interns names in that same canonical order, and
  re-stamps each declaration's own identifier to the canonically-interned one. The
  built universe — its id registry, name indices, declaration order, and declared
  schema's content identity — is therefore a **deterministic function of the
  assignment alone**, never of parse order (`tests/authority_assignment.rs`). This is
  the schema-side plumbing the sema-storage identity authority feeds: the authority
  (one logical seat per deployment, in sema — settled, not a lean) binds the same
  declared schema to the same identities across ingestions and processes, and this
  path turns those assignments into byte-stable Core.

  LEAN `authority-provided-universe`: `from_assignment` canonicalises the universe id,
  type ids, name interning order, and each declaration's own name identifier. It does
  **not** yet canonicalise field names or name-bearing (`Plain`) references inside
  declarations; a schema whose declarations cross-reference by name still needs those
  identifiers re-stamped for full cross-parse-order content-hash equivalence. That
  re-stamp — and the front-end wiring that computes an `AssignedMember` set from parsed
  schema text through a bind-or-mint call to the authority — is the **follow-up
  equivalence slice** (schema-engine / native ingestion), deliberately left out here.
  Revision trigger: that wiring landing.

### The Core/text granularity split

A struct's Core `signature` records its fields' **referenced types**
(`[CommitSequence, StateDigest, StateDigest]`) — the Core truth. Its structural
**form** is a product of `Delegate(Field)` slots — the text surface, where each
field is decoded through the `Field` meta-type's two disjoint constructors. Signature
(Core) and form (text) are deliberately decoupled at different granularities; this
is the Core-first split made concrete, and it is why the evaluator walks forms while
`validate_table` checks signatures against Core.

## TextualSchema — the first real Textual form

`TextualSchema` is one bidirectional codec over the universe. Decode: raw-discovery
recognizes text into a `Block`; structural-codec's trusted evaluator decodes it
(under the expected Core type) to a generic `StructuralValue`; `core-schema`
**reifies** that mirror into a real `EncodedType` with a real `NameTable`. Encode
**reflects** a `EncodedType` back into a `StructuralValue`, the evaluator renders it to
a `Block`, and it is written as canonical text. The `Field` elided-vs-explicit
alternatives are resolved against the real Core layout by name-table's derived-name
rule: a field name is elided in text exactly when it equals the `snake_case` of its
referenced type.

The reify/reflect pair is the hand-written stand-in for the future `nota-derive`
generated codec; the conformance harness in structural-codec (law 5) is where the
two will be proven equal in a later slice.

## Greenfield by design — the coordination boundary

`core-schema` does **not** edit `schema-language`, `schema`, `schema-rust`, `nota`,
`sema-engine`, or the four slice-one crates. Codex owns adapting the existing
`schema`-stack repositories on its release train; this crate models their proven
Core shapes in the new stringless discipline so convergence can happen later,
against a worked reference, rather than being invented during a live migration.

**Train status: currently NO-GO for riding the release train** (this session's
audit). Cross-repository consumption is by **pinned git rev** — the green path — not
by a materialized train. Convergence and the eventual swap to train-pinned or
path-unified dependencies readapt to the release-train flow when it is ready; until
then the git pins in `Cargo.toml` are authoritative.

## Flagged design forks (readings chosen, per the rulings)

1. **Struct field slots delegate to the `Field` meta-type** (form) while the struct
   **signature records referenced types** (Core). The alternative — inlining
   per-field forms and making the signature `[Field, Field, Field]` — loses the
   concrete referenced types from the signature. The chosen reading keeps the
   signature the most informative "Core field types, in order" and matches slice
   one's `Field` disjointness exercise. Flagged because both are defensible.
2. **`Field`'s constructor signatures are empty.** A field's payload is name
   identifiers (a type *name*, an optional field *name*), not typed sub-structures,
   and names are not types — so the positional **type** signature is empty. This
   both matches slice one's fixture and is now justified by the Core semantics.
3. **`Text` is a string-leaf primitive**, and the `Documentation -> Summary -> Text`
   chain is newtypes delegating to it; the terminal scalar leaf does the dotted-text
   rejoin. A future model could make `Text` a newtype over a distinct `String`
   primitive (one more delegate hop); the chosen reading matches the fixture's
   `Text`-as-leaf.
4. **Generic applications (Vector/Optional/Map/ScopeOf/Bytes-value) are modelled in
   `EncodedReference` but have no allocated universe type** in this PoC universe: the
   fixture family uses none, and `resolve_reference` returns a loud
   `UnsupportedApplication` rather than guessing. Allocating application types is the
   next universe-bridge extension.

## Upstream follow-ups for the manager

None blocking. structural-codec's public surface was sufficient for the universe
bridge: `AddressedStructuralTable::entry`, the public `ConstructorCodec.signature`
and `StructuralEntry.constructors` fields, `PositionalSignature::fields`, and the
`StructuralValue` mirror covered decode, encode, and validation without any fork.
One convenience note for a later slice: structural-codec could offer a first-class
"validate an entry's signature against an externally supplied Core signature" hook
so consumers need not read `constructors[i].signature` directly — minor, not a
blocker.
