use std::path::Path;

use serde::Serialize;

use super::normalize::invalid;
use super::{
    normalize_jwc_document, read_jwc_document_from_file, JwcConversionError, JwcConversionReport,
    JwcCoordinateSpace, JwcDocument,
};
use crate::dxf::{
    convert_view_with_options, document_to_string_with_adapter_metadata, ConversionLayer,
    ConversionView,
};
use crate::{ConvertOptions, DxfDocument, DxfEntity, DxfTargetVersion, Entity};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct JwcConvertOptions {
    pub coordinates: JwcCoordinateSpace,
    pub dxf: ConvertOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcDxfConversion {
    pub document: DxfDocument,
    pub report: JwcConversionReport,
    /// Width/height ratios in TEXT order, consumed by this result's writer.
    pub text_width_factors: Vec<f64>,
}

impl JwcDxfConversion {
    /// Preserve JWC text aspect ratios (group 41) and declare millimeter units.
    pub fn to_dxf_string(&self, target: DxfTargetVersion) -> String {
        document_to_string_with_adapter_metadata(
            &self.document,
            target,
            &self.text_width_factors,
            Some(4), // $INSUNITS = millimeters for both coordinate spaces.
        )
    }

    pub fn write_to_file(
        &self,
        path: impl AsRef<Path>,
        target: DxfTargetVersion,
    ) -> std::io::Result<()> {
        std::fs::write(path, self.to_dxf_string(target))
    }
}

pub fn convert_jwc_document(
    doc: &JwcDocument,
    options: JwcConvertOptions,
) -> Result<JwcDxfConversion, JwcConversionError> {
    if options.dxf.max_block_nesting == 0 {
        return Err(JwcConversionError::InvalidOptions(
            "max_block_nesting must be >= 1".into(),
        ));
    }
    let normalized = normalize_jwc_document(doc, options.coordinates)?;
    let layers = std::array::from_fn(|g| {
        std::array::from_fn(|l| {
            let layer = &normalized.layers[g][l];
            ConversionLayer {
                name: &layer.name,
                frozen: layer.frozen,
                locked: layer.locked,
            }
        })
    });
    let mut document = convert_view_with_options(
        &ConversionView {
            entities: &normalized.entities,
            block_defs: &[],
            layers: &layers,
            palette: None,
            is_metadata_text: |_| false,
            include_temporary_points: true,
        },
        // The supported JWC profile contains no inserts. Expanding an identity
        // transform would tessellate ellipses in the shared block path; there
        // is no block operation to perform here, so retain exact conics.
        ConvertOptions {
            explode_inserts: false,
            ..options.dxf
        },
    );
    if document.entities.len() != normalized.entities.len()
        || !document.unsupported_entities.is_empty()
    {
        return Err(invalid(
            None,
            "shared converter did not preserve every source record",
        ));
    }
    let mut text_width_factors = Vec::new();
    for (index, (entity, converted)) in normalized
        .entities
        .iter()
        .zip(&mut document.entities)
        .enumerate()
    {
        // Explicit JWC rendering policy. A numeric JWW pen/style identity is
        // not enough to reproduce the reference DXF colors and line patterns.
        let color = normalized.report.aci_colors[usize::from(entity.base().pen_color - 1)];
        let style = match entity.base().pen_style {
            1 => "CONTINUOUS",
            2 => "JWC_DASHED1",
            6 => "JWC_DASHED2",
            _ => return Err(invalid(Some(index), "unknown normalized line style")),
        };
        match converted {
            DxfEntity::Line(v) => {
                v.color = color;
                v.line_type = style.into();
            }
            DxfEntity::Circle(v) => {
                v.color = color;
                v.line_type = style.into();
            }
            DxfEntity::Arc(v) => {
                v.color = color;
                v.line_type = style.into();
            }
            DxfEntity::Ellipse(v) => {
                v.color = color;
                v.line_type = style.into();
            }
            DxfEntity::Point(v) => {
                v.color = color;
                v.line_type = style.into();
            }
            DxfEntity::Text(v) => {
                v.color = color;
                v.line_type = style.into();
                let Entity::Text(text) = entity else {
                    return Err(invalid(Some(index), "text mapping mismatch"));
                };
                v.width_factor = text.size_x / text.size_y;
                text_width_factors.push(v.width_factor);
            }
            _ => return Err(invalid(Some(index), "unexpected converted entity kind")),
        }
    }
    // No input layer color was recovered; the table uses a neutral rendering
    // default. Every entity above has an explicit ACI color.
    for layer in &mut document.layers {
        layer.color = 7;
    }
    Ok(JwcDxfConversion {
        document,
        report: normalized.report,
        text_width_factors,
    })
}

pub fn read_jwc_dxf_from_file(
    path: impl AsRef<Path>,
    options: JwcConvertOptions,
) -> Result<JwcDxfConversion, JwcConversionError> {
    convert_jwc_document(&read_jwc_document_from_file(path)?, options)
}
