//! `TextualSchema` — the first real Textual form: schema text ⇄ `CoreSchema`.
//!
//! Decoding recognizes source text into a raw `Block` (raw-discovery), runs
//! `structural-codec`'s trusted evaluator over the authored table to a generic
//! `StructuralValue`, then REIFIES that mirror into a real stringless `CoreType`
//! declaration with a real `NameTable`. Encoding REFLECTS a `CoreType` back into a
//! `StructuralValue`, lets the evaluator render it to a `Block`, and writes the
//! canonical text. The parser never classifies: the expected Core type drives the
//! evaluator, and reification reads only the mirror.
//!
//! The `Field` disjoint alternatives — an elided name derived from the type versus
//! an explicit `name.Type` — are handled here against the real Core layout: on
//! encode a field elides its name exactly when it equals the `snake_case` of its
//! referenced type (name-table's derived-name rule); on decode the elided name is
//! re-derived, never stored.

use name_table::NameTable;
use raw_discovery::Recognizer;
use structural_codec::ids::ScopedCoreTypeId;
use structural_codec::table::AddressedStructuralTable;
use structural_codec::value::StructuralValue;
use structural_codec::{CanonicalText, StructuralEvaluator};

use crate::declaration::{CoreField, CoreNewtype, CoreStruct, CoreType};
use crate::error::TextualError;
use crate::fixture::FixtureFamily;
use crate::reference::CoreReference;
use crate::universe::CoreUniverse;

/// A Textual view over one Core universe: the authored structural table plus the
/// universe it targets. One codec, both directions.
#[derive(Clone, Debug)]
pub struct TextualSchema {
    universe: CoreUniverse,
    table: AddressedStructuralTable,
}

impl TextualSchema {
    /// Build the Textual view for the fixture family with its standard table.
    pub fn fixture() -> Result<Self, TextualError> {
        let family = FixtureFamily::build();
        let table = family.standard_table()?;
        Ok(Self {
            universe: family.universe().clone(),
            table,
        })
    }

    /// Build a Textual view from an explicit universe and authored table.
    pub fn new(universe: CoreUniverse, table: AddressedStructuralTable) -> Self {
        Self { universe, table }
    }

    pub fn universe(&self) -> &CoreUniverse {
        &self.universe
    }

    pub fn table(&self) -> &AddressedStructuralTable {
        &self.table
    }

    /// Decode one declaration's schema text into a real `CoreType`, interning names
    /// into `names`. The expected type drives the evaluator; the raw layer only
    /// discovered structure.
    pub fn decode(
        &self,
        expected: ScopedCoreTypeId,
        text: &str,
        names: &mut NameTable,
    ) -> Result<CoreType, TextualError> {
        let document = Recognizer::standard().recognize(text)?;
        let block = document
            .root_object_at(0)
            .ok_or(TextualError::EmptySource)?;
        let evaluator = StructuralEvaluator::new(&self.table);
        let value = evaluator.decode(expected, block, names)?;
        self.reify(expected, &value, names)
    }

    // The reification helpers below take the names table mutably: an elided field
    // name is derived and interned on demand (never stored in the Core), so decode
    // can add it to the same table the type names were interned into.

    /// Encode a real `CoreType` back into canonical schema text, resolving names
    /// through `names` (interning any scalar keyword the value needs).
    pub fn encode(
        &self,
        expected: ScopedCoreTypeId,
        value: &CoreType,
        names: &mut NameTable,
    ) -> Result<String, TextualError> {
        let mirror = self.reflect(value, names)?;
        let evaluator = StructuralEvaluator::new(&self.table);
        let block = evaluator.encode(expected, &mirror, names)?;
        Ok(block.canonical_text())
    }

    // ===== reification: StructuralValue -> CoreType =====

