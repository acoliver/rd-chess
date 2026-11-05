#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::GameUi;
use glam::Vec2;
use std::collections::BTreeMap;
use worldrunner::{
    simulation::{ComponentDesc, ComponentHandle, WorldError},
    ui::{FontBook, HorizontalAlign, TextRun, TextSpan, VerticalAlign},
};

const HEADING: [f32; 4] = [1., 0.78, 0.35, 1.];
const MOVE: [f32; 4] = [0.45, 0.9, 1., 1.];

#[derive(Clone, Copy)]
pub(super) enum Flow {
    Fixed,
    Scroll,
}
pub(super) struct Paragraph {
    source: TextRun,
    flow: Flow,
    truncated: bool,
}
pub(super) type Paragraphs = BTreeMap<ComponentHandle, Paragraph>;

impl GameUi {
    pub(super) fn paragraph(
        &mut self,
        handle: ComponentHandle,
        text: &str,
        moves: &[String],
        heading_end: usize,
        flow: Flow,
    ) -> Result<(), WorldError> {
        let ComponentDesc::Text(mut run) = self.native.world.component(handle)?.desc.clone() else {
            unreachable!("paragraph text");
        };
        text.clone_into(&mut run.text);
        run.spans = spans(text, moves, heading_end);
        run.horizontal = HorizontalAlign::Left;
        run.vertical = VerticalAlign::Top;
        run.line_height = Some(run.font_size * 1.4);
        run.paragraph_spacing = 4.;
        self.native
            .world
            .set_component(handle, ComponentDesc::Text(run.clone()))?;
        self.paragraphs.insert(
            handle,
            Paragraph {
                source: run,
                flow,
                truncated: false,
            },
        );
        Ok(())
    }

    pub(super) fn fit_paragraphs(&mut self, viewport: [u32; 2]) -> Result<(), WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.fit_study_background(&layout)?;
        for (&handle, paragraph) in &mut self.paragraphs {
            let object = self.native.world.component(handle)?.object;
            let Some(rect) = layout.rect(object) else {
                continue;
            };
            let source = &paragraph.source;
            let measured = self.native.fonts.layout(source, rect.size)?;
            let run = match paragraph.flow {
                Flow::Scroll => {
                    let handle = self.native.rect_handle(object);
                    let ComponentDesc::RectTransform(mut transform) =
                        self.native.world.component(handle)?.desc.clone()
                    else {
                        unreachable!("paragraph rectangle");
                    };
                    transform.size_delta.y = measured.content_height.ceil();
                    self.native
                        .world
                        .set_component(handle, ComponentDesc::RectTransform(transform))?;
                    source.clone()
                }
                Flow::Fixed => {
                    let run = fit(&self.native.fonts, source, rect.size)?;
                    let truncated = run.text != source.text;
                    if truncated && !paragraph.truncated {
                        eprintln!(
                            "Teaching paragraph exceeds its panel; showing a prefix with ... (no text paging control)."
                        );
                    }
                    paragraph.truncated = truncated;
                    run
                }
            };
            self.native
                .world
                .set_component(handle, ComponentDesc::Text(run))?;
        }
        Ok(())
    }
}

fn spans(text: &str, moves: &[String], heading_end: usize) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    if heading_end > 0 {
        spans.push(TextSpan {
            range: 0..heading_end,
            color: HEADING,
        });
    }
    let mut start = heading_end;
    for token in text[heading_end..].split_inclusive(|c: char| c.is_whitespace()) {
        let word = token
            .trim()
            .trim_end_matches(['.', ',', ':', ';', '!', '?']);
        // Check/mate suffixes belong to SAN; punctuation around the token does not.
        let san = moves
            .iter()
            .find(|san| token.trim().trim_end_matches(['.', ',', ':', ';']) == san.as_str())
            .or_else(|| moves.iter().find(|san| word == san.as_str()));
        if let Some(san) = san {
            spans.push(TextSpan {
                range: start..start + san.len(),
                color: MOVE,
            });
        }
        start += token.len();
    }
    spans
}

fn prefix(source: &TextRun, end: usize) -> TextRun {
    let mut run = source.clone();
    run.text = format!("{}...", source.text[..end].trim_end());
    let end = run.text.len() - 3;
    run.spans.retain(|span| span.range.start < end);
    for span in &mut run.spans {
        span.range.end = span.range.end.min(end);
    }
    run
}

fn fits(fonts: &FontBook, run: &TextRun, size: Vec2) -> Result<bool, WorldError> {
    let layout = fonts.layout(run, size)?;
    Ok(layout.content_height <= size.y && layout.lines.iter().all(|line| line.width <= size.x))
}

fn fit(fonts: &FontBook, source: &TextRun, size: Vec2) -> Result<TextRun, WorldError> {
    if fits(fonts, source, size)? {
        return Ok(source.clone());
    }
    let boundaries: Vec<_> = source.text.char_indices().map(|(byte, _)| byte).collect();
    let (mut low, mut high) = (0, boundaries.len());
    while low < high {
        let mid = low + (high - low) / 2;
        if fits(fonts, &prefix(source, boundaries[mid]), size)? {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low > 0 {
        return Ok(prefix(source, boundaries[low - 1]));
    }
    let mut empty = source.clone();
    empty.text.clear();
    empty.spans.clear();
    Ok(empty)
}
