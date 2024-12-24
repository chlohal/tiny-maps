use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::Write,
};

use crate::util::open_json;

pub enum TagMatcher {
    Key(String),
    Tag(String, String),
}

pub enum DeprecationInstruction {
    Remove(String),
    Match(TagMatcher),
    LoadStore(String, String),
    Write(String, String),
}

impl DeprecationInstruction {
    pub fn is_remove(&self) -> bool {
        match self {
            DeprecationInstruction::Remove(_) => true,
            _ => false,
        }
    }
    pub fn is_match(&self) -> bool {
        match self {
            DeprecationInstruction::Match(_) => true,
            _ => false,
        }
    }
    pub fn is_load_store(&self) -> bool {
        match self {
            DeprecationInstruction::LoadStore(_, _) => true,
            _ => false,
        }
    }
    pub fn is_write(&self) -> bool {
        match self {
            DeprecationInstruction::Write(_, _) => true,
            _ => false,
        }
    }
    pub fn as_match_expression(&self) -> Option<String> {
        let DeprecationInstruction::Match(mat) = self else {
            return None;
        };

        Some(match mat {
            TagMatcher::Key(k) => format!("key == {k:?}"),
            TagMatcher::Tag(k, v) => format!("key == {k:?} && value == {v:?}"),
        })
    }

    fn as_write_expression(&self) -> Option<String> {
        let DeprecationInstruction::Write(key, value) = self else {
            return None;
        };

        if value == "*" {
            Some(format!("({key:?}.into(), \"yes\".into())"))
        } else {
            Some(format!("({key:?}.into(), {value:?}.into())"))
        }
    }

    fn as_single_load_expression(&self) -> Option<String> {
        let DeprecationInstruction::LoadStore(_from, to) = self else {
            return None;
        };

        Some(format!("({to:?}.into(), value)"))
    }

    fn load_from_key(&self) -> Option<&String> {
        match self {
            DeprecationInstruction::LoadStore(from, _) => Some(from),
            _ => None,
        }
    }

    fn match_key(&self) -> Option<&String> {
        match self {
            DeprecationInstruction::Match(TagMatcher::Key(k)) => Some(k),
            DeprecationInstruction::Match(TagMatcher::Tag(k, _)) => Some(k),
            _ => None,
        }
    }
}

fn load_discards() -> Vec<Vec<DeprecationInstruction>> {
    let discard_file =
        open_json("id-tagging-schema-data/discarded.json").expect("deprecated.json should exist");

    discard_file
        .as_object()
        .expect("discarded.json must be an object")
        .into_iter()
        .map(|(k, _)| {
            vec![DeprecationInstruction::Match(TagMatcher::Key(
                k.to_string(),
            ))]
        })
        .collect()
}

pub fn load_deprecations() -> Vec<Vec<DeprecationInstruction>> {
    return vec![];

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
        .map(|(old, new)| make_deprecation_instructions(old, new))
        .chain(load_discards())
        .collect()
}

fn make_deprecation_instructions(
    old: HashMap<&String, &str>,
    new: HashMap<&String, &str>,
) -> Vec<DeprecationInstruction> {
    let matchers = old.iter().map(|(k, v)| match *v {
        "*" => DeprecationInstruction::Match(TagMatcher::Key(k.to_string())),
        _ => DeprecationInstruction::Match(TagMatcher::Tag(k.to_string(), v.to_string())),
    });

    let old_keys = old.keys().collect::<HashSet<_>>();
    let new_keys = new.keys().collect::<HashSet<_>>();
    let removed_keys = old_keys
        .difference(&new_keys)
        .map(|x| DeprecationInstruction::Remove(x.to_string()));

    let loadstore = new
        .iter()
        .find(|(_, v)| **v == "$1")
        .map(|(to_key, _)| {
            let from_key = old
                .iter()
                .find(|(_, v)| **v == "*")
                .expect("If there is a wildcard acceptor, there must be a wildcard")
                .0
                .to_string();

            DeprecationInstruction::LoadStore(from_key, to_key.to_string())
        })
        .into_iter();

    let new_keys = new
        .iter()
        .filter(|(_, v)| **v != "$1")
        .map(|(k, v)| DeprecationInstruction::Write(k.to_string(), v.to_string()));

    matchers
        .chain(removed_keys)
        .chain(loadstore)
        .chain(new_keys)
        .collect()
}

