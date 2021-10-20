use crate::{
  common::{Footer, Payload},
  errors::V2LocalTokenBuilderError,
  keys::V2LocalSharedKey,
  tokens::v2::V2LocalToken,
  traits::PasetoClaim,
};
use core::marker::PhantomData;
use std::{collections::HashMap, mem::take};

pub struct TokenBuilder<'a, Version, Purpose> {
  version: PhantomData<Version>,
  purpose: PhantomData<Purpose>,
  claims: HashMap<String, Box<dyn erased_serde::Serialize>>,
  footer: Option<Footer<'a>>,
}

impl<Version, Purpose> TokenBuilder<'_, Version, Purpose> {
  pub fn new() -> Self {
    TokenBuilder::<Version, Purpose> {
      version: PhantomData::<Version>,
      purpose: PhantomData::<Purpose>,
      claims: HashMap::with_capacity(10),
      footer: None,
    }
  }

  pub fn set_claim<T: PasetoClaim + erased_serde::Serialize + 'static>(&mut self, value: T) -> &mut Self {
    self.claims.insert(value.get_key().to_owned(), Box::new(value));
    self
  }

  pub fn set_footer(&mut self, footer: Option<Footer<'static>>) -> &mut Self {
    self.footer = footer;
    self
  }
}

impl<Version, Purpose> Default for TokenBuilder<'_, Version, Purpose> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a, V2, Local> TokenBuilder<'a, V2, Local> {
  pub fn build(&mut self, key: &V2LocalSharedKey) -> Result<String, V2LocalTokenBuilderError> {
    //here we need to go through all the claims and serialize them to build a payload
    let mut payload = String::from('{');

    let claims = take(&mut self.claims);

    for claim in claims.into_values() {
      let raw = serde_json::to_string(&claim)?;
      let trimmed = raw.trim_start_matches('{').trim_end_matches('}');
      payload.push_str(&format!("{},", trimmed));
    }

    //get rid of that trailing comma (this feels like a dirty approach, there's probably a better
    //way to do this)
    payload = payload.trim_end_matches(',').to_string();
    payload.push('}');

    Ok(V2LocalToken::new(Payload::from(payload.as_str()), key, self.footer).to_string())
  }
}

#[cfg(test)]
mod builders {
  use std::convert::TryFrom;

  use super::*;
  use crate::claims::{Arbitrary, Audience, Expiration, Subject};
  use crate::common::{Local, V2};
  use crate::keys::{Key256Bit, V2LocalSharedKey};
  use crate::v2::local::V2LocalDecryptedToken;
  use anyhow::Result;
  use serde_json::value::Value;

  #[test]
  fn basic_builder_test() -> Result<()> {
    //create a key
    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
    let key = V2LocalSharedKey::from(KEY);

    //create a builder, add some claims and then build the token with the key
    let token = TokenBuilder::<V2, Local>::default()
      .set_claim(Audience::from("customers"))
      .set_claim(Subject::from("loyal subjects"))
      .set_claim(Expiration::try_from("2019-01-01T00:00:00+00:00")?)
      .set_claim(Arbitrary::try_from(("data".to_string(), "this is a secret message"))?)
      .set_claim(Arbitrary::try_from(("seats".to_string(), 4))?)
      .set_claim(Arbitrary::try_from(("any ol' pi".to_string(), 3.141526))?)
      .build(&key)?;

    //now let's decrypt the token and verify the values
    let decrypted = V2LocalDecryptedToken::parse(&token, None, &key)?;
    let json: Value = serde_json::from_str(decrypted.as_ref())?;

    assert_eq!(json["aud"], "customers");
    assert_eq!(json["data"], "this is a secret message");
    assert_eq!(json["exp"], "2019-01-01T00:00:00+00:00");
    assert_eq!(json["sub"], "loyal subjects");
    assert_eq!(json["any ol' pi"], 3.141526);
    assert_eq!(json["seats"], 4);
    Ok(())
  }

  #[test]
  fn dynamic_claims_test() -> Result<()> {
    //create a key
    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
    let key = V2LocalSharedKey::from(KEY);

    //create a builder, add some claims dynamically
    let mut builder = TokenBuilder::<V2, Local>::default();
    builder.set_claim(Expiration::try_from("2019-01-01T00:00:00+00:00")?);

    for n in 1..10 {
      builder.set_claim(Arbitrary::try_from((format!("n{}", n).to_string(), n))?);
    }

    //and then build the token with the key
    let token = builder.build(&key)?;

    //now let's decrypt the token and verify the values
    let decrypted = V2LocalDecryptedToken::parse(&token, None, &key)?;
    let json: Value = serde_json::from_str(decrypted.as_ref())?;

    for n in 1..10 {
      assert_eq!(json[format!("n{}", n)], n);
    }

    assert_eq!(json["exp"], "2019-01-01T00:00:00+00:00");

    Ok(())
  }

