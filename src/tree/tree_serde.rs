use std::marker::PhantomData;

use serde::{
    de::{self, DeserializeOwned, SeqAccess, Visitor},
    ser::SerializeTuple,
    Deserialize, Serialize,
};

use super::LongLatTree;

impl<T: Serialize + DeserializeOwned> Serialize for LongLatTree<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tuple = serializer.serialize_tuple(6)?;

        tuple.serialize_element(&*self.root_tree_info)?;
        tuple.serialize_element(&self.bbox)?;
        tuple.serialize_element(&self.direction)?;
        tuple.serialize_element(&self.children)?;
        tuple.serialize_element(&self.left_right_split)?;
        tuple.serialize_element(&self.id)?;

        tuple.end()
    }
}

impl<'de, T: Serialize + DeserializeOwned> Deserialize<'de> for LongLatTree<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_tuple(6, LongLatTreeVisitor(PhantomData))
    }
}

struct LongLatTreeVisitor<T>(PhantomData<T>);

impl<'de, T: DeserializeOwned + Serialize> Visitor<'de> for LongLatTreeVisitor<T> {
    type Value = LongLatTree<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a tuple of the serialized tree")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<LongLatTree<T>, V::Error>
    where
        V: serde::de::SeqAccess<'de>,
    {
        let root_tree_info = std::rc::Rc::new(seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?);
        let bbox = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
        let direction = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
        let children = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
        let left_right_split = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(4, &self))?;
        let id = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?;

        Ok(LongLatTree {
            root_tree_info,
            bbox,
            direction,
            children,
            left_right_split,
            id,
        })
    }
}

struct DurationVisitor;

impl<'de> Visitor<'de> for DurationVisitor {
    fn visit_seq<V>(self, mut seq: V) -> Result<Duration, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let secs = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let nanos = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        Ok(Duration { secs, nanos })
    }

    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a tuple of the serialized tree")
    }
}

struct Duration {
    secs: u32,
    nanos: u32,
}
