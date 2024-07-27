use osmpbfreader::Tags;

use crate::{compressor::literals::{literal_value::LiteralValue, string_prefix_view::StrAsciiPrefixView, Literal, LiteralPool}, storage::serialize_min::SerializeMinimal};

use super::insert_with_byte;

const MAX_TAG_LENGTH_PLUS_TWO: usize = 14;

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
    fn none() -> Self {
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

    pub fn make_from_tags(tags: &mut Tags, prefix: &str) -> Self {
        let mut pstr = prefix.to_string();
        pstr.push(':');

        //multiply the max length by 4 to get the absolute worst-case scenario for byte length in utf8
        let mut tag_building = StrAsciiPrefixView::new(pstr, MAX_TAG_LENGTH_PLUS_TWO * 4);

        let phone = LiteralValue::from_tag_and_remove(tags, &tag_building.with("phone"));
        let website = LiteralValue::from_tag_and_remove(tags, &tag_building.with("website"));
        let email = LiteralValue::from_tag_and_remove(tags, &tag_building.with("email"));
        let facebook = LiteralValue::from_tag_and_remove(tags, &tag_building.with("facebook"));

        let instagram = LiteralValue::from_tag_and_remove(tags, &tag_building.with("instagram"));
        let vk = LiteralValue::from_tag_and_remove(tags, &tag_building.with("vk"));
        let twitter = LiteralValue::from_tag_and_remove(tags, &tag_building.with("twitter"));

        let prefix = if prefix != "contact" {
            Some(prefix.to_string().into())
        } else {
            None
        };

        

        Self {
            phone,
            website,
            email,
            facebook,
            instagram,
            vk,
            twitter,
            prefix,
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


impl SerializeMinimal for OsmContactInfo {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, pool: Self::ExternalData<'s>) -> std::io::Result<()> {
        let mut buf = Vec::new();
        buf.push(0);

        let mut header_byte = 0;

        insert_with_byte(
            &self.phone,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            7,
        )?;

        insert_with_byte(
            &self.website,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            6,
        )?;

        insert_with_byte(
            &self.email,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            5,
        )?;

        insert_with_byte(
            &self.facebook,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            4,
        )?;

        insert_with_byte(
            &self.instagram,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            3,
        )?;

        insert_with_byte(
            &self.vk,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            2,
        )?;

        insert_with_byte(
            &self.twitter,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            1,
        )?;

        insert_with_byte(
            &self.prefix,
            &mut pool.1,
            &mut buf,
            &mut header_byte,
            0,
        )?;

        buf[0] = header_byte;

        write_to.write_all(&buf)
    }
}