use super::{
    attachment_extension, ordered_items, safe_filename, AttachmentSource, CompanionFile,
    ExportArtifact, ExportError, ExportOptions,
};
use crate::workspace_features::paper::{PaperSnapshot, QuestionType};
use std::collections::BTreeMap;

pub(crate) fn build_tex(
    paper: &PaperSnapshot,
    options: ExportOptions,
    attachments: &dyn AttachmentSource,
) -> Result<ExportArtifact, ExportError> {
    paper.validate().map_err(ExportError::InvalidPaper)?;
    let items = ordered_items(paper, options.question_order);
    let density = options.layout_density.resolved(items.len());
    let item_spacing = match density {
        super::LayoutDensity::Normal => "0.8em",
        super::LayoutDensity::Compact => "0.35em",
        super::LayoutDensity::Dense => "0.15em",
        super::LayoutDensity::Auto => unreachable!("density was resolved"),
    };
    let mut lines = vec![
        r"\documentclass[UTF8,12pt]{ctexart}".into(),
        r"\usepackage[a4paper,margin=2cm]{geometry}".into(),
        r"\usepackage{amsmath,amssymb}".into(),
        r"\usepackage{enumitem}".into(),
        r"\usepackage{graphicx}".into(),
        r"\usepackage{xcolor}".into(),
        r"\setlength{\parindent}{0pt}".into(),
        format!(r"\setlist[enumerate]{{leftmargin=*,itemsep={item_spacing}}}"),
        r"\begin{document}".into(),
        r"\begin{center}".into(),
        format!(r"{{\LARGE\bfseries {}}}\\[0.6em]", tex_text(&paper.title)),
        format!(
            r"{} \quad Duration: {} minutes \quad Total Marks: {}",
            tex_text(&paper.subject),
            paper.duration_minutes,
            tex_text(&paper.total_marks)
        ),
        r"\end{center}".into(),
        r"\vspace{0.8em}".into(),
    ];
    let mut companions = BTreeMap::<String, Vec<u8>>::new();
    let mut current_type = None;
    let mut enumerate_open = false;
    for item in items {
        let question = &item.question_snapshot;
        if current_type != Some(question.question_type) {
            if enumerate_open {
                lines.push(r"\end{enumerate}".into());
            }
            lines.push(format!(
                r"\section*{{{}}}",
                tex_text(question.question_type.label())
            ));
            lines.push(r"\begin{enumerate}".into());
            enumerate_open = true;
            current_type = Some(question.question_type);
        }
        let marks = item
            .marks
            .as_ref()
            .map(|marks| format!(r" \hfill \textit{{[{} marks]}}", tex_text(marks)))
            .unwrap_or_default();
        lines.push(format!(r"\item {}{marks}", tex_text(&question.text)));
        if let Some(options) = &question.options {
            if !options.is_empty() {
                lines.push(r"\begin{enumerate}[label=\Alph*.,itemsep=0.2em]".into());
                lines.extend(
                    options
                        .iter()
                        .map(|option| format!(r"\item {}", tex_text(option))),
                );
                lines.push(r"\end{enumerate}".into());
            }
        }
        for attachment in &question.attachments {
            let extension = attachment_extension(&attachment.media_type)?;
            let relative_path = format!("images/{}.{}", attachment.blob_hash, extension);
            if !companions.contains_key(&relative_path) {
                let bytes = attachments
                    .load_blob(&attachment.blob_hash)
                    .map_err(|detail| ExportError::MissingAttachment {
                        hash: attachment.blob_hash.clone(),
                        detail,
                    })?;
                companions.insert(relative_path.clone(), bytes);
            }
            let caption = attachment
                .caption
                .as_deref()
                .unwrap_or(&attachment.filename);
            lines.extend([
                r"\begin{center}".into(),
                format!(
                    r"\includegraphics[width=0.75\linewidth]{{\detokenize{{{relative_path}}}}}"
                ),
                format!(r"\\{{\small {}}}", tex_text(caption)),
                r"\end{center}".into(),
            ]);
        }
        if options.include_answers {
            lines.push(format!(
                r"\par\textcolor{{blue}}{{\textbf{{Answer:}} {}}}",
                tex_text(&question.answer.display_text())
            ));
        } else if question.question_type == QuestionType::Essay {
            let lines_count = question
                .essay_blank_space
                .as_ref()
                .map_or(6, |space| space.lines)
                .clamp(1, 20);
            lines.push(format!(
                r"\vspace{{{:.2}cm}}",
                f32::from(lines_count) * 0.55
            ));
        }
    }
    if enumerate_open {
        lines.push(r"\end{enumerate}".into());
    }
    lines.extend([r"\end{document}".into(), String::new()]);
    Ok(ExportArtifact {
        filename: safe_filename(&paper.title, "tex"),
        media_type: "application/x-tex",
        bytes: lines.join("\n").into_bytes(),
        companions: companions
            .into_iter()
            .map(|(relative_path, bytes)| CompanionFile {
                relative_path,
                bytes,
            })
            .collect(),
    })
}

