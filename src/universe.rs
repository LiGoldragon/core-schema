//! The universe bridge: how a set of stringless `EncodedSchema` declarations forms a
//! `structural-codec` Core universe.
//!
//! A [`EncodedUniverse`] allocates one [`ScopedEncodedTypeId`] per Core type (the scalar
//! leaf primitives, the `Field` meta-type, and each user declaration) and, from the
//! Core layout alone, derives every constructor's [`PositionalSignature`] — the
//! ordered list of its fields' referenced types. That derivation is the single
//! source of truth against which an authored structural table is checked.
//!
//! This closes `structural-codec`'s deferred deviation ("signature-vs-Core
//! validation deferred — no Core layout in the PoC"): [`validate_table`] proves
//! every `ConstructorCodec` signature in a table equals the Core constructor's
//! field signature, and a mismatched table fails loudly with
//! [`UniverseError::SignatureMismatch`].
//!
//! [`validate_table`]: EncodedUniverse::validate_table

use std::collections::{BTreeMap, HashMap};

use name_table::{Identifier, IdentifierNamespace, Name, NameResolver, NameTable, NameTableError};
use structural_codec::ids::{
    EncodedUniverseId, FIXTURE_UNIVERSE, PositionalSignature, ScopedEncodedTypeId,
};
use structural_codec::table::AddressedStructuralTable;

use crate::declaration::{EncodedDeclaration, EncodedSchema, EncodedType};
use crate::error::UniverseError;
use crate::reference::EncodedReference;

/// What a universe type is, for the purpose of deriving its Core constructor
/// signatures. A closed typed record: the constructor arity and each signature
/// follow from the kind, never from a flag.
#[derive(Clone, Debug)]
pub enum MemberKind {
    /// A scalar leaf primitive (`Integer`, `Text`, `Boolean`, `Bytes`). One
    /// terminal constructor whose field signature is empty.
    Primitive,
    /// The `Field` meta-type: ONE positional constructor, the bare type reference.
    /// Field names are illegal in every Protos surface (psyche ruling 2026-07-19:
    /// "field names are now COMPLETLY ILLEGAL EVERYWHERE"), so a field is nothing
    /// but the type standing at its position; its signature is empty.
    FieldMeta,
    /// A user declaration; its constructor signatures are derived from its layout.
    Declaration(EncodedDeclaration),
}

impl MemberKind {
    fn constructor_count(&self) -> usize {
        match self {
            Self::Primitive => 1,
            Self::FieldMeta => 1,
            Self::Declaration(declaration) => declaration.value().constructor_count(),
        }
    }
}

/// One universe type: its allocated id, its name identifier, and its kind.
#[derive(Clone, Debug)]
pub struct UniverseType {
    id: ScopedEncodedTypeId,
    name: Identifier,
    kind: MemberKind,
}

impl UniverseType {
    pub fn id(&self) -> ScopedEncodedTypeId {
        self.id
    }

    pub fn name(&self) -> Identifier {
        self.name
    }

    pub fn kind(&self) -> &MemberKind {
        &self.kind
    }
}

/// A set of stringless Core declarations resolved into a structural-codec Core
/// universe: id registry, name table, and the Core-layout signature derivation.
#[derive(Clone, Debug)]
pub struct EncodedUniverse {
    universe: EncodedUniverseId,
    names: NameTable,
    members: Vec<UniverseType>,
    by_id: BTreeMap<ScopedEncodedTypeId, usize>,
    by_name: HashMap<Identifier, ScopedEncodedTypeId>,
    integer: ScopedEncodedTypeId,
    text: ScopedEncodedTypeId,
    boolean: ScopedEncodedTypeId,
    bytes: ScopedEncodedTypeId,
}

impl EncodedUniverse {
    /// The universe these types belong to.
    pub fn universe(&self) -> EncodedUniverseId {
        self.universe
    }

    /// The schema's names. The projected (Textual) view resolves every identifier
    /// through this table; a rename is an edit here that never touches the Core.
    pub fn names(&self) -> &NameTable {
        &self.names
    }

    /// A mutable borrow of the names, for a decode that interns new names.
    pub fn names_mut(&mut self) -> &mut NameTable {
        &mut self.names
    }

