use std::fmt::Write;
use std::{fs::File, path::PathBuf};

use serde_json::Value;

use crate::util::{slugify, SlugificationMethod};
use crate::util::SlugificationMethod::*;

use super::load_field_data::load_field_data;

pub fn load_field(path: PathBuf) -> Option<Field> {
    let json: Value = serde_json::from_reader(File::open(&path).unwrap()).unwrap();

    let filename = path
        .ancestors()
        .map(|x| x.file_name().unwrap().to_str().unwrap())
        .find(|x| !x.starts_with("index"))
        .unwrap()
        .replace(".json", "");
    let filename = slugify(filename, crate::util::SlugificationMethod::RustStruct);

    let mut module = path
        .ancestors()
        .filter(|x| x.file_name().unwrap() != "index.json")
        .take_while(|x| x.as_os_str() != "id-tagging-schema-data")
        .map(|x| {
            slugify(
                x.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(".json", ""),
                crate::util::SlugificationMethod::RustIdent,
            )
        })
        .collect::<Vec<_>>();

    module.reverse();

    let data = load_field_data(&path, json)?;

    Some(Field {
        module,
        name: filename,
        data,
    })
}

pub struct Field {
    pub module: Vec<String>,
    pub name: String,
    pub data: FieldData
}

pub enum FieldData {
    Text {
        key: String,
    },
    Number {
        key: String,
    },
    UnitNumber {
        key: String,
    },
    Colour {
        key: String,
    },
    LocalizedString {
        root_key: String,
    },
    Date {
        key: String,
    },
    Combo {
        key: String,
        options: Vec<String>,
    },
    MultiYesCombo {
        label: String,
        keys: Vec<String>,
    },
    SemiCombo {
        key: String,
        options: Vec<String>,
    },
    DirectionalCombo {
        root_key: String,
        left_key: String,
        right_key: String,
        options: Vec<String>,
    },
    Checkbox {
        key: String,
    },
    Address {
        prefix: String,
    },
    Access {},
}

