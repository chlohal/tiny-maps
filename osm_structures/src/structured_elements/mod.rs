use minimal_storage::{pooled_storage::Pool, varint::ToVarint};
use osm_value_atom::LiteralValue;

pub mod address;
pub mod contact;
pub mod colour;

#[inline]
pub(self) fn insert_with_byte(
    value: &Option<LiteralValue>,
    pool: &Pool<LiteralValue>,
    extra_storage: &mut Vec<u8>,
    byte: &mut u8,
    byte_index: u8,
) -> std::io::Result<()> {
    match value {
        Some(t) => {
            *byte |= 1 << byte_index;
            let id = pool.insert(t, ())?;
            id.write_varint(extra_storage)?;
            Ok(())
        }
        None => Ok(())
    }
}