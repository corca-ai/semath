#[derive(Clone, Debug)]
pub struct SourceIndex {
    byte_to_utf16: Vec<u32>,
    utf16_to_byte: Vec<usize>,
}

impl SourceIndex {
    pub fn new(source: &str) -> Self {
        let mut byte_to_utf16 = vec![0; source.len() + 1];
        let utf16_len = source.encode_utf16().count();
        let mut utf16_to_byte = vec![source.len(); utf16_len + 1];
        let mut utf16_offset = 0usize;

        for (byte_offset, character) in source.char_indices() {
            for slot in &mut byte_to_utf16[byte_offset..byte_offset + character.len_utf8()] {
                *slot = utf16_offset as u32;
            }
            for unit in 0..character.len_utf16() {
                utf16_to_byte[utf16_offset + unit] = byte_offset;
            }
            utf16_offset += character.len_utf16();
        }

        byte_to_utf16[source.len()] = utf16_len as u32;
        utf16_to_byte[utf16_len] = source.len();
        Self {
            byte_to_utf16,
            utf16_to_byte,
        }
    }

    pub fn utf16_for_byte(&self, byte_offset: usize) -> u32 {
        self.byte_to_utf16[byte_offset.min(self.byte_to_utf16.len() - 1)]
    }

    pub fn byte_for_utf16(&self, utf16_offset: u32) -> usize {
        self.utf16_to_byte[(utf16_offset as usize).min(self.utf16_to_byte.len() - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::SourceIndex;

    #[test]
    fn maps_ascii_korean_and_astral_characters() {
        let source = "a한😀z";
        let index = SourceIndex::new(source);
        assert_eq!(index.utf16_for_byte(0), 0);
        assert_eq!(index.utf16_for_byte(1), 1);
        assert_eq!(index.utf16_for_byte(4), 2);
        assert_eq!(index.utf16_for_byte(8), 4);
        assert_eq!(index.byte_for_utf16(0), 0);
        assert_eq!(index.byte_for_utf16(1), 1);
        assert_eq!(index.byte_for_utf16(2), 4);
        assert_eq!(index.byte_for_utf16(4), 8);
    }
}
