//! NUT-XX: Quote Offers (draft)
//!
//! An operator-prepared offer that a wallet claims by creating a mint or melt
//! quote for itself, referencing a single-use ticket issued by the mint's
//! payment backend.
//!
//! <https://github.com/cashubtc/nuts/pull/409>

use std::fmt;
use std::str::FromStr;

use bitcoin::base64::engine::{general_purpose, GeneralPurpose};
use bitcoin::base64::{alphabet, Engine};
use serde::{Deserialize, Serialize};

use crate::mint_url::MintUrl;
use crate::nuts::{CurrencyUnit, PaymentMethod};
use crate::Amount;

const QUOTE_OFFER_PREFIX: &str = "cquoteA";

/// NUT-XX error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid prefix
    #[error("Invalid Prefix")]
    InvalidPrefix,
    /// Ciborium error
    #[error(transparent)]
    CiboriumError(#[from] ciborium::de::Error<std::io::Error>),
    /// Base64 error
    #[error(transparent)]
    Base64Error(#[from] bitcoin::base64::DecodeError),
}

/// Operation of a quote offer (`o` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OfferOperation {
    /// The offer is claimed by creating a mint quote (NUT-04).
    Mint,
    /// The offer is claimed by creating a melt quote (NUT-05).
    Melt,
}

/// NUT-XX quote offer.
///
/// Prepared by an operator (e.g. a teller or point of sale connected to the
/// mint through a payment processor) and handed to a wallet. The wallet claims
/// the offer by creating a mint or melt quote via the regular NUT-04/NUT-05
/// endpoints, referencing [`Self::ticket`]; the first claim wins. The offer
/// contains no quote — no quote ID is ever displayed.
// Field declaration order is load-bearing: ciborium serializes struct fields
// in declaration order and the spec test vector fixes the CBOR key order to
// (m, o, h, u, a, t, d, e).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuoteOffer {
    /// URL of the mint (`m`).
    #[serde(rename = "m")]
    pub mint_url: MintUrl,
    /// Operation of the offer (`o`).
    #[serde(rename = "o")]
    pub operation: OfferOperation,
    /// Payment method to use (`h`).
    #[serde(rename = "h")]
    pub method: PaymentMethod,
    /// Unit of the offer (`u`).
    #[serde(rename = "u")]
    pub unit: CurrencyUnit,
    /// Amount of the offer (`a`).
    ///
    /// MUST be set if the method's quote request requires an amount.
    #[serde(rename = "a", default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Amount>,
    /// Single-use ticket identifying the offered operation (`t`), issued by
    /// the mint's payment backend.
    #[serde(rename = "t")]
    pub ticket: String,
    /// Human readable description the wallet displays after scanning (`d`).
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Unix timestamp until which the offer can be claimed (`e`).
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
}

impl fmt::Display for QuoteOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use serde::ser::Error;
        let mut data = Vec::new();
        ciborium::into_writer(self, &mut data).map_err(|e| fmt::Error::custom(e.to_string()))?;
        // Unpadded, unlike NUT-18: the spec's reference encoder emits unpadded
        // base64_urlsafe. Decoding is padding-indifferent either way.
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(data);
        write!(f, "{QUOTE_OFFER_PREFIX}{encoded}")
    }
}

impl FromStr for QuoteOffer {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s
            .strip_prefix(QUOTE_OFFER_PREFIX)
            .ok_or(Error::InvalidPrefix)?;

        let decode_config = general_purpose::GeneralPurposeConfig::new()
            .with_decode_padding_mode(bitcoin::base64::engine::DecodePaddingMode::Indifferent);
        let decoded = match GeneralPurpose::new(&alphabet::URL_SAFE, decode_config).decode(s) {
            Ok(decoded) => decoded,
            Err(url_safe_err) => {
                let decode_config = general_purpose::GeneralPurposeConfig::new()
                    .with_decode_padding_mode(
                        bitcoin::base64::engine::DecodePaddingMode::Indifferent,
                    );
                GeneralPurpose::new(&alphabet::STANDARD, decode_config)
                    .decode(s)
                    .map_err(|_| url_safe_err)?
            }
        };