  #[test]
  fn test_no_claims() -> Result<()> {
    //create a key
    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
    let key = V2LocalSharedKey::from(KEY);

    //create a builder, add no claims and then build the token with the key
    let token = TokenBuilder::<V2, Local>::default().build(&key)?;

    //now let's decrypt the token and verify the values
    let decrypted = V2LocalDecryptedToken::parse(&token, None, &key)?;
    assert_eq!(decrypted.as_ref(), "{}");
    Ok(())
  }

  //  #[test]
  //  fn test_basic_builder_with_single_arbitrary_claim() -> Result<()> {
  //    //here's a valid 32 byte key
  //    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
  //    let key = &V2LocalSharedKey::from(KEY);

  //    //basic builder
  //    let mut builder = V2LocalTokenBuilder::new();

  //    //construct and add a single claim
  //    //    let new_claim = PasetoClaim::try_new("glasses", 4)?;
  //    //    builder.set_claim(new_claim);
  //    //builder.set_claim(PasetoClaim::try_new("wine", "merlot")?);

  //    //build
  //    let token = builder.build(KEY)?;

  //    //now let's decrypt it
  //    let decrypted = V2LocalDecryptedToken::parse(&token.to_string(), None, key)?;

  //    //verify the decrypted payload
  //    assert_eq!(
  //      decrypted.as_ref(),
  //      &serde_json::json!({"wine":"merlot", "glasses":4}).to_string()
  //    );

  //    Ok(())
  //  }

  //  #[test]
  //  fn test_basic_builder_with_single_claim_and_footer() -> Result<()> {
  //    //here's a valid 32 byte key
  //    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
  //    let key = &V2LocalSharedKey::from(KEY);

  //    //basic builder
  //    let mut builder = V2LocalTokenBuilder::new();

  //    //construct and add a single claim
  //    //   let claim: AudienceClaim = "I'm Pickle Rick!".into();
  //    //   builder.add_restricted_claim(claim);
  //    builder.set_footer(Some(Footer::from("universe c137")));

  //    //build
  //    let token = builder.build(KEY)?;

  //    //now let's decrypt it
  //    let _decrypted = V2LocalDecryptedToken::parse(&token.to_string(), Some(Footer::from("universe c137")), key)?;

  //    //verify the decrypted payload
  //    //    assert_eq!(
  //    //      decrypted.as_ref(),
  //    //      &serde_json::json!({"aud":"I'm Pickle Rick!"}).to_string()
  //    //    );

  //    Ok(())
  //  }

  //  #[test]
  //  fn test_basic_builder_with_single_claim() -> Result<()> {
  //    //here's a valid 32 byte key
  //    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
  //    let key = &V2LocalSharedKey::from(KEY);

  //    //basic builder
  //    let builder = V2LocalTokenBuilder::new();

  //    //construct and add a single claim
  //    //   let claim: AudienceClaim = "I'm Pickle Rick!".into();
  //    //   builder.add_restricted_claim(claim);

  //    //build
  //    let token = builder.build(KEY)?;

  //    //now let's decrypt it
  //    let _decrypted = V2LocalDecryptedToken::parse(&token.to_string(), None, key)?;

  //    //verify the decrypted payload
  //    //    assert_eq!(
  //    //      decrypted.as_ref(),
  //    //      &serde_json::json!({"aud":"I'm Pickle Rick!"}).to_string()
  //    //    );

  //    Ok(())
  //  }

  //  #[test]
  //  fn test_basic_builder() -> Result<()> {
  //    //here's a valid 32 byte key
  //    const KEY: Key256Bit = *b"wubbalubbadubdubwubbalubbadubdub";
  //    let key = &V2LocalSharedKey::from(KEY);

  //    //here's the simplest token you can build, no arbitrary claims, no footer,
  //    //24 hour expiration date
  //    let token = V2LocalTokenBuilder::new().build(KEY)?;
  //    //now let's decrypt it
  //    let _decrypted = V2LocalDecryptedToken::parse(&token.to_string(), None, key)?;

  //    //verify the decrypted and empty payload
  //    //      assert_eq!(decrypted.as_ref(), "{}");

  //    Ok(())
  //  }
}
