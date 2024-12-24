use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

#[derive(PartialEq, Clone, Debug, Copy)]
#[repr(u8)]
pub enum OsmColour {
    StandardColour(StandardColour),
    Hex(u8, u8, u8) //r, g, and b are 0-5
}

//SAFETY: transmutes are used with this type. It should have exactly
//        0b1111 variants. Do not remove variants please!
#[derive(PartialEq, Clone, Debug, Copy)]
#[repr(u8)]
pub enum StandardColour {
    Black = 0,
    Brown = 1,
    Yellow = 2,
    Green = 3,
    GrayWithA = 4,
    GreyWithE = 5,
    White = 6,
    Blue = 7,
    Orange = 8,
    Silver = 9,
    Purple = 10,
    DarkGreen = 11,
    Beige = 12,
    Maroon = 13,
    Red = 14,
    RedWhite = 15,
}

impl OsmColour {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "black" => Some(Self::StandardColour(StandardColour::Black)),
            "brown" => Some(Self::StandardColour(StandardColour::Brown)),
            "yellow" => Some(Self::StandardColour(StandardColour::Yellow)),
            "green" => Some(Self::StandardColour(StandardColour::Green)),
            "gray" => Some(Self::StandardColour(StandardColour::GrayWithA)),
            "grey" => Some(Self::StandardColour(StandardColour::GreyWithE)),
            "white" => Some(Self::StandardColour(StandardColour::White)),
            "blue" => Some(Self::StandardColour(StandardColour::Blue)),
            "orange" => Some(Self::StandardColour(StandardColour::Orange)),
            "silver" => Some(Self::StandardColour(StandardColour::Silver)),
            "purple" => Some(Self::StandardColour(StandardColour::Purple)),
            "darkgreen" => Some(Self::StandardColour(StandardColour::DarkGreen)),
            "beige" => Some(Self::StandardColour(StandardColour::Beige)),
            "maroon" => Some(Self::StandardColour(StandardColour::Maroon)),
            "red/white" => Some(Self::StandardColour(StandardColour::RedWhite)),
            _ => {
                if s.starts_with('#') && s.len() == 7 {
                    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
                    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
                    let b = u8::from_str_radix(&s[5..7], 16).ok()?;

                    const VALUES: [u8; 6] = [0x00, 0x33, 0x66, 0x99, 0xcc, 0xff];

                    let ri = VALUES.iter().position(|x| *x == r)? as u8;
                    let gi = VALUES.iter().position(|x| *x == g)? as u8;
                    let bi = VALUES.iter().position(|x| *x == b)? as u8;

                    Some(Self::Hex(ri, gi, bi))
                } else {
                    None
                }
            }
        }
    }
}

impl SerializeMinimal for OsmColour {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let b = match self {
            OsmColour::StandardColour(c) => {
                *c as u8
            },
            OsmColour::Hex(r, g, b) => {
                let cube_index = r * 6 * 6 + g * 6 + b;

                cube_index + 16
            },
        };

        write_to.write_all(&[b])
    }
}

impl DeserializeFromMinimal for OsmColour {
    type ExternalData<'d> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let mut b = [0];
        from.read_exact(&mut b)?;
        let b = b[0];

        if b < 16 {
            //safety: StandardColour is repr(u8) and defines a variant for every u8 value under 16
            return Ok(Self::StandardColour(unsafe { std::mem::transmute(b) }))
        }

        let cube_index = b - 16;

        debug_assert!(cube_index <= 6*6*6);

        let r = cube_index / 36;
        let b = (cube_index / 6) % 6;
        let g = cube_index % 6;

        Ok(Self::Hex(r, g, b))
    }
    
    fn read_past<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> std::io::Result<()> {
        from.read_exact(&mut [0])
    }
    
}