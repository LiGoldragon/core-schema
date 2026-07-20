//! Experimental, codec-backed candidates for the remaining schema source surface.
//!
//! This module belongs only to the `ProtosSourceFormsProposal` branch. It is not a
//! production `TextualSchema` grammar and it does not authorize Spirit source
//! migration. Its one sealed StructureTree deliberately proves both decode and
//! emission for the two candidates that can be expressed without inventing a name
//! carrier: unit-or-one-payload interface variants and closed streaming relations.

use std::collections::BTreeMap;

use name_table::{Identifier, NameTable};
use raw_discovery::{Delimiter, Recognizer};
use structural_codec::{
    AddressedStructuralTable, CanonicalText, ConstructorCodec, DecodeError, EncodeError,
    EncodedConstructorId, EncodedLayoutIdentity, PositionalSignature, RawProfileIdentity,
    SequenceForm, StructuralEntry, StructuralEvaluator, StructuralForm, StructuralRevision,
    StructuralValue, TableError, TableIdentityPayload,
};

use crate::{ENCODED_UNIVERSE, EncodedReference, EncodedVariant, StreamingRelation};

/// The expected type for a reference in this proposal. Its sole form is a PascalCase
/// atom; the expected position gives its reference meaning.
const REFERENCE: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(240);
/// The first relation slot: an input interface variant reference.
const INPUT_VARIANT_REFERENCE: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(241);
/// The second relation slot: an output interface variant reference.
const OUTPUT_VARIANT_REFERENCE: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(242);
/// A unit-or-one-payload interface alternative.
const INTERFACE_VARIANT: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(243);
/// The bracketed ordered interface-alternative list.
const INTERFACE_VARIANTS: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(244);
/// One five-position closed streaming relation.
const STREAMING_RELATION: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(245);
/// The bracketed ordered streaming-relation list proposed as the document's trailing slot.
const STREAMING_RELATIONS: structural_codec::ScopedEncodedTypeId =
    structural_codec::ScopedEncodedTypeId::fixture(246);

/// A source-surface candidate failure. Every shape failure names the expected typed
/// position; no fallback interprets a raw atom by content.
#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Recognize(#[from] raw_discovery::RecognizeError),
    #[error("candidate mirror did not fit the expected {0} position")]
    Shape(&'static str),
}

/// The one experimental, sealed StructureTree. Both directions below use this same
/// table; a source candidate cannot have a separate printer path.
#[derive(Clone, Debug)]
pub struct SourceSurfaceCandidates {
    table: AddressedStructuralTable,
}

