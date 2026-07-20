# Protos source-surface decision proposal

## Status

This source-surface decision is installed on the integration branch. It does not
author Spirit or access production data. The executable
`examples/source_surface_candidates.rs` generated
`SOURCE_SURFACE_CANDIDATES.md`; each printed block was encoded, recognized,
decoded, and encoded again through one sealed `SourceSurfaceCandidates`
StructureTree. The real `TextualSchema` document StructureTree now carries the
same accepted unit-or-one-payload interface alternatives and closed streaming
relation in its seventh trailing slot; `tests/document_roundtrip.rs` proves the
installed round trip.

This work does not add a Nomos macro spelling or a general schema-authored Rust
implementation language.

## Codec-emitted candidates

The following blocks are copied from the generated artifact, not hand-authored.

### Ordered interface alternatives

```
[Closed Opened.SubscriptionToken]
```

The outer expected type is the interface-alternative vector. Each child is read
under the expected `InterfaceVariant` type. Its first constructor is a PascalCase
atom and means a unit alternative; its second is a glued-dot application whose
head is the alternative identifier and whose payload is read under the expected
reference type. The two constructors are structurally disjoint by raw shape.

The encoded target is the existing `EncodedVariant { identifier, payload:
Option<EncodedReference> }` algebra. No field label enters the source, encoded
value, or transform. Declaration order remains the wire discriminant order.

### Closed streaming relations

```
[{OpenSubscription SubscriptionOpened SubscriptionToken IntentEvent CloseSubscription}]
```

The outer expected type is a vector of relations. Each braced relation has five
fixed positional slots, in order: input opener, output acknowledgement, token
reference, event reference, and close-token reference. The first two slots have
distinct expected reference types even though both encode as PascalCase atoms;
the last three are each read under the ordinary typed-reference expectation.
Position, not an authored label, assigns every meaning.

The encoded target is the accepted `StreamingRelation` record. It is reusable
and contains only encoded identifiers and typed references. A signal projection
can generate the current streaming frame topology from it; no Spirit-only
construct is introduced.

## Boundary evidence and consequences

- **One bidirectional StructureTree:** the same sealed table emits and decodes
  both candidate blocks. There is no parser/printer pair or handwritten textual
  special case.
- **NameTable ownership:** candidate decode interns names into the schema
  NameTable; encode resolves them from that same table. A Logos consumer borrows
  this completed schema slice and adds only its own names in the Logos slice.
- **Nomos remains typed:** `StreamingRelation` and `EncodedVariant` carry
  identifiers and references only. The candidate codec's temporary source strings
  terminate at the TextualForm and NameTable boundary; they do not enter Nomos.
- **Compatibility:** the interface candidate maps to the already accepted
  optional-payload encoded algebra, so it changes the schema StructureTree/source
  revision but not that algebra. Streaming already changes the `EncodedSchema`
  archive layout and therefore needs its deliberate layout/version policy. Neither
  candidate claims old/new frame compatibility; that needs the later signal
  projection and bidirectional daemon tests.

## Canonical names

Each encoded identifier has one canonical NameTable name. Schema has no
transparent alias declaration or alias-admission source form, and Rust projection
emits no transparent type alias. A reference always resolves to its one canonical
identifier name.

This does not govern domain values that happen to be called aliases. Those remain
ordinary encoded structure where their domain requires them; they are not
NameTable names or language aliases.

## Manifest imports and actor adapters

`SchemaManifest` already provides the accepted file-path dependency graph at the
TextualForm boundary. A path is a manifest index key, not an encoded identifier and
not a Nomos string. Manifest dependency ordering and its shared NameTable make an
imported declaration's existing encoded identifier available to a dependent file.
No in-document `imports` slot or new source spelling is needed.

The accepted shared-runtime boundary dissolves the prior general implementation-block
request. Schema authors typed contract, record, mail, interface, relation, and
manifest data. The shared runtime owns generic actor mechanics; bounded concrete
adapters are a reusable Rust projection from typed mail and relations. Therefore no
schema-authored general trait, associated-item, method-body, or implementation-block
surface is proposed here. Imports needed by a concrete projection are generated from
the typed component dependency graph, not authored as arbitrary source syntax.

## Accepted decisions and remaining gates

1. **Interface alternative:** installed as the codec-emitted unit-or-one-payload
   form in the ordinary interface positions, using the existing optional-payload
   encoded algebra and one bidirectional StructureTree.
2. **Streaming placement:** installed as the trailing seventh positional document
   slot. Its expected type distinguishes the relation vector from declarations;
   no section label or Spirit-specific construct was added.
3. **Canonical names:** every encoded identifier has one NameTable name. No
   transparent source, NameTable, or Rust alias mechanism is present.
4. **Imports:** manifest dependency edges remain the only import mechanism; the old
   in-document imports slot stays empty.
5. **Adapter scope:** generic actor machinery remains in shared runtime; no general
   schema-authored implementation-block surface is added.

- The installed streaming relation still needs signal-frame generation and old/new
  daemon compatibility evidence.
- No macro-definition spelling is described or changed.
