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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use name_table::{Identifier, IdentifierNamespace, Name, NameTable};
use structural_codec::ids::{
    CoreUniverseId, FIXTURE_UNIVERSE, PositionalSignature, ScopedCoreTypeId,
};
use structural_codec::table::AddressedStructuralTable;

use crate::declaration::{CoreDeclaration, CoreSchema, CoreType};
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
    /// The `Field` meta-type: ONE positional constructor, the bare type reference.
    /// Field names are illegal in every Protos surface (psyche ruling 2026-07-19:
    /// "field names are now COMPLETLY ILLEGAL EVERYWHERE"), so a field is nothing
    /// but the type standing at its position; its signature is empty.
    FieldMeta,
    /// A user declaration; its constructor signatures are derived from its layout.
    Declaration(CoreDeclaration),
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
    integer: Option<ScopedCoreTypeId>,
    text: Option<ScopedCoreTypeId>,
    boolean: Option<ScopedCoreTypeId>,
    bytes: Option<ScopedCoreTypeId>,
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

    /// The schema-whole this universe declares: its declaration members, in ascending
    /// id order, as a [`CoreSchema`]. The primitives and the `Field` meta-type are the
    /// universe's fixed substrate, not schema declarations, so they are not included.
    /// Under the authority-provided construction path ([`Self::from_assignment`]) the
    /// registration order already ascends by assigned local, so this schema's
    /// declaration order — and thus its content identity — is a deterministic function
    /// of the authority's assignment, never of parse order.
    pub fn declared_schema(&self) -> CoreSchema {
        let mut ordered: Vec<&UniverseType> = self.members.iter().collect();
        ordered.sort_by_key(|member| member.id);
        let declarations = ordered
            .into_iter()
            .filter_map(|member| match member.kind() {
                MemberKind::Declaration(declaration) => Some(declaration.clone()),
                MemberKind::Primitive | MemberKind::FieldMeta => None,
            })
            .collect();
        CoreSchema::new(declarations)
    }

    /// Build a universe from authority-assigned members and their complete composed
    /// name table. This transfers the table unchanged: its Schema home and every
    /// borrowed slice are retained, and no name is resolved and re-interned. CoreSchema
    /// members must use Schema identifiers; foreign identifiers are rejected at this
    /// typed boundary rather than silently converted by their spelling.
    pub fn from_assignment(
        universe: CoreUniverseId,
        mut members: Vec<AssignedMember>,
        names: NameTable,
    ) -> Result<Self, UniverseError> {
        members.sort_by_key(AssignedMember::local);

        let mut builder = CoreUniverseBuilder::from_name_table(names);
        for member in members {
            let id = ScopedCoreTypeId::new(universe, member.local);
            match member.kind {
                AssignedKind::ScalarPrimitive(slot) => {
                    builder.primitive_at(id, member.identifier, slot)
                }
                AssignedKind::LeafPrimitive => builder.leaf_at(id, member.identifier),
                AssignedKind::FieldMeta => builder.field_meta_at(id, member.identifier),
                AssignedKind::Declaration(declaration) => {
                    builder.assigned_declaration(id, member.identifier, declaration);
                }
            }
        }
        builder.build(universe)
    }

    fn validate_schema_identifier(identifier: Identifier) -> Result<(), UniverseError> {
        if identifier.namespace() == IdentifierNamespace::Schema {
            Ok(())
        } else {
            Err(UniverseError::WrongSchemaIdentifier(identifier))
        }
    }

    fn validate_scoped_type_id(
        expected: CoreUniverseId,
        member: ScopedCoreTypeId,
    ) -> Result<(), UniverseError> {
        if member.universe == expected {
            Ok(())
        } else {
            Err(UniverseError::UniverseScopeMismatch {
                expected,
                actual: member.universe,
                member,
            })
        }
    }

    fn validate_reference_identifiers(
        reference: &CoreReference,
        names: &NameTable,
        members: &[UniverseType],
        scalar_registrations: &[(ScalarSlot, ScopedCoreTypeId)],
        expected_universe: CoreUniverseId,
    ) -> Result<(), UniverseError> {
        let validate_scalar = |slot| {
            let id = scalar_registrations
                .iter()
                .find_map(|(registered, id)| (*registered == slot).then_some(*id))
                .ok_or_else(|| UniverseError::MissingScalarSlot {
                    slot,
                    reference: reference.clone(),
                })?;
            Self::validate_scoped_type_id(expected_universe, id)?;
            if members.iter().any(|member| member.id == id) {
                Ok(())
            } else {
                Err(UniverseError::MissingScalarSlot {
                    slot,
                    reference: reference.clone(),
                })
            }
        };
        match reference {
            CoreReference::String => validate_scalar(ScalarSlot::Text),
            CoreReference::Integer => validate_scalar(ScalarSlot::Integer),
            CoreReference::Boolean => validate_scalar(ScalarSlot::Boolean),
            CoreReference::Bytes => validate_scalar(ScalarSlot::Bytes),
            CoreReference::Plain(identifier) => {
                Self::validate_schema_identifier(*identifier)?;
                names
                    .resolve(*identifier)
                    .map_err(|_| UniverseError::ReferenceNameAbsent {
                        identifier: *identifier,
                        reference: reference.clone(),
                    })?;
                let member = members
                    .iter()
                    .find(|member| member.name == *identifier)
                    .ok_or_else(|| UniverseError::ReferenceTargetUnregistered {
                        identifier: *identifier,
                        reference: reference.clone(),
                    })?;
                Self::validate_scoped_type_id(expected_universe, member.id)
            }
            CoreReference::SingleTypeApplication { argument, .. } => {
                Self::validate_reference_identifiers(
                    argument,
                    names,
                    members,
                    scalar_registrations,
                    expected_universe,
                )
            }
            CoreReference::MultiTypeApplication { arguments, .. } => {
                arguments.iter().try_for_each(|argument| {
                    Self::validate_reference_identifiers(
                        argument,
                        names,
                        members,
                        scalar_registrations,
                        expected_universe,
                    )
                })
            }
            CoreReference::ValueApplication { .. } => Ok(()),
        }
    }

    fn validate_declaration_identifiers(
        declaration: &CoreDeclaration,
        names: &NameTable,
        members: &[UniverseType],
        scalar_registrations: &[(ScalarSlot, ScopedCoreTypeId)],
        expected_universe: CoreUniverseId,
    ) -> Result<(), UniverseError> {
        let validate_identifier = |identifier| {
            Self::validate_schema_identifier(identifier)?;
            names.resolve(identifier)?;
            Ok::<_, UniverseError>(())
        };
        validate_identifier(declaration.identifier())?;
        match declaration.value() {
            CoreType::Newtype(newtype) => Self::validate_reference_identifiers(
                newtype.reference(),
                names,
                members,
                scalar_registrations,
                expected_universe,
            ),
            CoreType::Struct(structure) => {
                for field in structure.fields() {
                    validate_identifier(field.identifier())?;
                    Self::validate_reference_identifiers(
                        field.reference(),
                        names,
                        members,
                        scalar_registrations,
                        expected_universe,
                    )?;
                }
                Ok(())
            }
            CoreType::Enumeration(enumeration) => {
                for variant in enumeration.variants() {
                    validate_identifier(variant.identifier())?;
                    if let Some(payload) = variant.payload() {
                        Self::validate_reference_identifiers(
                            payload,
                            names,
                            members,
                            scalar_registrations,
                            expected_universe,
                        )?;
                    }
                }
                Ok(())
            }
        }
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

    /// Resolve a by-kind reference to the universe type it names. A scalar leaf
    /// resolves only through its explicitly registered scalar slot; a `Plain`
    /// reference must still resolve in the NameTable and the member registry. A
    /// generic application has no allocated type in this PoC universe and is a loud,
    /// typed error rather than a silent guess.
    pub fn resolve_reference(
        &self,
        reference: &CoreReference,
    ) -> Result<ScopedCoreTypeId, UniverseError> {
        let scalar = |slot, id: Option<ScopedCoreTypeId>| {
            id.ok_or_else(|| UniverseError::MissingScalarSlot {
                slot,
                reference: reference.clone(),
            })
        };
        match reference {
            CoreReference::Integer => scalar(ScalarSlot::Integer, self.integer),
            CoreReference::String => scalar(ScalarSlot::Text, self.text),
            CoreReference::Boolean => scalar(ScalarSlot::Boolean, self.boolean),
            CoreReference::Bytes => scalar(ScalarSlot::Bytes, self.bytes),
            CoreReference::Plain(identifier) => {
                self.names.resolve(*identifier).map_err(|_| {
                    UniverseError::ReferenceNameAbsent {
                        identifier: *identifier,
                        reference: reference.clone(),
                    }
                })?;
                self.by_name.get(identifier).copied().ok_or_else(|| {
                    UniverseError::ReferenceTargetUnregistered {
                        identifier: *identifier,
                        reference: reference.clone(),
                    }
                })
            }
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
    /// A user declaration, carried whole so its visibility, role, identifiers, and
    /// references are preserved exactly through the build.
    Declaration(CoreDeclaration),
}

/// One central-authority-assigned universe member: its authority-minted local type
/// identity, the exact schema identifier it owns, and its kind. The identifier is
/// data, never reconstructed from a resolved name.
#[derive(Clone, Debug)]
pub struct AssignedMember {
    local: u32,
    identifier: Identifier,
    kind: AssignedKind,
}

impl AssignedMember {
    pub fn new(local: u32, identifier: Identifier, kind: AssignedKind) -> Self {
        Self {
            local,
            identifier,
            kind,
        }
    }

    /// The local identity the authority assigned — the `local` half of the member's
    /// [`ScopedCoreTypeId`] and the key its registration order sorts by.
    pub fn local(&self) -> u32 {
        self.local
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn kind(&self) -> &AssignedKind {
        &self.kind
    }
}

/// Builds a [`CoreUniverse`], owning the shared [`NameTable`] so declarations are
/// constructed against the same identifier space the universe resolves through.
#[derive(Debug)]
pub struct CoreUniverseBuilder {
    names: NameTable,
    members: Vec<UniverseType>,
    scalar_registrations: Vec<(ScalarSlot, ScopedCoreTypeId)>,
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

impl Default for CoreUniverseBuilder {
    fn default() -> Self {
        Self {
            names: NameTable::new(IdentifierNamespace::Schema),
            members: Vec::new(),
            scalar_registrations: Vec::new(),
        }
    }
}

impl CoreUniverseBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build against an already completed Schema-home table, preserving its complete
    /// composed slice set rather than copying or flattening it.
    pub fn from_name_table(names: NameTable) -> Self {
        Self {
            names,
            members: Vec::new(),
            scalar_registrations: Vec::new(),
        }
    }

    pub fn names(&self) -> &NameTable {
        &self.names
    }

    /// Intern a name into the shared table.
    pub fn intern(&mut self, name: &str) -> Result<Identifier, name_table::NameTableError> {
        self.names.intern(Name::new(name))
    }

    /// Register a scalar leaf primitive that is a reference target at an already
    /// interned identifier, filling its scalar slot.
    pub fn primitive_at(&mut self, id: ScopedCoreTypeId, name: Identifier, slot: ScalarSlot) {
        self.scalar_registrations.push((slot, id));
        self.register(id, name, MemberKind::Primitive);
    }

    /// Register a scalar leaf primitive that is never a reference target at an already
    /// interned identifier (fills no scalar slot).
    pub fn leaf_at(&mut self, id: ScopedCoreTypeId, name: Identifier) {
        self.register(id, name, MemberKind::Primitive);
    }

    /// Register the `Field` meta-type at an already interned identifier.
    pub fn field_meta_at(&mut self, id: ScopedCoreTypeId, name: Identifier) {
        self.register(id, name, MemberKind::FieldMeta);
    }

    fn register(&mut self, id: ScopedCoreTypeId, name: Identifier, kind: MemberKind) {
        self.members.push(UniverseType { id, name, kind });
    }

    /// Register a scalar leaf primitive under a well-known name and scalar slot.
    pub fn primitive(
        &mut self,
        id: ScopedCoreTypeId,
        name: &str,
        slot: ScalarSlot,
    ) -> Result<Identifier, name_table::NameTableError> {
        let identifier = self.intern(name)?;
        self.scalar_registrations.push((slot, id));
        self.register(id, identifier, MemberKind::Primitive);
        Ok(identifier)
    }

    /// Register a scalar leaf primitive that is never a reference target (so it
    /// fills no scalar slot) — `Float`, which the fixture uses only as a standalone
    /// leaf value type.
    pub fn primitive_leaf(
        &mut self,
        id: ScopedCoreTypeId,
        name: &str,
    ) -> Result<Identifier, name_table::NameTableError> {
        let identifier = self.intern(name)?;
        self.register(id, identifier, MemberKind::Primitive);
        Ok(identifier)
    }

    /// Register the `Field` meta-type under a name.
    pub fn field_meta(
        &mut self,
        id: ScopedCoreTypeId,
        name: &str,
    ) -> Result<Identifier, name_table::NameTableError> {
        let identifier = self.intern(name)?;
        self.register(id, identifier, MemberKind::FieldMeta);
        Ok(identifier)
    }

    /// Register a user declaration at an allocated id. The declaration's identifier
    /// must already be interned in the shared table (via [`intern`]).
    ///
    /// [`intern`]: CoreUniverseBuilder::intern
    pub fn declaration(&mut self, id: ScopedCoreTypeId, declaration: CoreDeclaration) {
        self.assigned_declaration(id, declaration.identifier(), declaration);
    }

    /// Register an authority-assigned declaration without validating it early. The
    /// final seal compares the assigned member identifier to the declaration's own
    /// identifier alongside every other universe invariant.
    fn assigned_declaration(
        &mut self,
        id: ScopedCoreTypeId,
        assigned: Identifier,
        declaration: CoreDeclaration,
    ) {
        self.register(id, assigned, MemberKind::Declaration(declaration));
    }

    /// Seal the universe. This is the sole validation point for NameTable ownership,
    /// every identifier and reachable scoped reference, assignment/declaration agreement,
    /// universe scope, and registry uniqueness; maps are created only after those checks
    /// have passed.
    pub fn build(self, universe: CoreUniverseId) -> Result<CoreUniverse, UniverseError> {
        if self.names.namespace() != IdentifierNamespace::Schema {
            return Err(UniverseError::WrongNameTableHome {
                actual: self.names.namespace(),
            });
        }

        let mut member_ids = BTreeSet::new();
        let mut member_names = HashSet::new();
        for member in &self.members {
            CoreUniverse::validate_schema_identifier(member.name)?;
            self.names.resolve(member.name)?;
            CoreUniverse::validate_scoped_type_id(universe, member.id)?;
            if let MemberKind::Declaration(declaration) = &member.kind {
                if declaration.identifier() != member.name {
                    return Err(UniverseError::AssignedDeclarationIdentifierMismatch {
                        assigned: member.name,
                        declared: declaration.identifier(),
                    });
                }
                CoreUniverse::validate_declaration_identifiers(
                    declaration,
                    &self.names,
                    &self.members,
                    &self.scalar_registrations,
                    universe,
                )?;
            }
            if !member_ids.insert(member.id) {
                return Err(UniverseError::DuplicateMemberIdentity(member.id));
            }
            if !member_names.insert(member.name) {
                return Err(UniverseError::DuplicateMemberName(member.name));
            }
        }

        let mut scalar_slots = HashSet::new();
        for (slot, _) in &self.scalar_registrations {
            if !scalar_slots.insert(*slot) {
                return Err(UniverseError::DuplicateScalarSlot(*slot));
            }
        }

        let by_id = self
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| (member.id, index))
            .collect();
        let by_name = self
            .members
            .iter()
            .map(|member| (member.name, member.id))
            .collect();
        let scalars: HashMap<_, _> = self.scalar_registrations.into_iter().collect();
        let scalar = |slot: ScalarSlot| scalars.get(&slot).copied();
        Ok(CoreUniverse {
            universe,
            integer: scalar(ScalarSlot::Integer),
            text: scalar(ScalarSlot::Text),
            boolean: scalar(ScalarSlot::Boolean),
            bytes: scalar(ScalarSlot::Bytes),
            names: self.names,
            members: self.members,
            by_id,
            by_name,
        })
    }
}

/// The explicit fixture universe id this proof-of-concept works in, re-exported so
/// callers name the same universe `structural-codec`'s fixture ids scope to.
pub const CORE_UNIVERSE: CoreUniverseId = FIXTURE_UNIVERSE;
