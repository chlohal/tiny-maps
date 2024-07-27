use std::collections::BTreeMap;

use osmpbfreader::Tags;

use crate::{compressor::{literals::{string_prefix_view::StrAsciiPrefixView, LiteralKey, WellKnownKeyVar}, varint::ToVarint}, storage::serialize_min::SerializeMinimal};

use super::{super::{
    literal_value::LiteralValue, Literal, LiteralPool,
}, insert_with_byte};

const MAX_TAG_LENGTH_PLUS_TWO: usize = 20;

#[derive(Clone)]
pub struct OsmAddress {
    number: Option<LiteralValue>,
    street: Option<LiteralValue>,
    city: Option<LiteralValue>,
    state: Option<LiteralValue>,

    prefix: Option<LiteralValue>,
    province: Option<LiteralValue>,
    extra: Option<OsmAddressExtra>,
}

impl From<OsmAddress> for Literal {
    fn from(value: OsmAddress) -> Self {
        Literal::WellKnownKeyVar(WellKnownKeyVar::Address(value))
    }
}

impl OsmAddress {
    pub fn as_option(self) -> Option<Self> {
        if self.is_none() {
            None
        } else {
            Some(self)
        }
    }
    pub fn is_none(&self) -> bool {
        self.state.is_none()
            && self.number.is_none()
            && self.street.is_none()
            && self.city.is_none()
            && self.prefix.is_none()
            && self.province.is_none()
            && self.extra.is_none()
    }

    pub fn is_karlsruhe_minimal(&self) -> bool {
        self.state.is_none()
            && self.number.is_some()
            && self.street.is_some()
            && self.city.is_none()
            && self.prefix.is_none()
            && self.province.is_none()
            && self.extra.is_none()
    }

    fn none() -> Self {
        Self {
            number: None,
            street: None,
            city: None,
            state: None,
            prefix: None,
            province: None,
            extra: None,
        }
    }

    pub fn make_from_tags(tags: &mut Tags, prefix: &str) -> Self {

        //multiply the max length by 4 to get the absolute worst-case scenario for byte length in utf8
        let mut tag_building = StrAsciiPrefixView::new(prefix, MAX_TAG_LENGTH_PLUS_TWO * 4);

        let number = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":housenumber"));
        let street = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":street"));
        let city = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":city"));
        let state = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":state"));

        let province = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":province"));

        if number.is_none()
            && street.is_none()
            && city.is_none()
            && state.is_none()
            && province.is_none()
        {
            return Self::none();
        }

        let prefix = if prefix != "addr" {
            Some(prefix.to_string().into())
        } else {
            None
        };

        let extra = OsmAddressExtra::make_from_tags(tags, &mut tag_building);

        Self {
            number,
            street,
            city,
            state,
            prefix,
            province,
            extra: if extra.is_none() { None } else { Some(extra) },
        }
    }
}

impl SerializeMinimal for OsmAddress {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, pool: Self::ExternalData<'s>) -> std::io::Result<()> {
        let mut first_byte = 0b0000_0000u8;

        let mut extra_storage = Vec::<u8>::new();

        //put a 0 in there.
        //it's more convenient to do this now and then plop the first_byte
        //in later, so we can get mutable access to first_byte for now.
        extra_storage.push(0);

        if self.is_karlsruhe_minimal() {
            match self.number.as_ref().unwrap().as_number() {
                Some(num) if num > 0 && num <= 0b11_1111 + 1 => {
                    first_byte |= 0b1000_0000;
                    first_byte |= 0b0100_0000;
                    first_byte |= (num - 1) as u8;

                    let street_id = pool.1.insert(self.street.as_ref().unwrap())?;
                    street_id.write_varint(&mut extra_storage)?;

                    extra_storage[0] = first_byte;
                    return write_to.write_all(&extra_storage);
                }
                _ => {}
            }
        }

        //alright, let's do the standard, non-niche'd version!

        //first, check if we'll need 2nd and 3rd header bytes
        //(i.e. if we're using Extra and EvenMoreExtra)
        let mut second_byte = 0u8;
        let mut third_byte = 0u8;

        //just like the original first_byte, just insert 0s for now
        //we'll put the actual values in later :)
        match &self.extra {
            Some(e) => {
                extra_storage.push(0u8);

                if e.even_more_extra.is_some() {
                    extra_storage.push(0u8);
                }
            }
            None => {}
        }

        //& use the quick function defined above to pop the values in.
        insert_with_byte(
            &self.number,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            6,
        )?;
        insert_with_byte(
            &self.street,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            5,
        )?;
        insert_with_byte(
            &self.city,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            4,
        )?;
        insert_with_byte(
            &self.state,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            3,
        )?;
        insert_with_byte(
            &self.province,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            2,
        )?;

        insert_with_byte(
            &self.prefix,
            &mut pool.1,
            &mut extra_storage,
            &mut first_byte,
            1,
        )?;

