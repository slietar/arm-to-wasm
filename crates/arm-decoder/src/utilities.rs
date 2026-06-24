pub const INSTRUCTION_SIZE: u64 = 4;

pub fn bit_mask(bits: u32) -> u32 {
    (1u32 << bits) - 1
}

pub fn get_bits(value: u32, start: u32, size: u32) -> u32 {
    (value >> start) & bit_mask(size)
}

pub fn get_bits_range(value: u32, start: u32, end: u32) -> u32 {
    // Both inclusive
    get_bits(value, start, end - start + 1)
}

pub fn decode_bool(value: u32, start: u32) -> bool {
    get_bits(value, start, 1) == 1
}

pub fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32 >> shift) as i32
}

pub fn equal_masked(value: u32, mask: u32, expe: u32) -> bool {
    (value & mask) == expe
}
