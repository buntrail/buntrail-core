use crate::{Text, contracts::*};
use regex::{Regex, RegexBuilder};
#[derive(Clone)]
pub struct FindOptions {
    pub expression: String,
    pub mode: SearchMode,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub scope: Option<BufferRange>,
}
pub const MAX_MATCHES: usize = 20_000;
/// Shared pattern source for file find, workspace process arguments and result remapping.
pub fn pattern_source(options: &FindOptions) -> Result<String, CoreError> {
    if options.expression.is_empty()
        || options.expression.contains('\n')
        || options.expression.len() > 65536
    {
        return Err(CoreError::new(
            ErrorCode::InvalidArgument,
            "Enter a nonempty, single-line search expression (at most 64 KiB)",
        ));
    }
    let expression = match options.mode {
        SearchMode::Literal => regex::escape(&options.expression),
        SearchMode::Regex => options.expression.clone(),
    };
    let expression = if options.whole_word {
        format!(r"\b{{start-half}}(?:{expression})\b{{end-half}}")
    } else {
        expression
    };
    Ok(format!("(?mR:{expression})"))
}
pub fn pattern(options: &FindOptions) -> Result<Regex, CoreError> {
    RegexBuilder::new(&pattern_source(options)?)
        .case_insensitive(!options.case_sensitive)
        .size_limit(8 * 1024 * 1024)
        .build()
        .map_err(|_| {
            CoreError::new(
                ErrorCode::InvalidArgument,
                "Invalid or too complex regular expression",
            )
        })
}
#[derive(Clone)]
pub struct FindPlan {
    pub revision: BufferRevision,
    pub matches: Vec<BufferRange>,
    pub replacements: Vec<Replacement>,
    pub limited: bool,
}
pub fn plan(
    text: &Text,
    revision: BufferRevision,
    options: &FindOptions,
    replacement: &str,
) -> Result<FindPlan, CoreError> {
    let regex = pattern(options)?;
    let scope = options
        .scope
        .as_ref()
        .map(|r| {
            r.to_bytes(&DocumentSnapshot {
                document: DocumentID([0; 16]),
                revision,
                text: std::sync::Arc::new(text.clone()),
                encoding: Encoding::Utf8 { bom: false },
                line_endings: LineEndings::Lf,
                language: None,
            })
        })
        .transpose()?;
    let mut result = FindPlan {
        revision,
        matches: vec![],
        replacements: vec![],
        limited: false,
    };
    let mut replacement_bytes = 0usize;
    let mut offset = 0;
    for line in text.lines() {
        if line.len_bytes() > 16 * 1024 * 1024 {
            return Err(CoreError::new(
                ErrorCode::Unavailable,
                "Find line exceeds 16 MiB; narrow the file before searching",
            ));
        }
        if line.len_bytes() == 0 {
            continue;
        }
        let line = line.to_string();
        let content = line.strip_suffix('\n').unwrap_or(&line);
        for capture in regex.captures_iter(content) {
            let m = capture.get(0).unwrap();
            if !line_match_allowed(&regex, &line, m.start(), m.end()) {
                continue;
            }
            let start = offset + m.start();
            let end = offset + m.end();
            if scope
                .as_ref()
                .is_some_and(|s| start < s.start as usize || end > s.end as usize)
            {
                continue;
            }
            if result.matches.len() == MAX_MATCHES {
                result.limited = true;
                return Ok(result);
            }
            let range = BufferRange {
                revision,
                start: text.byte_to_utf16(start).unwrap() as u64,
                end: text.byte_to_utf16(end).unwrap() as u64,
            };
            let value = replacement_value(
                &capture,
                &options.mode,
                replacement,
                32 * 1024 * 1024 - replacement_bytes,
            )?;
            replacement_bytes += value.len();
            result.matches.push(range.clone());
            result.replacements.push(Replacement { range, text: value });
        }
        offset += line.len();
    }
    Ok(result)
}
pub fn find(
    text: &Text,
    revision: BufferRevision,
    options: &FindOptions,
) -> Result<Vec<BufferRange>, CoreError> {
    Ok(plan(text, revision, options, "")?.matches)
}
/// Resolve against current text, never treating searched byte columns as native offsets.
pub fn resolve(
    text: &Text,
    revision: BufferRevision,
    options: &FindOptions,
    searched: &SearchMatch,
) -> Result<BufferRange, CoreError> {
    let regex = pattern(options)?;
    // Live-path versions are independent observations, not proof of ripgrep's searched bytes.
    let mut candidate = None;
    let mut offset = 0;
    for line in text.lines() {
        if line.len_bytes() > 16 * 1024 * 1024 {
            return Err(CoreError::new(
                ErrorCode::Unavailable,
                "Result line exceeds find limit",
            ));
        }
        let line = line.to_string();
        let content = line.strip_suffix('\n').unwrap_or(&line);
        if line == searched.preview {
            for m in regex.find_iter(content) {
                if line_match_allowed(&regex, &line, m.start(), m.end())
                    && m.start() as u64 == searched.preview_column_utf8
                {
                    let range = BufferRange {
                        revision,
                        start: text.byte_to_utf16(offset + m.start()).unwrap() as u64,
                        end: text.byte_to_utf16(offset + m.end()).unwrap() as u64,
                    };
                    if candidate.is_some() {
                        return Err(changed_result());
                    }
                    candidate = Some(range);
                }
            }
        }
        offset += line.len();
    }
    candidate.ok_or_else(changed_result)
}

