//! `TextualSchema` — the first real Textual form: schema text ⇄ `EncodedSchema`.
//!
//! Decoding recognizes source text into a raw `Block` (raw-discovery), runs
//! `structural-codec`'s trusted evaluator over the authored table to a generic
//! `StructuralValue`, then REIFIES that mirror into a real stringless `EncodedType`
//! declaration with a real `NameTable`. Encoding REFLECTS a `EncodedType` back into a
//! `StructuralValue`, lets the evaluator render it to a `Block`, and writes the
//! canonical text. The parser never classifies: the expected Core type drives the
//! evaluator, and reification reads only the mirror.
//!
//! A struct field is nothing but the type standing at its position. Field names are
//! illegal in every Protos surface (psyche ruling 2026-07-19: "field names are now
//! COMPLETLY ILLEGAL EVERYWHERE"), so on encode a field ALWAYS elides — its name is
//! never written — and on decode the name is re-derived from the type, never read
//! from the text. Two fields of the same type derive the same name; position, not the
//! name, tells them apart. An explicit `name.Type` in the text no longer parses as a
//! field and is rejected at decode.

use name_table::{Name, NameTable};
use raw_discovery::{Block, Delimiter, Recognizer};
use structural_codec::ids::ScopedEncodedTypeId;
use structural_codec::table::AddressedStructuralTable;
use structural_codec::value::StructuralValue;
use structural_codec::{CanonicalText, EncodedForm, StructuralEvaluator, Textual, TextualForm};

use crate::declaration::{
    DeclarationRole, EncodedDeclaration, EncodedEnum, EncodedField, EncodedNewtype, EncodedSchema,
    EncodedStruct, EncodedType, EncodedVariant,
};
use crate::document::{
    DOCUMENT_SLOTS, DeclarationConstructor, INTERFACE, ReferenceConstructor, SchemaDocumentGrammar,
    TYPES_BLOCK,
};
use crate::error::TextualError;
use crate::fixture::FixtureFamily;
use crate::manifest::{
    ManifestSchema, SchemaManifest, SchemaManifestFileStructure, SchemaManifestStructure,
};
use crate::reference::EncodedReference;
use crate::universe::{ENCODED_UNIVERSE, EncodedUniverse, EncodedUniverseBuilder};

/// A Textual view over one Core universe: the authored structural table plus the
/// universe it targets, and — for the document grammar — the keyword lexicon its
/// `Literal` forms resolve through. One codec, both directions.
#[derive(Clone, Debug)]
pub struct TextualSchema {
    universe: EncodedUniverse,
    table: AddressedStructuralTable,
    /// The lexicon for `Literal` keyword decode. `None` for tables that carry no
    /// literals (the single-declaration fixture); `Some` for the document grammar.
    lexicon: Option<NameTable>,
}

impl TextualSchema {
    /// Build the Textual view for the fixture family with its standard table.
    pub fn fixture() -> Result<Self, TextualError> {
        let family = FixtureFamily::build();
        let table = family.standard_table()?;
        Ok(Self {
            universe: family.universe().clone(),
            table,
            lexicon: None,
        })
    }

    /// Build the Textual view over the six-slot document grammar, so a whole
    /// spirit-min-shaped document decodes to a full [`EncodedSchema`] and encodes back.
    /// The grammar targets no single Core layout, so its universe carries no members;
    /// document decode dispatches on grammar constructor indices, not universe types.
    pub fn schema_document() -> Result<Self, TextualError> {
        let grammar = SchemaDocumentGrammar::build()?;
        Ok(Self {
            universe: EncodedUniverseBuilder::new().build(ENCODED_UNIVERSE),
            table: grammar.table().clone(),
            lexicon: Some(grammar.lexicon().clone()),
        })
    }

    /// Build a Textual view from an explicit universe and authored table.
    pub fn new(universe: EncodedUniverse, table: AddressedStructuralTable) -> Self {
        Self {
            universe,
            table,
            lexicon: None,
        }
    }

    pub fn universe(&self) -> &EncodedUniverse {
        &self.universe
    }

    pub fn table(&self) -> &AddressedStructuralTable {
        &self.table
    }

    /// Decode one declaration's schema text into a real `EncodedType`, interning names
    /// into `names`. The expected type drives the evaluator; the raw layer only
    /// discovered structure.
    pub fn decode(
        &self,
        expected: ScopedEncodedTypeId,
        text: &str,
        names: &mut NameTable,
    ) -> Result<EncodedType, TextualError> {
        let document = Recognizer::standard().recognize(text)?;
        let block = document
            .root_object_at(0)
            .ok_or(TextualError::EmptySource)?;
        let evaluator = StructuralEvaluator::new(&self.table);
        let value = evaluator.decode(expected, block, names)?;
        self.reify_type(expected, &value, names)
    }

