use core::str;

pub struct StrAsciiPrefixView {
    prefix_len: usize,
    string: Box<[u8]>,
}

impl StrAsciiPrefixView {
    pub fn new(mut prefix: &str, max_length: usize) -> Self {
        let prefix_len = prefix.as_bytes().len();

        let mut string = vec![0; max_length].into_boxed_slice();

        string[0..prefix_len].copy_from_slice(&prefix.as_bytes());

        Self { prefix_len, string }
    }

    pub fn with<'a>(&'a mut self, added: &str) -> &'a str {
        let added_bytes = added.as_bytes();
        let bytelen = added_bytes.len();

        self.string[self.prefix_len..(self.prefix_len + bytelen)].copy_from_slice(added_bytes);

        //safety: since we're splitting at character lengths from existing strings, then
        //the bytes will ALWAYS be utf8.
        unsafe { str::from_utf8_unchecked(&self.string[0..(self.prefix_len + bytelen)]) }
    }
}
