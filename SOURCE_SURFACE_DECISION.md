# Protos source-surface decision proposal

## Status

This is a proposal-branch artifact. It does not change `main`, does not author
Spirit, and does not access production data. The executable
`examples/source_surface_candidates.rs` generated
`SOURCE_SURFACE_CANDIDATES.md`; each printed block was encoded, recognized,
decoded, and encoded again through the one sealed `SourceSurfaceCandidates`
StructureTree. The accompanying integration tests prove the same round trips.

The candidate grammar deliberately covers only the accepted Spirit redesign:
unit-or-one-payload interface alternatives and the closed streaming relation.
It does not add a Nomos macro spelling or a general schema-authored Rust
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

## NameTree-only transparent aliases

No alias source form is proposed or emitted.

The accepted semantic route exists in the producer foundation: `NameTable` records
an additional spelling for an existing encoded identifier; decode resolves that
spelling to the original identifier; a Rust emitter can render the corresponding
transparent target-language alias. No encoded alias declaration is added.

The current generic codec gives a concrete reason not to fabricate a source form:
`StructuralEvaluator` resolves every `Atom` through `NameInterner::intern` before
language reification. An alias declaration head decoded as an atom would therefore
mint a new identifier, which contradicts a transparent alias's required identity
and cannot be repaired by `NameTable::add_alias` without a special bypass.

The minimal normal-form follow-up is a NameTable-boundary alias-admission codec:
it must pass the declared alias spelling to `add_alias` without allocating an
identifier, then prove source decode, NameTable alias lookup, and Rust emission
against one target. That is a new boundary capability, not evidence for adding an
encoded alias branch. No encoded alias branch is recommended.

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

## Decision slate

1. **Interface candidate — accept?** Adopt the codec-emitted unit-or-one-payload
   interface-alternative form above in the existing interface positions. It uses the
   existing optional-payload algebra and one StructureTree. **Recommendation: accept.**
2. **Streaming placement — accept?** Add one trailing, seventh positional document
   slot whose expected type is the codec-emitted streaming-relation vector above.
   It has no section label; the document type and position distinguish it from all
   ordinary declarations. **Recommendation: accept.**
3. **Alias admission — accept?** Keep aliases NameTree-only and add a narrow
   NameTable-boundary alias-admission codec rather than an encoded alias branch.
   Do not author an alias source spelling until that capability emits and round-trips
   one artifact. **Recommendation: accept.**
4. **Imports — accept?** Retain manifest dependency edges as the only import
   mechanism; do not reopen the old document `imports` slot. **Recommendation: accept.**
5. **Adapter scope — accept?** Keep generic actor machinery in the shared runtime
   and omit a general schema-authored implementation-block surface. **Recommendation: accept.**

## Deferred gates

- NameTable alias admission needs its own codec-backed source and generated-Rust
  fidelity proof.
- The accepted streaming relation still needs signal-frame generation and old/new
  daemon compatibility evidence.
- The candidate seventh document slot is not installed in `TextualSchema` until its
  source-surface ruling is accepted.
- No macro-definition spelling is described or changed.