fn tex_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    let mut math_delimiter = 0_u8;
    while let Some(character) = characters.next() {
        if character == '$' {
            let width = if characters.peek() == Some(&'$') {
                characters.next();
                2
            } else {
                1
            };
            if math_delimiter == 0 {
                math_delimiter = width;
            } else if math_delimiter == width {
                math_delimiter = 0;
            }
            output.push('$');
            if width == 2 {
                output.push('$');
            }
            continue;
        }
        if math_delimiter != 0 {
            output.push(character);
            continue;
        }
        match character {
            '\\' => output.push_str(r"\textbackslash{}"),
            '{' => output.push_str(r"\{"),
            '}' => output.push_str(r"\}"),
            '#' => output.push_str(r"\#"),
            '$' => output.push_str(r"\$"),
            '%' => output.push_str(r"\%"),
            '&' => output.push_str(r"\&"),
            '_' => output.push_str(r"\_"),
            '^' => output.push_str(r"\textasciicircum{}"),
            '~' => output.push_str(r"\textasciitilde{}"),
            '\n' => output.push_str("\\\\\n"),
            value => output.push(value),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_features::export::{LayoutDensity, QuestionOrder};
    use crate::workspace_features::paper::{
        AnswerSnapshot, Difficulty, PaperItemSnapshot, PaperStatus, QuestionSnapshot,
        ReplicationScope, PAPER_SCHEMA_VERSION,
    };

    fn paper() -> PaperSnapshot {
        let id = "018f0000-0000-7000-8000-000000000001";
        PaperSnapshot {
            id: id.into(),
            owner_id: "018f0000-0000-7000-8000-000000000002".into(),
            replication_scope: ReplicationScope::LocalPrivate,
            schema_version: PAPER_SCHEMA_VERSION,
            version: 1,
            content_hash: "a".repeat(64),
            created_at_micros: 1,
            updated_at_micros: 1,
            deleted_at_micros: None,
            title: "Algebra / Midterm".into(),
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
                    question_type: QuestionType::ShortAnswer,
                    subjects: vec!["Math".into()],
                    difficulty: Difficulty::Medium,
                    tags: vec![],
                    text: "Solve $x_1 & x_2$ then explain 50%.".into(),
                    options: None,
                    answer: AnswerSnapshot::Text("x_1 = 1".into()),
                    has_latex: true,
                    source: None,
                    essay_blank_space: None,
                    score_weight: "1".into(),
                    attachments: vec![],
                },
            }],
        }
    }

    #[test]
    fn preserves_math_and_escapes_plain_tex() {
        let artifact = build_tex(
            &paper(),
            ExportOptions {
                include_answers: true,
                question_order: QuestionOrder::Paper,
                layout_density: LayoutDensity::Normal,
            },
            &super::super::NoAttachments,
        )
        .unwrap();
        let text = String::from_utf8(artifact.bytes).unwrap();
        assert!(text.contains("$x_1 & x_2$"));
        assert!(text.contains("50\\%."));
        assert!(text.contains("Answer:"));
        assert_eq!(artifact.filename, "Algebra Midterm.tex");
    }
}
