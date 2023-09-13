pub fn avg_vecu32 (v: Vec<u32>) -> u32 { v.iter().sum::<u32>() / v.len() as u32 }

pub fn pop_4u8 (barry: &[u8]) -> [u8; 4] {  [barry, &[0, 0, 0, 0]].concat().try_into().expect("slice with incorrect length") }