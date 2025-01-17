use minimal_storage::pooled_storage::Pool;
use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use minimal_storage::varint::ToVarint;
use osm_value_atom::LiteralValue;

use crate::auxil::string_prefix_view::StrAsciiPrefixView;

use super::insert_with_byte;

const MAX_TAG_LENGTH_PLUS_TWO: usize = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct OsmAddress {
    number: Option<LiteralValue>,
    street: Option<LiteralValue>,
    city: Option<LiteralValue>,
    state: Option<LiteralValue>,

    prefix: Option<LiteralValue>,
    province: Option<LiteralValue>,
    extra: Option<OsmAddressExtra>,
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
}

pub struct OsmAddressBuilder {
    number: Option<LiteralValue>,
    street: Option<LiteralValue>,
    city: Option<LiteralValue>,
    state: Option<LiteralValue>,
    province: Option<LiteralValue>,
    housename: Option<LiteralValue>,
    unit: Option<LiteralValue>,
    floor: Option<LiteralValue>,
    postbox: Option<LiteralValue>,
    full: Option<LiteralValue>,
    postcode: Option<LiteralValue>,
    hamlet: Option<LiteralValue>,
    suburb: Option<LiteralValue>,
    subdistrict: Option<LiteralValue>,
    county: Option<LiteralValue>,
    door: Option<LiteralValue>,
    flats: Option<LiteralValue>,
    block: Option<LiteralValue>,
    block_number: Option<LiteralValue>,

    prefix: StrAsciiPrefixView,
}
impl OsmAddressBuilder {
    pub fn with_prefix(prefix: &str) -> Self {
        debug_assert!(prefix.ends_with(':') || prefix == "");
        
        let prefix = StrAsciiPrefixView::new(prefix, MAX_TAG_LENGTH_PLUS_TWO * 4 + prefix.len());

        Self {
            prefix,
            number: None,
            street: None,
            city: None,
            state: None,
            province: None,
            housename: None,
            unit: None,
            floor: None,
            postbox: None,
            full: None,
            postcode: None,
            hamlet: None,
            suburb: None,
            subdistrict: None,
            county: None,
            door: None,
            flats: None,
            block: None,
            block_number: None,
        }
    }

