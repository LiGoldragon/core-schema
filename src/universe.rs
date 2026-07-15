//! The universe bridge: how a set of stringless `CoreSchema` declarations forms a
//! `structural-codec` Core universe.
//!
//! A [`CoreUniverse`] allocates one [`ScopedCoreTypeId`] per Core type (the scalar
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
//! [`validate_table`]: CoreUniverse::validate_table

use std::collections::{BTreeMap, HashMap};

use name_table::{Identifier, Name, NameTable};
use structural_codec::ids::{
    CoreUniverseId, FIXTURE_UNIVERSE, PositionalSignature, ScopedCoreTypeId,
};
use structural_codec::table::AddressedStructuralTable;

use crate::declaration::{CoreDeclaration, CoreType};
use crate::error::UniverseError;
use crate::reference::CoreReference;

/// What a universe type is, for the purpose of deriving its Core constructor
/// signatures. A closed typed record: the constructor arity and each signature
/// follow from the kind, never from a flag.
#[derive(Clone, Debug)]
pub enum MemberKind {
    /// A scalar leaf primitive (`Integer`, `Text`, `Boolean`, `Bytes`). One
    /// terminal constructor whose field signature is empty.
    Primitive,
    /// The `Field` meta-type: two structurally-disjoint constructors (a bare
    /// elided-name `Type` and an explicit `name.Type`). A field's payload is name
    /// identifiers, not typed sub-structures, so both signatures are empty.
    FieldMeta,
    /// A user declaration; its constructor signatures are derived from its layout.
    Declaration(CoreDeclaration),
}

impl MemberKind {
    fn constructor_count(&self) -> usize {
        match self {
            Self::Primitive => 1,
            Self::FieldMeta => 2,
            Self::Declaration(declaration) => declaration.value().constructor_count(),
        }
    }
}

/// One universe type: its allocated id, its name identifier, and its kind.
#[derive(Clone, Debug)]
pub struct UniverseType {
    id: ScopedCoreTypeId,
    name: Identifier,
    kind: MemberKind,
}

