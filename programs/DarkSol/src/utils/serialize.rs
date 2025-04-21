use std::io::{Read, Write};

use borsh::{BorshDeserialize, BorshSerialize};

pub trait BorshSerializeWithLength {
    fn serialize_with_length<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error>;

    fn try_to_vec_with_length(&self) -> Result<Vec<u8>, std::io::Error>;
}

pub trait BorshDeserializeWithLength {
    fn deserialize_with_length<R: Read>(reader: &mut R) -> Result<Self, std::io::Error>
    where
        Self: Sized;

    fn try_from_slice_with_length(data: &[u8]) -> Result<Self, std::io::Error>
    where
        Self: Sized;
}

impl<T: BorshSerialize> BorshSerializeWithLength for T {
    fn serialize_with_length<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_all(&self.try_to_vec_with_length()?)
    }

    fn try_to_vec_with_length(&self) -> Result<Vec<u8>, std::io::Error> {
        let payload = borsh::to_vec(self)?;
        let len = payload.len() as u64;
        let mut buf = Vec::with_capacity(8 + payload.len());
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&payload);
        Ok(buf)
    }
}

impl<T: BorshDeserialize> BorshDeserializeWithLength for T {
    fn deserialize_with_length<R: Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut len_buf = [0u8; 8];
        reader.read_exact(&mut len_buf)?;
        let len = u64::from_le_bytes(len_buf) as usize;
        let mut data_buf = vec![0u8; len];
        reader.read_exact(&mut data_buf)?;
        Self::try_from_slice(&data_buf)
    }

    fn try_from_slice_with_length(data: &[u8]) -> Result<Self, std::io::Error> {
        let (len_bytes, rest) = data.split_at(8);
        let len = u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
        let payload = &rest[..len];
        Self::try_from_slice(payload)
    }
}
