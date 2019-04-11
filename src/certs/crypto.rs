use super::*;

use std::fmt::{Debug, Formatter};

pub struct Body;

pub trait Verifiable {
    type Output;

    fn verify(self) -> Result<Self::Output>;
}

pub trait Signer<T> {
    type Output;

    fn sign(&self, target: &mut T) -> Result<Self::Output>;
}

pub struct Signature {
    pub id: Option<u128>,
    pub sig: Vec<u8>,
    pub kind: pkey::Id,
    pub hash: hash::MessageDigest,
    pub usage: Usage,
}

pub struct PrivateKey<U> {
    pub id: Option<u128>,
    pub key: pkey::PKey<pkey::Private>,
    pub hash: hash::MessageDigest,
    pub usage: U,
}

impl<'a, U, C> codicon::Decoder<&'a C> for PrivateKey<U> where
    &'a C: TryInto<PublicKey<U>, Error=Error> {
    type Error = Error;

    fn decode(reader: &mut impl Read, params: &'a C) -> Result<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        let prv = pkey::PKey::private_key_from_der(&buf)?;
        let key = params.try_into()?;
        if !prv.public_eq(&key.key) {
            return Err(ErrorKind::InvalidData.into());
        }

        Ok(PrivateKey {
            usage: key.usage,
            hash: key.hash,
            id: key.id,
            key: prv,
        })
    }
}

impl<U> codicon::Encoder for PrivateKey<U> {
    type Error = Error;

    fn encode(&self, writer: &mut impl Write, _: ()) -> Result<()> {
        let buf = self.key.private_key_to_der()?;
        writer.write_all(&buf)
    }
}

pub struct PublicKey<U> {
    pub id: Option<u128>,
    pub key: pkey::PKey<pkey::Public>,
    pub hash: hash::MessageDigest,
    pub usage: U,
}

impl<U: Copy + Into<Usage>> std::fmt::Display for PublicKey<U> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use std::fmt::Error;

        let sig = match self.usage.into() {
            Usage::CEK | Usage::OCA | Usage::PEK => true,
            Usage::ARK | Usage::ASK => true,
            _ => false,
        };

        match (sig, self.key.id()) {
            (true, pkey::Id::RSA) => write!(f, "R{} R{}",
                    self.key.rsa()?.size() * 8,
                    self.hash.size() * 8),

            (true, pkey::Id::EC)  => write!(f, "EP{} E{}",
                    self.key.ec_key()?.group().degree(),
                    self.hash.size() * 8),

            (false, pkey::Id::EC) => write!(f, "EP{} D{}",
                    self.key.ec_key()?.group().degree(),
                    self.hash.size() * 8),

            _ => Err(Error),
        }
    }
}

impl<U> PublicKey<U> where U: Debug, Usage: PartialEq<U> {
    pub fn verify(&self, msg: &impl codicon::Encoder<Body, Error=Error>, sig: &Signature) -> Result<()> {
        let usage = sig.usage == self.usage;
        let kind = sig.kind == self.key.id();
        let hash = sig.hash == self.hash;
        let id = sig.id.is_none() || sig.id == self.id;
        if !usage || !kind || !hash || !id {
            return Err(ErrorKind::InvalidInput.into());
        }

        let mut ver = sign::Verifier::new(sig.hash, &self.key)?;
        if self.key.id() == pkey::Id::RSA {
            ver.set_rsa_padding(rsa::Padding::PKCS1_PSS)?;
            ver.set_rsa_pss_saltlen(sign::RsaPssSaltlen::DIGEST_LENGTH)?;
        }

        msg.encode(&mut ver, Body)?;
        ver.verify(&sig.sig)?;
        Ok(())
    }
}