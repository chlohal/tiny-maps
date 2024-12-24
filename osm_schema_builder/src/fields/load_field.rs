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
        const WRITE_STATEBITS: &str = "write_to.write_all(&[ external_data.1.into_inner() ])?;";

        match self {
            FieldData::Colour { .. } | FieldData::Number { .. } => ("write_to.write_all(&[ external_data.1.into_inner() ])?; self.0.minimally_serialize(write_to, ())", "Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, ())?))"),
            FieldData::Text { .. } => ("self.0.as_str().minimally_serialize(write_to, external_data.1)", "Ok(Self(minimal_storage::serialize_min::DeserializeFromMinimal::deserialize_minimal(from, Some(external_data.1.into_inner()))?))"),
            FieldData::Checkbox { .. } => ("let mut external_data = external_data; external_data.1.set_bit(0, self.0); write_to.write_all(&[ external_data.1.into_inner() ])", "Ok(Self(external_data.1.get_bit(0) != 0))"),
            FieldData::Address { .. } => ("write_to.write_all(&[ external_data.1.into_inner() ])?; self.0.minimally_serialize(write_to, external_data.0)", "osm_structures::structured_elements::address::OsmAddress::deserialize_minimal(from, external_data.0).map(|x| Self(x))"),
            _ => ( "todo!()", "todo!()" ),
            FieldData::UnitNumber { key } => todo!(),
            FieldData::LocalizedString { root_key } => todo!(),
            FieldData::Date { key } => todo!(),
            FieldData::Combo { .. } => todo!(),
            FieldData::MultiYesCombo { label, keys } => todo!(),
            FieldData::SemiCombo { key, options } => todo!(),
            FieldData::DirectionalCombo { root_key, left_key, right_key, options } => todo!(),
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

        fn update_state<S: std::convert::From<&'static str> + AsRef<str> + PartialEq<&'static str>>(tag: (S, S), {update_state_varname}: &mut Self::State)  -> Option<(S, S)> {{
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
            type ExternalData<'d> = (&'d mut minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<0, 3, u8>);

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                {deser_code}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {name} {{
            type ExternalData<'s> = (&'s mut minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<0, 3, u8>);

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

    fn trait_match_map(&self, name: &str) -> (bool, String) {
        let key = self.key();

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
                    let vtype = self.datatype();
                    (true, format!(
                        r#"
                            if v == "yes" {{
                                {code_to_set_property_based_on_key_with_value_known_to_be_yes}
                            }}
                            return Some((k,v))
                    "#
                    ))
                }
                FieldData::LocalizedString { root_key } => {
                    (false, format!("Some((k,v))"))
                }
                FieldData::Access {} | FieldData::Address { .. } => (false, "todo!()".to_string()),
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
            FieldData::Number { key } => "<f64 as std::str::FromStr>::from_str(&v.as_ref()).ok()".to_string(),
            FieldData::UnitNumber { key } => "todo!()".to_string(),
            FieldData::Colour { key } => "osm_structures::structured_elements::colour::OsmColour::from_str(v.as_ref())".to_string(),
            FieldData::LocalizedString { root_key } => "Some(v)".to_string(),
            FieldData::Date { key } => "todo!()".to_string(),
            FieldData::Combo { key, options } => options_to_formatted_kv(&format!("{}Value", slugify(key, RustStruct)), &options),
            FieldData::SemiCombo { key, options } => format!(
                "v.as_ref().split(';').map(|v| {}).collect::<Option<Vec<_>>>()",
                options_to_formatted_kv(&format!("{}Value", slugify(key, RustStruct)), options)
            ),
            FieldData::DirectionalCombo {
                root_key,
                left_key,
                right_key,
                options,
            } => todo!(),
            FieldData::Checkbox { key } => {
                r#"if v == "yes" { Some(true) } else if v == "no" { Some(false) } else { None }"#
                    .to_string()
            }
            FieldData::Address { .. } | FieldData::Access {} | FieldData::MultiYesCombo { .. } => {
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
                     pub(in crate::fields) fn try_into_field<S: AsRef<str> + std::convert::From<&'static str> + PartialEq<&'static str>>(k: S, v: S) -> Result<(S, S), crate::fields::AnyOsmField> {{
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
            FieldData::Address { .. } => format!("state.to_option().map(|x| crate::fields::AnyOsmField::from(({name}(x))))"),
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

    format!(
        "#[derive(PartialEq, Clone, Copy, Debug)]\npub enum {root_key}Directional {{ 
        Unidirectional({root_key}Value),
        Bidirectional({root_key}Value, {root_key}Value),
        {left_key}Only({root_key}Value),
        {right_key}Only({root_key}Value),
    }}
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
            ("external_data.copy_from(*self as u8); external_data.into_inner().minimally_serialize(write_to, ())", "let _from = from; Ok(unsafe { std::mem::transmute( external_data.into_inner_masked() ) })")
        },
        //larger: this can fit into one u8. we assert during build that there aren't more than 256 enum variants
        0..256 => {
            ("(*self as u8).minimally_serialize(write_to, ())", "Ok(unsafe { std::mem::transmute(u8::deserialize_minimal(from, ())?) })")
        },
        _ => unreachable!()
    };

    //implement serialization traits
    s += &format!("
    impl minimal_storage::serialize_min::DeserializeFromMinimal for {root_key}Value {{
            type ExternalData<'d> = &'d minimal_storage::bit_sections::LowNibble;

            fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
                {deser_code}
            }}
        }}

        impl minimal_storage::serialize_min::SerializeMinimal for {root_key}Value {{
            type ExternalData<'s> = &'s mut minimal_storage::bit_sections::LowNibble;

            fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
                {ser_code}
            }}
        }}
    ");

    s
}

fn generate_selections_struct(wrapper_struct: &str, root_key: &str, suffixes: &[String]) -> String {
    let root_key = slugify(root_key, RustStruct);

    let mut s = format!("#[derive(Default, PartialEq, Clone, Debug)]\npub struct {root_key}Selections {{\n");

    for suff in suffixes {
        s.push_str("    ");
        s.push_str(&slugify(suff, RustIdent));
        s.push_str(": bool,\n");
    }

    s.push('}');

    s
}
