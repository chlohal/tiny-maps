use crate::compressor::varint::ToVarint;

use super::{literal_value::LiteralValue, LiteralPool};

pub mod address;
pub mod public_transit;
pub mod shop_amenity;
pub mod contact;

#[inline]
pub(self) fn insert_with_byte(
    value: &Option<LiteralValue>,
    pool: &mut LiteralPool<LiteralValue>,
    extra_storage: &mut Vec<u8>,
    byte: &mut u8,
    byte_index: u8,
) -> std::io::Result<()> {
    match value {
        Some(t) => {
            *byte |= 1 << byte_index;
            let id = pool.insert(t)?;
            id.write_varint(extra_storage)?;
            Ok(())
        }
        None => Ok(())
    }
}