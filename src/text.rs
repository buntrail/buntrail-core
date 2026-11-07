use crate::Error;
use ropey::Rope;
use std::{ops::Deref, sync::OnceLock};
/// Persistent UTF-8 text. Contiguous materialization is reserved for explicit consumers.
#[derive(Debug)]
pub struct Text {
    rope: Rope,
    contiguous: OnceLock<String>,
}
impl Clone for Text {
    fn clone(&self) -> Self {
        Self {
            rope: self.rope.clone(),
            contiguous: OnceLock::new(),
        }
    }
}
impl From<String> for Text {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
            contiguous: OnceLock::new(),
        }
    }
}
impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.rope == other.rope
    }
}
impl Eq for Text {}
impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl Text {
    pub fn as_str(&self) -> &str {
        self.contiguous.get_or_init(|| self.rope.to_string())
    }
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }
    pub fn lines(&self) -> ropey::iter::Lines<'_> {
        self.rope.lines()
    }
    pub fn chunks(&self) -> ropey::iter::Chunks<'_> {
        self.rope.chunks()
    }
    pub fn read(&self, offset: usize, out: &mut [u8]) -> usize {
        if offset >= self.len() {
            return 0;
        }
        let mut n = 0;
        for (dst, src) in out.iter_mut().zip(self.rope.bytes_at(offset)) {
            *dst = src;
            n += 1;
        }
        n
    }
    pub fn utf16_to_char(&self, offset: usize) -> Result<usize, Error> {
        let c = self
            .rope
            .try_utf16_cu_to_char(offset)
            .map_err(|_| Error::InvalidRange)?;
        if self.rope.char_to_utf16_cu(c) != offset {
            return Err(Error::InvalidRange);
        }
        Ok(c)
    }
    pub fn utf16_to_byte(&self, offset: usize) -> Result<usize, Error> {
        Ok(self.rope.char_to_byte(self.utf16_to_char(offset)?))
    }
    pub fn byte_to_utf16(&self, offset: usize) -> Result<usize, Error> {
        let c = self
            .rope
            .try_byte_to_char(offset)
            .map_err(|_| Error::InvalidRange)?;
        if self.rope.char_to_byte(c) != offset {
            return Err(Error::InvalidRange);
        }
        Ok(self.rope.char_to_utf16_cu(c))
    }
    pub fn slice(&self, start: usize, end: usize) -> String {
        self.rope.slice(start..end).to_string()
    }
    pub(crate) fn replace(&mut self, start: usize, end: usize, text: &str) {
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        self.contiguous = OnceLock::new();
    }
}

impl serde::Serialize for Text {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> serde::Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(<String as serde::Deserialize>::deserialize(deserializer)?.into())
    }
}
#[cfg(test)]
mod adoption {
    #[test]
    fn find_keeps_persistent_text_without_contiguous_materialization() {
        use crate::{contracts::*, search::find::*};
        let text = crate::Text::from("needle 😀\n".repeat(25000));
        let p = plan(
            &text,
            BufferRevision(0),
            &FindOptions {
                expression: "needle".into(),
                mode: SearchMode::Literal,
                case_sensitive: false,
                whole_word: false,
                scope: None,
            },
            "replacement",
        )
        .unwrap();
        assert_eq!(p.matches.len(), 20000);
        assert!(p.limited);
        assert!(text.contiguous.get().is_none());
    }
    #[test]
    fn edit_mapping_and_undo_never_materialize_contiguous_text() {
        let mut document = crate::Document::new("abc😀\r\n".repeat(131072));
        let old = document.snapshot();
        for _ in 0..50 {
            document.edit(document.revision(), 1, 3, 2, "🐇").unwrap();
            let snapshot = document.snapshot();
            assert_eq!(snapshot.text.utf16_to_byte(5).unwrap(), 7);
            assert!(snapshot.text.contiguous.get().is_none());
            document.undo(document.revision(), 1).unwrap();
            assert!(document.snapshot().text.contiguous.get().is_none());
        }
        assert!(old.text.contiguous.get().is_none());
    }
}
