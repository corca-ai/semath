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

    #[test]
    fn maps_combining_marks_crlf_and_multiline_text_without_normalizing_source() {
        let source = "e\u{301}\r\n한😀\n";
        let index = SourceIndex::new(source);
        for (byte, _) in source.char_indices() {
            let utf16 = source[..byte].encode_utf16().count() as u32;
            assert_eq!(index.utf16_for_byte(byte), utf16);
            assert_eq!(index.byte_for_utf16(utf16), byte);
        }
        assert_eq!(
            index.utf16_for_byte(source.len()),
            source.encode_utf16().count() as u32
        );
    }

    #[test]
    fn round_trips_every_character_boundary_in_generated_unicode_sources() {
        let alphabet = ["a", "한", "😀", "e\u{301}", "\r\n", "\\forall", "_"];
        let mut state = 0x5EED_u64;
        for length in 0..128 {
            let mut source = String::new();
            for _ in 0..length {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                source.push_str(alphabet[(state as usize) % alphabet.len()]);
            }
            let index = SourceIndex::new(&source);
            for (byte, _) in source.char_indices() {
                let utf16 = source[..byte].encode_utf16().count() as u32;
                assert_eq!(index.utf16_for_byte(byte), utf16);
                assert_eq!(index.byte_for_utf16(utf16), byte);
            }
        }
    }
}
