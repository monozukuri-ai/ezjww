use super::{JwsDocument, JwsError};
use crate::{
    dxf::{convert_view_with_options, ConversionLayer, ConversionView},
    model::metadata_setting_from_text,
    ConvertOptions, Coord2D, DxfDocument, DxfTargetVersion,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwsConversionReport {
    pub coordinate_space: &'static str,
    pub origin: Coord2D,
    pub group_scales: [f64; 16],
    pub rendering_policy: &'static str,
    pub omitted_image_references: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwsDxfConversion {
    #[serde(flatten)]
    pub document: DxfDocument,
    pub jws_conversion_report: JwsConversionReport,
}
impl JwsDxfConversion {
    pub fn to_dxf_string(&self, target: DxfTargetVersion) -> String {
        crate::dxf::document_to_string_with_adapter_metadata(&self.document, target, &[], Some(4))
    }
    pub fn write_to_file(
        &self,
        path: impl AsRef<std::path::Path>,
        target: DxfTargetVersion,
    ) -> std::io::Result<()> {
        std::fs::write(path, self.to_dxf_string(target))
    }
}
/// Convert in the original paper millimeters, just like the JWW reader. Reading
/// a symbol does not insert it into a target drawing: its origin and group scale
/// metadata are preserved in the report, never silently applied to coordinates.
pub fn convert_jws_document(
    doc: &JwsDocument,
    options: ConvertOptions,
) -> Result<JwsDxfConversion, JwsError> {
    if options.max_block_nesting == 0
        || !options.text_em_scale.is_finite()
        || options.text_em_scale <= 0.
    {
        return Err(JwsError::invalid(0, "invalid DXF conversion options"));
    }
    let layers = [[ConversionLayer {
        name: "",
        frozen: false,
        locked: false,
    }; 16]; 16];
    let mut document = convert_view_with_options(
        &ConversionView {
            entities: &doc.entities,
            block_defs: &doc.block_defs,
            layers: &layers,
            palette: None,
            line_types: None,
            pens: None,
            // Image control strings are source metadata, not visible labels.
            is_metadata_text: |text| {
                metadata_setting_from_text(text).is_some() || text.image_reference().is_some()
            },
            include_temporary_points: false,
        },
        options,
    );
    let omitted_image_references = doc.entities.iter()
        .chain(doc.block_defs.iter().flat_map(|block| &block.entities))
        .filter(|entity| matches!(entity, crate::Entity::Text(text) if text.image_reference().is_some()))
        .count();
    // Use the existing audit path so omitted image placements are visible to
    // API/CLI callers, including when the reference belongs to a block.
    document.unsupported_entities.extend(std::iter::repeat_n(
        "IMAGE_REFERENCE".to_string(),
        omitted_image_references,
    ));
    Ok(JwsDxfConversion {
        document,
        jws_conversion_report: JwsConversionReport {
            coordinate_space: "paper_millimeters",
            origin: doc.header.origin,
            group_scales: doc.header.group_scales,
            omitted_image_references,
            rendering_policy: concat!(
                "Stored coordinates; insertion origin and group scales are not applied. ",
                "Generated layer names and default pen colors/line types are used because ",
                "JWS does not store drawing settings. Image references are retained in the ",
                "source document; bitmap contents are not loaded or exported."
            ),
        },
    })
}
