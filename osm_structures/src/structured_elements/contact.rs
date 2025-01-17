use minimal_storage::bit_sections::Byte;
use minimal_storage::pooled_storage::{Pool, PooledId};
use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use minimal_storage::varint::ToVarint;
use osm_value_atom::LiteralValue;


use crate::auxil::string_prefix_view::StrAsciiPrefixView;

const MAX_TAG_LENGTH_PLUS_TWO: usize = 14;

#[derive(Clone, Debug, Default)]
pub struct OsmContactInfo {
    phone: Option<LiteralValue>,
    website: Option<LiteralValue>,
    email: Option<LiteralValue>,
    facebook: Option<LiteralValue>,

    instagram: Option<LiteralValue>,
    vk: Option<LiteralValue>,
    twitter: Option<LiteralValue>,
    prefix: Option<LiteralValue>,
}

impl OsmContactInfo {
    pub fn is_none(&self) -> bool {
        return self.phone.is_none() &&
        self.website.is_none() &&
        self.email.is_none() &&
        self.facebook.is_none() &&
        self.instagram.is_none() &&
        self.vk.is_none() &&
        self.twitter.is_none() &&
        self.prefix.is_none()
    }

    pub fn none() -> Self {
        Self {
            phone: None,
            website: None,
            email: None,
            facebook: None,
            instagram: None,
            vk: None,
            twitter: None,
            prefix: None,
        }
    }

    pub fn as_option(self) -> Option<Self> {
        if self.is_none() {
            return None;
        } else {
            return Some(self);
        }
    }
}

pub struct OsmContactInfoBuilder {
    prefix: StrAsciiPrefixView,
    contact_info: OsmContactInfo
}

impl OsmContactInfoBuilder {
    pub fn with_prefix(prefix: &str) -> Self {
        debug_assert!(prefix.ends_with(':') || prefix == "");
    
        Self {
            prefix: StrAsciiPrefixView::new(prefix, MAX_TAG_LENGTH_PLUS_TWO * 4),
            contact_info: OsmContactInfo::none(),
        }
    }

    pub fn update<S: for<'a> PartialEq<&'a str> + AsRef<str>>(
        &mut self,
        key: S,
        value: S,
    ) -> Option<(S, S)>  {
        if key == self.prefix.with("phone") {
            self.contact_info.phone = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("website") {
            self.contact_info.website = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("email") {
            self.contact_info.email = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("facebook") {
            self.contact_info.facebook = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("instagram") {
            self.contact_info.instagram = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("vk") {
            self.contact_info.vk = Some(LiteralValue::from(&value));
            return None;
        }
        if key == self.prefix.with("twitter") {
            self.contact_info.twitter = Some(LiteralValue::from(&value));
            return None;
        }

        return Some((key,value));
    }

    pub fn to_option(self) -> Option<OsmContactInfo> {
        self.into()
    }
}

impl From<OsmContactInfoBuilder> for Option<OsmContactInfo> {
    fn from(value: OsmContactInfoBuilder) -> Self {
        let OsmContactInfoBuilder { mut prefix, mut contact_info } = value;

        let prefix: Option<LiteralValue> = if prefix.with("") == "contact" {
            None 
        } else {
            Some(prefix.with("").into())
        };

        contact_info.prefix = prefix;

        contact_info.as_option()
    }
}


impl DeserializeFromMinimal for OsmContactInfo {
    type ExternalData<'d> = &'d Pool<LiteralValue>;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, _pool: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        let header: Byte = u8::deserialize_minimal(from, ())?.into();

        let slf = Self::default();

        if header.get_bit(0) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(1) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(2) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(3) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(4) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(5) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(6) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        if header.get_bit(7) != 0 {
            let _id = PooledId::deserialize_minimal(from, ())?;
        }

        return Ok(slf); //todo!("Reading from pool");
    }
}


impl SerializeMinimal for OsmContactInfo {
    type ExternalData<'a> = &'a Pool<LiteralValue>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, pool: Self::ExternalData<'s>) -> std::io::Result<()> {
        let mut buf = Vec::new();
        buf.push(0);

        let mut header_byte = Byte::from(0);

        header_byte.set_bit(0, self.phone.is_some());
        header_byte.set_bit(1, self.website.is_some());
        header_byte.set_bit(2, self.email.is_some());
        header_byte.set_bit(3, self.facebook.is_some());

        header_byte.set_bit(4, self.instagram.is_some());
        header_byte.set_bit(5, self.vk.is_some());
        header_byte.set_bit(6, self.twitter.is_some());
        header_byte.set_bit(7, self.prefix.is_some());

        header_byte.into_inner().minimally_serialize(write_to, ())?;


        if let Some(v) = &self.phone {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.website {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }
        if let Some(v) = &self.email {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.facebook {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.instagram {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.vk {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.twitter {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        if let Some(v) = &self.prefix {
            let id = pool.insert(&v, ())?;
            id.write_varint(write_to)?;
        }

        Ok(())
    }
}