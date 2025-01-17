use std::{
    collections::HashMap,
    io::Write,
};

use crate::util::open_json;

pub enum TagMatcher {
    Key(String),
    Tag(String, String),
}

pub enum Deprecation {
    RenameKey {
        from: String,
        to: String,
    },
    RemoveKey {
        key: String,
    },
    RemoveKeyval {
        key: String,
        value: String,
    },
    RenameKeyval {
        from: (String, String),
        to: (String, String),
    },
    RenameValue {
        key: String,
        from: String,
        to: String,
    },
    ExpandSingleToMultiple {
        from: (String, String),
        to: Vec<(String, String)>,
    },
    StoreSingleAndExpandToMultiple {
        from_key: String,
        to_key: String,
        add: Vec<(String, String)>,
    },
    ArbitraryStateful {
        from: Vec<(String, String)>,
        to: Vec<(String, String)>,
        copy_value_fromto: Option<(String, String)>,
    },
}
impl Deprecation {
    fn as_arbitrary_stateful(
        &self,
    ) -> Option<(
        &Vec<(String, String)>,
        &Vec<(String, String)>,
        &Option<(String, String)>,
    )> {
        match self {
            Self::ArbitraryStateful {
                from,
                to,
                copy_value_fromto,
            } => Some((from, to, copy_value_fromto)),
            _ => None,
        }
    }
}

fn load_discards() -> Vec<Deprecation> {
    let discard_file =
        open_json("id-tagging-schema-data/discarded.json").expect("deprecated.json should exist");

    match discard_file {
        serde_json::Value::Object(o) => Some(o),
        _ => None,
    }
    .expect("discarded.json must be an object")
    .into_iter()
    .map(|(k, _)| Deprecation::RemoveKey { key: k })
    .collect()
}

pub fn load_deprecations() -> Vec<Deprecation> {
    let deprecated_file =
        open_json("id-tagging-schema-data/deprecated.json").expect("deprecated.json should exist");

    let deprecated = deprecated_file
        .as_array()
        .expect("deprecated.json must be an array")
        .into_iter()
        .map(|x| {
            x.as_object()
                .expect("deprecated.json must be an array of objects")
        });

    let mut deps = load_discards();

    deps.extend(
        deprecated
            .map(|x| {
                let old = x
                    .get("old")
                    .map(|x| x.as_object())
                    .flatten()
                    .expect("deprecated.json instructions must have an object 'old' key")
                    .iter()
                    .map(|(k, v)| {
                        (
                            k,
                            v.as_str()
                                .expect("all values in deprecation instructions must be strings!"),
                        )
                    })
                    .collect::<HashMap<_, _>>();
                let new = x
                    .get("replace")
                    .map(|x| {
                        x.as_object()
                            .expect("deprecated.json's 'replace' must be an object")
                            .iter()
                            .map(|(k, v)| {
                                (
                                    k,
                                    v.as_str().expect(
                                        "all values in deprecation instructions must be strings!",
                                    ),
                                )
                            })
                            .collect::<HashMap<_, _>>()
                    })
                    .unwrap_or_default();

                (old, new)
            })
            .map(|(old, new)| make_deprecation_instructions(old, new)),
    );

    deps
}