        Ok(ciborium::from_reader(&decoded[..])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_VECTOR: &str = "cquoteAp2FteBhodHRwczovL21pbnQuZXhhbXBsZS5jb21hb2RtaW50YWhmYnJhbmNoYXVjb3JhYWEZAfRhdHgkMDE5OGMwZWYtM2YxMS03MDAwLWEzZjctMmY0YjZlMmQ5YzFhYWRsQ2FzaCBkZXBvc2l0";

    fn spec_offer() -> QuoteOffer {
        QuoteOffer {
            mint_url: MintUrl::from_str("https://mint.example.com").expect("valid mint url"),
            operation: OfferOperation::Mint,
            method: PaymentMethod::from_str("branch").expect("valid method"),
            unit: CurrencyUnit::from_str("ora").expect("valid unit"),
            amount: Some(Amount::from(500)),
            ticket: "0198c0ef-3f11-7000-a3f7-2f4b6e2d9c1a".to_string(),
            description: Some("Cash deposit".to_string()),
            expiry: None,
        }
    }

    #[test]
    fn test_quote_offer_spec_vector_encode() {
        assert_eq!(spec_offer().to_string(), SPEC_VECTOR);
    }

    #[test]
    fn test_quote_offer_spec_vector_decode() {
        let offer = QuoteOffer::from_str(SPEC_VECTOR).expect("valid offer");
        assert_eq!(offer, spec_offer());
        assert_eq!(offer.mint_url.to_string(), "https://mint.example.com");
        assert_eq!(offer.operation, OfferOperation::Mint);
        assert_eq!(offer.method.to_string(), "branch");
        assert_eq!(offer.unit.to_string(), "ora");
        assert_eq!(offer.amount, Some(Amount::from(500)));
        assert_eq!(offer.ticket, "0198c0ef-3f11-7000-a3f7-2f4b6e2d9c1a");
        assert_eq!(offer.description.as_deref(), Some("Cash deposit"));
        assert_eq!(offer.expiry, None);
    }

    #[test]
    fn test_quote_offer_round_trip_all_fields() {
        let offer = QuoteOffer {
            expiry: Some(1_750_000_000),
            operation: OfferOperation::Melt,
            ..spec_offer()
        };
        let decoded = QuoteOffer::from_str(&offer.to_string()).expect("round trip");
        assert_eq!(decoded, offer);
    }

    #[test]
    fn test_quote_offer_round_trip_required_fields_only() {
        let offer = QuoteOffer {
            amount: None,
            description: None,
            ..spec_offer()
        };
        let decoded = QuoteOffer::from_str(&offer.to_string()).expect("round trip");
        assert_eq!(decoded, offer);
    }

    #[test]
    fn test_quote_offer_invalid_prefix() {
        assert!(matches!(
            QuoteOffer::from_str("creqAp2FteBho"),
            Err(Error::InvalidPrefix)
        ));
        assert!(matches!(
            QuoteOffer::from_str("not an offer"),
            Err(Error::InvalidPrefix)
        ));
    }

    #[test]
    fn test_quote_offer_invalid_payload() {
        // Invalid base64 after a valid prefix
        assert!(QuoteOffer::from_str("cquoteA!!!").is_err());
        // Valid base64, invalid CBOR
        let garbage = general_purpose::URL_SAFE_NO_PAD.encode(b"not cbor at all");
        assert!(QuoteOffer::from_str(&format!("cquoteA{garbage}")).is_err());
    }

    #[test]
    fn test_quote_offer_unknown_field_tolerance() {
        // A future spec revision may add keys; decoding must ignore them.
        let value = serde_json::json!({
            "m": "https://mint.example.com",
            "o": "melt",
            "h": "branch",
            "u": "ora",
            "t": "MELT-123",
            "z": "future-field",
        });
        let mut data = Vec::new();
        ciborium::into_writer(&value, &mut data).expect("cbor");
        let encoded = format!("cquoteA{}", general_purpose::URL_SAFE_NO_PAD.encode(data));

        let offer = QuoteOffer::from_str(&encoded).expect("unknown fields are ignored");
        assert_eq!(offer.operation, OfferOperation::Melt);
        assert_eq!(offer.ticket, "MELT-123");
        assert_eq!(offer.amount, None);
    }

    #[test]
    fn test_quote_offer_json_uses_single_letter_keys() {
        let json = serde_json::to_value(spec_offer()).expect("json");
        let obj = json.as_object().expect("object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["a", "d", "h", "m", "o", "t", "u"]);
        assert_eq!(json["o"], "mint");
        assert_eq!(json["a"], 500);
    }
}