fn write_depr(
    write_to: &mut impl Write,
    instructions: Vec<DeprecationInstruction>,
) -> std::io::Result<()> {
    let matches = instructions
        .iter()
        .filter(|x| x.is_match())
        .collect::<Vec<_>>();
    let is_singlematch = matches.len() == 1;
    let singlematch = if is_singlematch {
        instructions.iter().find(|x| x.is_match())
    } else {
        None
    };

    let load_store = instructions.iter().find(|x| x.is_load_store());

    if is_singlematch {
        let matcher = singlematch.unwrap().as_match_expression().unwrap();
        let emitters = instructions.iter().filter_map(|x| x.as_write_expression());
        let loadstorer = load_store
            .and_then(|x| x.as_single_load_expression())
            .into_iter();

        let emitter_exprs = emitters.chain(loadstorer).collect::<Vec<_>>();

        //if there's exactly one emitter, we can use an array because it'll have length 1 regardless of branch
        //otherwise, we need to use the `vec` macro to make a vector.
        //if there are 0 emitters, then we're good to use a `filter` (as filter_map b/c of ownership)
        let (method, typ_prefix, typ_suffix) = match emitter_exprs.len() {
            1 => ("map", "", ""),
            0 => ("filter_map", "Some(", ")"),
            _ => ("flat_map", "vec![", "]"),
        };

        let (typ_prefix_mch, typ_suffix_mch) = if emitter_exprs.is_empty() {
            ("None", "")
        } else {
            (typ_prefix, typ_suffix)
        };

        let emitter_exprs = emitter_exprs.join(",\n");

        return write!(
            write_to,
            r".{method}(|(key, value)| {{
            if {matcher} {{
    {typ_prefix_mch}{emitter_exprs}{typ_suffix_mch}
            }} else {{
                {typ_prefix}(key, value){typ_suffix}
            }}
        }})"
        );
    }

    //non-singlematch: we need to statefully iterate

    //whether or not we have a load/store, the state type is always a `Vec<(key, value)>`
    //because keys are unique, we coouulldd just use the number of keys we've seen to determine if all matches are fulfilled or not,
    //but we have to hold them back to see if we've found everything or not, so we've got to have a vec.
    //at the end, the `stateful_filter` will unbuffer everything we don't consume back into the iterator

    //to make loads faster, since there will always be at most one store, we put it in the first slot.

    let matcher_ops = matches
        .iter()
        .map(|x| {
            let matchexpr = x.as_match_expression().unwrap();
            if load_store.is_some_and(|ls| Some(ls.load_from_key().unwrap()) == x.match_key()) {
                format!("if {matchexpr} {{ if state.len() == 0 {{ state.push((key, value)); }} else {{ let o = std::mem::replace(&mut state[0], (key,value)); state.push(o); }} }}")
            } else {
                format!("if {matchexpr} {{ state.push((key, value)); }}")
            }
        })
        .collect::<Vec<_>>()
        .join("else \n");

    let matcher_target_number = matches.len();

    let ls_code = if load_store.is_some() {
        format!("let value = state.swap_remove(0).1;")
    } else {
        String::new()
    };

    let emitters = instructions.iter().filter_map(|x| x.as_write_expression());
    let loadstorer = load_store
        .and_then(|x| x.as_single_load_expression())
        .into_iter();

    let emitter_exprs = emitters.chain(loadstorer).collect::<Vec<_>>();

    let emitter_exprs = emitter_exprs.join(",\n");

    return write!(
        write_to,
        r".stateful_filter(Vec::new(), |state, (key, value)| {{

            {matcher_ops} else {{ return vec![(key, value)]; }}

            if state.len() == {matcher_target_number} {{
                {ls_code}
                state.clear();
                vec![{emitter_exprs}]
            }} else {{
                vec![] 
            }}
        }})"
    );
}

pub fn make_deprecations(write_to: &mut impl Write) -> std::io::Result<()> {
    write!(
        write_to,
        "use crate::stateful_iterate::StatefulIterate;\n\npub fn apply_deprecations<S: From<&'static str> + PartialEq<&'static str>>(tags: impl Iterator<Item = (S,S)>) -> impl Iterator<Item = (S, S)> {{\n"
    )?;

    for depr in load_deprecations() {
        write!(write_to, "let tags = tags")?;
        write_depr(write_to, depr)?;
        write!(write_to, ";\n")?;
    }

    write!(write_to, "tags\n}}")?;

    Ok(())
}