fn make_deprecation_instructions(
    old: HashMap<&String, &str>,
    new: HashMap<&String, &str>,
) -> Deprecation {
    //verify that this conforms to the invariant that every match WILL have a corresponding capture
    if old.values().any(|x| *x == "*") {
        if !new.is_empty() && !new.values().any(|x| *x == "$1") {
            panic!("Match without a corresponding capture!");
        }
    }

    //replace wildcards in `new` with "yes"
    let new = {
        let mut new = new;
        for v in new.values_mut() {
            if *v == "*" {
                *v = "yes";
            }
        }
        new
    };

    if old.len() == 1 {
        let single_old_value = old.into_iter().next().unwrap();
        //all removals
        if new.len() == 0 {
            if single_old_value.1 == "*" {
                return Deprecation::RemoveKey {
                    key: single_old_value.0.to_string(),
                };
            } else {
                return Deprecation::RemoveKeyval {
                    key: single_old_value.0.to_string(),
                    value: single_old_value.1.to_string(),
                };
            }
        }

        //all single swaps
        if new.len() == 1 {
            let single_new_value = new.into_iter().next().unwrap();

            if single_old_value.1 == "*" && single_new_value.1 == "$1" {
                return Deprecation::RenameKey {
                    from: single_old_value.0.to_string(),
                    to: single_new_value.0.to_string(),
                };
            }

            //no match without capture, so we MUST have a rename scenario

            if single_old_value.0 == single_new_value.0 {
                return Deprecation::RenameValue {
                    key: single_old_value.0.to_string(),
                    from: single_old_value.1.to_string(),
                    to: single_new_value.1.to_string(),
                };
            }

            return Deprecation::RenameKeyval {
                from: (
                    single_old_value.0.to_string(),
                    single_old_value.1.to_string(),
                ),
                to: (
                    single_new_value.0.to_string(),
                    single_new_value.1.to_string(),
                ),
            };
        }

        //expanding! we've dealt with the 0 and the 1 scenarios, so now we have the 2+ scenarios.

        //first, the annoying one of storing:
        if single_old_value.1 == "*" {
            let store_to_key = new.iter().find(|x| *x.1 == "$1").unwrap().0.to_string();

            let add = new
                .into_iter()
                .filter(|x| *x.0 != store_to_key)
                .map(|x| (x.0.to_string(), x.1.to_string()))
                .collect();

            return Deprecation::StoreSingleAndExpandToMultiple {
                from_key: single_old_value.0.to_string(),
                to_key: store_to_key,
                add,
            };
        }

        //and then, the simpler one!
        return Deprecation::ExpandSingleToMultiple {
            from: (
                single_old_value.0.to_string(),
                single_old_value.1.to_string(),
            ),
            to: new
                .into_iter()
                .map(|x| (x.0.to_string(), x.1.to_string()))
                .collect(),
        };
    }

    //thus, we've dealt with every stateless case: every case with only one old tag.
    //now, we deal with the arbitrary cases that can result from stateful matches.

    let match_key = old.iter().find(|x| *x.1 == "*").map(|x| x.0.to_string());

    let match_capture_keys = match_key.map(|from_key| {
        let to_key = new.iter().find(|x| *x.1 == "$1").unwrap().0.to_string();

        (from_key, to_key)
    });

    let mut old = old;
    let mut new = new;

    if let Some((from, to)) = &match_capture_keys {
        old.remove(from);
        new.remove(to);
    }

    Deprecation::ArbitraryStateful {
        from: old
            .into_iter()
            .map(|x| (x.0.to_string(), x.1.to_string()))
            .collect(),
        to: new
            .into_iter()
            .map(|x| (x.0.to_string(), x.1.to_string()))
            .collect(),
        copy_value_fromto: match_capture_keys,
    }
}

pub fn make_deprecations(write_to: &mut impl Write) -> std::io::Result<()> {
    write!(
        write_to,
        "pub fn apply_deprecations<S: From<&'static str> + for<'a> PartialEq<&'a str>>(mut tags: impl Iterator<Item = (S,S)>) -> impl Iterator<Item = (S, S)> {{\n"
    )?;

    let depres = load_deprecations();

    let state_storage_needed: Vec<_> = depres
        .iter()
        .enumerate()
        .filter_map(|(i, x)| match x {
            Deprecation::ArbitraryStateful { .. } => Some(i),
            _ => None,
        })
        .collect();

    //make state storage variables! allocate storage for stored variables if it's needed; otherwise, don't bother
    for i in state_storage_needed.iter() {
        if depres[*i].as_arbitrary_stateful().unwrap().2.is_some() {
            write!(
                write_to,
                "let mut state_{i}: (Option<(S, S)>, Vec<(S, S)>) = (None, vec![]);\n"
            )?;
        } else {
            write!(write_to, "let mut state_{i}: Vec<(S, S)> = vec![];\n")?;
        }
    }

    let drain_state_code = make_drain_state_code(&state_storage_needed, &depres);

    let deprecation_iteration_code = make_deprecation_iteration_code(&depres);

    write!(
        write_to,
        r"
        let mut drained = false;
        std::iter::from_fn(move || loop {{
            let Some(mut tag) = tags.next() else {{  {drain_state_code} }};

            "
    )?;

    for s in deprecation_iteration_code {
        write_to.write_all(s.as_bytes())?;
    }

    write!(
        write_to,
        r"
        
            return Some(vec![tag]);
        }}).flatten()
    
    "
    )?;

    write!(write_to, "}}")?;

    Ok(())
}