    fn reify(
        &self,
        expected: ScopedCoreTypeId,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<CoreType, TextualError> {
        match self.universe.core_type(expected) {
            Some(CoreType::Newtype(_)) => self.reify_newtype(value, names),
            Some(CoreType::Struct(_)) => self.reify_struct(value, names),
            Some(CoreType::Enumeration(_)) => Err(TextualError::ReifyShape("enumeration")),
            None => Err(TextualError::ReifyShape("non-declaration expected type")),
        }
    }

    fn reify_newtype(
        &self,
        value: &StructuralValue,
        names: &NameTable,
    ) -> Result<CoreType, TextualError> {
        let (name, body) = Self::declaration_head(value, "newtype")?;
        let inner = match body {
            [StructuralValue::Atom(inner)] => *inner,
            _ => return Err(TextualError::ReifyShape("newtype body")),
        };
        let reference = self.reference_from_atom(inner, names)?;
        Ok(CoreType::Newtype(CoreNewtype::new(name, reference)))
    }

    fn reify_struct(
        &self,
        value: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<CoreType, TextualError> {
        let (name, body) = Self::declaration_head(value, "struct")?;
        // The body slice borrows `value`, not `names`, so interning per field is free
        // of a borrow conflict.
        let body: Vec<StructuralValue> = body.to_vec();
        let mut fields = Vec::with_capacity(body.len());
        for field_value in &body {
            fields.push(self.reify_field(field_value, names)?);
        }
        Ok(CoreType::Struct(CoreStruct::new(name, fields)))
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
    ) -> Result<CoreField, TextualError> {
        let StructuralValue::Delegated(inner) = field_value else {
            return Err(TextualError::ReifyShape("struct field delegate"));
        };
        let StructuralValue::Chosen {
            constructor,
            payload,
        } = inner.as_ref()
        else {
            return Err(TextualError::ReifyShape("struct field constructor"));
        };
        match (constructor, payload.as_ref()) {
            // Elided name: the payload is the type atom; the name is derived and
            // interned on demand — never stored in the Core.
            (0, StructuralValue::Atom(type_id)) => {
                let reference = self.reference_from_atom(*type_id, names)?;
                let derived = reference.derived_field_name(names)?;
                let identifier = names.intern(name_table::Name::new(derived));
                Ok(CoreField::new(identifier, reference))
            }
            // Explicit name: `Application(Atom(name), Atom(type))`.
            (1, StructuralValue::Application(name, type_atom)) => {
                let StructuralValue::Atom(name_id) = name.as_ref() else {
                    return Err(TextualError::ReifyShape("named field name"));
                };
                let StructuralValue::Atom(type_id) = type_atom.as_ref() else {
                    return Err(TextualError::ReifyShape("named field type"));
                };
                let reference = self.reference_from_atom(*type_id, names)?;
                Ok(CoreField::new(*name_id, reference))
            }
            _ => Err(TextualError::ReifyShape("struct field alternative")),
        }
    }

    fn reference_from_atom(
        &self,
        type_id: name_table::Identifier,
        names: &NameTable,
    ) -> Result<CoreReference, TextualError> {
        let name = names.resolve(type_id)?;
        Ok(CoreReference::from_type_name(name, type_id))
    }

    // ===== reflection: CoreType -> StructuralValue =====

    fn reflect(
        &self,
        value: &CoreType,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        match value {
            CoreType::Newtype(newtype) => self.reflect_newtype(newtype, names),
            CoreType::Struct(structure) => self.reflect_struct(structure, names),
            CoreType::Enumeration(_) => Err(TextualError::ReifyShape("enumeration encode")),
        }
    }

    fn reflect_newtype(
        &self,
        newtype: &CoreNewtype,
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
        structure: &CoreStruct,
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
        field: &CoreField,
        names: &mut NameTable,
    ) -> Result<StructuralValue, TextualError> {
        let type_id = field
            .reference()
            .type_atom_identifier(names)
            .ok_or(TextualError::ReifyShape("field type reference"))?;
        let chosen = if field.name_is_derivable(names)? {
            // The name equals the type's snake_case — elide it.
            StructuralValue::chosen(0, StructuralValue::Atom(type_id))
        } else {
            StructuralValue::chosen(
                1,
                StructuralValue::Application(
                    Box::new(StructuralValue::Atom(field.identifier())),
                    Box::new(StructuralValue::Atom(type_id)),
                ),
            )
        };
        Ok(StructuralValue::Delegated(Box::new(chosen)))
    }
}