impl FieldData {
    pub fn enum_name(&self) -> String {
        slugify(
            format!(
                "{} {}",
                self.key().cloned().unwrap_or_default(),
                self.typename()
            ),
            SlugificationMethod::RustStruct,
        )
    }
    pub fn datatype(&self) -> String {
        match self {
            FieldData::Text { .. } => "String".to_string(),
            FieldData::Number { .. } => "f64".to_string(),
            FieldData::UnitNumber { .. } => "(f64, String)".to_string(),
            FieldData::Colour { .. } => "osm_structures::structured_elements::colour::OsmColour".to_string(),
            FieldData::LocalizedString { .. } => {
                "std::collections::HashMap<[u8; 2], String>".to_string()
            }
            FieldData::Date { .. } => "(u16, u8, u8)".to_string(),
            FieldData::Combo { key, .. } => format!("{}Value", slugify(key, RustStruct)),
            FieldData::MultiYesCombo {
                label: root_key, ..
            } => format!("{}Selections", slugify(root_key, RustStruct)),
            FieldData::SemiCombo { key, .. } => format!("Vec<{}Value>", slugify(key, RustStruct)),
            FieldData::DirectionalCombo { root_key, .. } => {
                format!("{}Directional", slugify(root_key, RustStruct))
            }
            FieldData::Checkbox { .. } => "bool".to_string(),
            FieldData::Address { .. } => {
                "osm_structures::structured_elements::address::OsmAddress".to_string()
            }
            FieldData::Access {} => "Access".to_string(),
        }
    }
    fn serialization_code(&self) -> (&'static str, &'static str) {
        match self {
            FieldData::MultiYesCombo { .. } | FieldData::Colour { .. } | FieldData::Number { .. } => ("write_to.write_all(&[ external_data.1.into_inner() ])?; self.0.minimally_serialize(write_to, ())", "let _ = external_data; Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, ())?))"),
            FieldData::Text { .. } => ("self.0.as_str().minimally_serialize(write_to, external_data.1)", "Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, Some(external_data.1.into_inner()))?))"),
            FieldData::Checkbox { .. } => ("let mut external_data = external_data; external_data.1.set_bit(0, self.0); write_to.write_all(&[ external_data.1.into_inner() ])", "let _ = from; Ok(Self(external_data.1.get_bit(0) != 0))"),
            FieldData::Address { .. } => ("write_to.write_all(&[ external_data.1.into_inner() ])?; self.0.minimally_serialize(write_to, external_data.0)", "osm_structures::structured_elements::address::OsmAddress::deserialize_minimal(from, external_data.0).map(|x| Self(x))"),
            FieldData::Combo { .. } => ("self.0.minimally_serialize(write_to, external_data.1.reduce_extent::<4, 8>())", "Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, external_data.1.reduce_extent::<4,8>())?))"),
            FieldData::Date { .. } => (
                r"external_data.1.into_inner().minimally_serialize(write_to, ())?; 
                    self.0.0.minimally_serialize(write_to, ())?;
                    self.0.1.minimally_serialize(write_to, ())?;
                    self.0.2.minimally_serialize(write_to, ())", 
                r"let _ = external_data; Ok(Self((
                    u16::deserialize_minimal(from, ())?,
                    u8::deserialize_minimal(from, ())?,
                    u8::deserialize_minimal(from, ())?,
                )))"
            ),
            FieldData::UnitNumber { .. } => (
                r"self.0.1.as_str().minimally_serialize(write_to, external_data.1)?;
                self.0.0.minimally_serialize(write_to, ())
                ",
                r"
                let u = minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, Some(external_data.1.into_inner()))?;
                let n = f64::deserialize_minimal(from, ())?;
                Ok(Self((n, u)))
                ",
            ),
            FieldData::SemiCombo { options, .. } => {
                assert!(options.len() < 256);

                (r"
                    external_data.1.into_inner().minimally_serialize(write_to, ())?;
                    self.0.len().minimally_serialize(write_to, ())?;

                    for i in self.0.iter() {
                        (*i as u8).minimally_serialize(write_to, ())?;
                    }

                    Ok(())
                ", r"
                let _ = external_data;
                let len = usize::deserialize_minimal(from, ())?;
                let mut v = Vec::with_capacity(len);

                for _ in 0..len {
                    let val = u8::deserialize_minimal(from, ())?;
                    let val = unsafe { std::mem::transmute(val)  };
                    v.push(val)
                }

                Ok(Self(v))
                ")
            },
            FieldData::DirectionalCombo { .. } => ( "self.0.minimally_serialize(write_to, external_data.1.into_inner().into())",
                "Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, external_data.1.reduce_extent())?))"
            ),
            FieldData::LocalizedString { .. } => (
                r"let mut nibble = external_data.1.into_inner_masked();
                
                if self.0.len() < 16 {    
                    nibble |= 0b1_0000 | (self.0.len() as u8);
                    nibble.minimally_serialize(write_to, ())?;
                } else {
                    nibble.minimally_serialize(write_to, ())?;
                    self.0.len().minimally_serialize(write_to, ())?;
                }

                for (k, v) in self.0.iter() {
                    let ch1_index = k[0] - b'a';
                    let ch2_index = k[1] - b'a';

                    debug_assert!(ch1_index < 32);
                    debug_assert!(ch2_index < 32);

                    let ch1 = (k[0] - b'a' << 3) + k[1] >> 2;
                    let ch2 = (k[1] & 0b11) << 6;

                    ch1.minimally_serialize(write_to, ())?;
                    ch2.minimally_serialize(write_to, ())?;
                    v.as_str().minimally_serialize(write_to, ch2.into())?;
                }
                Ok(())
                ",

                r"let nibble = external_data.1.into_inner();
                
                let length = if nibble & 0b1_0000 != 0 {
                    (nibble & 0b1111) as usize
                } else {
                    usize::deserialize_minimal(from, ())?
                 };

                 let mut map = std::collections::HashMap::with_capacity(length);

                 for _ in 0..length {
                    let ch1 = u8::deserialize_minimal(from, ())?;
                    let ch2 = u8::deserialize_minimal(from, ())?;

                    let k = [ (ch1 >> 3) + b'a' , (((ch1 & 0b111) << 2) & (ch2 >> 6)) + b'a'];

                    let v = minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, Some(ch1))?;

                    map.insert(k, v);
                 }

                 Ok(Self(map))
                "
            ),
            FieldData::Access {  } => ("todo!()", "todo!()"),
        }
    }
    pub fn datatype_def(&self, wrapper_struct: &str) -> Option<String> {
        let (root_key, options) = match self {
            FieldData::SemiCombo { key, options } => (key, options),
            FieldData::Combo { key, options } => (key, options),

            FieldData::DirectionalCombo {
                root_key,
                options,
                left_key,
                right_key,
            } => {
                return Some(generate_directional_enum(
                    root_key, left_key, right_key, options,
                ))
            }
            FieldData::MultiYesCombo {
                label: root_key,
                keys,
                ..
            } => return Some(generate_selections_struct( wrapper_struct, root_key, keys)),
            _ => return None,
        };

        Some(generate_values_enum(root_key, options))
    }
    pub fn traitimpl(&self, name: &str, id: usize, enum_name: &str) -> String {
        let (uses_state, code_to_match_valid_kv_sets) = self.trait_match_map(enum_name);

        let single_osm_field_code = self.make_singlefield_traitimpl(name, enum_name);

        let (transformation_state, state_init, state_end) = self.transformation_state(name);

        let update_state_varname = if uses_state { "state" } else { "_state" };

        let (ser_code, deser_code) = self.serialization_code();

        let stateful_osm_field_code = if self.is_single() { format!("") } else {
            format!(r##"
                impl crate::fields::StatefulOsmField for {name} {{
        type State = {transformation_state};

        fn init_state() -> Self::State {{
            {state_init}
        }}

        fn update_state<S: std::convert::From<&'static str> + AsRef<str> + for<'a> PartialEq<&'a str>>(tag: (S, S), {update_state_varname}: &mut Self::State)  -> Option<(S, S)> {{
            let (k,v) = tag;
            {code_to_match_valid_kv_sets}
        }}

        fn end_state(state: Self::State) -> Option<crate::fields::AnyOsmField> {{
            {state_end}
        }}
        }}
            "##)
        };

        format!(
            r##"
            
            {single_osm_field_code}
        {stateful_osm_field_code}
            impl crate::fields::OsmField for {name} {{
        const FIELD_ID: u16 = {id};
        }}

        impl minimal_storage::serialize_min::DeserializeFromMinimal for {name} {{
            type ExternalData<'d> = (&'d minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<3, 8, u8>);

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                {deser_code}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {name} {{
            type ExternalData<'s> = (&'s minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<0, 3, u8>);

            fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
                {ser_code}
            }}
        }}
        "##
        )
    }

    pub fn is_single(&self) -> bool {
        if let AbstractKeyCount::One = self.key_count() {
            true
        } else {
            false
        }
    }

    fn trait_match_map(&self, _name: &str) -> (bool, String) {

        match self.key_count() {
            AbstractKeyCount::One => {
                (true,format!(
                    r#"
                    if state.is_some() {{
                        return Some((k,v));
                    }}

                    match Self::try_into_field(k,v) {{
                        Ok(kv) => return Some(kv),
                        Err(f) => *state = Some(f),
                    }};
                    return None;
                    "#
                ))
            }
            AbstractKeyCount::DirectionalCombo => (false, "return Some((k,v)); todo!()".to_string()),
            AbstractKeyCount::Many => match self {
                FieldData::MultiYesCombo { keys, .. } => {
                    let code_to_set_property_based_on_key_with_value_known_to_be_yes =
                        gen_multiyes_set_property(keys);
                    (true, format!(
                        r#"
                            if v == "yes" {{
                                {code_to_set_property_based_on_key_with_value_known_to_be_yes}
                            }}
                            return Some((k,v))
                    "#
                    ))
                }
                FieldData::LocalizedString { .. } => {
                    (false, format!("Some((k,v))"))
                }
                FieldData::Address { .. } => (true, "state.update(k, v)".to_string()),
                FieldData::Access {} => (false, "todo!()".to_string()),
                _ => unreachable!(),
            },
        }
    }

    fn key_count(&self) -> AbstractKeyCount {
        match self {
            FieldData::SemiCombo { .. }
            | FieldData::Combo { .. }
            | FieldData::Checkbox { .. }
            | FieldData::Date { .. }
            | FieldData::Colour { .. }
            | FieldData::UnitNumber { .. }
            | FieldData::Number { .. }
            | FieldData::Text { .. } => AbstractKeyCount::One,

            FieldData::DirectionalCombo { .. } => AbstractKeyCount::DirectionalCombo,

            FieldData::MultiYesCombo { .. }
            | FieldData::LocalizedString { .. }
            | FieldData::Address { .. }
            | FieldData::Access {} => AbstractKeyCount::Many,
        }
    }

    pub(crate) fn key(&self) -> Option<&String> {
        use FieldData::*;
        match self {
            Address { prefix: key }
            | Checkbox { key }
            | DirectionalCombo { root_key: key, .. }
            | SemiCombo { key, .. }
            | MultiYesCombo { label: key, .. }
            | Combo { key, .. }
            | Date { key }
            | LocalizedString { root_key: key }
            | Colour { key }
            | UnitNumber { key }
            | Number { key }
            | Text { key } => Some(key),
            Access {} => None,
        }
    }

    pub(crate) fn typename(&self) -> &'static str {
        match self {
            FieldData::Text { .. } => "text",
            FieldData::Number { .. } => "number",
            FieldData::UnitNumber { .. } => "unit number",
            FieldData::Colour { .. } => "colour",
            FieldData::LocalizedString { .. } => "localized string",
            FieldData::Date { .. } => "date",
            FieldData::Combo { .. } => "combo",
            FieldData::MultiYesCombo { .. } => "multi yes combo",
            FieldData::SemiCombo { .. } => "semi combo",
            FieldData::DirectionalCombo { .. } => "directional combo",
            FieldData::Checkbox { .. } => "checkbox",
            FieldData::Address { .. } => "address",
            FieldData::Access { .. } => "access",
        }
    }

    fn make_value_pattern_matcher(&self) -> String {
        match self {
            FieldData::Text { .. } => "Some(v.as_ref().into())".to_string(),
            FieldData::Number { .. } => "<f64 as std::str::FromStr>::from_str(&v.as_ref()).ok()".to_string(),
            FieldData::UnitNumber { .. } => r"
                                                        {
                                                            let s = v.as_ref();
                                                            let mut f = 0;
                                                            if s.chars().nth(0).is_some_and(|x| x == '-') {
                                                                f += 1;
                                                            }

                                                            for c in s.chars() {
                                                                if c.is_ascii_digit() || c == '.' {
                                                                    f += 1;
                                                                    continue;
                                                                } else {
                                                                    break;
                                                                }   
                                                            }
                                                            <f64 as std::str::FromStr>::from_str(&s[0..f]).ok().map(|x| (x, s[f..].to_string()))
                                                        }
                                                        ".to_string(),
            FieldData::Colour { .. } => "osm_structures::structured_elements::colour::OsmColour::from_str(v.as_ref())".to_string(),
            FieldData::Date { .. } => r" loop {
                let mut components = v.as_ref().split('-');
                let Some(y) = components.next() else { break None; };
                let m = components.next();
                let d = components.next();

                let Some(y) = <u16 as std::str::FromStr>::from_str(y).ok() else { break None; };
                let m = m.and_then(|m| <u8 as std::str::FromStr>::from_str(m).ok()).unwrap_or(0xff);
                let d = d.and_then(|d| <u8 as std::str::FromStr>::from_str(d).ok()).unwrap_or(0xff);

                break Some((y,m,d));
            }
            ".to_string(),
            FieldData::Combo { key, options } => options_to_formatted_kv(&format!("{}Value", slugify(key, RustStruct)), &options),
            FieldData::SemiCombo { key, options } => format!(
                "v.as_ref().split(';').map(|v| {}).collect::<Option<Vec<_>>>()",
                options_to_formatted_kv(&format!("{}Value", slugify(key, RustStruct)), options)
            ),
            FieldData::Checkbox { .. } => {
                r#"if v == "yes" { Some(true) } else if v == "no" { Some(false) } else { None }"#
                    .to_string()
            }
            FieldData::LocalizedString { .. } | FieldData::DirectionalCombo { ..} | FieldData::Address { .. } | FieldData::Access {} | FieldData::MultiYesCombo { .. } => {
                unreachable!()
            }
        }
    }
    
    fn make_singlefield_traitimpl(&self, typename: &str, enum_name: &str) -> String {
        match self.key_count() {
            AbstractKeyCount::One => {
                let key = self.key().unwrap();
                let try_make_value = self.make_value_pattern_matcher();
                format!(
                    r#"impl {typename} {{
                     pub(in crate::fields) fn try_into_field<S: AsRef<str> + std::convert::From<&'static str> + for<'a> PartialEq<&'a str>>(k: S, v: S) -> Result<(S, S), crate::fields::AnyOsmField> {{
                        if k == {key:?} {{
                            if let Some(val) = {try_make_value} {{
                                return Err(crate::fields::AnyOsmField::{enum_name}({typename}(val)))
                            }}
                        }}

                        Ok((k,v))
                    }}
                }}"#
                )
            },
            AbstractKeyCount::DirectionalCombo | AbstractKeyCount::Many => String::new(),
        }
    }
    
    fn transformation_state(&self, name: &str) -> (String, String, String) {
        let typ = match self {
            FieldData::SemiCombo { .. }
            | FieldData::Combo { .. }
            | FieldData::Checkbox { .. }
            | FieldData::Date { .. }
            | FieldData::Colour { .. }
            | FieldData::UnitNumber { .. }
            | FieldData::Number { .. }
            | FieldData::Text { .. } => "Option<crate::fields::AnyOsmField>".to_string(),

            FieldData::LocalizedString { .. } => self.datatype(),
            FieldData::MultiYesCombo { .. } => self.datatype(),
            FieldData::DirectionalCombo { .. } => format!("Option<{}>", self.datatype()),
            FieldData::Address { .. } => "osm_structures::structured_elements::address::OsmAddressBuilder".to_string(),
            FieldData::Access {  } => "()".to_string(),
        };

        let init = match self {
            FieldData::Address { prefix } => format!("osm_structures::structured_elements::address::OsmAddressBuilder::with_prefix(\"{}:\")", prefix),
          _ => "Default::default()".to_string()  
        };

        let end = match self {
            FieldData::LocalizedString { .. } |
            FieldData::MultiYesCombo { .. } => format!("Some(({name}(state)).into())"),
            FieldData::DirectionalCombo { .. } => format!("Some(({name}(state?)).into())"),
            FieldData::Access { .. } => "todo!()".to_string(),
            FieldData::Address { .. } => format!("state.to_option().map(|x| crate::fields::AnyOsmField::from({name}(x)))"),
            _ => "state".to_string()
        };

        (typ, init, end)
    }
}

