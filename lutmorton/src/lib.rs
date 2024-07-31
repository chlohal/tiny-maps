static MORTON_X: [u16; 256] = table_gen::mortontable(0);
static MORTON_Y: [u16; 256] = table_gen::mortontable(1);

static UNMORTON: [(u32, u32); 256] = table_gen::un_mortontable();

mod table_gen;

pub fn morton(x: u32, y: u32) -> u64 {
    let x = x as usize;
    let y = y as usize;

    MORTON_X[x & 0xff] as u64
        | MORTON_Y[y & 0xff] as u64
        | ((MORTON_X[(x & 0xff00) >> 8] as u64 | MORTON_Y[(y & 0xff00) >> 8] as u64) << 16)
        | ((MORTON_X[(x & 0xff0000) >> 16] as u64 | MORTON_Y[(y & 0xff0000) >> 16] as u64) << 32)
        | ((MORTON_X[(x & 0xff000000) >> 24] as u64 | MORTON_Y[(y & 0xff000000) >> 24] as u64)
            << 48)
}

pub fn unmorton(morton: u64) -> (u32, u32) {
    let (x0a, y0a) = UNMORTON[(morton & 0xff) as usize];
    let (x0b, y0b) = UNMORTON[(morton & 0xff00) as usize >> 8];
    let (x1a, y1a) = UNMORTON[(morton & 0xff0000) as usize >> 16];
    let (x1b, y1b) = UNMORTON[(morton & 0xff000000) as usize >> 24];
    let (x2a, y2a) = UNMORTON[(morton & 0xff00000000) as usize >> 32];
    let (x2b, y2b) = UNMORTON[(morton & 0xff0000000000) as usize >> 40];
    let (x3a, y3a) = UNMORTON[(morton & 0xff000000000000) as usize >> 48];
    let (x3b, y3b) = UNMORTON[(morton & 0xff00000000000000) as usize >> 56];

    let x = x0a | x0b << 4 | x1a << 8 | x1b << 12 | x2a << 16 | x2b << 20 | x3a << 24 | x3b << 28;
    let y = y0a | y0b << 4 | y1a << 8 | y1b << 12 | y2a << 16 | y2b << 20 | y3a << 24 | y3b << 28;

    (x, y)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn roundtrip() {
        for (a, b) in [
            (2, 4),
            (8, 16),
            (32, 64),
            (128, 256),
            (1, 282472958),
            (142361806, 6791104),
            (39406836, 17391677),
            (4796168, 148478827),
            (5703434, 2026716),
            (16612077, 21815112),
            (25611391, 50736485),
            (145740861, 15962560),
            (7512008, 62085279),
            (142461646, 8125243),
            (27030150, 12038051),
            (16506797, 1454362439),
            (24122395, 31770804),
            (3632437, 151495884),
            (3539001, 41138433),
            (209021241, 4009362),
            (6166955, 386708171),
            (63864899, 11287631),
            (1645593, 2592461),
            (22285206, 62192392),
            (37433174, 9810054),
            (5631421, 2931019),
            (94732639, 31287186),
            (102597093, 30068762),
            (15248553, 21227468),
            (5188914, 54738497),
            (40546372, 20332593),
            (252899588, 54391102),
            (797344187, 1603410060),
            (1418367550, 460978379),
            (107041910, 99933461),
            (12656623, 11977039),
            (354395629, 27319534),
            (2970785, 274430),
            (3499419, 109323045),
        ] {
            let (new_a, new_b) = unmorton(morton(a, b));

            assert_eq!(a, new_a);
            assert_eq!(b, new_b);
        }
    }
}
