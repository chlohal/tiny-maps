use std::{fs::File, path::PathBuf};

use serde_json::Value;

use super::load_field::FieldData;

pub fn load_field_data(path: &PathBuf, value: Value) -> Option<FieldData> {
    let field_type = value.get("type").and_then(|x| x.as_str()).unwrap();

    Some(match field_type {
        "wikipedia" | "wikidata" | "textarea" | "identifier" | "url" | "email" | "tel" | "text" => {
            FieldData::Text {
                key: get_key(&value)?,
            }
        }
        "number" => FieldData::Number {
            key: get_key(&value)?,
        },
        "localized" => FieldData::LocalizedString {
            root_key: get_key(&value)?,
        },
        "colour" => FieldData::Colour {
            key: get_key(&value)?,
        },
        "date" => FieldData::Date {
            key: get_key(&value)?,
        },
        "combo" | "onewayCheck" | "radio" => FieldData::Combo {
            key: get_key(&value)?,
            options: load_options(&path, &value),
        },
        "typeCombo" => FieldData::Combo {
            key: get_key(&value)?,
            options: {
                let mut opts = load_options(&path, &value);

                let y = "yes".to_string();
                if !opts.contains(&y) {
                    opts.push(y);
                }
                opts
            },
        },
        "multiCombo" => {
            let options = load_options(&path, &value);
            let root_key = get_key(&value)?;
            let keys = options
                .into_iter()
                .map(|x| format!("{root_key}{x}"))
                .collect();

            FieldData::MultiYesCombo {
                label: root_key,
                keys,
            }
        }
        "manyCombo" | "structureRadio" => FieldData::MultiYesCombo {
            label: get_label(&value)?,
            keys: load_options(&path, &value),
        },
        "networkCombo" => FieldData::Combo {
            key: get_key(&value)?,
            options: load_options(&path, &value),
        },
        "semiCombo" => FieldData::SemiCombo {
            key: get_key(&value)?,
            options: load_options(&path, &value),
        },
        "directionalCombo" => {
            let left_right_key = value.get("keys").and_then(Value::as_array).unwrap();
            let left_key = left_right_key[0].as_str().unwrap().to_string();
            let right_key = left_right_key[1].as_str().unwrap().to_string();

            FieldData::DirectionalCombo {
                root_key: get_key(&value)?,
                left_key,
                right_key,
                options: load_options(&path, &value),
            }
        },
        "defaultCheck" | "check" => FieldData::Checkbox {
            key: get_key(&value)?,
        },
        "access" => FieldData::Access {},
        "address" => FieldData::Address {
            prefix: get_key(&value)?,
        },
        "unit_number" | "roadspeed" | "roadheight" => FieldData::UnitNumber {
            key: get_key(&value)?,
        },
        "restrictions" => return None,
        unexpected => panic!("Unexpected type value {unexpected}"),
    })
}

fn get_label(value: &Value) -> Option<String> {
    Some(value.get("label")?.as_str()?.to_string())
}

fn load_options(path: &PathBuf, value: &Value) -> Vec<String> {
    let field = value.as_object().unwrap();

    let field_type = field.get("type").and_then(|x| x.as_str()).unwrap();

    if field_type == "structureRadio" || field_type == "manyCombo" {
        return field
            .get("keys")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
    }

    let inlined_options = field
        .get("options")
        .and_then(|x| x.as_array())
        .map(|x| {
            x.into_iter()
                .flat_map(|x| x.as_str().map(|x| x.to_string()))
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            field
                .get("strings")
                .and_then(|x| x.as_object())
                .and_then(|x| x.get("options"))
                .and_then(|x| x.as_object())
                .and_then(|x| Some(x.keys().cloned().collect()))
        })
        .or_else(|| {
            field
                .get("stringsCrossReference")
                .and_then(|x| x.as_str())
                .map(|x| x.replace('{', "").replace('}', ""))
                .and_then(|x| open_field_from_path(x))
                .map(|x| load_options(path, &x))
        });

    if let Some(opts) = inlined_options {
        return opts;
    }

    //no options were found in the JSON. use the tag to search for taginfos.
    let key = field.get("key").unwrap().as_str().unwrap();

    let taginfo: Value = reqwest::blocking::get(format!("https://taginfo.openstreetmap.org/api/4/key/values?key={key}&filter=all&lang=en&sortname=count&sortorder=desc&rp=62&page=1"))
        .map(|x| serde_json::from_reader(x))
        .unwrap()
        .unwrap();

    let values = taginfo.get("data").unwrap().as_array().unwrap();

    let mut percentage_occupied = 0.;
    let mut options = Vec::new();

    for option in values.iter().take(62) {
        let fraction = option.get("fraction").unwrap().as_f64().unwrap();

        let value = option.get("value").unwrap().as_str().unwrap().to_string();
        for itm in value.split(";") {
            let itm = itm.to_string();
            if itm != "" && !options.contains(&itm) {
                options.push(itm);
            }
        }

        percentage_occupied += fraction;

        if percentage_occupied >= 0.95 {
            break;
        }
    }

    let mut field = field.clone();
    field.insert("options".to_owned(), options.clone().into());

    serde_json::to_writer_pretty(File::create(path).unwrap(), &field).unwrap();

    options
}

fn open_field_from_path(field_path: String) -> Option<Value> {
    serde_json::from_reader(
        File::open(&format!("id-tagging-schema-data/fields/{field_path}.json"))
            .or_else(|_| {
                File::open(&format!(
                    "id-tagging-schema-data/fields/{field_path}/index.json"
                ))
            })
            .unwrap(),
    )
    .ok()
}

fn get_key(value: &Value) -> Option<String> {
    Some(value.get("key")?.as_str()?.to_string())
}