fn gen_multiyes_set_property(keys: &[String]) -> String {
    keys.into_iter()
        .map(|k| {
            let prop = slugify(k, RustIdent);
            format!("if k == {k:?} {{ state.{prop} = true; return None; }}")
        })
        .collect()
}

enum AbstractKeyCount {
    One,
    DirectionalCombo,
    Many,
}

fn options_to_formatted_kv(enum_name: &str, options: &Vec<String>) -> String {
    let kv = options
        .iter()
        .map(|x| format!("({x:?}, {enum_name}::{})", slugify(x, RustStruct)))
        .collect::<Vec<_>>()
        .join(",");

    format!("[{kv}].iter().find(|x| v == x.0).map(|x| x.1)")
}

fn generate_directional_enum(
    root_key: &str,
    left_key: &str,
    right_key: &str,
    options: &[String],
) -> String {
    let left_key = slugify(left_key, RustStruct);
    let right_key = slugify(right_key, RustStruct);
    let root_key = slugify(root_key, RustStruct);

    let ser_deser_code = format!("
    impl minimal_storage::serialize_min::DeserializeFromMinimal for {root_key}Directional {{
            type ExternalData<'d> = minimal_storage::bit_sections::LowNibble;

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                let discrim = external_data.into_inner() & 0b11;
                match discrim {{
                    0b00 => {{
                        let nib = u8::deserialize_minimal(from, ())?;
                        {root_key}Value::deserialize_minimal(from, nib.into()).map(|x| Self::Unidirectional(x))
                    }},
                    0b11 => {{
                        let nib_left = u8::deserialize_minimal(from, ())?;
                        let left = {root_key}Value::deserialize_minimal(from, nib_left.into())?;

                        let nib_right = u8::deserialize_minimal(from, ())?;
                        let right = {root_key}Value::deserialize_minimal(from, nib_right.into())?;

                        Ok(Self::Bidirectional(left, right))
                    }},
                    0b10 => {{
                        let nib = u8::deserialize_minimal(from, ())?;
                        {root_key}Value::deserialize_minimal(from, nib.into()).map(|x| Self::{left_key}Only(x))
                    }},
                    0b01 => {{
                        let nib = u8::deserialize_minimal(from, ())?;
                        {root_key}Value::deserialize_minimal(from, nib.into()).map(|x| Self::{right_key}Only(x))
                    }},
                    _ => unreachable!()
                }}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {root_key}Directional {{
            type ExternalData<'s> = minimal_storage::bit_sections::LowNibble;

            fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
                let mut nib = external_data.into_inner();
                match self {{
                    Self::Unidirectional(v) => {{
                        nib |= 0b00;
                        nib.minimally_serialize(write_to, ())?;

                        v.minimally_serialize(write_to, 0.into())
                    }},
                    Self::Bidirectional(l, r) => {{
                        nib |= 0b11;
                        nib.minimally_serialize(write_to, ())?;

                        l.minimally_serialize(write_to, 0.into())?;
                        r.minimally_serialize(write_to, 0.into())
                    }},
                    Self::{left_key}Only(v) => {{
                        nib |= 0b10;
                        nib.minimally_serialize(write_to, ())?;

                        v.minimally_serialize(write_to, 0.into())
                    }},
                    Self::{right_key}Only(v) => {{
                        nib |= 0b01;
                        nib.minimally_serialize(write_to, ())?;

                        v.minimally_serialize(write_to, 0.into())
                    }},
                }}
            }}
        }}
    ");

    format!(
        "#[derive(PartialEq, Clone, Copy, Debug)]\npub enum {root_key}Directional {{ 
        Unidirectional({root_key}Value),
        Bidirectional({root_key}Value, {root_key}Value),
        {left_key}Only({root_key}Value),
        {right_key}Only({root_key}Value),
    }}\n\n{ser_deser_code}\n
    \n{}",
        generate_values_enum(&root_key, options)
    )
}