impl SourceSurfaceCandidates {
    /// Build the experimental table from expected-type entries. It has no literal
    /// lexicon: every name atom is interpreted only by the typed position it occupies.
    pub fn build() -> Result<Self, CandidateError> {
        let entries = [
            Self::single(REFERENCE, StructuralForm::pascal_atom()),
            Self::single(INPUT_VARIANT_REFERENCE, StructuralForm::pascal_atom()),
            Self::single(OUTPUT_VARIANT_REFERENCE, StructuralForm::pascal_atom()),
            Self::interface_variant_entry(),
            Self::single(
                INTERFACE_VARIANTS,
                StructuralForm::Delimited {
                    delimiter: Delimiter::SquareBracket,
                    sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(
                        INTERFACE_VARIANT,
                    )),
                },
            ),
            Self::single(
                STREAMING_RELATION,
                StructuralForm::Delimited {
                    delimiter: Delimiter::Brace,
                    sequence: SequenceForm::Product(vec![
                        StructuralForm::Delegate(INPUT_VARIANT_REFERENCE),
                        StructuralForm::Delegate(OUTPUT_VARIANT_REFERENCE),
                        StructuralForm::Delegate(REFERENCE),
                        StructuralForm::Delegate(REFERENCE),
                        StructuralForm::Delegate(REFERENCE),
                    ]),
                },
            ),
            Self::single(
                STREAMING_RELATIONS,
                StructuralForm::Delimited {
                    delimiter: Delimiter::SquareBracket,
                    sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(
                        STREAMING_RELATION,
                    )),
                },
            ),
        ];
        let entries = entries
            .into_iter()
            .map(|entry| (entry.core_type, entry))
            .collect::<BTreeMap<_, _>>();
        let table = AddressedStructuralTable::seal(
            StructuralRevision::new(1),
            TableIdentityPayload {
                core_universe: ENCODED_UNIVERSE,
                core_layout_identity: EncodedLayoutIdentity([0x73; 32]),
                raw_profile_identity: RawProfileIdentity([1; 32]),
                committed_lexicon: b"core-schema-source-surface-candidates".to_vec(),
                leaf_codec_contracts: Vec::new(),
                entries,
            },
        )?;
        table
            .validate_disjoint()
            .map_err(|_| CandidateError::Shape("disjoint candidate forms"))?;
        Ok(Self { table })
    }

    /// Emit the bracketed interface list from its actual encoded variant algebra.
    pub fn emit_interface(
        &self,
        variants: &[EncodedVariant],
        names: &NameTable,
    ) -> Result<String, CandidateError> {
        let values = variants
            .iter()
            .map(|variant| {
                Self::interface_variant_value(variant)
                    .map(|value| StructuralValue::Delegated(Box::new(value)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.emit(
            INTERFACE_VARIANTS,
            Self::chosen(StructuralValue::Delimited(values)),
            names,
        )
    }

    /// Decode the bracketed interface list using that same StructureTree.
    pub fn decode_interface(
        &self,
        source: &str,
        names: &mut NameTable,
    ) -> Result<Vec<EncodedVariant>, CandidateError> {
        let value = self.decode(INTERFACE_VARIANTS, source, names)?;
        let entries = Self::delimited(&value, "interface alternatives")?;
        entries
            .iter()
            .map(|entry| {
                Self::reify_interface_variant(Self::delegated(entry, "interface alternative")?)
            })
            .collect()
    }

    /// Emit the bracketed closed streaming-relation list. Each braced record has five
    /// positional slots: opener, acknowledgement, token, event, and close token.
    pub fn emit_streaming_relations(
        &self,
        relations: &[StreamingRelation],
        names: &NameTable,
    ) -> Result<String, CandidateError> {
        let values = relations
            .iter()
            .map(|relation| {
                Self::streaming_relation_value(relation)
                    .map(|value| StructuralValue::Delegated(Box::new(value)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.emit(
            STREAMING_RELATIONS,
            Self::chosen(StructuralValue::Delimited(values)),
            names,
        )
    }

    /// Decode the bracketed closed streaming-relation list through the same table.
    pub fn decode_streaming_relations(
        &self,
        source: &str,
        names: &mut NameTable,
    ) -> Result<Vec<StreamingRelation>, CandidateError> {
        let value = self.decode(STREAMING_RELATIONS, source, names)?;
        let relations = Self::delimited(&value, "streaming relations")?;
        relations
            .iter()
            .map(|relation| {
                Self::reify_streaming_relation(Self::delegated(relation, "streaming relation")?)
            })
            .collect()
    }

    fn emit(
        &self,
        expected: structural_codec::ScopedEncodedTypeId,
        value: StructuralValue,
        names: &NameTable,
    ) -> Result<String, CandidateError> {
        Ok(self
            .evaluator()
            .encode(expected, &value, names)?
            .canonical_text())
    }

    fn decode(
        &self,
        expected: structural_codec::ScopedEncodedTypeId,
        source: &str,
        names: &mut NameTable,
    ) -> Result<StructuralValue, CandidateError> {
        let document = Recognizer::standard().recognize(source)?;
        let block = document
            .root_object_at(0)
            .ok_or(CandidateError::Shape("candidate root"))?;
        Ok(self.evaluator().decode(expected, block, names)?)
    }

    fn evaluator(&self) -> StructuralEvaluator<'_> {
        StructuralEvaluator::new(&self.table)
    }

    fn single(
        core_type: structural_codec::ScopedEncodedTypeId,
        form: StructuralForm,
    ) -> StructuralEntry {
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    fn interface_variant_entry() -> StructuralEntry {
        let unit = StructuralForm::pascal_atom();
        let payload = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delegate(REFERENCE),
        );
        StructuralEntry::new(
            INTERFACE_VARIANT,
            vec![
                ConstructorCodec::new(
                    EncodedConstructorId::new(INTERFACE_VARIANT, 0),
                    vec![unit.clone()],
                    unit,
                    PositionalSignature::default(),
                ),
                ConstructorCodec::new(
                    EncodedConstructorId::new(INTERFACE_VARIANT, 1),
                    vec![payload.clone()],
                    payload,
                    PositionalSignature::default(),
                ),
            ],
        )
    }

    fn chosen(payload: StructuralValue) -> StructuralValue {
        StructuralValue::chosen(0, payload)
    }

    fn reference_value(reference: &EncodedReference) -> Result<StructuralValue, CandidateError> {
        let EncodedReference::Plain(identifier) = reference else {
            return Err(CandidateError::Shape("plain candidate reference"));
        };
        Ok(Self::chosen(StructuralValue::Atom(*identifier)))
    }

    fn interface_variant_value(
        variant: &EncodedVariant,
    ) -> Result<StructuralValue, CandidateError> {
        let payload = match variant.payload() {
            None => StructuralValue::Atom(variant.identifier()),
            Some(reference) => StructuralValue::Application(
                Box::new(StructuralValue::Atom(variant.identifier())),
                Box::new(StructuralValue::Delegated(Box::new(Self::reference_value(
                    reference,
                )?))),
            ),
        };
        Ok(StructuralValue::Chosen {
            constructor: u32::from(variant.payload().is_some()),
            payload: Box::new(payload),
        })
    }

    fn streaming_relation_value(
        relation: &StreamingRelation,
    ) -> Result<StructuralValue, CandidateError> {
        let reference = |reference: &EncodedReference| {
            Self::reference_value(reference)
                .map(|value| StructuralValue::Delegated(Box::new(value)))
        };
        Ok(Self::chosen(StructuralValue::Delimited(vec![
            StructuralValue::Delegated(Box::new(Self::chosen(StructuralValue::Atom(
                relation.opening_input_variant(),
            )))),
            StructuralValue::Delegated(Box::new(Self::chosen(StructuralValue::Atom(
                relation.acknowledgement_output_variant(),
            )))),
            reference(relation.token())?,
            reference(relation.event())?,
            reference(relation.close_token())?,
        ])))
    }

    fn reify_interface_variant(value: &StructuralValue) -> Result<EncodedVariant, CandidateError> {
        let (constructor, payload) = Self::choice(value, "interface alternative")?;
        match constructor {
            0 => Ok(EncodedVariant::new(
                Self::atom(payload, "unit variant")?,
                None,
            )),
            1 => {
                let (head, reference) = Self::application(payload, "payload variant")?;
                Ok(EncodedVariant::new(
                    Self::atom(head, "payload variant name")?,
                    Some(Self::reify_reference(Self::delegated(
                        reference,
                        "payload reference",
                    )?)?),
                ))
            }
            _ => Err(CandidateError::Shape("interface alternative constructor")),
        }
    }

    fn reify_streaming_relation(
        value: &StructuralValue,
    ) -> Result<StreamingRelation, CandidateError> {
        let positions = Self::delimited(value, "streaming relation")?;
        let [opening, acknowledgement, token, event, close_token] = positions else {
            return Err(CandidateError::Shape("five streaming relation positions"));
        };
        Ok(StreamingRelation::new(
            Self::atom(
                Self::choice(
                    Self::delegated(opening, "opening variant")?,
                    "opening variant",
                )?
                .1,
                "opening variant identifier",
            )?,
            Self::atom(
                Self::choice(
                    Self::delegated(acknowledgement, "acknowledgement variant")?,
                    "acknowledgement variant",
                )?
                .1,
                "acknowledgement variant identifier",
            )?,
            Self::reify_reference(Self::delegated(token, "streaming token")?)?,
            Self::reify_reference(Self::delegated(event, "streaming event")?)?,
            Self::reify_reference(Self::delegated(close_token, "streaming close token")?)?,
        ))
    }

    fn reify_reference(value: &StructuralValue) -> Result<EncodedReference, CandidateError> {
        let (_, payload) = Self::choice(value, "reference")?;
        Ok(EncodedReference::Plain(Self::atom(
            payload,
            "reference identifier",
        )?))
    }

    fn choice<'value>(
        value: &'value StructuralValue,
        expected: &'static str,
    ) -> Result<(u32, &'value StructuralValue), CandidateError> {
        let StructuralValue::Chosen {
            constructor,
            payload,
        } = value
        else {
            return Err(CandidateError::Shape(expected));
        };
        Ok((*constructor, payload))
    }

    fn delegated<'value>(
        value: &'value StructuralValue,
        expected: &'static str,
    ) -> Result<&'value StructuralValue, CandidateError> {
        let StructuralValue::Delegated(value) = value else {
            return Err(CandidateError::Shape(expected));
        };
        Ok(value)
    }

    fn delimited<'value>(
        value: &'value StructuralValue,
        expected: &'static str,
    ) -> Result<&'value [StructuralValue], CandidateError> {
        let (_, payload) = Self::choice(value, expected)?;
        let StructuralValue::Delimited(values) = payload else {
            return Err(CandidateError::Shape(expected));
        };
        Ok(values)
    }

    fn application<'value>(
        value: &'value StructuralValue,
        expected: &'static str,
    ) -> Result<(&'value StructuralValue, &'value StructuralValue), CandidateError> {
        let StructuralValue::Application(head, payload) = value else {
            return Err(CandidateError::Shape(expected));
        };
        Ok((head, payload))
    }

    fn atom(value: &StructuralValue, expected: &'static str) -> Result<Identifier, CandidateError> {
        let StructuralValue::Atom(identifier) = value else {
            return Err(CandidateError::Shape(expected));
        };
        Ok(*identifier)
    }
}
