use ethers::utils::hex;
use sha3::{Digest, Keccak256};

// Return the checksum address of an ethereum address
// ref: https://github.com/miguelmota/rust-eth-checksum/blob/a29820686995955aea39dce475c862288d83d613/src/lib.rs
pub fn checksum(address: &str) -> String {
    let address = address.trim_start_matches("0x").to_lowercase();

    let address_hash= {
        let mut hasher = Keccak256::new();
        hasher.update(address.as_bytes());
        hex::encode(hasher.finalize())
    };

    address
        .char_indices()
        .fold(String::from("0x"), |mut acc, (index, address_char)| {
            // this cannot fail since it's Keccak256 hashed
            let n = u16::from_str_radix(&address_hash[index..index + 1], 16).unwrap();

            if n > 7 {
                // make char uppercase if ith character is 9..f
                acc.push_str(&address_char.to_uppercase().to_string())
            } else {
                // already lowercased
                acc.push(address_char)
            }

            acc
        })
}