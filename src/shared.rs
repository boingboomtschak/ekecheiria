use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EkcImage {
    pub image_data : Vec<u8>,
    pub width : u32,
    pub height : u32
}

impl From<&EkcImage> for Vec<u8> {
    fn from(value: &EkcImage) -> Self {
        return bincode::serialize(value).unwrap();
    }
}

impl From<EkcImage> for Vec<u8> {
    fn from(value: EkcImage) -> Self {
        return bincode::serialize(&value).unwrap();
    }
}

impl TryFrom<&[u8]> for EkcImage {
    type Error = Box<bincode::ErrorKind>;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        return bincode::deserialize(value);
    }
}