    // The reification helpers below take the names table mutably: an elided field
    // name is derived and interned on demand (never stored in the Core), so decode
    // can add it to the same table the type names were interned into.

    /// Encode a real `EncodedType` back into canonical schema text, resolving names
    /// through `names` (interning any scalar keyword the value needs).
    pub fn encode(
        &self,
        expected: ScopedEncodedTypeId,
        value: &EncodedType,
        names: &mut NameTable,
    ) -> Result<String, TextualError> {
        let mirror = self.reflect_type(value, names)?;
        let evaluator = StructuralEvaluator::new(&self.table);
        let block = evaluator.encode(expected, &mirror, names)?;
        Ok(block.canonical_text())
    }

    // ===== reification: StructuralValue -> EncodedType =====

    fn reify_type(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<EncodedType, TextualError> {
        match self.universe.core_type(expected) {
            Some(EncodedType::Newtype(_)) => self.reify_newtype(value, names),
            Some(EncodedType::Struct(_)) => self.reify_struct(value, names),
            Some(EncodedType::Enumeration(_)) => Self::reify_enumeration(value),
            None => Err(TextualError::ReifyShape("non-declaration expected type")),
        }
    }

    fn reify_newtype(
        &self,
        value: &StructuralValue,
        names: &NameTable,
    ) -> Result<EncodedType, TextualError> {
        let (name, body) = Self::declaration_head(value, "newtype")?;
        let inner = match body {
            [StructuralValue::Atom(inner)] => *inner,
            _ => return Err(TextualError::ReifyShape("newtype body")),
        };
        let reference = self.reference_from_atom(inner, names)?;
        Ok(EncodedType::Newtype(EncodedNewtype::new(name, reference)))
    }

    fn reify_struct(
        &self,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<EncodedType, TextualError> {
        let (name, body) = Self::declaration_head(value, "struct")?;
        // The body slice borrows `value`, not `names`, so interning per field is free
        // of a borrow conflict.
        let body: Vec<StructuralValue> = body.to_vec();
        let mut fields = Vec::with_capacity(body.len());
        for field_value in &body {
            fields.push(self.reify_field(field_value, names)?);
        }
        Ok(EncodedType::Struct(EncodedStruct::new(name, fields)))
    }

    /// A declaration value is `Chosen{0, Application(Atom(name), Delimited(body))}`.
    fn declaration_head<'value>(
        value: &'value StructuralValue,
        what: &'static str,
    ) -> Result<(name_table::Identifier, &'value [StructuralValue]), TextualError> {
        let StructuralValue::Chosen { payload, .. } = value else {
            return Err(TextualError::ReifyShape(what));
        };
        let StructuralValue::Application(head, tail) = payload.as_ref() else {
            return Err(TextualError::ReifyShape(what));
        };
        let StructuralValue::Atom(name) = head.as_ref() else {
            return Err(TextualError::ReifyShape(what));
        };
        let StructuralValue::Delimited(body) = tail.as_ref() else {
            return Err(TextualError::ReifyShape(what));
        };
        Ok((*name, body))
    }

    fn reify_field(
        &self,
        field_value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<EncodedField, TextualError> {
        let StructuralValue::Delegated(inner) = field_value else {
            return Err(TextualError::ReifyShape("struct field delegate"));
        };
        let StructuralValue::Chosen { payload, .. } = inner.as_ref() else {
            return Err(TextualError::ReifyShape("struct field constructor"));
        };
        // A field is nothing but the type standing at its position. The payload is the
        // type atom; the field's name is DERIVED from that type and interned on demand,
        // never read from the text (field names are illegal). Two same-typed fields
        // derive the same name — position, not the name, distinguishes them.
        let StructuralValue::Atom(type_id) = payload.as_ref() else {
            return Err(TextualError::ReifyShape("struct field type"));
        };
        let reference = self.reference_from_atom(*type_id, names)?;
        let derived = reference.derived_field_name(names)?;
        let identifier = names.intern(name_table::Name::new(derived));
        Ok(EncodedField::new(identifier, reference))
    }

    fn reference_from_atom(
        &self,
        type_id: name_table::Identifier,
        names: &NameTable,
    ) -> Result<EncodedReference, TextualError> {
        let name = names.resolve(type_id)?;
        Ok(EncodedReference::from_type_name(name, type_id))
    }

    // ===== reflection: EncodedType -> StructuralValue =====

    fn reflect_type(
        &self,
        value: &EncodedType,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        match value {
            EncodedType::Newtype(newtype) => self.reflect_newtype(newtype, names),
            EncodedType::Struct(structure) => self.reflect_struct(structure, names),
            EncodedType::Enumeration(enumeration) => Self::reflect_enumeration(enumeration),
        }
    }

    fn reflect_newtype(
        &self,
        newtype: &EncodedNewtype,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let inner = newtype
            .reference()
            .type_atom_identifier(names)
            .ok_or(TextualError::ReifyShape("newtype inner reference"))?;
        let body = StructuralValue::Delimited(vec![StructuralValue::Atom(inner)]);
        Ok(StructuralValue::chosen(
            0,
            StructuralValue::Application(
                Box::new(StructuralValue::Atom(newtype.identifier())),
                Box::new(body),
            ),
        ))
    }

    fn reflect_struct(
        &self,
        structure: &EncodedStruct,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let mut field_values = Vec::with_capacity(structure.fields().len());
        for field in structure.fields() {
            field_values.push(self.reflect_field(field, names)?);
        }
        Ok(StructuralValue::chosen(
            0,
            StructuralValue::Application(
                Box::new(StructuralValue::Atom(structure.identifier())),
                Box::new(StructuralValue::Delimited(field_values)),
            ),
        ))
    }

    fn reflect_field(
        &self,
        field: &EncodedField,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let type_id = field
            .reference()
            .type_atom_identifier(names)
            .ok_or(TextualError::ReifyShape("field type reference"))?;
        // A field is nothing but the type standing at its position. Its name is NEVER
        // written (field names are illegal in every Protos surface, psyche ruling
        // 2026-07-19), so every field elides to its bare type atom — even two fields of
        // the same type, told apart by position alone.
        let chosen = StructuralValue::chosen(0, StructuralValue::Atom(type_id));
        Ok(StructuralValue::Delegated(Box::new(chosen)))
    }

    // ===== enumeration declarations (single-declaration path) =====

    /// Reify an enumeration declaration `Name.[ Variant* ]` — a `Chosen` wrapping the
    /// name-applied bracket of unit variants — into a real [`EncodedEnum`].
    fn reify_enumeration(value: &StructuralValue) -> Result<EncodedType, TextualError> {
        let (name, body) = Self::declaration_head(value, "enumeration")?;
        Ok(EncodedType::Enumeration(EncodedEnum::new(
            name,
            Self::variants_from_atoms(body)?,
        )))
    }

    /// Reflect a [`EncodedEnum`] back into the enumeration-declaration mirror.
    fn reflect_enumeration(enumeration: &EncodedEnum) -> Result<StructuralValue, TextualError> {
        Ok(StructuralValue::chosen(
            0,
            StructuralValue::Application(
                Box::new(StructuralValue::Atom(enumeration.identifier())),
                Box::new(StructuralValue::Delimited(Self::variant_atoms(
                    enumeration,
                )?)),
            ),
        ))
    }

    /// The unit variants a bracket of name atoms carries. A payload-bearing atom
    /// cannot appear here — a declaration bracket lists variant names only.
    fn variants_from_atoms(atoms: &[StructuralValue]) -> Result<Vec<EncodedVariant>, TextualError> {
        atoms
            .iter()
            .map(|atom| match atom {
                StructuralValue::Atom(identifier) => Ok(EncodedVariant::new(*identifier, None)),
                _ => Err(TextualError::ReifyShape("enumeration variant")),
            })
            .collect()
    }

    /// The name atoms an enumeration's unit variants encode to. A payload variant has
    /// no square-bracket declaration form and is rejected loudly.
    fn variant_atoms(enumeration: &EncodedEnum) -> Result<Vec<StructuralValue>, TextualError> {
        enumeration
            .variants()
            .iter()
            .map(|variant| {
                if variant.payload().is_some() {
                    Err(TextualError::ReifyShape(
                        "enumeration declaration payload variant",
                    ))
                } else {
                    Ok(StructuralValue::Atom(variant.identifier()))
                }
            })
            .collect()
    }

    // ===== type references (by kind and projection) =====

    /// Reify a `TypeReference` mirror into a real [`EncodedReference`], dispatching on
    /// the winning grammar constructor index — never a head string. A `Delegate`
    /// wrapper is transparent; a scalar constructor yields its leaf; a single-type
    /// projection yields the application over its recursively reified argument; the
    /// `Plain` fallback carries the name identifier.
    fn reify_reference(value: &StructuralValue) -> Result<EncodedReference, TextualError> {
        match value {
            StructuralValue::Delegated(inner) => Self::reify_reference(inner),
            StructuralValue::Chosen {
                constructor,
                payload,
            } => {
                let constructor = ReferenceConstructor::from_index(*constructor)
                    .ok_or(TextualError::ReifyShape("type reference constructor"))?;
                if let Some(scalar) = constructor.scalar() {
                    Ok(scalar)
                } else if let Some(projection) = constructor.single_projection() {
                    let StructuralValue::Application(_keyword, argument) = payload.as_ref() else {
                        return Err(TextualError::ReifyShape(
                            "single-type projection application",
                        ));
                    };
                    Ok(EncodedReference::SingleTypeApplication {
                        projection,
                        argument: Box::new(Self::reify_reference(argument)?),
                    })
                } else {
                    let StructuralValue::Atom(identifier) = payload.as_ref() else {
                        return Err(TextualError::ReifyShape("plain reference name"));
                    };
                    Ok(EncodedReference::Plain(*identifier))
                }
            }
            _ => Err(TextualError::ReifyShape("type reference")),
        }
    }

    /// Reflect a [`EncodedReference`] into its `TypeReference` mirror. Scalar and
    /// projection keywords are interned into `names` so the evaluator's `Literal`
    /// encode resolves them; a `Plain` reference carries its stored identifier.
    fn reflect_reference(
        &self,
        reference: &EncodedReference,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        match reference {
            EncodedReference::Integer => Ok(Self::reference_scalar_mirror(
                ReferenceConstructor::Integer,
                names,
            )),
            EncodedReference::String => Ok(Self::reference_scalar_mirror(
                ReferenceConstructor::String,
                names,
            )),
            EncodedReference::Boolean => Ok(Self::reference_scalar_mirror(
                ReferenceConstructor::Boolean,
                names,
            )),
            EncodedReference::Bytes => Ok(Self::reference_scalar_mirror(
                ReferenceConstructor::Bytes,
                names,
            )),
            EncodedReference::Plain(identifier) => Ok(StructuralValue::chosen(
                ReferenceConstructor::Plain.index(),
                StructuralValue::Atom(*identifier),
            )),
            EncodedReference::SingleTypeApplication {
                projection,
                argument,
            } => {
                let constructor = ReferenceConstructor::from_single_projection(*projection);
                let keyword = constructor
                    .keyword()
                    .ok_or(TextualError::ReifyShape("projection keyword"))?;
                let keyword_id = names.intern(Name::new(keyword));
                let inner = self.reflect_reference(argument, names)?;
                Ok(StructuralValue::chosen(
                    constructor.index(),
                    StructuralValue::Application(
                        Box::new(StructuralValue::Atom(keyword_id)),
                        Box::new(StructuralValue::Delegated(Box::new(inner))),
                    ),
                ))
            }
            EncodedReference::MultiTypeApplication { .. } => {
                Err(TextualError::ReifyShape("multi-type application encode"))
            }
            EncodedReference::ValueApplication { .. } => {
                Err(TextualError::ReifyShape("value application encode"))
            }
        }
    }

    /// A scalar reference's mirror: the constructor tag over the keyword atom the
    /// evaluator's `Literal` encode resolves.
    fn reference_scalar_mirror(
        constructor: ReferenceConstructor,
        names: &mut NameTable,
    ) -> StructuralValue {
        let keyword = constructor
            .keyword()
            .expect("a scalar constructor has a keyword");
        StructuralValue::chosen(
            constructor.index(),
            StructuralValue::Atom(names.intern(Name::new(keyword))),
        )
    }

    // ===== the six-slot document layout =====

    /// Decode a whole six-slot document — `imports {} input [] output [] types {}
    /// generics {} impls {}` — into a full [`EncodedSchema`]. The two interface lines
    /// decode into role-tagged enumeration declarations (the [`InterfaceInput`] /
    /// [`InterfaceOutput`] roots) and the `types` block into data declarations; all
    /// three land in the one declaration substrate, interface roots first. The
    /// imports, generics, and impls slots must be empty braces (a non-empty one is
    /// not yet modelled and is rejected, never dropped).
    ///
    /// [`InterfaceInput`]: DeclarationRole::InterfaceInput
    /// [`InterfaceOutput`]: DeclarationRole::InterfaceOutput
    pub fn decode_document(
        &self,
        text: &str,
        names: &mut NameTable,
    ) -> Result<EncodedSchema, TextualError> {
        let document = Recognizer::standard().recognize(text)?;
        let roots = document.root_objects();
        if roots.len() != DOCUMENT_SLOTS {
            return Err(TextualError::DocumentArity(roots.len()));
        }
        Self::require_empty_brace(&roots[0], "imports")?;
        let input =
            self.decode_interface_slot(&roots[1], DeclarationRole::InterfaceInput, names)?;
        let output =
            self.decode_interface_slot(&roots[2], DeclarationRole::InterfaceOutput, names)?;
        let types = self.decode_types_slot(&roots[3], names)?;
        Self::require_empty_brace(&roots[4], "generics")?;
        Self::require_empty_brace(&roots[5], "impls")?;
        let mut declarations =
            Vec::with_capacity(types.len() + input.iter().count() + output.iter().count());
        if let Some(input) = input {
            declarations.push(input);
        }
        if let Some(output) = output {
            declarations.push(output);
        }
        declarations.extend(types);
        Ok(EncodedSchema::new(declarations))
    }

    /// Encode a [`EncodedSchema`] back into six-slot document text, one slot per line.
    /// The interface roots render into the `input` / `output` brackets and the data
    /// declarations into the `types` block; a schema missing an interface root is
    /// rejected loudly rather than rendered with an empty protocol line.
    pub fn encode_document(
        &self,
        schema: &EncodedSchema,
        names: &mut NameTable,
    ) -> Result<String, TextualError> {
        let input = match schema.input() {
            Some(input) => self.encode_interface_slot(input, names)?,
            None => Self::empty_square(),
        };
        let output = match schema.output() {
            Some(output) => self.encode_interface_slot(output, names)?,
            None => Self::empty_square(),
        };
        let slots = [
            Self::empty_brace(),
            input,
            output,
            self.encode_types_slot(schema, names)?,
            Self::empty_brace(),
            Self::empty_brace(),
        ];
        Ok(slots.join("\n"))
    }

    /// Decode an explicit manifest of schema source files into one encoded schema and
    /// the file-layout StructureTree needed to emit the same TextualForm again. Files
    /// are decoded in dependency order, while output retains the manifest's explicit
    /// order. The NameTable is shared across the entire manifest, so cross-file
    /// encoded identifiers occupy one continuous namespace without entering Nomos.
    pub fn decode_manifest(
        &self,
        manifest: &SchemaManifest,
        view: &TextualForm<SchemaLanguage>,
        names: &mut NameTable,
    ) -> Result<ManifestSchema, TextualError> {
        for chunk in view.chunks() {
            if !manifest
                .files()
                .iter()
                .any(|file| file.path() == &chunk.name)
            {
                return Err(crate::manifest::SchemaManifestError::UnexpectedSourceFile {
                    path: chunk.name.0.clone(),
                }
                .into());
            }
        }

        let mut declarations = Vec::new();
        let mut positions = std::collections::BTreeMap::new();
        let mut identifiers = std::collections::BTreeSet::new();
        for file in manifest.dependency_order()? {
            let chunk = view.named_chunk(file.path())?;
            let decoded = self.decode_document(&chunk.text, names)?;
            let start = declarations.len();
            for declaration in decoded.declarations() {
                if !identifiers.insert(declaration.identifier()) {
                    return Err(
                        crate::manifest::SchemaManifestError::DuplicateDeclarationIdentifier {
                            identifier: declaration.identifier(),
                        }
                        .into(),
                    );
                }
                declarations.push(declaration.clone());
            }
            positions.insert(
                file.path().0.clone(),
                (start..declarations.len()).collect::<Vec<_>>(),
            );
        }

        let structure = SchemaManifestStructure::new(
            manifest
                .files()
                .iter()
                .map(|file| {
                    SchemaManifestFileStructure::new(
                        file.path().clone(),
                        positions
                            .remove(&file.path().0)
                            .expect("validated manifest file has a decoded position list"),
                    )
                })
                .collect(),
        );
        structure.validate(manifest, declarations.len())?;
        Ok(ManifestSchema::new(
            EncodedSchema::new(declarations),
            structure,
        ))
    }

    /// Emit a manifest-backed encoded schema through the StructureTree captured or
    /// authored for it. No file path is stored in the EncodedForm: the structure owns
    /// file allocation, the NameTable owns names, and the encoder uses the same
    /// document grammar in the reverse direction.
    pub fn encode_manifest(
        &self,
        manifest: &SchemaManifest,
        decoded: &ManifestSchema,
        names: &mut NameTable,
    ) -> Result<TextualForm<SchemaLanguage>, TextualError> {
        decoded
            .structure()
            .validate(manifest, decoded.encoded().declarations().len())?;
        let mut chunks = Vec::with_capacity(manifest.files().len());
        for file in manifest.files() {
            let allocation = decoded
                .structure()
                .file(file.path())
                .expect("validated structure has every manifest file");
            let declarations = allocation
                .declaration_positions()
                .iter()
                .map(|position| decoded.encoded().declarations()[*position].clone())
                .collect();
            let file_schema = EncodedSchema::new(declarations);
            chunks.push(structural_codec::TextChunk {
                name: file.path().clone(),
                text: self.encode_document(&file_schema, names)?,
            });
        }
        Ok(TextualForm::from_chunks(chunks))
    }

    /// The evaluator for the document grammar: with the keyword lexicon when the table
    /// carries `Literal` forms, plain otherwise.
    fn document_evaluator(&self) -> StructuralEvaluator<'_> {
        match &self.lexicon {
            Some(lexicon) => StructuralEvaluator::with_lexicon(&self.table, lexicon),
            None => StructuralEvaluator::new(&self.table),
        }
    }

    /// The canonical empty brace an unmodelled document slot renders to.
    fn empty_brace() -> String {
        Delimiter::Brace.wrap(std::iter::empty::<String>())
    }

    /// The canonical empty interface slot. A manifest library file carries no
    /// protocol root; it is an empty bracket, never a fabricated empty `Input` or
    /// `Output` declaration.
    fn empty_square() -> String {
        Delimiter::SquareBracket.wrap(std::iter::empty::<String>())
    }

    /// Prove a document slot is an empty brace; otherwise a loud, typed slot error.
    fn require_empty_brace(block: &Block, slot: &'static str) -> Result<(), TextualError> {
        match block.as_delimited(Delimiter::Brace) {
            Some([]) => Ok(()),
            _ => Err(TextualError::DocumentSlot(slot)),
        }
    }

    /// Decode one interface-line bracket into its role-tagged enumeration
    /// declaration: the `Name.Payload` entries become the enumeration's variants, and
    /// the declaration takes the role's canonical protocol-line name (`Input` /
    /// `Output`) — the same name legacy ingestion carries, so the two front ends
    /// agree on the interface surface.
    fn decode_interface_slot(
        &self,
        block: &Block,
        role: DeclarationRole,
        names: &mut NameTable,
    ) -> Result<Option<EncodedDeclaration>, TextualError> {
        if matches!(block.as_delimited(Delimiter::SquareBracket), Some([])) {
            return Ok(None);
        }
        let value = self.document_evaluator().decode(INTERFACE, block, names)?;
        let variants = Self::reify_interface_variants(&value)?;
        let name = names.intern(Name::new(
            role.interface_root_name()
                .ok_or(TextualError::ReifyShape("interface role"))?,
        ));
        Ok(Some(EncodedDeclaration::interface(
            role,
            EncodedType::Enumeration(EncodedEnum::new(name, variants)),
        )))
    }

    fn encode_interface_slot(
        &self,
        interface: &EncodedDeclaration,
        names: &mut NameTable,
    ) -> Result<String, TextualError> {
        let EncodedType::Enumeration(enumeration) = interface.value() else {
            return Err(TextualError::ReifyShape("interface root enumeration"));
        };
        let mirror = self.reflect_interface(enumeration, names)?;
        let block = self
            .document_evaluator()
            .encode(INTERFACE, &mirror, names)?;
        Ok(block.canonical_text())
    }

    fn decode_types_slot(
        &self,
        block: &Block,
        names: &mut NameTable,
    ) -> Result<Vec<EncodedDeclaration>, TextualError> {
        let value = self
            .document_evaluator()
            .decode(TYPES_BLOCK, block, names)?;
        self.reify_types(&value, names)
    }

    /// Encode the schema's `types` block — its data declarations only. The interface
    /// roots are rendered by [`encode_interface_slot`](Self::encode_interface_slot)
    /// into their own brackets, never the `types` block.
    fn encode_types_slot(
        &self,
        schema: &EncodedSchema,
        names: &mut NameTable,
    ) -> Result<String, TextualError> {
        let mirror = self.reflect_types(schema.data_declarations(), names)?;
        let block = self
            .document_evaluator()
            .encode(TYPES_BLOCK, &mirror, names)?;
        Ok(block.canonical_text())
    }

    /// Reify the `types` block mirror into the declaration set. Each child is a
    /// `Delegate` over a `Declaration` mirror.
    fn reify_types(
        &self,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<Vec<EncodedDeclaration>, TextualError> {
        let StructuralValue::Chosen { payload, .. } = value else {
            return Err(TextualError::ReifyShape("types block"));
        };
        let StructuralValue::Delimited(declarations) = payload.as_ref() else {
            return Err(TextualError::ReifyShape("types block declarations"));
        };
        let mut result = Vec::with_capacity(declarations.len());
        for declaration in declarations {
            let StructuralValue::Delegated(inner) = declaration else {
                return Err(TextualError::ReifyShape("declaration delegate"));
            };
            result.push(self.reify_declaration(inner, names)?);
        }
        Ok(result)
    }

    /// Reflect a declaration set into the `types` block mirror.
    fn reflect_types<'declaration>(
        &self,
        declarations: impl Iterator<Item = &'declaration EncodedDeclaration>,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let mut mirrors = Vec::new();
        for declaration in declarations {
            let declaration_mirror = self.reflect_declaration(declaration, names)?;
            mirrors.push(StructuralValue::Delegated(Box::new(declaration_mirror)));
        }
        Ok(StructuralValue::chosen(
            0,
            StructuralValue::Delimited(mirrors),
        ))
    }

    /// Reify one `Declaration` mirror, dispatching on the winning grammar constructor
    /// index to a newtype, struct, or enumeration. Every document declaration is
    /// public — the surface carries no visibility marker in this layout.
    fn reify_declaration(
        &self,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<EncodedDeclaration, TextualError> {
        let StructuralValue::Chosen {
            constructor,
            payload,
        } = value
        else {
            return Err(TextualError::ReifyShape("declaration"));
        };
        let constructor = DeclarationConstructor::from_index(*constructor)
            .ok_or(TextualError::ReifyShape("declaration constructor"))?;
        let StructuralValue::Application(head, body) = payload.as_ref() else {
            return Err(TextualError::ReifyShape("declaration application"));
        };
        let StructuralValue::Atom(name) = head.as_ref() else {
            return Err(TextualError::ReifyShape("declaration name"));
        };
        let core_type = match constructor {
            DeclarationConstructor::Newtype => {
                EncodedType::Newtype(EncodedNewtype::new(*name, Self::reify_reference(body)?))
            }
            DeclarationConstructor::Struct => {
                let StructuralValue::Delimited(fields) = body.as_ref() else {
                    return Err(TextualError::ReifyShape("struct fields"));
                };
                let mut core_fields = Vec::with_capacity(fields.len());
                for field in fields {
                    core_fields.push(self.reify_field(field, names)?);
                }
                // A single-field braced body lowers to a newtype canonically, matching
                // the legacy front end (psyche ruling, bead primary-56d1.36).
                EncodedType::from_braced_body(*name, core_fields)
            }
            DeclarationConstructor::Enumeration => {
                let StructuralValue::Delimited(variants) = body.as_ref() else {
                    return Err(TextualError::ReifyShape("enumeration variants"));
                };
                EncodedType::Enumeration(EncodedEnum::new(
                    *name,
                    Self::variants_from_atoms(variants)?,
                ))
            }
        };
        Ok(EncodedDeclaration::public(core_type))
    }

    /// Reflect a [`EncodedDeclaration`] into its `Declaration` mirror.
    fn reflect_declaration(
        &self,
        declaration: &EncodedDeclaration,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let core_type = declaration.value();
        let (constructor, body) = match core_type {
            EncodedType::Newtype(newtype) => {
                let reference_mirror = self.reflect_reference(newtype.reference(), names)?;
                (
                    DeclarationConstructor::Newtype,
                    StructuralValue::Delegated(Box::new(reference_mirror)),
                )
            }
            EncodedType::Struct(structure) => {
                let mut fields = Vec::with_capacity(structure.fields().len());
                for field in structure.fields() {
                    fields.push(self.reflect_field(field, names)?);
                }
                (
                    DeclarationConstructor::Struct,
                    StructuralValue::Delimited(fields),
                )
            }
            EncodedType::Enumeration(enumeration) => (
                DeclarationConstructor::Enumeration,
                StructuralValue::Delimited(Self::variant_atoms(enumeration)?),
            ),
        };
        Ok(StructuralValue::chosen(
            constructor.index(),
            StructuralValue::Application(
                Box::new(StructuralValue::Atom(core_type.identifier())),
                Box::new(body),
            ),
        ))
    }

    /// Reify an interface line mirror into its enumeration variants — the
    /// `Name.Payload` entries that [`decode_interface_slot`](Self::decode_interface_slot)
    /// wraps in the role-tagged interface-root declaration.
    fn reify_interface_variants(
        value: &StructuralValue,
    ) -> Result<Vec<EncodedVariant>, TextualError> {
        let StructuralValue::Chosen { payload, .. } = value else {
            return Err(TextualError::ReifyShape("interface"));
        };
        let StructuralValue::Delimited(entries) = payload.as_ref() else {
            return Err(TextualError::ReifyShape("interface entries"));
        };
        entries.iter().map(Self::reify_interface_variant).collect()
    }

    /// Reify one `Name.Payload` interface entry into a payload-carrying variant.
    fn reify_interface_variant(entry: &StructuralValue) -> Result<EncodedVariant, TextualError> {
        let StructuralValue::Delegated(inner) = entry else {
            return Err(TextualError::ReifyShape("interface entry delegate"));
        };
        let StructuralValue::Chosen { payload, .. } = inner.as_ref() else {
            return Err(TextualError::ReifyShape("interface entry constructor"));
        };
        let StructuralValue::Application(head, reference) = payload.as_ref() else {
            return Err(TextualError::ReifyShape("interface entry application"));
        };
        let StructuralValue::Atom(name) = head.as_ref() else {
            return Err(TextualError::ReifyShape("interface entry name"));
        };
        Ok(EncodedVariant::new(
            *name,
            Some(Self::reify_reference(reference)?),
        ))
    }

    /// Reflect an interface root's enumeration into its interface-line mirror.
    fn reflect_interface(
        &self,
        interface: &EncodedEnum,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let mut entries = Vec::with_capacity(interface.variants().len());
        for variant in interface.variants() {
            entries.push(self.reflect_interface_variant(variant, names)?);
        }
        Ok(StructuralValue::chosen(
            0,
            StructuralValue::Delimited(entries),
        ))
    }

    /// Reflect one interface variant into its `Name.Payload` entry mirror.
    fn reflect_interface_variant(
        &self,
        variant: &EncodedVariant,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let reference = variant
            .payload()
            .ok_or(TextualError::ReifyShape("interface entry payload"))?;
        let reference_mirror = self.reflect_reference(reference, names)?;
        let entry = StructuralValue::chosen(
            0,
            StructuralValue::Application(
                Box::new(StructuralValue::Atom(variant.identifier())),
                Box::new(StructuralValue::Delegated(Box::new(reference_mirror))),
            ),
        );
        Ok(StructuralValue::Delegated(Box::new(entry)))
    }
}

/// `TextualSchema` is the REFERENCE instance of the shared [`TextualForm`] operation:
/// the two organs are its authored structural table (the structuretree) and the
/// caller's `NameTable` (the nametree), and its EncodedForm is a `EncodedType`
/// declaration. The provided `view` / `unview` reproduce this crate's own
/// single-declaration `encode` / `decode` exactly — the operation was generalized OUT
/// of schema, not bolted on — so schema's existing behavior proves the shared shape
/// fits with no change (witnessed by `tests/textual_form.rs`).
impl Textual for TextualSchema {
    type Encoded = EncodedType;
    type Language = SchemaLanguage;
    type Error = TextualError;

    fn structuretree(&self) -> &AddressedStructuralTable {
        &self.table
    }

    fn lexicon(&self) -> Option<&dyn name_table::NameResolver> {
        self.lexicon
            .as_ref()
            .map(|table| table as &dyn name_table::NameResolver)
    }

    fn missing_root_object(&self) -> TextualError {
        TextualError::EmptySource
    }

    fn reify(
        &self,
        expected: ScopedEncodedTypeId,
        mirror: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<EncodedType, TextualError> {
        self.reify_type(expected, mirror, names)
    }

    fn reflect(
        &self,
        _expected: ScopedEncodedTypeId,
        encoded: &EncodedType,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        self.reflect_type(encoded, names)
    }
}

/// The schema language identity — the `T` shared by schema's truth side
/// ([`EncodedForm`] for [`EncodedSchema`]), its view side ([`Textual`] for
/// [`TextualSchema`] producing a `TextualForm<SchemaLanguage>`), and any conversion off
/// the schema layer. A stringless marker; it carries no runtime value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaLanguage;

/// [`EncodedSchema`] is the reference [`EncodedForm`] of the Protos pairing: the whole-
/// language stringless truth a [`Textual`] mouth views and an `EncodedConversion` (the
/// schema→logos lowering in `core-nomos`) moves. Its language identity is
/// [`SchemaLanguage`].
impl EncodedForm for EncodedSchema {
    type Language = SchemaLanguage;
}
