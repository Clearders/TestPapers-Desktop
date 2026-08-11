use super::{
    attachment_extension, ordered_items, safe_filename, AttachmentSource, ExportArtifact,
    ExportError, ExportOptions, LayoutDensity,
};
use crate::workspace_features::hash::sha256;
use crate::workspace_features::paper::{PaperSnapshot, QuestionType};
use crate::workspace_features::zip_store::{write_stored_zip, ZipEntry};
use std::collections::BTreeMap;

struct EmbeddedImage {
    relationship_id: String,
    package_path: String,
    extension: &'static str,
    media_type: String,
    bytes: Vec<u8>,
}

pub(crate) fn build_docx(
    paper: &PaperSnapshot,
    options: ExportOptions,
    attachments: &dyn AttachmentSource,
) -> Result<ExportArtifact, ExportError> {
    paper.validate().map_err(ExportError::InvalidPaper)?;
    let items = ordered_items(paper, options.question_order);
    let density = options.layout_density.resolved(items.len());
    let mut images = BTreeMap::<String, EmbeddedImage>::new();
    for item in &items {
        for attachment in &item.question_snapshot.attachments {
            if let Some(existing) = images.get(&attachment.blob_hash) {
                if existing.media_type != attachment.media_type {
                    return Err(ExportError::InvalidAttachment(
                        "the same blob hash has conflicting media types",
                    ));
                }
                continue;
            }
            let extension = attachment_extension(&attachment.media_type)?;
            let bytes = attachments
                .load_blob(&attachment.blob_hash)
                .map_err(|detail| ExportError::MissingAttachment {
                    hash: attachment.blob_hash.clone(),
                    detail,
                })?;
            if sha256(&bytes).to_hex() != attachment.blob_hash {
                return Err(ExportError::InvalidAttachment(
                    "blob content does not match its declared SHA-256 hash",
                ));
            }
            let number = images.len() + 1;
            images.insert(
                attachment.blob_hash.clone(),
                EmbeddedImage {
                    relationship_id: format!("rId{number}"),
                    package_path: format!("word/media/{}.{}", attachment.blob_hash, extension),
                    extension,
                    media_type: attachment.media_type.clone(),
                    bytes,
                },
            );
        }
    }

    let document_xml = document_xml(paper, &items, options.include_answers, density, &images);
    let content_types = content_types_xml(&images);
    let relationships = document_relationships_xml(&images);
    let mut entries = vec![
        ZipEntry::new("[Content_Types].xml", content_types.into_bytes()),
        ZipEntry::new("_rels/.rels", package_relationships_xml().into_bytes()),
        ZipEntry::new("word/document.xml", document_xml.into_bytes()),
        ZipEntry::new("word/_rels/document.xml.rels", relationships.into_bytes()),
    ];
    entries.extend(
        images
            .values()
            .map(|image| ZipEntry::new(image.package_path.clone(), image.bytes.clone())),
    );
    let bytes =
        write_stored_zip(&entries).map_err(|error| ExportError::Archive(error.to_string()))?;
    Ok(ExportArtifact {
        filename: safe_filename(&paper.title, "docx"),
        media_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        bytes,
        companions: vec![],
    })
}

fn document_xml(
    paper: &PaperSnapshot,
    items: &[&crate::workspace_features::paper::PaperItemSnapshot],
    include_answers: bool,
    density: LayoutDensity,
    images: &BTreeMap<String, EmbeddedImage>,
) -> String {
    let (question_size, option_size, spacing_after) = match density {
        LayoutDensity::Normal => (23, 22, 140),
        LayoutDensity::Compact => (21, 19, 70),
        LayoutDensity::Dense => (19, 17, 20),
        LayoutDensity::Auto => unreachable!("density was resolved"),
    };
    let mut body = String::new();
    body.push_str(&paragraph(&paper.title, 32, true, "center", 180));
    body.push_str(&paragraph(
        &format!(
            "Subject: {}    Duration: {} minutes    Total Marks: {}",
            paper.subject, paper.duration_minutes, paper.total_marks
        ),
        22,
        false,
        "center",
        200,
    ));
    let mut current_type = None;
    let mut drawing_id = 1_u32;
    for (display_index, item) in items.iter().enumerate() {
        let question = &item.question_snapshot;
        if current_type != Some(question.question_type) {
            body.push_str(&paragraph(
                question.question_type.label(),
                26,
                true,
                "left",
                100,
            ));
            current_type = Some(question.question_type);
        }
        let marks = item
            .marks
            .as_ref()
            .map(|marks| format!(" ({marks} marks)"))
            .unwrap_or_default();
        body.push_str(&paragraph(
            &format!("{}. {}{marks}", display_index + 1, question.text),
            question_size,
            true,
            "left",
            spacing_after,
        ));
        if let Some(options) = &question.options {
            if density == LayoutDensity::Dense {
                let text = options
                    .iter()
                    .enumerate()
                    .map(|(index, option)| format!("{}. {option}", option_label(index)))
                    .collect::<Vec<_>>()
                    .join("    ");
                body.push_str(&paragraph(&text, option_size, false, "left", spacing_after));
            } else {
                for (index, option) in options.iter().enumerate() {
                    body.push_str(&paragraph(
                        &format!("    {}. {option}", option_label(index)),
                        option_size,
                        false,
                        "left",
                        spacing_after,
                    ));
                }
            }
        }
        for attachment in &question.attachments {
            if let Some(image) = images.get(&attachment.blob_hash) {
                body.push_str(&image_paragraph(
                    &image.relationship_id,
                    drawing_id,
                    attachment
                        .caption
                        .as_deref()
                        .unwrap_or(&attachment.filename),
                ));
                drawing_id += 1;
            }
        }
        if include_answers {
            body.push_str(&colored_paragraph(
                &format!("Answer: {}", question.answer.display_text()),
                20,
                "0000FF",
                spacing_after,
            ));
        } else if question.question_type == QuestionType::Essay {
            let blank_lines = question
                .essay_blank_space
                .as_ref()
                .map_or(6, |space| space.lines)
                .clamp(1, 20);
            let scale = match density {
                LayoutDensity::Normal => 1.0,
                LayoutDensity::Compact => 0.75,
                LayoutDensity::Dense => 0.55,
                LayoutDensity::Auto => unreachable!(),
            };
            for _ in 0..((f32::from(blank_lines) * scale).ceil() as usize).max(1) {
                body.push_str(&paragraph(" ", 20, false, "left", 240));
            }
        }
    }
    body.push_str(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#,
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}</w:body></w:document>"#
    )
}

