use anyhow::Result;

pub fn read_u16(data: &[u8], pos: usize) -> Result<u16> {
    let bytes: [u8; 2] = data.get(pos..pos + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("Unexpected EOF"))?;
    Ok(u16::from_be_bytes(bytes))
}

pub fn read_u32(data: &[u8], pos: usize) -> Result<u32> {
    let bytes: [u8; 4] = data.get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("Unexpected EOF"))?;
    Ok(u32::from_be_bytes(bytes))
}

pub fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub fn write_u16_at(out: &mut [u8], pos: usize, value: u16) {
    debug_assert!(pos + 2 <= out.len(), "write_u16_at out of bounds");
    out[pos..pos + 2].copy_from_slice(&value.to_be_bytes());
}

pub fn write_u32_at(out: &mut [u8], pos: usize, value: u32) {
    debug_assert!(pos + 4 <= out.len(), "write_u32_at out of bounds");
    out[pos..pos + 4].copy_from_slice(&value.to_be_bytes());
}

pub fn calc_checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut bytes = [0u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(bytes))
    })
}

// Used for binary search range parameters in the font offset table.
pub fn largest_power_of_two(n: u16) -> u16 {
    if n == 0 { return 1; }
    1u16 << (15 - n.leading_zeros())
}