fn generate_values_enum(root_key: &str, options: &[String]) -> String {
    let root_key = slugify(root_key, RustStruct);

    let mut s = format!("#[derive(PartialEq, Clone, Copy, Debug)]#[repr(u8)]\npub enum {root_key}Value {{\n");

    for suff in options {
        s.push_str("    ");
        s.push_str(&slugify(suff, RustStruct));
        s.push_str(",\n");
    }

    s.push('}');

    let enum_count = options.len();

    assert!(enum_count < 256);

    let (ser_code, deser_code) = match enum_count {
        //Smallest: this can fit into the nibble of extra data we get, without adding any more bytes!
        0..16 => {
            ("let mut external_data = external_data; external_data.copy_from(*self as u8); external_data.into_inner().minimally_serialize(write_to, ())", "let _from = from; Ok(unsafe { std::mem::transmute( external_data.into_inner_masked() ) })")
        },
        //larger: this can fit into one u8. we assert during build that there aren't more than 256 enum variants
        0..256 => {
            ("external_data.into_inner().minimally_serialize(write_to, ())?; (*self as u8).minimally_serialize(write_to, ())", "let _ = external_data; Ok(unsafe { std::mem::transmute(u8::deserialize_minimal(from, ())?) })")
        },
        _ => unreachable!()
    };

    //implement serialization traits
    s += &format!("
    impl minimal_storage::serialize_min::DeserializeFromMinimal for {root_key}Value {{
            type ExternalData<'d> = minimal_storage::bit_sections::LowNibble;

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                {deser_code}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {root_key}Value {{
            type ExternalData<'s> = minimal_storage::bit_sections::LowNibble;

            fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
                {ser_code}
            }}
        }}
    ");

    s
}

fn generate_selections_struct(_wrapper_struct: &str, root_key: &str, suffixes: &[String]) -> String {
    let root_key = slugify(root_key, RustStruct);

    let mut ser_code = "".to_string();
    let mut deser_code = "let mut s = Self::default();\n".to_string();

    let mut s = format!("#[derive(Default, PartialEq, Clone, Debug)]\npub struct {root_key}Selections {{\n");

    for suff in suffixes.iter() {
        s.push_str("    ");
        s.push_str(&slugify(suff, RustIdent));
        s.push_str(": bool,\n");
    }

    s.push('}');

    for chunk in suffixes.chunks(32) {
        ser_code.push_str("let mut n = 0u32;\n");
        deser_code.push_str("let n = u32::deserialize_minimal(from, ())?;\n");

        for (i, suf) in chunk.iter().enumerate() {
            write!(&mut ser_code, "if self.{} {{ n |= 1 << {i}; }}\n", slugify(suf, RustIdent)).unwrap();

            write!(&mut deser_code, "if (n & (1 << {i})) != 0 {{ s.{} = true; }} \n", slugify(suf, RustIdent)).unwrap();
        }

        ser_code.push_str("n.minimally_serialize(write_to, ())?;\n");
    }

    ser_code.push_str("Ok(())");
    deser_code.push_str("Ok(s)");


    let enum_count = suffixes.len();
    assert!(enum_count < 256);


    //implement serialization traits
    s += &format!("
    impl minimal_storage::serialize_min::DeserializeFromMinimal for {root_key}Selections {{
            type ExternalData<'d> = ();

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, _external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                {deser_code}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {root_key}Selections {{
            type ExternalData<'s> = ();

            fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, _external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
                {ser_code}
            }}
        }}
    ");

    s
}
