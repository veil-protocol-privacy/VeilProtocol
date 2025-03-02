use primitive_types::U256;

// Define the SNARK SCALAR FIELD as a constant U256
pub const ZERO_VALUE: U256 = U256([
    0x30644E72E131A029, 0xB85045B68181585D, 0x2833E84879B97091, 0x1A0111EA397FE69A,
]);




pub fn u256_to_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    bytes
}