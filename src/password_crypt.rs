use cbc::Encryptor;
use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};

const KEY: &[u8] = b"VQZBJ6TD8M9WBUWT";
const IV: &[u8] = b"joiwef08u23j341a";

pub fn encrypt_password(password: &str) -> String {
    // CBC-AES-128 with PKCS#7 padding
    let cipher = Encryptor::<aes::Aes128>::new_from_slices(KEY, IV).unwrap();
    let encrypted = cipher.encrypt_padded_vec_mut::<Pkcs7>(password.as_bytes());

    let mut hex = String::new();
    for byte in encrypted {
        hex = format!("{hex}{byte:02X}");
    }
    hex
}
