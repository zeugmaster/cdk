//! NUT-XX quote offer FFI types (draft)

use std::sync::Arc;

use super::amount::{Amount, CurrencyUnit};
use super::quote::{MeltQuote, MintQuote};
use crate::error::FfiError;

/// Operation of a quote offer
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum QuoteOfferOperation {
    /// The offer is claimed by creating a mint quote (NUT-04)
    Mint,
    /// The offer is claimed by creating a melt quote (NUT-05)
    Melt,
}

impl From<cdk::nuts::OfferOperation> for QuoteOfferOperation {
    fn from(operation: cdk::nuts::OfferOperation) -> Self {
        match operation {
            cdk::nuts::OfferOperation::Mint => Self::Mint,
            cdk::nuts::OfferOperation::Melt => Self::Melt,
        }
    }
}

impl From<QuoteOfferOperation> for cdk::nuts::OfferOperation {
    fn from(operation: QuoteOfferOperation) -> Self {
        match operation {
            QuoteOfferOperation::Mint => Self::Mint,
            QuoteOfferOperation::Melt => Self::Melt,
        }
    }
}

/// NUT-XX Quote Offer (draft)
///
/// An operator-prepared mint or melt operation referencing a single-use
/// ticket. Encoded as a string with the `cquoteA` prefix.
#[derive(Debug, uniffi::Object)]
pub struct QuoteOffer {
    inner: cdk::nuts::QuoteOffer,
}

impl QuoteOffer {
    /// Get inner reference
    pub(crate) fn inner(&self) -> &cdk::nuts::QuoteOffer {
        &self.inner
    }
}

#[uniffi::export]
impl QuoteOffer {
    /// Parse a quote offer from its encoded string representation
    #[uniffi::constructor]
    pub fn from_string(encoded: String) -> Result<Arc<Self>, FfiError> {
        use std::str::FromStr;
        let inner = cdk::nuts::QuoteOffer::from_str(&encoded).map_err(FfiError::internal)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Encode the quote offer to a string
    pub fn to_string_encoded(&self) -> String {
        self.inner.to_string()
    }

    /// URL of the mint
    pub fn mint_url(&self) -> String {
        self.inner.mint_url.to_string()
    }

    /// Operation of the offer
    pub fn operation(&self) -> QuoteOfferOperation {
        self.inner.operation.into()
    }

    /// Payment method to use
    pub fn method(&self) -> String {
        self.inner.method.to_string()
    }

    /// Unit of the offer
    pub fn unit(&self) -> CurrencyUnit {
        self.inner.unit.clone().into()
    }

    /// Amount of the offer, when set
    pub fn amount(&self) -> Option<Amount> {
        self.inner.amount.map(Into::into)
    }

    /// Single-use ticket identifying the offered operation
    pub fn ticket(&self) -> String {
        self.inner.ticket.clone()
    }

    /// Human readable description to display after scanning
    pub fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Unix timestamp until which the offer can be claimed
    pub fn expiry(&self) -> Option<u64> {
        self.inner.expiry
    }
}

/// Result of claiming a NUT-XX quote offer
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ClaimedOffer {
    /// A mint quote was created; wait for the operator-side payment, then mint
    Mint {
        /// The claimed mint quote, locked to a wallet key (NUT-20)
        quote: MintQuote,
    },
    /// A melt quote was created; prepare and confirm it to pay
    Melt {
        /// The claimed melt quote; its ID doubles as the payout proof
        quote: MeltQuote,
    },
}

impl From<cdk::wallet::ClaimedOffer> for ClaimedOffer {
    fn from(claimed: cdk::wallet::ClaimedOffer) -> Self {
        match claimed {
            cdk::wallet::ClaimedOffer::Mint(quote) => Self::Mint {
                quote: quote.into(),
            },
            cdk::wallet::ClaimedOffer::Melt(quote) => Self::Melt {
                quote: quote.into(),
            },
        }
    }
}

/// Decode a quote offer from its encoded string representation
#[uniffi::export]
pub fn decode_quote_offer(encoded: String) -> Result<Arc<QuoteOffer>, FfiError> {
    QuoteOffer::from_string(encoded)
}