    /// Every registered universe type, in allocation order.
    pub fn members(&self) -> &[UniverseType] {
        &self.members
    }

    /// The schema-whole this universe declares: its declaration members, in ascending
    /// id order, as a [`EncodedSchema`]. The primitives and the `Field` meta-type are the
    /// universe's fixed substrate, not schema declarations, so they are not included.
    /// Under the authority-provided construction path ([`Self::from_assignment`]) the
    /// registration order already ascends by assigned local, so this schema's
    /// declaration order — and thus its content identity — is a deterministic function
    /// of the authority's assignment, never of parse order.
    pub fn declared_schema(&self) -> EncodedSchema {
        let mut ordered: Vec<&UniverseType> = self.members.iter().collect();
        ordered.sort_by_key(|member| member.id);
        let declarations = ordered
            .into_iter()
            .filter_map(|member| match member.kind() {
                MemberKind::Declaration(declaration) => Some(declaration.clone()),
                MemberKind::Primitive | MemberKind::FieldMeta => None,
            })
            .collect();
        EncodedSchema::new(declarations)
    }

    /// Build a universe from central-authority-assigned identities — the
    /// authority-provided construction path. Members are registered in ascending
    /// assigned-local order and their names interned in that same canonical order, so
    /// the id registry, the name indices, and every declaration's own identifier are a
    /// deterministic function of the assignment alone, never of the order an ingestion
    /// parsed its declarations. Two ingestions of one declared schema that received the
    /// same assignment therefore build byte-identical Core values — the identity
    /// keystone realized at the schema layer, replacing parse-order interning.
    ///
    /// The self-contained [`EncodedUniverseBuilder`] path (see [`crate::fixture`]) is
    /// retained as the local / offline mode.
    ///
    /// LEAN `authority-provided-universe` (realizes v2 keystone / L5 at the schema
    /// layer): a universe is built from authority assignments keyed by declared name,
    /// with names canonically interned by ascending assigned local, and each
    /// declaration fully re-stamped ([`EncodedType::restamp`]) into that canonical name
    /// space — its own name, every field and variant name, and the target of every
    /// `Plain` cross-reference. Interior names are resolved through the `source` name
    /// space the assignment's declarations were parsed against and re-interned in a
    /// fixed positional walk (ascending local, then declaration bodies in order), so
    /// the built universe's bytes are a pure function of (assignment, declaration
    /// content), never of parse order. Revision trigger: the authority returning
    /// explicit per-name index assignments so the NameTable order is dictated rather
    /// than derived, or an elided-string-field spelling ruling (bead .31) that changes
    /// what a derived field name canonicalises to.
    pub fn from_assignment<Source>(
        universe: EncodedUniverseId,
        members: Vec<AssignedMember>,
        source: &Source,
    ) -> Result<Self, UniverseError>
    where
        Source: NameResolver + ?Sized,
    {
        let mut ordered = members;
        ordered.sort_by_key(AssignedMember::local);
        for adjacent in ordered.windows(2) {
            if adjacent[0].local == adjacent[1].local {
                return Err(UniverseError::DuplicateAssignedIdentity(adjacent[0].local));
            }
        }
        let mut builder = EncodedUniverseBuilder::new();
        // Phase 1: intern every member's declared name in canonical ascending-local
        // order, so declaration names hold the lowest canonical identifiers in a
        // parse-order-independent order.
        let canonical: Vec<Identifier> = ordered
            .iter()
            .map(|member| builder.intern_name(member.name.clone()))
            .collect::<Result<_, NameTableError>>()?;
        // Phase 2: register each member, re-stamping declaration bodies' interior
        // names into the same canonical table through the source name space.
        for (member, own) in ordered.iter().zip(canonical) {
            let id = ScopedEncodedTypeId::new(universe, member.local);
            match &member.kind {
                AssignedKind::ScalarPrimitive(slot) => builder.primitive_at(id, own, *slot),
                AssignedKind::LeafPrimitive => builder.leaf_at(id, own),
                AssignedKind::FieldMeta => builder.field_meta_at(id, own),
                AssignedKind::Declaration(declaration) => {
                    let restamped = declaration.restamp(own, source, builder.names_mut())?;
                    builder.declaration(id, restamped);
                }
            }
        }
        Ok(builder.build(universe))
    }

