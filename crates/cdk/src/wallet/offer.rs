//! NUT-XX quote offer claiming (draft)
//!
//! An operator hands the wallet a [`QuoteOffer`] (a `cquoteA…` string); the
//! wallet claims it by creating a mint or melt quote for itself referencing
//! the offer's single-use ticket. See
//! <https://github.com/cashubtc/nuts/pull/409>.

use cdk_common::nuts::{MintQuoteCustomRequest, OfferOperation, QuoteOffer, SecretKey};
use cdk_common::wallet::{MeltQuote, MintQuote};
use cdk_common::MintQuoteRequest;
use tracing::instrument;

use crate::util::unix_time;
use crate::{Error, Wallet};

/// Result of claiming a NUT-XX quote offer.
#[derive(Debug, Clone)]
pub enum ClaimedOffer {
    /// A mint quote was created.
    ///
    /// Wait for the operator-side payment and mint, e.g. with
    /// [`Wallet::wait_and_mint_quote`]; the quote's stored secret key signs
    /// the mint request (NUT-20) automatically.
    Mint(MintQuote),
    /// A melt quote was created.
    ///
    /// Prepare and confirm it with [`Wallet::prepare_melt`] followed by
    /// `confirm_prefer_async` — offer melts are always asynchronous, so the
    /// mint responds `PENDING` and the wallet must monitor the quote (e.g.
    /// [`Wallet::check_melt_quote_status`]). The quote ID is known only to
    /// wallet and mint; per the spec it (or a code derived from it) can be
    /// shown to the operator as proof of who initiated the payment.
    Melt(MeltQuote),
}

impl Wallet {
    /// Claim a NUT-XX quote offer against this wallet's mint.
    ///
    /// Validates that the offer targets this wallet's mint and unit and has
    /// not expired, then creates the quote referencing the offer's ticket:
    ///
    /// - Mint offers become a custom-method mint quote locked to a freshly
    ///   generated NUT-20 key (required by the spec — the offer is public
    ///   once displayed, and the lock is what makes a front-running claim
    ///   fail visibly instead of stealing the funds).
    /// - Melt offers become a custom-method melt quote whose `request` is the
    ///   ticket itself; the payment backend answers with the authoritative
    ///   amount, so the wallet cannot choose the payout.
    #[instrument(skip(self, offer))]
    pub async fn claim_quote_offer(&self, offer: &QuoteOffer) -> Result<ClaimedOffer, Error> {
        if offer.mint_url != self.mint_url {
            return Err(Error::IncorrectMint);
        }

        // Compare via Display: `CurrencyUnit::Custom` equality is
        // case-sensitive while Display lowercases both sides.
        if offer.unit.to_string() != self.unit.to_string() {
            return Err(Error::UnitMismatch);
        }

        let now = unix_time();
        if let Some(expiry) = offer.expiry {
            if expiry < now {
                return Err(Error::ExpiredQuote(expiry, now));
            }
        }

        // Tickets exist only on custom quote requests; known methods carry
        // their operation data in the request itself.
        if !offer.method.is_custom() {
            return Err(Error::UnsupportedPaymentMethod);
        }

        // Draft mints may not advertise the settings entry yet — warn and let
        // the mint reject definitively if it truly lacks support.
        match self.load_mint_info().await {
            Ok(info) => {
                let advertised = info
                    .nuts
                    .nutxx
                    .as_ref()
                    .is_some_and(|settings| settings.supported);
                if !advertised {
                    tracing::warn!(
                        "Mint does not advertise NUT-XX quote offer support; attempting claim anyway"
                    );
                }
            }
            Err(err) => {
                tracing::warn!("Could not load mint info before claiming offer: {err}");
            }
        }

        match offer.operation {
            OfferOperation::Mint => {
                self.keysets(Default::default()).await?;

                let secret_key = SecretKey::generate();
                let request = MintQuoteRequest::Custom {
                    method: offer.method.clone(),
                    request: MintQuoteCustomRequest {
                        // May be None for amountless offers; the backend
                        // validates the amount against the ticket either way.
                        amount: offer.amount,
                        unit: self.unit.clone(),
                        // The offer description is display-only for the
                        // wallet; the backend already knows it via the ticket.
                        description: None,
                        pubkey: Some(secret_key.public_key()),
                        ticket: Some(offer.ticket.clone()),
                        extra: serde_json::Value::Null,
                    },
                };

                let quote = self
                    .submit_mint_quote_request(
                        offer.method.clone(),
                        request,
                        offer.amount,
                        secret_key,
                    )
                    .await?;

                Ok(ClaimedOffer::Mint(quote))
            }
            OfferOperation::Melt => {
                let quote = self
                    .melt_quote_custom(&offer.method.to_string(), offer.ticket.clone(), None, None)
                    .await?;

                Ok(ClaimedOffer::Melt(quote))
            }
        }
    }
}
