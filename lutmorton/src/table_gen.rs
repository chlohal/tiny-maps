

pub const fn mortontable(shift: u16) -> [u16; 256] {
    let mut arr = [0; 256];

    let mut i = 0;
    while i < 256 {
        arr[i] = morton_value(i as u8) << shift;
        i += 1;
    }

    arr
}

pub const fn un_mortontable() -> [(u32, u32); 256] {
    let mut arr = [(0, 0); 256];

    let mut i = 0;
    while i < 256 {
        arr[i] = unmorton_value(i as u8);
        i += 1;
    }

    arr
}

pub const fn unmorton_value(value: u8) -> (u32, u32) {
    let mut a = 0;
    let mut b = 0;

    let mut i = 0;
    while i < u8::BITS {
        a |= (value & (1 << i)) >> (i / 2);
        b |= (value & (1 << (i + 1))) >> (1 + (i / 2));
        i += 2;
    }

    (a as u32, b as u32)
}

pub const fn morton_value(value: u8) -> u16 {
    let mut answer: u16 = 0;

    let mut i = 0;
    while i < u8::BITS {
        answer |= ((value & (1 << i)) as u16) << i;
        i += 1;
    }


    answer
}