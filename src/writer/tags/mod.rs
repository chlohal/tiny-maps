use std::io::Write;

macro_rules! match_str_trait_invoke {
    (
        trait $trait_name:path;
        fn $fn_name:ident $args:tt;
        for $name:ident;

        $($str:pat => $ty:ty),*
    ) => {
        {
        match $name {
            $(
                $str => Some(<$ty>::$fn_name $args),
            )*
            _ => None
        }
    }
    };
}

trait OsmTag {
    fn write(value:&str, writer: impl Write);
}

struct Amenity();

impl OsmTag for Amenity {
    fn write(value:&str, writer: impl Write) {
        todo!()
    }
}

pub fn write_tag_name(name: &str, value: &str, writer: impl Write) -> Option<()> {
    match_str_trait_invoke!{
        trait OsmTag;
        fn write(value, writer);
        for name;

        "amenity" => Amenity
    }
}