impl UniverseType {
    pub fn id(&self) -> ScopedCoreTypeId {
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
pub struct CoreUniverse {
    universe: CoreUniverseId,
    names: NameTable,
    members: Vec<UniverseType>,
    by_id: BTreeMap<ScopedCoreTypeId, usize>,
    by_name: HashMap<Identifier, ScopedCoreTypeId>,
    integer: ScopedCoreTypeId,
    text: ScopedCoreTypeId,
    boolean: ScopedCoreTypeId,
    bytes: ScopedCoreTypeId,
}

impl CoreUniverse {
    /// The universe these types belong to.
    pub fn universe(&self) -> CoreUniverseId {
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

    fn member(&self, id: ScopedCoreTypeId) -> Result<&UniverseType, UniverseError> {
        self.by_id
            .get(&id)
            .and_then(|index| self.members.get(*index))
            .ok_or(UniverseError::UnknownType(id))
    }

    /// The declared Core type at `id`, if the type is a user declaration (not a
    /// primitive or the `Field` meta-type). Reification dispatches on its shape.
    pub fn core_type(&self, id: ScopedCoreTypeId) -> Option<&CoreType> {
        match self.member(id).ok()?.kind() {
            MemberKind::Declaration(declaration) => Some(declaration.value()),
            MemberKind::Primitive | MemberKind::FieldMeta => None,
        }
    }

    /// The universe type a name identifier names, if any.
    pub fn type_of_name(&self, name: Identifier) -> Option<ScopedCoreTypeId> {
        self.by_name.get(&name).copied()
    }

    /// Resolve a by-kind reference to the universe type it names. Scalar leaves map
    /// to their primitive types; a `Plain` reference resolves its name through the
    /// registry; a generic application has no allocated type in this PoC universe
    /// and is a loud, typed error rather than a silent guess.
    pub fn resolve_reference(
        &self,
        reference: &CoreReference,
    ) -> Result<ScopedCoreTypeId, UniverseError> {
        match reference {
            CoreReference::Integer => Ok(self.integer),
            CoreReference::String => Ok(self.text),
            CoreReference::Boolean => Ok(self.boolean),
            CoreReference::Bytes => Ok(self.bytes),
            CoreReference::Plain(identifier) => self
                .by_name
                .get(identifier)
                .copied()
                .ok_or(UniverseError::UnresolvedName(*identifier)),
            CoreReference::SingleTypeApplication { .. } => Err(
                UniverseError::UnsupportedApplication("single-type generic application"),
            ),
            CoreReference::MultiTypeApplication { .. } => Err(
                UniverseError::UnsupportedApplication("multi-type generic application"),
            ),
            CoreReference::ValueApplication { .. } => {
                Err(UniverseError::UnsupportedApplication("value application"))
            }
        }
    }

    /// The number of Core constructors the type at `id` has.
    pub fn constructor_count(&self, id: ScopedCoreTypeId) -> Result<usize, UniverseError> {
        Ok(self.member(id)?.kind.constructor_count())
    }

    /// Derive, from the Core layout alone, the positional field signature of one
    /// constructor: the ordered universe-type ids of its fields' referenced types.
    /// This is the ground truth the authored structural table is checked against.
    pub fn core_signature(
        &self,
        id: ScopedCoreTypeId,
        constructor: u32,
    ) -> Result<PositionalSignature, UniverseError> {
        let member = self.member(id)?;
        let fields: Vec<ScopedCoreTypeId> = match &member.kind {
            MemberKind::Primitive | MemberKind::FieldMeta => Vec::new(),
            MemberKind::Declaration(declaration) => match declaration.value() {
                CoreType::Newtype(newtype) => vec![self.resolve_reference(newtype.reference())?],
                CoreType::Struct(structure) => structure
                    .fields()
                    .iter()
                    .map(|field| self.resolve_reference(field.reference()))
                    .collect::<Result<_, _>>()?,
                CoreType::Enumeration(enumeration) => {
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

/// Builds a [`CoreUniverse`], owning the shared [`NameTable`] so declarations are
/// constructed against the same identifier space the universe resolves through.
#[derive(Debug, Default)]
pub struct CoreUniverseBuilder {
    names: NameTable,
    members: Vec<UniverseType>,
    scalars: HashMap<ScalarSlot, ScopedCoreTypeId>,
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

impl CoreUniverseBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a name into the shared table.
    pub fn intern(&mut self, name: &str) -> Identifier {
        self.names.intern(Name::new(name))
    }

    fn register(&mut self, id: ScopedCoreTypeId, name: Identifier, kind: MemberKind) {
        self.members.push(UniverseType { id, name, kind });
    }

    /// Register a scalar leaf primitive under a well-known name and scalar slot.
    pub fn primitive(&mut self, id: ScopedCoreTypeId, name: &str, slot: ScalarSlot) -> Identifier {
        let identifier = self.intern(name);
        self.scalars.insert(slot, id);
        self.register(id, identifier, MemberKind::Primitive);
        identifier
    }

    /// Register a scalar leaf primitive that is never a reference target (so it
    /// fills no scalar slot) — `Float`, which the fixture uses only as a standalone
    /// leaf value type.
    pub fn primitive_leaf(&mut self, id: ScopedCoreTypeId, name: &str) -> Identifier {
        let identifier = self.intern(name);
        self.register(id, identifier, MemberKind::Primitive);
        identifier
    }

    /// Register the `Field` meta-type under a name.
    pub fn field_meta(&mut self, id: ScopedCoreTypeId, name: &str) -> Identifier {
        let identifier = self.intern(name);
        self.register(id, identifier, MemberKind::FieldMeta);
        identifier
    }

    /// Register a user declaration at an allocated id. The declaration's identifier
    /// must already be interned in the shared table (via [`intern`]).
    ///
    /// [`intern`]: CoreUniverseBuilder::intern
    pub fn declaration(&mut self, id: ScopedCoreTypeId, declaration: CoreDeclaration) {
        let name = declaration.identifier();
        self.register(id, name, MemberKind::Declaration(declaration));
    }

    /// Seal the universe, building the id and name registries.
    pub fn build(self, universe: CoreUniverseId) -> CoreUniverse {
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
                .unwrap_or_else(|| ScopedCoreTypeId::new(universe, u32::MAX))
        };
        CoreUniverse {
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
pub const CORE_UNIVERSE: CoreUniverseId = FIXTURE_UNIVERSE;
