use std::{
    collections::{BTreeMap, HashMap},
    fs::read_dir,
    io::Write,
    path::PathBuf,
};

use load_field::load_field;

use crate::util::{slugify, SlugificationMethod};

mod load_field;
mod load_field_data;
mod parser;

pub fn make_fields(write_to: &mut impl Write) -> std::io::Result<()> {
    write_to.write_all(
        br"use crate::stateful_iterate::StatefulIterate;
        
        pub trait OsmField {
        const FIELD_ID: usize;
        type State: Default;

        fn update_state<S: std::convert::From<&'static str> + PartialEq<&'static str>>(tag: (S, S), state: &mut Self::State) -> Option<(S, S)>;

        fn end_state(state: Self::State) -> Option<crate::fields::AnyOsmField>;
    }
    
    pub trait SingleOsmField {
        fn try_into_field<S: From<&'static str> + PartialEq<&'static str>>(key: S, value: S) -> Result<(S, S), AnyOsmField>;
    }",
    )?;

    let field_types = make_field_structs(write_to, "id-tagging-schema-data/fields".into(), &mut 0)?;

    write!(
        write_to,
        "pub enum AnyOsmField {{\n{}\n}}",
        field_types
            .iter()
            .map(
                |(
                    enum_name,
                    FieldReferenceData {
                        fully_qualified_struct_name: p, ..
                    },
                )| { format!("    {enum_name}(crate::{p}),\n",) }
            )
            .collect::<String>()
    )?;

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

    let (single_fields, multi_fields) = field_types.into_iter().partition::<Vec<_>, _>(|x| x.1.is_single_field);

    write_to.write_all(b"pub fn parse_tags_to_fields(tags: &mut osmpbfreader::Tags) -> (Vec<crate::fields::AnyOsmField>, osmpbfreader::Tags) {
    \n    let tags = crate::deprecations::apply_deprecations(tags.into_iter());\nlet mut fields = Vec::new();")?;

    for (_enum_name, fieldref) in multi_fields.iter() {
        let varname = slugify(&fieldref.fully_qualified_struct_name, SlugificationMethod::RustIdent);
        write!(write_to, "let mut {varname}_state: crate::{}::State = Default::default();\n", fieldref.fully_qualified_struct_name)?;
    }

    write_to.write_all(br"
    let tags = tags.filter_map(|(k,v)| {
    let t = (|| { ")?;
    for (_enum_name, fieldref) in single_fields {
        write!(write_to, "let (k,v) = crate::{}::try_into_field(k,v)?;\n", fieldref.fully_qualified_struct_name)?;
    }

    write_to.write_all(br"Ok((k,v)) })();")?;

    write_to.write_all(br"let (k,v) = match t {
        Ok(kv) => kv,
        Err(f) => {
            fields.push(f);
            return None;
        }
    };")?;

    for (_enum_name, fieldref) in multi_fields {
        let varname = slugify(&fieldref.fully_qualified_struct_name, SlugificationMethod::RustIdent);


        write!(write_to, "let (k,v) = crate::{}::update_state((k,v), &mut {varname}_state)?;\n", fieldref.fully_qualified_struct_name)?;
    }

    write_to.write_all(b"Some((k,v))\n});\n\n")?;

    
    write_to.write_all(b"(fields, tags)\n}")?;

    Ok(())
}

struct FieldReferenceData {
    fully_qualified_struct_name: String,
    is_single_field: bool,
}

fn make_field_structs(
    write_to: &mut impl Write,
    folder: PathBuf,
    field_id_counter: &mut usize,
) -> std::io::Result<BTreeMap<String, FieldReferenceData>> {
    let mut result_types = BTreeMap::new();

    let mut directory = read_dir(folder)?.collect::<Vec<_>>();

    //sort files first
    directory.sort_by_key(|x| x.as_ref().unwrap().file_type().as_ref().unwrap().is_file());

    for item in directory {
        let file = item?;

        if file.file_name().eq_ignore_ascii_case("@template") {
            continue;
        }

        let mod_name = slugify(
            file.file_name().into_string().unwrap().replace(".json", ""),
            SlugificationMethod::RustIdent,
        );

        if mod_name != "index" {
            write!(write_to, "\npub mod {mod_name} {{\n")?;
        }
        if file.file_type().unwrap().is_dir() {
            result_types.extend(make_field_structs(
                &mut *write_to,
                file.path().into(),
                field_id_counter,
            )?);
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
                    let enum_name = slugify(
                        format!(
                            "{} {}",
                            field.data.key().cloned().unwrap_or_default(),
                            field.data.typename()
                        ),
                        SlugificationMethod::RustStruct,
                    );

                    write!(
                        write_to,
                        "#[derive(Default, PartialEq)]\npub struct {}({});\n\n{}\n{}",
                        field.name,
                        field.data.datatype(),
                        field.data.datatype_def(&field.name).unwrap_or_default(),
                        field
                            .data
                            .traitimpl(&&field.name, *field_id_counter, &enum_name)
                    )
                    .unwrap();

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

                    *field_id_counter += 1;
                }
            }
        }

        if mod_name != "index" {
            write!(write_to, "}}\n")?;
        }
    }

    Ok(result_types)
}
