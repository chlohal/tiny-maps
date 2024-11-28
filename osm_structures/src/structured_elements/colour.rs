pub enum OsmColour {
    StandardColour(StandardColour),
    Hex(u8, u8, u8)
}

pub enum StandardColour {
    Black,
    Brown,
    Yellow,
    Green,
    GrayWithA,
    GreyWithE,
    White,
    Blue,
    Orange,
    Silver,
    Purple,
    DarkGreen,
    Beige,
    Maroon,
}

impl OsmColour {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "black" => Some(Self::StandardColour(StandardColour::Black)),
            "brown" => Some(Self::StandardColour(StandardColour::Brown)),
            "yellow" => Some(Self::StandardColour(StandardColour::Yellow)),
            "Green" => Some(Self::StandardColour(StandardColour::Green)),
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