fn changed_result() -> CoreError {
    CoreError::new(
        ErrorCode::StaleRevision,
        "Result changed or is ambiguous. Search again.",
    )
}

fn replacement_limit() -> CoreError {
    CoreError::new(
        ErrorCode::Unavailable,
        "Replacement exceeds 32 MiB; narrow the query or selection",
    )
}
fn replacement_parts(
    capture: &regex::Captures<'_>,
    template: &str,
    mut consume: impl FnMut(&str) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    static REFERENCES: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let references =
        REFERENCES.get_or_init(|| Regex::new(r"\$(?:(\$)|\{([^}]*)\}|([0-9A-Za-z_]+))").unwrap());
    let mut cursor = 0;
    for part in references.captures_iter(template) {
        let token = part.get(0).unwrap();
        consume(&template[cursor..token.start()])?;
        if part.get(1).is_some() {
            consume("$")?;
        } else {
            let reference = part.get(2).or_else(|| part.get(3)).unwrap().as_str();
            let value = match reference.parse::<usize>() {
                Ok(index) => capture.get(index),
                Err(_) => capture.name(reference),
            };
            if let Some(value) = value {
                consume(value.as_str())?;
            }
        }
        cursor = token.end();
    }
    consume(&template[cursor..])
}
fn replacement_value(
    capture: &regex::Captures<'_>,
    mode: &SearchMode,
    template: &str,
    remaining: usize,
) -> Result<String, CoreError> {
    let literal = *mode == SearchMode::Literal || !template.contains('$');
    let mut length = 0usize;
    let mut measure = |part: &str| {
        length = length
            .checked_add(part.len())
            .filter(|n| *n <= remaining)
            .ok_or_else(replacement_limit)?;
        Ok(())
    };
    if literal {
        measure(template)?;
    } else {
        replacement_parts(capture, template, &mut measure)?;
    }
    let mut value = String::new();
    value.try_reserve_exact(length).map_err(|_| {
        CoreError::new(
            ErrorCode::Unavailable,
            "Replacement allocation unavailable; narrow the query or selection",
        )
    })?;
    if literal {
        value.push_str(template);
    } else {
        replacement_parts(capture, template, |part| {
            value.push_str(part);
            Ok(())
        })?;
    }
    Ok(value)
}

/// Match LF-stripped lines, retaining CR; validate zero-width CRLF splits against untrimmed text.
pub fn line_match_allowed(regex: &Regex, line: &str, start: usize, end: usize) -> bool {
    let content_end = line.strip_suffix('\n').unwrap_or(line).len();
    if end > content_end {
        return false;
    }
    if start == end && end == content_end && line.ends_with("\r\n") {
        return regex
            .find_at(line, start)
            .is_some_and(|found| found.start() == start && found.end() == end);
    }
    true
}