        //if we have extra?
        if let Some(extra) = &self.extra {
            first_byte |= 1 << 0;

            insert_with_byte(
                &extra.housename,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                7,
            )?;
            insert_with_byte(
                &extra.unit,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                6,
            )?;
            insert_with_byte(
                &extra.floor,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                5,
            )?;
            insert_with_byte(
                &extra.postbox,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                4,
            )?;
            insert_with_byte(
                &extra.full,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                3,
            )?;
            insert_with_byte(
                &extra.postcode,
                &mut pool.1,
                &mut extra_storage,
                &mut second_byte,
                2,
            )?;

            if let Some(even_more_extra) = &extra.even_more_extra {
                second_byte |= 1 << 1;

                insert_with_byte(
                    &even_more_extra.hamlet,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    7,
                )?;
                insert_with_byte(
                    &even_more_extra.suburb,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    6,
                )?;
                insert_with_byte(
                    &even_more_extra.subdistrict,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    5,
                )?;
                insert_with_byte(
                    &even_more_extra.county,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    4,
                )?;
                insert_with_byte(
                    &even_more_extra.door,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    3,
                )?;
                insert_with_byte(
                    &even_more_extra.flats,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    2,
                )?;
                insert_with_byte(
                    &even_more_extra.block,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    1,
                )?;
                insert_with_byte(
                    &even_more_extra.block_number,
                    &mut pool.1,
                    &mut extra_storage,
                    &mut third_byte,
                    0,
                )?;

                extra_storage[2] = third_byte;
            }

            if let Some(arbitrary) = &extra.arbitrary {
                for kv in arbitrary.iter() {

                    let id = LiteralPool::<Literal>::insert(pool, kv)?;

                    id.write_varint(&mut extra_storage)?;
                }
            }

            extra_storage[1] = second_byte;
        }

        extra_storage[0] = first_byte;

        return write_to.write_all(&extra_storage);
    }
}

#[derive(Clone)]
pub struct OsmAddressExtra {
    housename: Option<LiteralValue>,
    unit: Option<LiteralValue>,
    floor: Option<LiteralValue>,
    postbox: Option<LiteralValue>,

    full: Option<LiteralValue>,
    postcode: Option<LiteralValue>,
    even_more_extra: Option<OsmAddressEvenMoreExtra>,
    //ONLY Literal::KeyVar
    arbitrary: Option<Vec<Literal>>,
}
impl OsmAddressExtra {
    fn none() -> Self {
        Self {
            housename: None,
            unit: None,
            floor: None,
            postbox: None,
            full: None,
            postcode: None,
            even_more_extra: None,
            arbitrary: None,
        }
    }

    fn is_none(&self) -> bool {
        self.housename.is_none() &&
        self.unit.is_none() &&
        self.floor.is_none() &&
        self.postbox.is_none() &&
        self.full.is_none() &&
        self.postcode.is_none() &&
        self.even_more_extra.is_none() &&
        self.arbitrary.is_none()
    }

    fn make_from_tags(tags: &mut Tags, tag_building: &mut StrAsciiPrefixView) -> Self {
        let housename = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":housename"));
        let unit = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":unit"));
        let floor = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":floor"));
        let postbox = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":postbox"));
        let full = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":full"));
        let postcode = LiteralValue::from_tag_and_remove(tags, &tag_building.with(":postcode"));

        let mut arbitrary = Vec::new();

        let prefix = tag_building.with(":");
        tags.retain(|k, v| {
            if k.starts_with(prefix) {
                let k_packed = LiteralKey::from(&k[prefix.len()..]);
                let v_packed = LiteralValue::from(v);

                arbitrary.push(Literal::KeyVar(k_packed, v_packed));

                return true;
            }
            false
        });

        if housename.is_none()
            && unit.is_none()
            && floor.is_none()
            && postbox.is_none()
            && full.is_none()
            && postcode.is_none()
            && arbitrary.len() == 0
        {
            return Self::none();
        }

        let even_more_extra = OsmAddressEvenMoreExtra::make_from_tags(tags, tag_building);

        Self {
            housename,
            unit,
            floor,
            postbox,
            full,
            postcode,
            even_more_extra: if even_more_extra.is_none() { None } else { Some(even_more_extra) },
            arbitrary: if arbitrary.is_empty() { None } else { Some(arbitrary) },
        }
    }
}

#[derive(Clone)]
pub struct OsmAddressEvenMoreExtra {
    hamlet: Option<LiteralValue>,
    suburb: Option<LiteralValue>,
    subdistrict: Option<LiteralValue>,
    county: Option<LiteralValue>,

    door: Option<LiteralValue>,
    flats: Option<LiteralValue>,
    block: Option<LiteralValue>,
    block_number: Option<LiteralValue>,
}
impl OsmAddressEvenMoreExtra {
    pub fn is_none(&self) -> bool {
        return self.hamlet.is_none()
            && self.suburb.is_none()
            && self.subdistrict.is_none()
            && self.county.is_none()
            && self.door.is_none()
            && self.flats.is_none()
            && self.block.is_none()
            && self.block_number.is_none();
    }
    fn make_from_tags(tags: &mut Tags, tag_building: &mut StrAsciiPrefixView) -> Self {
        Self {
            hamlet: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":hamlet")),
            suburb: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":suburb")),
            subdistrict: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":subdistrict")),
            county: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":county")),
            door: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":door")),
            flats: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":flats")),
            block: LiteralValue::from_tag_and_remove(tags, &tag_building.with(":block")),
            block_number: LiteralValue::from_tag_and_remove(
                tags,
                &tag_building.with(":block_number"),
            ),
        }
    }
}
