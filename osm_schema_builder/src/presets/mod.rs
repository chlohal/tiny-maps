use std::{collections::HashSet, fs::read_dir, io::Write, path::{Path, PathBuf}};

use serde_json::Value;

use crate::util::{open_json, slugify, SlugificationMethod};


pub fn make_presets(
    write_to: &mut impl Write,
    folder: PathBuf,
    preset_id: &mut usize,
) -> std::io::Result<()> {
    let mod_name = slugify(
        folder.file_name()
            .expect("Modules should be a file name")
            .to_str()
            .unwrap(),
        SlugificationMethod::RustIdent,
    );

    write!(write_to, "\npub mod {mod_name} {{\n")?;

    for item in read_dir(folder)? {
        let file = item?;

        if file.file_name().eq_ignore_ascii_case("@templates") {
            continue;
        }

        if file.file_type().unwrap().is_dir() {
            make_presets(&mut *write_to, file.path().into(), preset_id)?;
        } else {
            let preset = open_json(file.path())
                .expect("presets should be valid JSON");

            let name = slugify(
                file.file_name()
                    .to_str()
                    .expect("ASCII filenames")
                    .replace(".json", ""),
                SlugificationMethod::RustStruct,
            );

            if make_single_preset(preset, write_to, name, *preset_id)? {
                *preset_id += 1;
            }
        }
    }

    write!(write_to, "}}")?;

    Ok(())
}

fn make_single_preset(
    preset: Value,
    write_to: &mut impl Write,
    name: String,
    id: usize,
) -> std::io::Result<bool> {
    let original_label = preset
        .get("name")
        .map(|x| x.as_str())
        .flatten()
        .expect("Presets must have a string 'name'");

    if original_label.starts_with('{') {
        return Ok(false);
    }

    let fields = get_field_types(&preset, FieldTypeProperty::Fields)
    .into_iter()
    .collect::<HashSet<_>>();

    if fields.is_empty() {
        return Ok(false);
    }


    write!(write_to, "pub struct {name} {{ // {original_label} \n")?;

    for field in fields {
        write!(write_to, "{field}")?;
    }


    write!(write_to, "}}\n\n")?;

    let tag_match_all_expr = preset.get("tags")
    .and_then(|x| x.as_object())
    .map(|x| x.iter().flat_map(|(k,v)| Some(match v.as_str()? {
        "*" => format!("tags.contains_key({k:?})"),
        v => format!("tags.contains({k:?}, {v:?})")
    })
).collect::<Vec<String>>()
).unwrap_or_default()
.join(" & ");

    writeln!(
        write_to,
        r"impl crate::structured_elements::schema::OsmPreset for {name} {{
        const PRESET_ID: usize = {id};

        fn match_set(tags: &mut osmpbfreader::Tags) -> Option<Self> {{
            if {tag_match_all_expr} {{
            
            }}
            None
        }}
    }}"
    )?;

    Ok(true)
}

fn reference_other_preset(other: &str, field: FieldTypeProperty) -> Vec<String> {
    let filename = other.replace('{', "").replace('}', "") + ".json";

    let path = Path::new("id-tagging-schema-data/presets").join(&filename);

    let json = open_json(&path)
    .or_else(|_| {
        //try with an underscore on the start of the basename i guess
        let underscore_path = path.with_file_name(format!("_{}", path.file_name().unwrap().to_str().unwrap()));

        open_json(&underscore_path)
    });

    match json {
        Ok(json) => get_field_types(&json, field),
        Err(err) => {
            println!("cargo::warning=Preset {filename} should exist and be valid JSON. Error: {err:?}");
            return Default::default()
        },
    }

    
}

#[derive(Clone, Copy)]
enum FieldTypeProperty {
    Fields,
    MoreFields
}

fn get_field_types(preset: &Value, prop: FieldTypeProperty) -> Vec<String> {

    let prop_str = match prop {
        FieldTypeProperty::Fields => "fields",
        FieldTypeProperty::MoreFields => "moreFields",
    };

    preset
        .get(prop_str)
        .and_then(|x| x.as_array())
        .map(|x| {
            x.iter()
                .flat_map(|fieldname| -> Option<Vec<String>> {
                    if fieldname.as_str()?.starts_with('{') {
                        Some(reference_other_preset(fieldname.as_str()?, prop))
                    } else {
                        let field = fieldname.as_str()?;


                        let fieldname = slugify(field, SlugificationMethod::RustIdent);
                        let typename = slugify(field, SlugificationMethod::RustIdent);
                        
                        let mut set = Vec::new();
                        let (type_wrap_start, type_wrap_end) = match prop {
                            FieldTypeProperty::Fields => ("", ""),
                            FieldTypeProperty::MoreFields => ("Option<", ">"),
                        };

                        set.push(format!("{fieldname}: {type_wrap_start}crate::structured_elements::schema::fields::{typename}{type_wrap_end},\n"));
                        Some(set)
                    }
                })
                .flatten()
                .collect()
        })
        .unwrap_or_default()
}