fn make_deprecation_iteration_code<'a>(
    deps: &'a Vec<Deprecation>,
) -> impl Iterator<Item = String> + 'a {
    deps.iter().enumerate().map(|(i,d)| {
        match d {
            Deprecation::RenameKey { from, to } => format!("if tag.0 == {from:?} {{ tag.0 = {to:?}.into(); return Some(vec![tag]); }}\n"),
            Deprecation::RemoveKey { key } => format!("if tag.0 == {key:?} {{ return Some(Vec::with_capacity(0)); }}\n"),
            Deprecation::RemoveKeyval { key, value } => format!("if tag.0 == {key:?} && tag.1 == {value:?} {{ return Some(Vec::with_capacity(0)); }}\n"),
            Deprecation::RenameKeyval { from: (from_k, from_v), to: (to_k, to_v) } => format!("if tag.0 == {from_k:?} && tag.1 == {from_v:?} {{ tag.0 = {to_k:?}.into(); tag.1 = {to_v:?}.into(); return Some(vec![tag]); }}\n"),
            Deprecation::RenameValue { key, from, to } => format!("if tag.0 == {key:?} && tag.1 == {from:?} {{ tag.1 = {to:?}.into(); return Some(vec![tag]); }}\n"),
            Deprecation::ExpandSingleToMultiple { from: (key, value), to } => format!("if tag.0 == {key:?} && tag.1 == {value:?} {{ return Some(vec![{}]);  }}\n", to.iter().map(|(k,v)| format!("({k:?}.into(), {v:?}.into()),") ).collect::<String>() ),
            Deprecation::StoreSingleAndExpandToMultiple { from_key, to_key, add } => format!("if tag.0 == {from_key:?} {{ return Some(vec![ ({to_key:?}.into(), tag.1), {} ]); }}\n", add.iter().map(|(k,v)| format!("({k:?}.into(), {v:?}.into()),") ).collect::<String>()),
            Deprecation::ArbitraryStateful { from, to, copy_value_fromto } => {
                let mut s = String::new();
                let from_taglen = from.len();
                let (succeed_code, vec_addr) = if let Some((copy_from, copy_to)) = copy_value_fromto {
                    let suc = format!(r"
                        if state_{i}.1.len() == {from_taglen} && state_{i}.0.is_some() {{
                            return Some(vec![({copy_to:?}.into(), state_{i}.0.take().unwrap().1), {}]);
                        }}
                    ", to.iter().map(|(k,v)| format!("({k:?}.into(), {v:?}.into()),")).collect::<String>() );


                    s += &format!(r"
                        if tag.0 == {copy_from:?} {{
                            state_{i}.0 = Some(tag);
                            {suc}
                            continue;
                        }}
                    ");

                    (suc, format!("state_{i}.1"))
                } else {
                    (format!(r"
                    if state_{i}.len() == {from_taglen} {{
                        return Some(vec![{}]);
                    }}
                ", to.iter().map(|(k,v)| format!("({k:?}.into(), {v:?}.into()),")).collect::<String>() ), format!("state_{i}"))
                };

                for (k, v) in from.iter() {
                    s += &format!("if tag.0 == {k:?} && tag.1 == {v:?} {{
                    {vec_addr}.push(tag);
                    {succeed_code}
                    continue;
                    }}")
                }

                s
            },
        }
    })
}

fn make_drain_state_code(index_of_stateful: &Vec<usize>, deps: &Vec<Deprecation>) -> String {
    let mut s = format!("if drained {{ return None; }} else {{ drained = true; }}\n let mut s = vec![];");

    for i in index_of_stateful.iter() {
        if deps[*i].as_arbitrary_stateful().unwrap().2.is_some() {
            s +=
                &format!("s.extend(state_{i}.0.take().into_iter()); s.extend(state_{i}.1.drain(..));\n");
        } else {
            s += &format!("s.extend(state_{i}.drain(..));\n");
        }
    }

    s += "return Some(s)";

    s
}