    fn member(&self, id: ScopedEncodedTypeId) -> Result<&UniverseType, UniverseError> {
        self.by_id
            .get(&id)
            .and_then(|index| self.members.get(*index))
            .ok_or(UniverseError::UnknownType(id))
    }

    /// The declared Core type at `id`, if the type is a user declaration (not a
    /// primitive or the `Field` meta-type). Reification dispatches on its shape.
    pub fn core_type(&self, id: ScopedEncodedTypeId) -> Option<&EncodedType> {
        match self.member(id).ok()?.kind() {
            MemberKind::Declaration(declaration) => Some(declaration.value()),
            MemberKind::Primitive | MemberKind::FieldMeta => None,
        }
    }

    /// The universe type a name identifier names, if any.
    pub fn type_of_name(&self, name: Identifier) -> Option<ScopedEncodedTypeId> {
        self.by_name.get(&name).copied()
    }

    /// Resolve a by-kind reference to the universe type it names. Scalar leaves map
    /// to their primitive types; a `Plain` reference resolves its name through the
    /// registry; a generic application has no allocated type in this PoC universe
    /// and is a loud, typed error rather than a silent guess.
    pub fn resolve_reference(
        &self,
        reference: &EncodedReference,
    ) -> Result<ScopedEncodedTypeId, UniverseError> {
        match reference {
            EncodedReference::Integer => Ok(self.integer),
            EncodedReference::String => Ok(self.text),
            EncodedReference::Boolean => Ok(self.boolean),
            EncodedReference::Bytes => Ok(self.bytes),
            EncodedReference::Plain(identifier) => self
                .by_name
                .get(identifier)
                .copied()
                .ok_or(UniverseError::UnresolvedName(*identifier)),
            EncodedReference::SingleTypeApplication { .. } => Err(
                UniverseError::UnsupportedApplication("single-type generic application"),
            ),
            EncodedReference::MultiTypeApplication { .. } => Err(
                UniverseError::UnsupportedApplication("multi-type generic application"),
            ),
            EncodedReference::ValueApplication { .. } => {
                Err(UniverseError::UnsupportedApplication("value application"))
            }
        }
    }

    /// The number of Core constructors the type at `id` has.
    pub fn constructor_count(&self, id: ScopedEncodedTypeId) -> Result<usize, UniverseError> {
        Ok(self.member(id)?.kind.constructor_count())
    }

    /// Derive, from the Core layout alone, the positional field signature of one
    /// constructor: the ordered universe-type ids of its fields' referenced types.
    /// This is the ground truth the authored structural table is checked against.
    pub fn core_signature(
        &self,
        id: ScopedEncodedTypeId,
        constructor: u32,
    ) -> Result<PositionalSignature, UniverseError> {
        let member = self.member(id)?;
        let fields: Vec<ScopedEncodedTypeId> = match &member.kind {
            MemberKind::Primitive | MemberKind::FieldMeta => Vec::new(),
            MemberKind::Declaration(declaration) => match declaration.value() {
                EncodedType::Newtype(newtype) => vec![self.resolve_reference(newtype.reference())?],
                EncodedType::Struct(structure) => structure
                    .fields()
                    .iter()
                    .map(|field| self.resolve_reference(field.reference()))
                    .collect::<Result<_, _>>()?,
                EncodedType::Enumeration(enumeration) => {
                    let variant = enumeration.variants().get(constructor as usize).ok_or(
                        UniverseError::ConstructorCountMismatch {
                            core_type: id,
                            members: enumeration.variants().len(),
                            codecs: constructor as usize + 1,
                        },
                    )?;
                    match variant.payload() {
                        Some(payload) => vec![self.resolve_reference(payload)?],
                        None => Vec::new(),
                    }
                }
            },
        };
        Ok(PositionalSignature::new(fields))
    }

