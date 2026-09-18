use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "serde")]
use core::fmt;

#[cfg(feature = "serde")]
use serde::{
    Deserialize as SerdeDeserialize, Deserializer as SerdeDeserializer,
    Serialize as SerdeSerialize, Serializer as SerdeSerializer,
    de::{Error, SeqAccess, Visitor},
    ser::SerializeTuple,
};

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Sig(pub [u8; 64]);

impl Default for Sig {
    fn default() -> Self {
        Sig([0u8; 64])
    }
}

#[cfg(feature = "serde")]
impl SerdeSerialize for Sig {
    fn serialize<S: SerdeSerializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(64)?;
        for b in self.0 {
            tup.serialize_element(&b)?;
        }
        tup.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> SerdeDeserialize<'de> for Sig {
    fn deserialize<D: SerdeDeserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SigVisitor;

        impl<'de> Visitor<'de> for SigVisitor {
            type Value = Sig;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a 64-byte array")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = [0u8; 64];
                for (i, b) in out.iter_mut().enumerate() {
                    *b = seq
                        .next_element()?
                        .ok_or(A::Error::invalid_length(i, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(A::Error::invalid_length(65, &self));
                }
                Ok(Sig(out))
            }
        }
        deserializer.deserialize_tuple(64, SigVisitor)
    }
}