    pub fn update<S: for<'a> PartialEq<&'a str> + AsRef<str>>(
        &mut self,
        key: S,
        value: S,
    ) -> Option<(S, S)> {
        if key == self.prefix.with("housenumber") {
            self.number = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("street") {
            self.street = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("city") {
            self.city = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("state") {
            self.state = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("province") {
            self.province = Some(LiteralValue::from(&value));
            return None;
        }
        //EXTRA:
        else if key == self.prefix.with("housename") {
            self.housename = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("unit") {
            self.unit = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("floor") {
            self.floor = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("postbox") {
            self.postbox = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("full") {
            self.full = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("postcode") {
            self.postcode = Some(LiteralValue::from(&value));
            return None;
        }
        //EVEN MORE EXTRA:
        else if key == self.prefix.with("hamlet") {
            self.hamlet = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("suburb") {
            self.suburb = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("subdistrict") {
            self.subdistrict = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("county") {
            self.county = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("door") {
            self.door = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("flats") {
            self.flats = Some(LiteralValue::from(&value));
            return None;
        } else if key == self.prefix.with("block") {
            self.block = Some(LiteralValue::from(&value));
            return None;
        }

        Some((key, value))
    }

    pub fn to_option(self) -> Option<OsmAddress> {
        self.into()
    }
}

impl From<OsmAddressBuilder> for Option<OsmAddress> {
    fn from(from: OsmAddressBuilder) -> Self {
        let OsmAddressBuilder {
            number,
            street,
            city,
            state,
            province,
            housename,
            unit,
            floor,
            postbox,
            full,
            postcode,
            hamlet,
            suburb,
            subdistrict,
            county,
            door,
            flats,
            block,
            block_number,
            mut prefix,
        } = from;

        let prefix = prefix.with("");

        let prefix = if prefix == "addr" {
            Some(LiteralValue::from(prefix))
        } else {
            None
        };

        let even_more_extra = OsmAddressEvenMoreExtra {
            hamlet,
            suburb,
            subdistrict,
            county,
            door,
            flats,
            block,
            block_number,
        };

        let even_more_extra = if even_more_extra.is_none() {
            None
        } else {
            Some(even_more_extra)
        };

        let extra = OsmAddressExtra {
            housename,
            unit,
            floor,
            postbox,
            full,
            postcode,
            even_more_extra,
        };

        let extra = if extra.is_none() {
            None
        } else {
            Some(extra)
        };

        let address = OsmAddress {
            number,
            street,
            city,
            state,
            prefix,
            province,
            extra,
        };


        if address.is_none() {
            None
        } else {
            Some(address)
        }
    }
}

impl DeserializeFromMinimal for OsmAddress {
    type ExternalData<'d> = &'d Pool<LiteralValue>;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        _from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        todo!()
    }
}

impl SerializeMinimal for OsmAddress {
    type ExternalData<'a> = &'a Pool<LiteralValue>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        pool: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
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

                    let street_id = pool.insert(self.street.as_ref().unwrap(), ())?;
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
            pool,
            &mut extra_storage,
            &mut first_byte,
            6,
        )?;
        insert_with_byte(
            &self.street,
            pool,
            &mut extra_storage,
            &mut first_byte,
            5,
        )?;
        insert_with_byte(
            &self.city,
            pool,
            &mut extra_storage,
            &mut first_byte,
            4,
        )?;
        insert_with_byte(
            &self.state,
            pool,
            &mut extra_storage,
            &mut first_byte,
            3,
        )?;
        insert_with_byte(
            &self.province,
            pool,
            &mut extra_storage,
            &mut first_byte,
            2,
        )?;

        insert_with_byte(
            &self.prefix,
            pool,
            &mut extra_storage,
            &mut first_byte,
            1,
        )?;

        //if we have extra?
        if let Some(extra) = &self.extra {
            first_byte |= 1 << 0;

            insert_with_byte(
                &extra.housename,
                pool,
                &mut extra_storage,
                &mut second_byte,
                7,
            )?;
            insert_with_byte(
                &extra.unit,
                pool,
                &mut extra_storage,
                &mut second_byte,
                6,
            )?;
            insert_with_byte(
                &extra.floor,
                pool,
                &mut extra_storage,
                &mut second_byte,
                5,
            )?;
            insert_with_byte(
                &extra.postbox,
                pool,
                &mut extra_storage,
                &mut second_byte,
                4,
            )?;
            insert_with_byte(
                &extra.full,
                pool,
                &mut extra_storage,
                &mut second_byte,
                3,
            )?;
            insert_with_byte(
                &extra.postcode,
                pool,
                &mut extra_storage,
                &mut second_byte,
                2,
            )?;

            if let Some(even_more_extra) = &extra.even_more_extra {
                second_byte |= 1 << 1;

                insert_with_byte(
                    &even_more_extra.hamlet,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    7,
                )?;
                insert_with_byte(
                    &even_more_extra.suburb,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    6,
                )?;
                insert_with_byte(
                    &even_more_extra.subdistrict,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    5,
                )?;
                insert_with_byte(
                    &even_more_extra.county,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    4,
                )?;
                insert_with_byte(
                    &even_more_extra.door,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    3,
                )?;
                insert_with_byte(
                    &even_more_extra.flats,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    2,
                )?;
                insert_with_byte(
                    &even_more_extra.block,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    1,
                )?;
                insert_with_byte(
                    &even_more_extra.block_number,
                    pool,
                    &mut extra_storage,
                    &mut third_byte,
                    0,
                )?;

                extra_storage[2] = third_byte;
            }

            extra_storage[1] = second_byte;
        }

        extra_storage[0] = first_byte;

        return write_to.write_all(&extra_storage);
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct OsmAddressExtra {
    housename: Option<LiteralValue>,
    unit: Option<LiteralValue>,
    floor: Option<LiteralValue>,
    postbox: Option<LiteralValue>,

    full: Option<LiteralValue>,
    postcode: Option<LiteralValue>,
    even_more_extra: Option<OsmAddressEvenMoreExtra>,
}

impl OsmAddressExtra {
    fn is_none(&self) -> bool {
        self.housename.is_none()
            && self.unit.is_none()
            && self.floor.is_none()
            && self.postbox.is_none()
            && self.full.is_none()
            && self.postcode.is_none()
            && self.even_more_extra.is_none()
    }

}

#[derive(Clone, Debug, Default, PartialEq)]
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
}