    /// Validate an authored structural table against the Core layout: every type
    /// must have a table entry with one codec per Core constructor, and every
    /// codec's authored signature must equal the Core-derived one. A mismatch is
    /// the loud [`UniverseError::SignatureMismatch`] — the deferred deviation,
    /// closed.
    pub fn validate_table(&self, table: &AddressedStructuralTable) -> Result<(), UniverseError> {
        for member in &self.members {
            let entry = table
                .entry(member.id)
                .ok_or(UniverseError::TableEntryAbsent(member.id))?;
            let expected = member.kind.constructor_count();
            if entry.constructors.len() != expected {
                return Err(UniverseError::ConstructorCountMismatch {
                    core_type: member.id,
                    members: expected,
                    codecs: entry.constructors.len(),
                });
            }
            for (index, codec) in entry.constructors.iter().enumerate() {
                let core = self.core_signature(member.id, index as u32)?;
                if codec.signature.fields() != core.fields() {
                    return Err(UniverseError::SignatureMismatch {
                        core_type: member.id,
                        constructor: index as u32,
                        authored: codec.signature.fields().to_vec(),
                        core: core.fields().to_vec(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The kind of one central-authority-assigned universe member, mirroring the
/// builder's registration verbs so a single assignment covers the scalar leaf
/// primitives, the `Field` meta-type, and user declarations. A closed typed record:
/// the registration follows from the kind, never from a flag.
#[derive(Clone, Debug)]
pub enum AssignedKind {
    /// A scalar leaf primitive that is a reference target, filling a scalar slot.
    ScalarPrimitive(ScalarSlot),
    /// A scalar leaf primitive that is never a reference target — e.g. `Float`.
    LeafPrimitive,
    /// The `Field` meta-type.
    FieldMeta,
    /// A user declaration, carried whole so its visibility and
    /// [`DeclarationRole`](crate::declaration::DeclarationRole) are preserved through
    /// the build; its value's own name and every interior name are re-stamped to the
    /// canonically interned identifiers when the universe is built.
    Declaration(EncodedDeclaration),
}

/// One central-authority-assigned universe member: the local identity the authority
/// minted or bound for it, the declared name it carries, and its kind. A universe
/// built from a set of these ([`EncodedUniverse::from_assignment`]) is a deterministic
/// function of the assignment, so two ingestions of one declared schema bind identical
/// identities whatever order each parsed.
#[derive(Clone, Debug)]
pub struct AssignedMember {
    local: u32,
    name: Name,
    kind: AssignedKind,
}

impl AssignedMember {
    pub fn new(local: u32, name: Name, kind: AssignedKind) -> Self {
        Self { local, name, kind }
    }

    /// The local identity the authority assigned — the `local` half of the member's
    /// [`ScopedEncodedTypeId`] and the key its canonical registration order sorts by.
    pub fn local(&self) -> u32 {
        self.local
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn kind(&self) -> &AssignedKind {
        &self.kind
    }
}

/// Builds a [`EncodedUniverse`], owning the shared [`NameTable`] so declarations are
/// constructed against the same identifier space the universe resolves through.
#[derive(Debug)]
pub struct EncodedUniverseBuilder {
    names: NameTable,
    members: Vec<UniverseType>,
    scalars: HashMap<ScalarSlot, ScopedEncodedTypeId>,
}

impl Default for EncodedUniverseBuilder {
    fn default() -> Self {
        Self {
            names: NameTable::new(IdentifierNamespace::Schema),
            members: Vec::new(),
            scalars: HashMap::new(),
        }
    }
}

/// Which scalar leaf a primitive registration fills. Naming the slot as data keeps
/// `resolve_reference` free of stringly primitive lookups.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScalarSlot {
    Integer,
    Text,
    Boolean,
    Bytes,
}

impl EncodedUniverseBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a name into the shared table.
    pub fn intern(&mut self, name: &str) -> Result<Identifier, NameTableError> {
        self.names.intern(Name::new(name))
    }

    /// Intern an owned [`Name`] into the shared table. The authority-provided path
    /// interns in canonical assigned-id order, so it controls the interning order
    /// directly rather than through the `&str` convenience above.
    pub fn intern_name(&mut self, name: Name) -> Result<Identifier, NameTableError> {
        self.names.intern(name)
    }

    /// A mutable borrow of the shared table, for a re-stamp that resolves interior
    /// names from a source name space and re-interns them here in canonical order.
    pub fn names_mut(&mut self) -> &mut NameTable {
        &mut self.names
    }

    /// Register a scalar leaf primitive that is a reference target at an already
    /// interned identifier, filling its scalar slot.
    pub fn primitive_at(&mut self, id: ScopedEncodedTypeId, name: Identifier, slot: ScalarSlot) {
        self.scalars.insert(slot, id);
        self.register(id, name, MemberKind::Primitive);
    }

    /// Register a scalar leaf primitive that is never a reference target at an already
    /// interned identifier (fills no scalar slot).
    pub fn leaf_at(&mut self, id: ScopedEncodedTypeId, name: Identifier) {
        self.register(id, name, MemberKind::Primitive);
    }

    /// Register the `Field` meta-type at an already interned identifier.
    pub fn field_meta_at(&mut self, id: ScopedEncodedTypeId, name: Identifier) {
        self.register(id, name, MemberKind::FieldMeta);
    }

    fn register(&mut self, id: ScopedEncodedTypeId, name: Identifier, kind: MemberKind) {
        self.members.push(UniverseType { id, name, kind });
    }

    /// Register a scalar leaf primitive under a well-known name and scalar slot.
    pub fn primitive(
        &mut self,
        id: ScopedEncodedTypeId,
        name: &str,
        slot: ScalarSlot,
    ) -> Result<Identifier, NameTableError> {
        let identifier = self.intern(name)?;
        self.scalars.insert(slot, id);
        self.register(id, identifier, MemberKind::Primitive);
        Ok(identifier)
    }

    /// Register a scalar leaf primitive that is never a reference target (so it
    /// fills no scalar slot) — `Float`, which the fixture uses only as a standalone
    /// leaf value type.
    pub fn primitive_leaf(
        &mut self,
        id: ScopedEncodedTypeId,
        name: &str,
    ) -> Result<Identifier, NameTableError> {
        let identifier = self.intern(name)?;
        self.register(id, identifier, MemberKind::Primitive);
        Ok(identifier)
    }

    /// Register the `Field` meta-type under a name.
    pub fn field_meta(
        &mut self,
        id: ScopedEncodedTypeId,
        name: &str,
    ) -> Result<Identifier, NameTableError> {
        let identifier = self.intern(name)?;
        self.register(id, identifier, MemberKind::FieldMeta);
        Ok(identifier)
    }

    /// Register a user declaration at an allocated id. The declaration's identifier
    /// must already be interned in the shared table (via [`intern`]).
    ///
    /// [`intern`]: EncodedUniverseBuilder::intern
    pub fn declaration(&mut self, id: ScopedEncodedTypeId, declaration: EncodedDeclaration) {
        let name = declaration.identifier();
        self.register(id, name, MemberKind::Declaration(declaration));
    }

    /// Seal the universe, building the id and name registries.
    pub fn build(self, universe: EncodedUniverseId) -> EncodedUniverse {
        let mut by_id = BTreeMap::new();
        let mut by_name = HashMap::new();
        for (index, member) in self.members.iter().enumerate() {
            by_id.insert(member.id, index);
            by_name.insert(member.name, member.id);
        }
        let scalar = |slot: ScalarSlot| {
            self.scalars
                .get(&slot)
                .copied()
                .unwrap_or_else(|| ScopedEncodedTypeId::new(universe, u32::MAX))
        };
        EncodedUniverse {
            universe,
            integer: scalar(ScalarSlot::Integer),
            text: scalar(ScalarSlot::Text),
            boolean: scalar(ScalarSlot::Boolean),
            bytes: scalar(ScalarSlot::Bytes),
            names: self.names,
            members: self.members,
            by_id,
            by_name,
        }
    }
}

/// The explicit fixture universe id this proof-of-concept works in, re-exported so
/// callers name the same universe `structural-codec`'s fixture ids scope to.
pub const ENCODED_UNIVERSE: EncodedUniverseId = FIXTURE_UNIVERSE;
