use std::{fs::File, io::Write, ops::AddAssign, path::Path};

use serde_json::Value;


pub enum SlugificationMethod {
    RustStruct,
    RustIdent
}

impl SlugificationMethod {
    pub fn start_str<'a>(&self, ch: &'a u8) -> char {
        match self {
            SlugificationMethod::RustStruct => ch.to_ascii_uppercase() as char,
            SlugificationMethod::RustIdent => ch.to_ascii_lowercase() as char,
        }
    }

    pub fn start_word<'a>(&self, ch: &'a u8) -> char {
        (match self {
            SlugificationMethod::RustStruct => ch.to_ascii_uppercase(),
            SlugificationMethod::RustIdent => {
                ch.to_ascii_lowercase()
            }
        }) as char
    }

    pub fn inner<'a>(&self, ch: &'a u8) -> impl Iterator<Item = char> + 'a {
        match self {
            Self::RustIdent => (vec![ch.to_ascii_lowercase() as char]).into_iter(),
            Self::RustStruct => vec![*ch as char].into_iter()
        }
    }

    fn inner_word_separator<'a>(&self, ch: &'a u8) -> impl Iterator<Item = char> + 'a {
        
        (match (ch, self) {
            (_, SlugificationMethod::RustStruct) => vec![],
            (_, SlugificationMethod::RustIdent) => vec!['_'],
        }).into_iter()
    }

    fn fixup_word(&self, word: String, separator: u8) -> String {
        word
    }

    fn fixup(&self, str: String, original_str: &str) -> String {

        if str == "" {
            let mut s = match self {
                SlugificationMethod::RustStruct => "Unicode",
                SlugificationMethod::RustIdent => "unicode",
            }.to_string();

            for b in original_str.bytes() {
                s += &format!("u{b:x}");
            }

            return s;
        }

        match self {
            SlugificationMethod::RustStruct => {
                if str.bytes().nth(0).is_some_and(|x| x.is_ascii_digit()) {
                    let ogstr = original_str.replace(".", "Point").replace(",", "EUPoint");
                    slugify(format!("num {ogstr}"), Self::RustStruct)
                } else {
                    str
                }
            }
            SlugificationMethod::RustIdent => match str.as_str() {
                "pub" => "is_pub",
                "ref" => "is_ref",
                "mod" => "is_mod",
                "type" => "of_type",
                _ => &str,
            }
            .into(),
            
        }
    }
}

pub fn slugify<S: AsRef<str>>(str: S, kind: SlugificationMethod) -> String {
    let str = str.as_ref();

    let str = if str.starts_with("-") {
        &format!("negative {str}")
    } else if str.starts_with(">") {
        &format!("gt {str}")
    } else {
        str
    };

    let original_str = str;

    let bs = original_str.bytes().filter(|x| *x != b'\'');

    let mut is_start_word = true;
    let mut is_start_str = true;
    let mut is_start_inner_rem = false;

    let mut str = String::new();

    let mut word = String::new();

    bs.for_each(|ch| {
        if ch.is_ascii_alphanumeric() {
            if is_start_str {
                word.push(kind.start_str(&ch));
            } else if is_start_word {
                word.push(kind.start_word(&ch));
            } else {
                word.extend(kind.inner(&ch));
            };
            is_start_inner_rem = true;
            is_start_word = false;
            is_start_str = false;
        } else {
            if is_start_inner_rem {
                let word = std::mem::take(&mut word);
                str.push_str(&kind.fixup_word(word, ch));
                is_start_word = true;
                
                str.extend(kind.inner_word_separator(&ch));
            }
            is_start_inner_rem = false;
        }
    });

    str.push_str(&word);

    kind.fixup(str, original_str)
}



pub fn open_json<P: AsRef<Path>>(path: P) -> std::io::Result<Value> {
    let path = path.as_ref();

    let file = File::open(path)?;

    Ok(serde_json::from_reader(file)?)
}