fn paragraph(text: &str, size: u16, bold: bool, align: &str, spacing_after: u16) -> String {
    let bold_xml = if bold { "<w:b/>" } else { "" };
    let runs = text
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let break_xml = if index == 0 { "" } else { "<w:br/>" };
            format!(
                r#"<w:r><w:rPr>{bold_xml}<w:sz w:val="{size}"/><w:szCs w:val="{size}"/></w:rPr>{break_xml}<w:t xml:space="preserve">{}</w:t></w:r>"#,
                xml_escape(line)
            )
        })
        .collect::<String>();
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{align}"/><w:spacing w:after="{spacing_after}"/></w:pPr>{runs}</w:p>"#
    )
}

fn colored_paragraph(text: &str, size: u16, color: &str, spacing_after: u16) -> String {
    format!(
        r#"<w:p><w:pPr><w:spacing w:after="{spacing_after}"/></w:pPr><w:r><w:rPr><w:i/><w:color w:val="{color}"/><w:sz w:val="{size}"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
        xml_escape(text)
    )
}

fn image_paragraph(relationship_id: &str, document_id: u32, caption: &str) -> String {
    let caption = xml_escape(caption);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5029200" cy="2829150"/><wp:docPr id="{document_id}" name="{caption}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="{caption}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{relationship_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="5029200" cy="2829150"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

fn content_types_xml(images: &BTreeMap<String, EmbeddedImage>) -> String {
    let mut defaults = BTreeMap::<&str, &str>::new();
    for image in images.values() {
        defaults.insert(image.extension, image.media_type.as_str());
    }
    let image_defaults = defaults
        .into_iter()
        .map(|(extension, media_type)| {
            format!(
                r#"<Default Extension="{extension}" ContentType="{}"/>"#,
                xml_escape(media_type)
            )
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{image_defaults}<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#
    )
}

fn package_relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.into()
}

fn document_relationships_xml(images: &BTreeMap<String, EmbeddedImage>) -> String {
    let relationships = images
        .values()
        .map(|image| {
            let target = image.package_path.strip_prefix("word/").expect("word media path");
            format!(
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>"#,
                image.relationship_id,
                xml_escape(target)
            )
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn option_label(index: usize) -> char {
    char::from_u32('A' as u32 + index as u32).unwrap_or('?')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_features::export::{ExportOptions, NoAttachments};
    use crate::workspace_features::paper::{
        AnswerSnapshot, Difficulty, PaperItemSnapshot, PaperStatus, QuestionSnapshot,
        ReplicationScope, PAPER_SCHEMA_VERSION,
    };
    use crate::workspace_features::zip_store::{read_stored_zip, ZipLimits};

    fn paper() -> PaperSnapshot {
        PaperSnapshot {
            id: "018f0000-0000-7000-8000-000000000001".into(),
            owner_id: "018f0000-0000-7000-8000-000000000002".into(),
            replication_scope: ReplicationScope::LocalPrivate,
            schema_version: PAPER_SCHEMA_VERSION,
            version: 1,
            content_hash: "a".repeat(64),
            created_at_micros: 1,
            updated_at_micros: 1,
            deleted_at_micros: None,
            title: "Exam".into(),
            subject: "Math & Logic".into(),
            duration_minutes: 60,
            total_marks: "10".into(),
            status: PaperStatus::Draft,
            items: vec![PaperItemSnapshot {
                id: "018f0000-0000-7000-8000-000000000003".into(),
                question_id: Some("018f0000-0000-7000-8000-000000000004".into()),
                order: 0,
                marks: Some("10".into()),
                question_snapshot: QuestionSnapshot {
                    id: "018f0000-0000-7000-8000-000000000004".into(),
                    version: 1,
                    content_hash: "b".repeat(64),
                    question_type: QuestionType::Essay,
                    subjects: vec!["Math".into()],
                    difficulty: Difficulty::Hard,
                    tags: vec![],
                    text: "Prove x < y & y > 0".into(),
                    options: None,
                    answer: AnswerSnapshot::Text("By induction".into()),
                    has_latex: false,
                    source: None,
                    essay_blank_space: Some(Default::default()),
                    score_weight: "1".into(),
                    attachments: vec![],
                },
            }],
        }
    }

    #[test]
    fn emits_a_real_docx_package_with_semantic_content() {
        let artifact = build_docx(
            &paper(),
            ExportOptions {
                include_answers: true,
                ..ExportOptions::default()
            },
            &NoAttachments,
        )
        .unwrap();
        let entries = read_stored_zip(&artifact.bytes, ZipLimits::default()).unwrap();
        let document = entries
            .iter()
            .find(|entry| entry.path == "word/document.xml")
            .unwrap();
        let document = String::from_utf8(document.bytes.clone()).unwrap();
        assert!(document.contains("Prove x &lt; y &amp; y &gt; 0"));
        assert!(document.contains("Answer: By induction"));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "[Content_Types].xml"));
    }
}
