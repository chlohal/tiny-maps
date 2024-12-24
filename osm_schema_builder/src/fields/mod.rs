use std::{
    collections::{BTreeMap, HashMap},
    fs::read_dir,
    io::Write,
    path::PathBuf,
};

use load_field::{load_field, Field, FieldData};
use mod_tree::ModuleTree;

use crate::util::{slugify, SlugificationMethod};

mod load_field;
mod load_field_data;
mod mod_tree;
mod parser;

pub fn make_fields(write_to: &mut impl Write) -> std::io::Result<()> {
    write_to.write_all(
        br"use crate::stateful_iterate::StatefulIterate;
        use std::str::FromStr;
        
        trait OsmField {
        const FIELD_ID: u16;
    }
        trait StatefulOsmField {
        type State;

        fn init_state() -> Self::State;

        fn update_state<S: std::convert::From<&'static str> + AsRef<str> + PartialEq<&'static str>>(tag: (S, S), state: &mut Self::State)  -> Option<(S, S)>;

        fn end_state(state: Self::State) -> Option<crate::fields::AnyOsmField>;
    }
    ",
    )?;

    let field_types = make_field_structs("id-tagging-schema-data/fields".into())?;

    let mut field_id = 0;
    let field_types = write_field_structs(write_to, field_types, &mut field_id)?;

    writeln!(write_to, "pub const MAX_FIELD_ID: usize = {field_id};")?;

    write!(
        write_to,
        "#[derive(Clone, Debug)]\npub enum AnyOsmField {{\n{}\n}}",
        field_types
            .iter()
            .map(
                |(
                    enum_name,
                    FieldReferenceData {
                        fully_qualified_struct_name: p,
                        ..
                    },
                )| { format!("    {enum_name}(crate::{p}),\n",) }
            )
            .collect::<String>()
    )?;

    write!(
        write_to,
        r##"
    impl minimal_storage::serialize_min::DeserializeFromMinimal for AnyOsmField {{
        type ExternalData<'d> = (&'d mut minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<1, 16, u16>);

        fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {{
            let low_byte = (external_data.1.into_inner() & 0b1111_1111) as u8;

            match 0b1111_1111_11 & (external_data.1.into_inner_masked() >> 5) {{
            
            "##
    )?;

    for (
        enum_name,
        FieldReferenceData {
            fully_qualified_struct_name,
            ..
        },
    ) in field_types.iter()
    {
        write!(write_to, "crate::{fully_qualified_struct_name}::FIELD_ID => Ok(AnyOsmField::{enum_name}(crate::{fully_qualified_struct_name}::deserialize_minimal(from, (external_data.0, low_byte.into()))?)),\n")?
    }

    write!(write_to, " _ => unreachable!() }}}}}}")?;

    write!(
        write_to,
        r##"

    impl minimal_storage::serialize_min::SerializeMinimal for AnyOsmField {{
        type ExternalData<'s> = (&'s mut minimal_storage::pooled_storage::Pool<osm_value_atom::LiteralValue>, minimal_storage::bit_sections::BitSection<1, 16, u16>);
        
        fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> Result<(), std::io::Error> {{
            let head = external_data.1.into_inner();

            match self {{
    "##
    )?;

    for (
        enum_name,
        FieldReferenceData {
            fully_qualified_struct_name,
            ..
        },
    ) in field_types.iter()
    {
        write!(write_to, "AnyOsmField::{enum_name}(f) => {{
            let head = head | crate::{fully_qualified_struct_name}::FIELD_ID << 5;
            write_to.write_all(&[ (head >> 8) as u8 ])?;

            let nib = ((head & 0b1111_1111) as u8).into();

            f.minimally_serialize(write_to, (external_data.0, nib))
        }}")?
    }

    write!(write_to, "}}}}}}")?;

    field_types.iter().for_each(
        |(
            enum_name,
            FieldReferenceData {
                fully_qualified_struct_name: p,
                ..
            },
        )| {
            write!(
                write_to,
                "impl From<crate::{p}> for AnyOsmField {{
            fn from(value: crate::{p}) -> Self {{
                Self::{enum_name}(value)
            }}
        }}\n\n",
            )
            .unwrap();
        },
    );

    let (single_fields, multi_fields) = field_types
        .into_iter()
        .partition::<Vec<_>, _>(|x| x.1.is_single_field);

    write_to.write_all(b"pub fn parse_tags_to_fields(tags: osmpbfreader::Tags) -> (Vec<crate::fields::AnyOsmField>, osmpbfreader::Tags) {
    \n    let tags = tags.into_inner().into_iter();
    \n    let tags = crate::deprecations::apply_deprecations(tags);\nlet mut fields = Vec::new();")?;

    for (_enum_name, fieldref) in multi_fields.iter() {
        let fqsn = &fieldref.fully_qualified_struct_name;
        let varname = slugify(&fqsn, SlugificationMethod::RustIdent);
        write!(write_to, "let mut {varname}_state: <crate::{fqsn} as crate::fields::StatefulOsmField>::State = crate::{fqsn}::init_state();\n")?;
    }

    write_to.write_all(
        br"
    let tags = tags.filter_map(|(k,v)| {
    let t = (|| { ",
    )?;
    for (_enum_name, fieldref) in single_fields {
        write!(
            write_to,
            "let (k,v) = crate::{}::try_into_field(k,v)?;\n",
            fieldref.fully_qualified_struct_name
        )?;
    }

    write_to.write_all(br"Ok((k,v)) })();")?;

    write_to.write_all(
        br"let (k,v) = match t {
        Ok(kv) => kv,
        Err(f) => {
            fields.push(f);
            return None;
        }
    };",
    )?;

    for (_enum_name, fieldref) in multi_fields {
        let varname = slugify(
            &fieldref.fully_qualified_struct_name,
            SlugificationMethod::RustIdent,
        );

        write!(
            write_to,
            "let (k,v) = crate::{}::update_state((k,v), &mut {varname}_state)?;\n",
            fieldref.fully_qualified_struct_name
        )?;
    }

    write_to.write_all(b"Some((k,v))\n});\n\n")?;

    write_to.write_all(b"let tags: osmpbfreader::Tags = tags.collect();\n")?;
    write_to.write_all(b"(fields, tags)\n}")?;

    Ok(())
}

struct FieldReferenceData {
    fully_qualified_struct_name: String,
    is_single_field: bool,
}

fn write_field_structs(
    write_to: &mut impl Write,
    fields: ModuleTree<String, Field>,
    field_id_counter: &mut usize,
) -> std::io::Result<BTreeMap<String, FieldReferenceData>> {
    let mut result_types = BTreeMap::new();

    if let Some(field) = fields.value {
        let enum_name = field.data.enum_name();

        write!(
            write_to,
            "#[derive(PartialEq, Clone, Debug)]\npub struct {}({});\n\n{}\n{}",
            field.name,
            field.data.datatype(),
            field.data.datatype_def(&field.name).unwrap_or_default(),
            field
                .data
                .traitimpl(&&field.name, *field_id_counter, &enum_name)
        )
        .unwrap();

        *field_id_counter += 1;

        if !result_types.contains_key(&enum_name) {
            result_types.insert(
                enum_name,
                FieldReferenceData {
                    is_single_field: field.data.is_single(),
                    fully_qualified_struct_name: format!(
                        "{}::{}",
                        field.module.join("::"),
                        field.name
                    ),
                },
            );
        }
    }

    for (mod_name, fields) in fields.children.into_iter() {
        write!(write_to, "\npub mod {mod_name} {{\n")?;
        result_types.extend(write_field_structs(write_to, fields, field_id_counter)?);
        write!(write_to, "}}\n")?;
    }

    Ok(result_types)
}

fn make_field_structs(folder: PathBuf) -> std::io::Result<ModuleTree<String, Field>> {
    let mut result_types = ModuleTree::new();

    let mut directory = read_dir(folder)?.collect::<Vec<_>>();

    //sort directories first
    directory.sort_by_key(|x| x.as_ref().unwrap().file_type().as_ref().unwrap().is_dir());

    for item in directory {
        let file = item?;

        if file.file_name().eq_ignore_ascii_case("@template") {
            continue;
        }

        let mod_name = slugify(
            file.file_name().into_string().unwrap().replace(".json", ""),
            SlugificationMethod::RustIdent,
        );

        if file.file_type().unwrap().is_dir() {
            result_types
                .children
                .insert(mod_name, make_field_structs(file.path().into())?);
        } else {
            //if it's not index.json, attempt to move it into its folder.
            //this will fail if the folder doesn't exist; that's ok!
            //it tells us that it is indeed in an okay location already
            let file_is_in_canonnical_location = mod_name == "index"
                || std::fs::rename(
                    file.path(),
                    file.path()
                        .to_str()
                        .unwrap()
                        .replace(".json", "/index.json"),
                )
                .is_err();

            if file_is_in_canonnical_location {
                if let Some(field) = load_field(file.path()) {
                    if let FieldData::Access { .. } = field.data {
                        continue;
                    }

                    let value = if mod_name == "index" {
                        &mut result_types.value
                    } else {
                        &mut result_types
                            .children
                            .entry(mod_name)
                            .or_insert(ModuleTree::new())
                            .value
                    };
                    assert!(value.is_none());
                    *value = Some(field);
                }
            }
        }
    }

    Ok(result_types)
}
