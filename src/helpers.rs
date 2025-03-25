pub fn avg_vecu32(v: Vec<u32>) -> u32 {
    v.iter().sum::<u32>() / v.len() as u32
}

pub fn pop_4u8(barry: &[u8]) -> [u8; 4] {
    [barry, &[0, 0, 0, 0]].concat()[0..4].try_into().unwrap()
}

pub fn nvd_r2u64(res: String) -> Option<u64> {
    let mut chars = res.chars();
    chars.next();
    chars.next_back();

    match chars.as_str().split(" ").collect::<Vec<&str>>()[0]
        .trim()
        .parse()
    {
        Ok(v) => Some(v),
        _ => None,
    }
}
