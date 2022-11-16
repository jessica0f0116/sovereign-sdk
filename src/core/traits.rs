use core::fmt::Debug;

use bytes::Bytes;

pub trait Blockheader: PartialEq + Debug + CanonicalHash<Output = Self::Hash> {
    type Hash: Clone;
    fn prev_hash(&self) -> &Self::Hash;
}

pub trait CanonicalHash {
    type Output;
    fn hash(&self) -> Self::Output;
}

pub trait Block: PartialEq + Debug {
    type Header: Blockheader;
    type Transaction: Transaction;
    fn header(&self) -> &Self::Header;
    fn transactions(&self) -> &[Self::Transaction];
    fn take_transactions(self) -> Vec<Self::Transaction>;
}

pub trait Transaction: PartialEq + Debug + CanonicalHash<Output = Self::Hash> {
    type Hash;
}

pub trait Address: PartialEq + Debug + Clone {}

pub struct InvalidAddress;
pub trait AsBytes
where
    Self: Sized,
{
    fn as_bytes(&self) -> Bytes;
    fn from_bytes(addr: &[u8]) -> Result<Self, InvalidAddress>;
}