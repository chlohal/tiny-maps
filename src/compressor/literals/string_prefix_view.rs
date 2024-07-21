pub struct StrAsciiPrefixView {
    prefix_len: usize,
    string: String
}

impl StrAsciiPrefixView {
    pub fn new(mut prefix: String, max_length: usize) -> Self {

        for _ in prefix.len()..max_length {
            prefix.push('\0');
        }

        Self {
            prefix_len: prefix.as_bytes().len(),
            string: prefix
        }
    }

    pub fn with<'a>(&'a mut self, added: &str) -> &'a str {
        let added_bytes = added.as_bytes();
        let bytelen = added_bytes.len();

        //safety: since we're splitting at exact byte lengths, then
        //original_bytes will ALWAYS be utf8.
        unsafe {
            let original_bytes = self.string.as_bytes_mut();

            original_bytes[self.prefix_len..(self.prefix_len + bytelen)].copy_from_slice(added_bytes);
        }

        &self.string[self.prefix_len..(self.prefix_len + bytelen)]
    }
}